"""Draws the README's pictures in MP Forge's own look: its palette, hard edges and pixel letters.

GitHub loads no web fonts inside an SVG, so every letter is drawn as squares.

Run it from the docs folder, naming the folder the pictures go in:  python3 tools/make_readme_art.py public/images
"""
import sys
from itertools import groupby
from pathlib import Path

ACCENT, ACCENT_INK = "#b5271f", "#ffffff"
FACE, WELL, LINE, INK, SOFT, HI, LO = "#34323d", "#18171d", "#0c0b10", "#ecebf2", "#b0adbd", "#5d5a6b", "#1f1e26"
STEP_RESTING = "#8b879c"
LIGHT_EDGE, DARK_EDGE = ("#ffffff", "0.35"), ("#000000", "0.3")

LETTERS = {
    "A": [".##.", "#..#", "####", "#..#", "#..#"], "B": ["###.", "#..#", "###.", "#..#", "###."],
    "C": [".###", "#...", "#...", "#...", ".###"], "D": ["###.", "#..#", "#..#", "#..#", "###."],
    "E": ["####", "#...", "###.", "#...", "####"], "F": ["####", "#...", "###.", "#...", "#..."],
    "G": [".###", "#...", "#.##", "#..#", ".###"], "H": ["#..#", "#..#", "####", "#..#", "#..#"],
    "I": ["###", ".#.", ".#.", ".#.", "###"], "J": ["..##", "...#", "...#", "#..#", ".##."],
    "K": ["#..#", "#.#.", "##..", "#.#.", "#..#"], "L": ["#...", "#...", "#...", "#...", "####"],
    "M": ["#...#", "##.##", "#.#.#", "#...#", "#...#"], "N": ["#...#", "##..#", "#.#.#", "#..##", "#...#"],
    "O": [".##.", "#..#", "#..#", "#..#", ".##."], "P": ["###.", "#..#", "###.", "#...", "#..."],
    "Q": [".##.", "#..#", "#..#", "#.#.", ".#.#"], "R": ["###.", "#..#", "###.", "#.#.", "#..#"],
    "S": [".###", "#...", ".##.", "...#", "###."], "T": ["###", ".#.", ".#.", ".#.", ".#."],
    "U": ["#..#", "#..#", "#..#", "#..#", ".##."], "V": ["#...#", "#...#", "#...#", ".#.#.", "..#.."],
    "W": ["#...#", "#...#", "#.#.#", "##.##", "#...#"], "X": ["#...#", ".#.#.", "..#..", ".#.#.", "#...#"],
    "Y": ["#...#", ".#.#.", "..#..", "..#..", "..#.."], "Z": ["####", "...#", ".##.", "#...", "####"],
    "0": [".##.", "#..#", "#..#", "#..#", ".##."], "1": [".#.", "##.", ".#.", ".#.", "###"],
    "2": ["###.", "...#", ".##.", "#...", "####"], "3": ["###.", "...#", ".##.", "...#", "###."],
    "4": ["#..#", "#..#", "####", "...#", "...#"], "5": ["####", "#...", "###.", "...#", "###."],
    "6": [".##.", "#...", "###.", "#..#", ".##."], "7": ["####", "...#", "..#.", ".#..", ".#.."],
    "8": [".##.", "#..#", ".##.", "#..#", ".##."], "9": [".##.", "#..#", ".###", "...#", ".##."],
    ".": [".", ".", ".", ".", "#"], "+": ["...", ".#.", "###", ".#.", "..."], " ": ["..", "..", "..", "..", ".."],
    "-": ["...", "...", "###", "...", "..."],
}
LETTER_ROWS = 5
GAP_BETWEEN_LETTERS = 1

# The app's own icons, drawn on its sixteen-by-sixteen grid.
PLAY = "M5 3h2v1h2v1h2v1h1v1h1v2h-1v1h-1v1H9v1H7v1H5z"
SEND = "M7 2h2v1h1v1h1v1h1v1h-2v4H6V6H4V5h1V4h1V3h1zM2 11h2v2h8v-2h2v4H2z"
COG = "M7 1h2v2h1v1h1V3h2v2h-1v1h1v1h2v2h-2v1h-1v1h1v2h-2v-1h-1v1H9v2H7v-2H6v-1H5v1H3v-2h1v-1H3V9H1V7h2V6h1V5H3V3h2v1h1V3h1zM7 6v1H6v2h1v1h2V9h1V7H9V6z"
SPEECH = "M2 2h12v9H8v1H7v1H6v1H5v1H4v-4H2zM4 4v5h8V4z"
ICON_GRID = 16

# A cup with a heart on it, the sign of a tip on Ko-fi, written out as it looks: a letter to a square.
CUP = [
    ".KKKKKKKKKKK....",
    ".KWWWWWWWWWKKK..",
    ".KWWWWWWWWWKWWK.",
    ".KWWRRWRRWWKWWK.",
    ".KWRRRRRRRWKWWK.",
    ".KWRRRRRRRWKKK..",
    ".KWWRRRRRWWK....",
    ".KWWWRRRWWWK....",
    ".KWWWWRWWWWK....",
    ".KWWWWWWWWWK....",
    "..KKKKKKKKK.....",
]
CUP_COLOURS = {"K": LINE, "W": "#ffffff", "R": ACCENT}


def rect(x, y, width, height, fill, opacity=None):
    see_through = f' fill-opacity="{opacity}"' if opacity else ""
    return f'<rect x="{x}" y="{y}" width="{width}" height="{height}" fill="{fill}"{see_through}/>'


def width_of(words):
    return sum(len(LETTERS[letter][0]) for letter in words) + GAP_BETWEEN_LETTERS * (len(words) - 1)


def runs_in(row):
    """Each unbroken run of filled squares in one row of a letter, as (start, length)."""
    runs, start = [], None
    for column, mark in enumerate(row + "."):
        if mark == "#" and start is None:
            start = column
        if mark != "#" and start is not None:
            runs.append((start, column - start))
            start = None
    return runs


def lettering(words, x, y, square, fill):
    squares, across = [], 0
    for letter in words:
        rows = LETTERS[letter]
        for down, row in enumerate(rows):
            squares += [rect(x + (across + start) * square, y + down * square, length * square, square, fill) for start, length in runs_in(row)]
        across += len(rows[0]) + GAP_BETWEEN_LETTERS
    return "".join(squares)


def bevel(x, y, width, height, edge, light, dark):
    """The lit top and left edges and the shaded bottom and right edges of a raised part."""
    (light_fill, light_opacity), (dark_fill, dark_opacity) = light, dark
    return "".join([
        rect(x, y, width, edge, light_fill, light_opacity), rect(x, y + edge, edge, height - edge, light_fill, light_opacity),
        rect(x, y + height - edge, width, edge, dark_fill, dark_opacity), rect(x + width - edge, y, edge, height - edge, dark_fill, dark_opacity),
    ])


def icon(path, x, y, size, fill):
    scale = size / ICON_GRID
    return f'<path transform="translate({x} {y}) scale({scale:g})" fill="{fill}" fill-rule="evenodd" d="{path}"/>'


def runs_of_one_colour(row):
    """Each unbroken run of one letter in a row of a drawing, as (letter, start, length)."""
    runs, start = [], 0
    for letter, run in groupby(row):
        length = len(list(run))
        runs.append((letter, start, length))
        start += length
    return runs


def cup(x, y, square):
    return "".join(
        rect(x + start * square, y + down * square, length * square, square, CUP_COLOURS[letter])
        for down, row in enumerate(CUP)
        for letter, start, length in runs_of_one_colour(row)
        if letter in CUP_COLOURS
    )


def picture(width, height, title, parts):
    opening = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}" shape-rendering="crispEdges" role="img" aria-label="{title}">'
    return f"{opening}<title>{title}</title>{''.join(parts)}</svg>\n"


def get_started_button():
    words, square, edge, shadow = "GET STARTED", 4, 3, 6
    pad_across, pad_down = 30, 18
    width = width_of(words) * square + 2 * pad_across
    height = LETTER_ROWS * square + 2 * pad_down
    return picture(width + shadow, height + shadow, "Get started", [
        rect(shadow, shadow, width, height, LINE),
        rect(0, 0, width, height, LINE),
        rect(edge, edge, width - 2 * edge, height - 2 * edge, ACCENT),
        bevel(edge, edge, width - 2 * edge, height - 2 * edge, edge, LIGHT_EDGE, DARK_EDGE),
        lettering(words, pad_across, pad_down, square, ACCENT_INK),
    ])


def top_bar():
    """The app's top bar: its wordmark, tabs, the status readout and the settings cog."""
    square, edge, height, pad = 2, 2, 38, 14
    text_y = (height - LETTER_ROWS * square) // 2
    parts, x = [], edge

    def cell(words, fill, ink, line_after=True):
        nonlocal x
        cell_width = width_of(words) * square + 2 * pad
        if fill:
            parts.append(rect(x, edge, cell_width, height - 2 * edge, fill))
        parts.append(lettering(words, x + pad, text_y, square, ink))
        x += cell_width
        if line_after:
            parts.append(rect(x, edge, edge, height - 2 * edge, LINE))
            x += edge

    tabs = [("MP FORGE", ACCENT, ACCENT_INK), ("FREE AND OPEN SOURCE", WELL, INK), ("PLUGINS FOR AKAI MPC", None, SOFT)]
    status_label, status_word = "STATUS", "SAFE TO SEND"
    chip_pad = 6
    right_width = edge + pad + width_of(status_label) * square + 8 + width_of(status_word) * square + 2 * chip_pad + pad + edge + height
    left_width = sum(width_of(words) * square + 2 * pad + edge for words, _, _ in tabs)
    width = edge + left_width + 24 + right_width
    frame = [rect(0, 0, width, height, LINE), rect(edge, edge, width - 2 * edge, height - 2 * edge, FACE), bevel(edge, edge, width - 2 * edge, height - 2 * edge, edge, (HI, None), (LO, None))]
    for words, fill, ink in tabs:
        cell(words, fill, ink)
    x = width - right_width
    parts.append(rect(x, edge, edge, height - 2 * edge, LINE))
    x += edge + pad
    parts.append(lettering(status_label, x, text_y, square, SOFT))
    x += width_of(status_label) * square + 8
    chip_width = width_of(status_word) * square + 2 * chip_pad
    parts.append(rect(x, text_y - 4, chip_width, LETTER_ROWS * square + 8, ACCENT))
    parts.append(lettering(status_word, x + chip_pad, text_y, square, ACCENT_INK))
    x += chip_width + pad
    parts.append(rect(x, edge, edge, height - 2 * edge, LINE))
    parts.append(icon(COG, x + edge + (height - edge - 18) // 2 - 1, (height - 18) // 2, 18, INK))
    return picture(width, height, "MP Forge: free and open source, plugins for Akai MPC. Status: safe to send.", frame + parts)


def ko_fi_cup():
    """The cup alone, for the website's own donate button."""
    square = 2
    return picture(len(CUP[0]) * square, len(CUP) * square, "", [cup(0, 0, square)])


def donate_button():
    """The app's quiet button with the cup on it."""
    words, square, edge, shadow = "DONATE ON KO-FI", 3, 3, 5
    pad_across, pad_down, between = 20, 14, 14
    cup_width, cup_height = len(CUP[0]) * square, len(CUP) * square
    width = pad_across + cup_width + between + width_of(words) * square + pad_across
    height = cup_height + 2 * pad_down
    return picture(width + shadow, height + shadow, "Donate on Ko-fi", [
        rect(shadow, shadow, width, height, LINE),
        rect(0, 0, width, height, LINE),
        rect(edge, edge, width - 2 * edge, height - 2 * edge, FACE),
        bevel(edge, edge, width - 2 * edge, height - 2 * edge, edge, (HI, None), (LO, None)),
        cup(pad_across, pad_down, square),
        lettering(words, pad_across + cup_width + between, (height - LETTER_ROWS * square) // 2, square, INK),
    ])


def step_row():
    """Sixteen steps in a row, the first of every four lit, to stand between two parts of the page."""
    step, gap, steps = 10, 6, 16
    lit = lambda number: number % 4 == 0
    squares = [rect(number * (step + gap), 0, step, step, ACCENT if lit(number) else STEP_RESTING) for number in range(steps)]
    return picture(steps * step + (steps - 1) * gap, step, "", squares)


def four_pads(x, y, pad, gap):
    lit = [ACCENT, "#6f2422", LO, LO]
    places = [(0, 0), (1, 0), (0, 1), (1, 1)]
    return "".join(rect(x + across * (pad + gap), y + down * (pad + gap), pad, pad, fill) for (across, down), fill in zip(places, lit))


def tile(title, drawing):
    """One of the app's square buttons, with a picture on it."""
    size, edge, shadow = 64, 2, 4
    return picture(size + shadow, size + shadow, title, [
        rect(shadow, shadow, size, size, LINE), rect(0, 0, size, size, LINE),
        rect(edge, edge, size - 2 * edge, size - 2 * edge, WELL),
        bevel(edge, edge, size - 2 * edge, size - 2 * edge, edge, (HI, None), (LO, None)),
        drawing,
    ])


def every_picture():
    return {
        "get-started.svg": get_started_button(),
        "ko-fi-cup.svg": ko_fi_cup(),
        "donate-on-ko-fi.svg": donate_button(),
        "top-bar.svg": top_bar(),
        "step-row.svg": step_row(),
        "step-describe.svg": tile("Describe it", icon(SPEECH, 8, 9, 48, INK)),
        "step-build.svg": tile("Build it", four_pads(14, 14, 16, 4)),
        "step-play.svg": tile("Play it", icon(PLAY, 8, 8, 48, INK)),
        "step-send.svg": tile("Send it", icon(SEND, 8, 8, 48, INK)),
    }


if __name__ == "__main__":
    folder = Path(sys.argv[1])
    folder.mkdir(parents=True, exist_ok=True)
    for name, drawn in every_picture().items():
        (folder / name).write_text(drawn)
        print(name, len(drawn))
