#!/usr/bin/env python3
"""Writes the source file and the hand-made screen of every test card.

    python3 screen-tests/make_test_cards.py

"Test Card" holds the checks the edit mode depends on and uses only widgets Akai's reference calls
well-trodden. Each "Draw ..." card holds one kind of less tested widget, because one entry the MPC
dislikes makes it drop a card's whole screen: kept apart, a blank card names its own culprit.

Captions are drawn into each page's background picture. Fixed text written the way the reference
shows did not appear on a real MPC, and pictures did.
"""

import os
import shutil
from collections import namedtuple

import test_pictures
from card_sources import (KNOBS_THAT_HOLD_A_VALUE, LOOKS_AT_THE_SCREEN, PICTURES_AT_FOUR_SPEEDS, PICTURES_THAT_MOVE, PLAYS_A_STATION,
                          REACHES_THE_INTERNET, write_readme, write_source)
from skin_parts import (ACCENT, CENTRED, KNOB_OVERLAY, NAME_HEIGHT, VALUE_HEIGHT, WHITE, FilmStrip, Page, bounds_of, box,
                        colour_block, control, display, entry, fixed_words, font, kit_strip, knob_component,
                        knob_overlay_component, live_words, named_knob_component, obj, parameter, picture, picture_data,
                        picture_strip, shown, strip_component, text_style, untouchable_component, write_skin)

HERE = os.path.dirname(os.path.abspath(__file__))
SHIPPED_FONT_FILE = os.path.basename(test_pictures.OWN_FONT_PATH)
SHIPPED_FONT_FAMILY = "JetBrainsMono NF"
SHIPPED_FONT_LICENCE = "/opt/homebrew/Caskroom/font-jetbrains-mono-nerd-font/3.5.1/OFL.txt"

Note = namedtuple("Note", "text x y")
TestPage = namedtuple("TestPage", "name parts turned_by_knobs group_boxes on_ruler", defaults=((), (), False))
Card = namedtuple("Card", "name pages ships_font shared_plugin", defaults=(False, KNOBS_THAT_HOLD_A_VALUE))

# Parameter 0 is Volume; parameter n is "Knob n+1".
KNOB_2, KNOB_3, KNOB_4, KNOB_5, KNOB_6 = 1, 2, 3, 4, 5
WHOLE_PAGE = box(0, 0, 1280, 628)
SEE_THROUGH_RED = "80ff3b30"
SOLID_GREEN = "ff3cff78"
RED, CYAN, PINK, YELLOW, ORANGE = "ffff3b30", "ff00d0ff", "ffff00aa", "ffffdc00", "ffff9500"
PANEL = "ff2c2c31"
HELLO_COMPONENT = "hello word"
NAMED_KNOB = "named knob"
WORDED_KNOB = "worded knob"
NAMED_KNOB_WIDTH = 160
NEW_NAME = "SPEED"

# Knob sizes on screen. The big kit strip is 90 x 90 and the small one 56 x 56; the others ask the MPC to resize it.
NATURAL, SHRUNK, ENLARGED, SQUASHED, LITTLE = (90, 90), (54, 54), (231, 231), (240, 80), (56, 56)
KIT_STRIP_FOR = {NATURAL: "knob_uni", SHRUNK: "knob_uni", ENLARGED: "knob_uni", SQUASHED: "knob_uni", LITTLE: "knob_small_uni"}


# --------------------------------------------------------------------------------------------
# Parts every card can use
# --------------------------------------------------------------------------------------------
def sized_knob(size):
    return "knob %dx%d" % size


def knob(parameter_index, x, y, size=LITTLE):
    width, height = size
    return control(sized_knob(size), parameter_index, box(x, y, width, height + VALUE_HEIGHT))


def turn_me(parameter_index, x, y):
    below = y + LITTLE[1] + VALUE_HEIGHT + 6
    return [knob(parameter_index, x, y), Note("turn Knob %d" % (parameter_index + 1), x - 30, below)]


def named_knob(parameter_index, x, y, shown_name=None, component=NAMED_KNOB):
    height = LITTLE[1] + NAME_HEIGHT + VALUE_HEIGHT
    return control(component, parameter_index, box(x, y, NAMED_KNOB_WIDTH, height), shown_name)


def hello_component():
    word = shown("word", "Label", fixed_words("Hello 4", text_style(font(height=32.0)), "Data"), box(0, 0, 700, 50))
    return untouchable_component([word])


def test_strip(file):
    shape = test_pictures.STRIPS[file]
    return FilmStrip(file, shape.frames, shape.width, shape.height)


def shows(file):
    return "shows " + file


def components(folder, card):
    strips = {name: kit_strip(name, folder) for name in set(KIT_STRIP_FOR.values())}
    parts = {sized_knob(size): knob_component(strips[strip], *size) for size, strip in KIT_STRIP_FOR.items()}
    parts[HELLO_COMPONENT] = hello_component()
    parts[NAMED_KNOB] = named_knob_component(strips[KIT_STRIP_FOR[LITTLE]], NAMED_KNOB_WIDTH)
    parts[WORDED_KNOB] = named_knob_component(strips[KIT_STRIP_FOR[LITTLE]], NAMED_KNOB_WIDTH, fixed_name=NEW_NAME)
    parts[KNOB_OVERLAY] = knob_overlay_component()
    moving_strips = STRIPS_SHOWN_BY.get(card.shared_plugin, [])
    parts.update({shows(file): strip_component(test_strip(file)) for file in moving_strips})
    return parts


# --------------------------------------------------------------------------------------------
# Test Card: placing, stretching, text, fonts and the knob groups
# --------------------------------------------------------------------------------------------
def place_page():
    return TestPage("PLACE", [
        knob(KNOB_2, 120, 40, SHRUNK), Note("1 knob made small, 54", 60, 130),
        knob(KNOB_3, 300, 40), Note("2 knob at its own size", 260, 166),
        knob(KNOB_4, 480, 40, ENLARGED), Note("3 knob made big, 231", 480, 306),
        knob(KNOB_5, 760, 40, SQUASHED), Note("4 knob squashed, 240 x 80", 760, 156),
        knob(KNOB_6, 1160, 40), Note("5 knob at the edge", 1090, 166),
        picture("disc.png", box(60, 380, 200, 200)), Note("6 picture, own size", 60, 588),
        picture("disc.png", box(300, 420, 400, 120)), Note("7 same picture, stretched", 300, 588),
        colour_block(SEE_THROUGH_RED, box(760, 200, 300, 90)), Note("8 see-through red block", 760, 294),
        picture("glass.png", box(760, 380, 200, 200)), Note("9 white fading to clear", 760, 588),
    ], turned_by_knobs=[KNOB_2, KNOB_3, KNOB_4, KNOB_5, KNOB_6], on_ruler=True)


TEXT_ROW_HEIGHT = 60
SAMPLE_PLACE_X, SAMPLE_WIDTH, SAMPLE_HEIGHT = 560, 700, 50


def sample_place(row):
    return box(SAMPLE_PLACE_X, 14 + row * TEXT_ROW_HEIGHT, SAMPLE_WIDTH, SAMPLE_HEIGHT)


def text_tries():
    """Ways of putting words on the screen, each paired with what the caption says about it."""
    plain = text_style(font(height=32.0))
    as_the_generator_writes = text_style(font(height=30.0), WHITE, "left")
    orange_at_the_right = text_style(font(height=32.0), ORANGE, "right verticallyCentred")
    bold = text_style(font(style="Bold", height=32.0))
    knob_2 = parameter(KNOB_2)
    return [
        ("1 fixed words, the generator's way: Hello 1", "Label", fixed_words("Hello 1", as_the_generator_writes, "Data")),
        ("2 fixed words, the reference's way: Hello 2", "Label", fixed_words("Hello 2", plain, "Invalid")),
        ("3 fixed words tied to Knob 2: Hello 3", "Label", fixed_words("Hello 3", plain, knob_2)),
        ("4 fixed words inside a component: Hello 4", HELLO_COMPONENT, obj(("version", 1), ("handleName", "Data"))),
        ("5 a knob's own name: Knob 2", "Label", live_words("Name", plain, knob_2)),
        ("6 a knob's name, replaced: Renamed", "Label", live_words("Name", plain, "WithName[Renamed]/" + knob_2)),
        ("7 a knob's value, size 20", "Label", live_words("Value", text_style(font(height=20.0)), knob_2)),
        ("8 a knob's value, size 40", "Label", live_words("Value", text_style(font(height=40.0)), knob_2)),
        ("9 a value in orange, at the right end", "Label", live_words("Value", orange_at_the_right, knob_2)),
        ("10 a value in bold (compare with 9)", "Label", live_words("Value", bold, knob_2)),
    ]


def text_row(row, caption, widget, data):
    """A caption, then a grey panel showing where the words are allowed to be, then the words themselves."""
    tied_to_knob_2 = [("Data", parameter(KNOB_2))]
    sample = entry(caption, widget, data, bounds_of(sample_place(row)), tied_to_knob_2)
    return [Note(caption, 24, 26 + row * TEXT_ROW_HEIGHT), colour_block(PANEL, sample_place(row)), sample]


def text_page():
    rows = [text_row(row, *attempt) for row, attempt in enumerate(text_tries())]
    return TestPage("TEXT", [part for row in rows for part in row])


FONTS_TO_TRY = [
    ("Titillium Web", "the MPC's own font"),
    ("Roboto", "built in"),
    ("Nexa", "built in"),
    (SHIPPED_FONT_FAMILY, "shipped font, by its family name"),
    (SHIPPED_FONT_FILE, "shipped font, by its file name"),
    (os.path.splitext(SHIPPED_FONT_FILE)[0], "shipped font, file name, no ending"),
    ("DejaVu Sans Mono", "not shipped, common on Linux"),
    ("Courier", "not shipped"),
]
FONT_SAMPLE = "Illicit WAVE 0123"
FONT_ROW_HEIGHT = 74


def font_row(row, name, about):
    """Three samples in one font. The name and the value are sure to draw; the fixed words may not."""
    y = 16 + row * FONT_ROW_HEIGHT
    style = text_style(font(name, height=40.0))
    knob_2 = parameter(KNOB_2)
    return [
        Note("%d  %s" % (row + 1, name), 20, y),
        Note(about, 20, y + 28),
        shown("words in " + name, "Label", fixed_words(FONT_SAMPLE, style, knob_2), box(400, y, 430, 60)),
        shown("name in " + name, "Label", live_words("Name", style, knob_2), box(850, y, 200, 60)),
        shown("value in " + name, "Label", live_words("Value", style, knob_2), box(1070, y, 200, 60)),
    ]


def fonts_page():
    rows = [font_row(row, name, about) for row, (name, about) in enumerate(FONTS_TO_TRY)]
    return TestPage("FONTS", [part for row in rows for part in row])


# Four groups of four knobs. Each group gets a box of a different shape, and the last group is scattered.
GROUP_BOXES = [box(30, 20, 620, 130), box(700, 20, 300, 240), box(30, 190, 160, 430), box(240, 280, 760, 340)]
GROUP_LABEL_PLACES = [(36, 24), (706, 24), (36, 194), (246, 284)]
GROUP_KNOB_PLACES = [
    [(77, 54), (227, 54), (377, 54), (527, 54)],
    [(757, 54), (877, 54), (757, 156), (877, 156)],
    [(82, 226), (82, 324), (82, 422), (82, 520)],
    [(287, 330), (537, 310), (717, 430), (897, 510)],
]


def group_parts(group):
    x, y = GROUP_LABEL_PLACES[group]
    knobs = [knob(group * 4 + position, knob_x, knob_y) for position, (knob_x, knob_y) in enumerate(GROUP_KNOB_PLACES[group])]
    return [Note("group %d" % (group + 1), x, y)] + knobs


def groups_page(name, group_boxes):
    parts = [part for group in range(4) for part in group_parts(group)]
    return TestPage(name, parts, turned_by_knobs=list(range(16)), group_boxes=group_boxes)


def more_page():
    by_shares_of_the_page = bounds_of("0.5 0.5 0.25 0.25", boundsType="Proportional")
    the_mpcs_own_number = live_words("Value", text_style(font(height=44.0), WHITE, CENTRED), parameter(KNOB_2))
    return TestPage("MORE", [
        picture("disc.jpg", box(60, 60, 200, 200)), Note("1 a JPEG picture", 60, 266),
        entry("placed by shares", "Image", picture_data("", SOLID_GREEN), by_shares_of_the_page),
        Note("2 green: 640 to 960 across, 314 to 471 down", 640, 484),
        picture_strip(test_strip("number_strip.png"), KNOB_2, box(60, 320, 120, 60)), Note("3 number in our own font", 200, 338),
        shown("the MPC's own number", "Label", the_mpcs_own_number, box(60, 400, 120, 60)), Note("4 the MPC's own number", 200, 418),
        picture("own_font_words.png", box(60, 490, 520, 50)), Note("5 fixed words in our own font", 60, 548),
        named_knob(KNOB_4, 300, 60), Note("6 its own name", 300, 190),
        named_knob(KNOB_5, 520, 60, shown_name=NEW_NAME), Note("7 renamed by its link", 520, 190),
        named_knob(KNOB_6, 740, 60, component=WORDED_KNOB), Note("8 name swapped for words", 740, 190),
    ] + turn_me(KNOB_2, 500, 330), turned_by_knobs=[KNOB_2, KNOB_4, KNOB_5, KNOB_6], on_ruler=True)


# --------------------------------------------------------------------------------------------
# The Draw cards: one kind of less tested widget each
# --------------------------------------------------------------------------------------------
def shape(path, colour, opacity=1.0, draw_path=True, handle="Invalid"):
    return obj(("version", 1), ("handleName", handle), ("path", path), ("drawPath", draw_path), ("colour", colour), ("opacity", opacity))


SQUARE = "m 0 0 l 100 0 l 100 100 l 0 100 z"
TRIANGLE = "m 100 0 l 200 200 l 0 200 z"
WAVE = "m 0 100 q 50 0 100 100 q 150 200 200 100 c 200 0 100 0 100 50"


def path_page():
    return TestPage("PATH", [
        shown("square", "Path", shape(SQUARE, "ff6a6a6c"), box(60, 60, 200, 200)), Note("1 grey square, 100 wide", 60, 266),
        shown("triangle", "Path", shape(TRIANGLE, RED), box(320, 60, 200, 200)), Note("2 red triangle", 320, 266),
        shown("faint triangle", "Path", shape(TRIANGLE, RED, opacity=0.4), box(580, 60, 200, 200)),
        Note("3 same, see-through", 580, 266),
        shown("wave", "Path", shape(WAVE, CYAN), box(840, 60, 200, 200)), Note("4 blue curved line", 840, 266),
        shown("undrawn triangle", "Path", shape(TRIANGLE, RED, draw_path=False), box(60, 340, 200, 200)),
        Note("5 triangle, drawing off", 60, 546),
        shown("turned triangle", "Path", shape(TRIANGLE, YELLOW, handle=parameter(KNOB_2)), box(320, 340, 200, 200)),
        Note("6 yellow, follows Knob 2", 320, 546),
    ] + turn_me(KNOB_2, 660, 400), turned_by_knobs=[KNOB_2])


def plot(line_start, line_end, fill_start, fill_end, sideways):
    return obj(("version", 1), ("backgroundColour", PANEL),
               ("lineColourStart", line_start), ("lineColourEnd", line_end), ("horizLineGradient", sideways), ("lineWidth", 3.0),
               ("fillColourStart", fill_start), ("fillColourEnd", fill_end), ("horizFillGradient", sideways),
               ("fillZero", 0.0), ("dataHandle", parameter(KNOB_2)))


def plot_page():
    return TestPage("PLOT", [
        shown("plot", "Plotter", plot("ffd31145", "ffd31145", "40d31145", "00d31145", False), box(60, 60, 500, 200)),
        Note("1 red line, faint red below", 60, 266),
        shown("fade down", "Plotter", plot(WHITE, WHITE, CYAN, PINK, False), box(660, 60, 500, 200)),
        Note("2 fill fades blue to pink, top to bottom", 660, 266),
        shown("fade across", "Plotter", plot(YELLOW, RED, CYAN, PINK, True), box(60, 340, 500, 200)),
        Note("3 fill fades blue to pink, left to right", 60, 546),
    ] + turn_me(KNOB_2, 780, 400), turned_by_knobs=[KNOB_2])


def turning_picture(scale):
    return obj(("version", 4), ("knobType", "RotatingImage"), ("foregroundImage", "knob_pointer.png"),
               ("backgroundImage", "knob_body.png"), ("imageScale", scale), ("dragOrientation", "Vertical"),
               ("handleName", parameter(KNOB_2)))


def built_in_knob():
    return obj(("version", 5), ("knobType", "GenericNoLabel"), ("polarity", "UniPolar"), ("dragOrientation", "Vertical"),
               ("handleName", parameter(KNOB_2)))


def value_to_drag():
    centred = text_style(font(height=28.0), WHITE, CENTRED)
    return obj(("version", 4), ("knobType", "ValueSlider"), ("textStyle", centred), ("dragOrientation", "Vertical"),
               ("handleName", parameter(KNOB_2)))


def knobs_page():
    return TestPage("KNOBS", [
        shown("turning picture", "Knob", turning_picture(1.0), box(60, 60, 120, 120)), Note("1 pointer turns", 60, 190),
        shown("turning picture x2", "Knob", turning_picture(2.0), box(260, 60, 240, 240)), Note("2 same, twice the size", 260, 310),
        shown("built-in knob", "Knob", built_in_knob(), box(580, 60, 120, 120)), Note("3 the MPC's own knob", 560, 190),
        shown("value to drag", "Knob", value_to_drag(), box(800, 90, 200, 60)), Note("4 number you can drag", 800, 190),
    ] + turn_me(KNOB_2, 660, 400), turned_by_knobs=[KNOB_2])


def guess_page():
    return TestPage("GUESS", [
        picture("arrow.png", box(60, 60, 160, 160)), Note("1 arrow pointing up", 60, 230),
        picture("arrow.png", box(320, 60, 160, 160), ("rotation", 45.0)), Note("2 turned, if it leans", 320, 230),
        picture("arrow.png", box(580, 60, 160, 160), ("opacity", 0.3)), Note("3 faint, if it is paler", 580, 230),
    ])


def halo(style):
    return obj(("version", 1), ("lineWidth", 5.0), ("lineColour", ACCENT), ("clipColour", RED), ("style", style),
               ("handleName", parameter(KNOB_2)), ("arcMin", -0.67), ("arcMax", 0.67))


def lamp(steps):
    return obj(("version", 1), ("onImage", "lamp_on.png"), ("offImage", "lamp_off.png"), ("indicatorId", 1),
               ("numIndicatorsInGroup", steps), ("handleName", parameter(KNOB_2)))


def halo_page():
    return TestPage("HALO", [
        shown("round halo", "HaloDisplay", halo("Circular"), box(60, 60, 160, 160)), Note("1 blue arc", 60, 230),
        shown("flat halo", "HaloDisplay", halo("Horizontal"), box(300, 120, 300, 40)), Note("2 blue bar, sideways", 300, 230),
        shown("tall halo", "HaloDisplay", halo("Vertical"), box(700, 60, 40, 160)), Note("3 blue bar, upright", 660, 230),
        shown("lamp", "Indicator", lamp(1), box(80, 380, 40, 40)), Note("4 lamp", 60, 430),
        shown("lamp of two", "Indicator", lamp(2), box(220, 380, 40, 40)), Note("5 lamp", 200, 430),
    ] + turn_me(KNOB_2, 660, 400), turned_by_knobs=[KNOB_2])


def xy_page():
    pad = obj(("version", 1), ("xPositionHandle", parameter(KNOB_2)), ("yPositionHandle", parameter(KNOB_3)), ("image", "puck.png"))
    return TestPage("XY", [
        colour_block(PANEL, box(60, 40, 520, 520)),
        shown("pad", "XYPad", pad, box(60, 40, 520, 520)),
        Note("1 drag the dot: across is Knob 2, up and down is Knob 3", 60, 570),
    ] + turn_me(KNOB_2, 720, 100) + turn_me(KNOB_3, 900, 100), turned_by_knobs=[KNOB_2, KNOB_3])


def meter(direction, picture_start):
    return obj(("version", 3), ("direction", direction), ("invert", False),
               ("inactiveImage", picture_start + "off.png"), ("peakImage", picture_start + "peak.png"), ("rmsImage", picture_start + "rms.png"),
               ("peakHandle", parameter(KNOB_2)), ("rmsHandle", parameter(KNOB_3)), ("peakHoldHandle", "Hold/" + parameter(KNOB_2)),
               ("peakHoldSize", 10), ("peakHoldColour", WHITE))


def meter_page():
    return TestPage("METER", [
        shown("tall meter", "Meter", meter("Up", "meter_"), box(100, 60, 40, 300)),
        Note("1 green rises with Knob 2, yellow with Knob 3", 60, 372),
        shown("wide meter", "Meter", meter("Right", "wide_meter_"), box(260, 120, 300, 40)),
        Note("2 the same, growing to the right", 260, 172),
    ] + turn_me(KNOB_2, 720, 100) + turn_me(KNOB_3, 900, 100), turned_by_knobs=[KNOB_2, KNOB_3])


# Draw Live's parameters, in the order shared/live_card_plugin.h declares them.
LIVE_SPEED, LIVE_SWEEP, LIVE_LEVEL, LIVE_FIRST_WAVE_COLUMN = 1, 2, 3, 4
LIVE_WAVE_COLUMNS = 24
BAR, DOT, COLUMN, SPOKE = "bar_strip.png", "dot_strip.png", "column_strip.png", "spoke_strip.png"
LIVE_STRIPS = [BAR, DOT, COLUMN]


def travelling_wave(x, y):
    return [display(shows(COLUMN), LIVE_FIRST_WAVE_COLUMN + column, box(x + column * 20, y, 16, 160))
            for column in range(LIVE_WAVE_COLUMNS)]


def live_page():
    counting = live_words("Value", text_style(font(height=40.0)), parameter(LIVE_SWEEP))
    return TestPage("LIVE", [
        knob(LIVE_SPEED, 80, 70), Note("1 speed, pictures a second", 30, 170),
        display(shows(BAR), LIVE_SWEEP, box(330, 40, 40, 200)), Note("2 bar moves by itself", 270, 250),
        picture_strip(test_strip(BAR), LIVE_SWEEP, box(590, 40, 40, 200)), Note("3 same, as a picture strip", 510, 250),
        shown("counting", "Label", counting, box(800, 100, 200, 60)), Note("4 number counts by itself", 780, 250),
        display(shows(BAR), LIVE_LEVEL, box(1140, 40, 40, 200)), Note("5 follows the sound", 1070, 250),
        display(shows(DOT), LIVE_SWEEP, box(60, 320, 600, 40)), Note("6 dot slides by itself", 60, 370),
        picture_strip(test_strip(SPOKE), LIVE_SWEEP, box(60, 460, 80, 80), loops_by_itself=True),
        Note("8 spins with no help from the plugin", 160, 490),
        Note("7 a wave that travels", 720, 490),
    ] + travelling_wave(720, 310), turned_by_knobs=[LIVE_SPEED])


# Draw Fast's parameters, in the order shared/fast_card_plugin.h declares them.
FAST_OWN_SPEED, FAST_FIRST_SWEEP = 1, 2
QUICK_DOT, QUICK_SPOKE = "quick_dot_strip.png", "quick_spoke_strip.png"
FAST_ROWS = ["1  30 a second", "2  45 a second", "3  60 a second", "4  as fast as the knob says"]
FAST_ROW_TOP, FAST_ROW_STEP = 60, 130
STRIPS_SHOWN_BY = {PICTURES_THAT_MOVE: LIVE_STRIPS, PICTURES_AT_FOUR_SPEEDS: [QUICK_DOT, QUICK_SPOKE]}


def speed_row(row, caption):
    """The same dot and spoke on every row. Only how often a second they are given a new picture differs."""
    sweep, top = FAST_FIRST_SWEEP + row, FAST_ROW_TOP + row * FAST_ROW_STEP
    return [Note(caption, 30, top + 20),
            display(shows(QUICK_DOT), sweep, box(330, top + 18, 400, 28)),
            display(shows(QUICK_SPOKE), sweep, box(790, top, 64, 64))]


def fast_page():
    rows = [part for row, caption in enumerate(FAST_ROWS) for part in speed_row(row, caption)]
    own_speed = [knob(FAST_OWN_SPEED, 1010, 420), Note("turn: 1 to 240 pictures a second", 900, 530)]
    return TestPage("FAST", rows + own_speed, turned_by_knobs=[FAST_OWN_SPEED])


# Net Check's parameters, in the order shared/net_card_plugin.h declares them, each with what its number says.
NET_READINGS = [
    "1  how far the last try got",
    "2  the web server's answer (200 means it answered)",
    "3  tries made, one every ten seconds",
    "4  tries that were answered",
    "5  how long the last try took, in ms",
    "6  counts up while the card's own thread runs",
]
# Radio Check's parameters, in the order shared/radio_card_plugin.h declares them.
RADIO_READINGS = [
    "1  how far it has got with the station",
    "2  the station's answer (200 means here it is)",
    "3  seconds of the station played",
    "4  times the sound ran out and waited",
    "5  how full the store of waiting sound is",
    "6  the station's sample rate (the MPC's is 44100)",
    "7  frames the MPC asked for in the last second",
    "8  calls it asked for them in",
    "9  fewest frames in one call",
    "10  most frames in one call",
]
# On both cards the readings follow Volume, which is parameter 0.
FIRST_READING = 1
READING_ROWS_TOP, READING_ROWS_HEIGHT, TALLEST_READING_ROW = 30, 570, 90


def reading_row(row, row_height, caption, shown_parameter):
    top = READING_ROWS_TOP + row * row_height
    lettering = text_style(font(height=min(40.0, row_height * 0.6)))
    reading = live_words("Value", lettering, parameter(shown_parameter))
    return [Note(caption, 40, top + row_height // 2 - 12), shown("reading %d" % (row + 1), "Label", reading, box(720, top, 500, row_height))]


def reading_rows(captions, first_reading=FIRST_READING):
    """What a card found out, one numbered reading to a row. The rows share the page's height."""
    row_height = min(TALLEST_READING_ROW, READING_ROWS_HEIGHT // len(captions))
    return [part for row, caption in enumerate(captions) for part in reading_row(row, row_height, caption, first_reading + row)]


def readings_page(name, captions):
    return TestPage(name, reading_rows(captions))


# Screen Probe's parameters, in the order shared/screen_probe_plugin.h declares them: two knobs, then the readings.
PROBE_PAINT, PROBE_FRESH, PROBE_FIRST_SCREEN_READING = 1, 2, 3
PROBE_SCREEN_READINGS = [
    "1  the screen's file",
    "2  the name it gives",
    "3  dots across",
    "4  dots down",
    "5  bits in a dot",
    "6  first bit of red, green, blue",
    "7  the paint test",
    "8  kept, painted once (%)",
    "9  kept, painted over (%)",
]
PROBE_CALL_READINGS = [
    "1  text asked for, last second",
    "2  longest quiet of those, in s",
    "3  values read, last second",
    "4  longest quiet of those, in s",
    "5  idle calls, last second",
    "6  longest quiet of those, in s",
    "7  other calls, last second",
    "8  longest quiet of those, in s",
    "9  number of the last other call",
    "10  counts up once a second",
]
PROBE_FIRST_CALL_READING = PROBE_FIRST_SCREEN_READING + len(PROBE_SCREEN_READINGS)


def probe_knob(parameter_index, what_turning_it_does):
    """Between the captions and the readings, below where the painted block is expected to land."""
    return [knob(parameter_index, 520, 470), Note(what_turning_it_does, 430, 572)]


def probe_screen_page():
    """On the ruler, so a photo shows where on the page the dots of the screen's file land."""
    parts = reading_rows(PROBE_SCREEN_READINGS, PROBE_FIRST_SCREEN_READING) + probe_knob(PROBE_PAINT, "past halfway: paint")
    return TestPage("SCREEN", parts, turned_by_knobs=[PROBE_PAINT], on_ruler=True)


def probe_calls_page():
    parts = reading_rows(PROBE_CALL_READINGS, PROBE_FIRST_CALL_READING) + probe_knob(PROBE_FRESH, "turn: count afresh")
    return TestPage("CALLS", parts, turned_by_knobs=[PROBE_FRESH])


CARDS = [
    Card("Test Card", [place_page(), text_page(), fonts_page(), groups_page("GROUPS", GROUP_BOXES), groups_page("NO BOX", []),
                       more_page()], ships_font=True),
    Card("Draw Path", [path_page()]),
    Card("Draw Plot", [plot_page()]),
    Card("Draw Knobs", [knobs_page()]),
    Card("Draw Guess", [guess_page()]),
    Card("Draw Extras", [halo_page(), xy_page(), meter_page()]),
    Card("Draw Live", [live_page()], shared_plugin=PICTURES_THAT_MOVE),
    Card("Draw Fast", [fast_page()], shared_plugin=PICTURES_AT_FOUR_SPEEDS),
    Card("Net Check", [readings_page("ONLINE", NET_READINGS)], shared_plugin=REACHES_THE_INTERNET),
    Card("Radio Check", [readings_page("RADIO", RADIO_READINGS)], shared_plugin=PLAYS_A_STATION),
    Card("Screen Probe", [probe_screen_page(), probe_calls_page()], shared_plugin=LOOKS_AT_THE_SCREEN),
]


# --------------------------------------------------------------------------------------------
# Writing the cards
# --------------------------------------------------------------------------------------------
def notes_file(page):
    return "notes_%s.png" % page.name.lower().replace(" ", "_")


def with_notes_baked_in(page, folder):
    notes = [part for part in page.parts if isinstance(part, Note)]
    entries = [part for part in page.parts if not isinstance(part, Note)]
    test_pictures.page_with_notes(notes, page.on_ruler).save(os.path.join(folder, notes_file(page)))
    return Page(page.name, [picture(notes_file(page), WHOLE_PAGE)] + entries, page.turned_by_knobs, page.group_boxes)


def empty_skin_folder(card_folder):
    folder = os.path.join(card_folder, "skins")
    shutil.rmtree(folder, ignore_errors=True)
    os.makedirs(folder)
    return folder


def ship_the_font(folder):
    shutil.copy(test_pictures.OWN_FONT_PATH, folder)
    shutil.copy(SHIPPED_FONT_LICENCE, os.path.join(folder, "JetBrainsMono-OFL.txt"))


def write_card(card):
    card_folder = os.path.join(HERE, card.name)
    write_source(card_folder, card.shared_plugin)
    write_readme(card_folder)
    folder = empty_skin_folder(card_folder)
    pages = [with_notes_baked_in(page, folder) for page in card.pages]
    pictures_used = write_skin(folder, pages, components(folder, card), card.shared_plugin.parameters)
    test_pictures.save_pictures(pictures_used, folder)
    if card.ships_font:
        ship_the_font(folder)


if __name__ == "__main__":
    for card in CARDS:
        write_card(card)
    print("Wrote %d test cards: %s." % (len(CARDS), ", ".join(card.name for card in CARDS)))
