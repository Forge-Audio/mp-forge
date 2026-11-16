"""The pictures the screen test cards put on the MPC, drawn here so each one tests one thing."""

import math
import os
from collections import namedtuple

from PIL import Image, ImageDraw, ImageFilter, ImageFont

PAGE_WIDTH, PAGE_HEIGHT = 1280, 628
RULER_STEP = 100
RULER_TICK = 20
TOP_OF_PAGE = (22, 34, 64)
BOTTOM_OF_PAGE = (58, 24, 70)
RULER_LINE = (255, 255, 255, 70)
RULER_TICK_LINE = (255, 255, 255, 28)
EDGE_LINE = (255, 220, 0, 255)
ORANGE, DEEP_ORANGE = (255, 170, 40), (190, 70, 10)
CLEAR = (0, 0, 0, 0)
PLAIN_PAGE = (30, 29, 30, 255)
NOTE_SIZE = 20
NOTE_PLATE = (0, 0, 0, 150)
NOTE_PADDING = 3
# A font the MPC does not have. Anything shown in it has to be drawn here, on the computer.
OWN_FONT_PATH = os.path.expanduser("~/Library/Fonts/JetBrainsMonoNerdFont-Regular.ttf")
HIGHEST_NUMBER = 100


def number_font(size):
    return ImageFont.load_default(size=size)


def own_font(size):
    return ImageFont.truetype(OWN_FONT_PATH, size)


def blend(start, end, share):
    return tuple(round(a + (b - a) * share) for a, b in zip(start, end))


def top_to_bottom_fade(width, height, top, bottom):
    picture = Image.new("RGBA", (width, height))
    draw = ImageDraw.Draw(picture)
    for y in range(height):
        draw.line([(0, y), (width, y)], fill=blend(top, bottom, y / (height - 1)) + (255,))
    return picture


def draw_ruler_lines(draw):
    for x in range(0, PAGE_WIDTH, RULER_TICK):
        draw.line([(x, 0), (x, PAGE_HEIGHT)], fill=RULER_LINE if x % RULER_STEP == 0 else RULER_TICK_LINE)
    for y in range(0, PAGE_HEIGHT, RULER_TICK):
        draw.line([(0, y), (PAGE_WIDTH, y)], fill=RULER_LINE if y % RULER_STEP == 0 else RULER_TICK_LINE)


def draw_ruler_numbers(draw):
    for x in range(RULER_STEP, PAGE_WIDTH, RULER_STEP):
        draw.text((x + 4, 2), str(x), font=number_font(16), fill="white")
    for y in range(RULER_STEP, PAGE_HEIGHT, RULER_STEP):
        draw.text((4, y + 2), str(y), font=number_font(16), fill="white")


def ruler():
    """The whole page, with a baked-in colour fade, a line every 20 pixels and a number every 100."""
    picture = top_to_bottom_fade(PAGE_WIDTH, PAGE_HEIGHT, TOP_OF_PAGE, BOTTOM_OF_PAGE)
    lines = Image.new("RGBA", picture.size, CLEAR)
    draw = ImageDraw.Draw(lines)
    draw_ruler_lines(draw)
    draw_ruler_numbers(draw)
    draw.rectangle([0, 0, PAGE_WIDTH - 1, PAGE_HEIGHT - 1], outline=EDGE_LINE, width=2)
    return Image.alpha_composite(picture, lines)


def write_note(draw, note):
    lettering = number_font(NOTE_SIZE)
    left, top, right, bottom = draw.textbbox((note.x, note.y), note.text, font=lettering)
    draw.rectangle([left - NOTE_PADDING, top - NOTE_PADDING, right + NOTE_PADDING, bottom + NOTE_PADDING], fill=NOTE_PLATE)
    draw.text((note.x, note.y), note.text, font=lettering, fill="white")


def page_with_notes(notes, on_ruler):
    """A whole page with its captions drawn in, because pictures are the one thing known to show on the MPC."""
    page = ruler() if on_ruler else Image.new("RGBA", (PAGE_WIDTH, PAGE_HEIGHT), PLAIN_PAGE)
    lettering = Image.new("RGBA", page.size, CLEAR)
    draw = ImageDraw.Draw(lettering)
    for note in notes:
        write_note(draw, note)
    return Image.alpha_composite(page, lettering)


def disc(size=200):
    """A round shape with a soft shadow and a colour fade, both baked into the picture."""
    shadow = Image.new("RGBA", (size, size), CLEAR)
    ImageDraw.Draw(shadow).ellipse([28, 36, size - 12, size - 4], fill=(0, 0, 0, 170))
    picture = shadow.filter(ImageFilter.GaussianBlur(8))
    face = top_to_bottom_fade(size, size, ORANGE, DEEP_ORANGE)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse([12, 8, size - 28, size - 32], fill=255)
    picture.paste(face, (0, 0), mask)
    ImageDraw.Draw(picture).line([(size // 2 - 8, size // 2 - 12), (size // 2 - 8, 20)], fill="white", width=6)
    return picture


def glass(size=200):
    """Solid white on the left, fading to fully clear on the right."""
    picture = Image.new("RGBA", (size, size))
    draw = ImageDraw.Draw(picture)
    for x in range(size):
        draw.line([(x, 0), (x, size)], fill=(255, 255, 255, round(255 * (1 - x / (size - 1)))))
    return picture


def arrow(size=160):
    picture = Image.new("RGBA", (size, size), CLEAR)
    draw = ImageDraw.Draw(picture)
    middle = size // 2
    draw.polygon([(middle, 8), (size - 24, middle), (middle + 22, middle), (middle + 22, size - 8),
                  (middle - 22, size - 8), (middle - 22, middle), (24, middle)], fill=ORANGE + (255,))
    return picture


def dot(size, colour):
    picture = Image.new("RGBA", (size, size), CLEAR)
    ImageDraw.Draw(picture).ellipse([2, 2, size - 3, size - 3], fill=colour, outline="white", width=2)
    return picture


def bar(width, height, colour):
    return Image.new("RGBA", (width, height), colour)


def knob_body(size=120):
    picture = Image.new("RGBA", (size, size), CLEAR)
    ImageDraw.Draw(picture).ellipse([4, 4, size - 5, size - 5], fill=(60, 60, 66, 255), outline=(150, 150, 160, 255), width=3)
    return picture


def knob_pointer(size=120):
    picture = Image.new("RGBA", (size, size), CLEAR)
    ImageDraw.Draw(picture).line([(size // 2, size // 2), (size // 2, 12)], fill=ORANGE + (255,), width=8)
    return picture


def without_see_through(picture):
    solid = Image.new("RGB", picture.size, (40, 40, 44))
    solid.paste(picture, (0, 0), picture)
    return solid


def stacked(frames, width, height, draw_frame):
    """A film strip: every frame drawn by `draw_frame(draw, share)` and stacked downwards, lowest value first."""
    strip = Image.new("RGBA", (width, height * frames), CLEAR)
    for index in range(frames):
        frame = Image.new("RGBA", (width, height), CLEAR)
        draw_frame(ImageDraw.Draw(frame), index / (frames - 1))
        strip.paste(frame, (0, index * height))
    return strip


def rising_bar(draw, share, width=40, height=200):
    draw.rectangle([0, 0, width - 1, height - 1], fill=(50, 50, 56, 255))
    filled = round(height * share)
    if filled:
        draw.rectangle([0, height - filled, width - 1, height - 1], fill=(60, 255, 120, 255))


def sliding_dot(draw, share, width=600, height=40):
    draw.line([(20, height // 2), (width - 20, height // 2)], fill=(90, 90, 100, 255), width=2)
    x = 20 + share * (width - 40)
    draw.ellipse([x - 14, height // 2 - 14, x + 14, height // 2 + 14], fill=ORANGE + (255,))


def wave_column(draw, share, width=16, height=160):
    top = round((height - 8) * (1 - share))
    draw.rectangle([0, top, width - 1, height - 1], fill=(0, 208, 255, 60))
    draw.rectangle([0, top, width - 1, top + 7], fill=(0, 208, 255, 255))


def turning_spoke(draw, share, size=80):
    middle = size / 2
    angle = 2 * math.pi * share
    draw.ellipse([4, 4, size - 5, size - 5], outline=(150, 150, 160, 255), width=3)
    draw.line([(middle, middle), (middle + 30 * math.sin(angle), middle - 30 * math.cos(angle))], fill=ORANGE + (255,), width=6)


def number_in_own_font(draw, share, width=120, height=60):
    """One reading of a 0 to 100 value, so a strip of these shows the value in a font the MPC lacks."""
    draw.text((width / 2, height / 2), str(round(share * HIGHEST_NUMBER)), font=own_font(44), fill="white", anchor="mm")


def words_in_own_font(words="Drawn on the computer", width=520, height=50):
    picture = Image.new("RGBA", (width, height), CLEAR)
    ImageDraw.Draw(picture).text((0, height / 2), words, font=own_font(36), fill=ORANGE + (255,), anchor="lm")
    return picture


StripShape = namedtuple("StripShape", "frames width height draw_frame")
STRIPS = {
    "bar_strip.png": StripShape(41, 40, 200, rising_bar),
    "dot_strip.png": StripShape(61, 600, 40, sliding_dot),
    "column_strip.png": StripShape(33, 16, 160, wave_column),
    "spoke_strip.png": StripShape(24, 80, 80, turning_spoke),
    "number_strip.png": StripShape(HIGHEST_NUMBER + 1, 120, 60, number_in_own_font),
}


PICTURES = {
    "disc.png": disc,
    "disc.jpg": lambda: without_see_through(disc()),
    "glass.png": glass,
    "arrow.png": arrow,
    "puck.png": lambda: dot(48, ORANGE + (255,)),
    "lamp_on.png": lambda: dot(40, (60, 255, 120, 255)),
    "lamp_off.png": lambda: dot(40, (40, 60, 46, 255)),
    "meter_off.png": lambda: bar(40, 300, (50, 50, 56, 255)),
    "meter_peak.png": lambda: bar(40, 300, (60, 255, 120, 255)),
    "meter_rms.png": lambda: bar(40, 300, (255, 220, 0, 255)),
    "wide_meter_off.png": lambda: bar(300, 40, (50, 50, 56, 255)),
    "wide_meter_peak.png": lambda: bar(300, 40, (60, 255, 120, 255)),
    "wide_meter_rms.png": lambda: bar(300, 40, (255, 220, 0, 255)),
    "knob_body.png": knob_body,
    "knob_pointer.png": knob_pointer,
    "own_font_words.png": words_in_own_font,
}
PICTURES.update({name: (lambda shape=shape: stacked(*shape)) for name, shape in STRIPS.items()})


def save_pictures(names, folder):
    """Saves the named pictures that are drawn here. Other names belong to someone else and are left alone."""
    for name in sorted(set(names) & set(PICTURES)):
        PICTURES[name]().save(os.path.join(folder, name))
