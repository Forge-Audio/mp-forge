"""Building blocks for hand-written MPC screens, on top of the ones in Akai's own generator.

A screen is a list of pages. Each page is a list of entries, and every entry sits at an exact place in
the 1280 x 628 page area. `write_skin` turns the pages into the two files the MPC reads.
"""

import os
import sys
from collections import namedtuple

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "vendor", "mpc-plugin-builder-sdk", "tools"))

import gen_ui  # noqa: E402  (Akai's generator, found through the path added above)
import make_ui_kit  # noqa: E402
from gen_ui import Obj, obj  # noqa: E402

PAGE_COLOUR = "ff1e1d1e"
NO_COLOUR = "0"
ACCENT = "ff35c5e8"
WHITE = "ffffffff"
MPC_FONT = "Titillium Web"
KIT_THEME = "cyan"
KNOB_OVERLAY = "knob overlay"
VALUE_HEIGHT = 32
NAME_HEIGHT = 30
PICTURE_ENDINGS = (".png", ".jpg")
KNOB_COUNT = 16
NO_PARAMETER = -1
# The order the MPC's own default layout hands parameters to the 4 x 4 knobs: down each column, left to right.
KNOBS_DOWN_THE_COLUMNS = [13, 9, 5, 1, 14, 10, 6, 2, 15, 11, 7, 3, 16, 12, 8, 4]

FilmStrip = namedtuple("FilmStrip", "file frames width height")
Page = namedtuple("Page", "name entries turned_by_knobs group_boxes", defaults=((), ()))


def box(x, y, width, height):
    return "%g %g %g %g" % (x, y, width, height)


def parameter(index):
    return "Parameter %d" % index


def bounds_of(place, **changes):
    settings = {"acceptsHWFocus": "No", "showWhenDataModelInvalid": "Show", "whenVisible": "Always", "boundsType": "Absolute"}
    settings.update(changes)
    return obj(("version", 1), *settings.items(), ("bounds", place))


def entry(name, widget, data, bounds, remap=()):
    renamed = [obj(("key", inside), ("value", outside)) for inside, outside in remap]
    return obj(("version", 2),
               ("componentData", obj(("version", 1), ("name", name), ("type", widget), ("data", data))),
               ("handle remapping", obj(("version", 1), ("map", renamed))),
               ("bounds", bounds))


def shown(name, widget, data, place):
    """Something that is only looked at: it never takes the hardware focus."""
    return entry(name, widget, data, bounds_of(place))


def link_to(parameter_index, shown_name=None):
    """What a control is tied to. With `shown_name`, the MPC shows that name in place of the parameter's own."""
    plain = parameter(parameter_index)
    return "WithName[%s]/%s" % (shown_name, plain) if shown_name else plain


def control(component, parameter_index, place, shown_name=None):
    """One of our own components, tied to a parameter and able to take the hardware focus."""
    bounds = bounds_of(place, acceptsHWFocus="Yes", showWhenDataModelInvalid="Hide")
    tied_to = [("Data", link_to(parameter_index, shown_name))]
    return entry(parameter(parameter_index), component, obj(("version", 1), ("handleName", "Data")), bounds, tied_to)


# --------------------------------------------------------------------------------------------
# Words
# --------------------------------------------------------------------------------------------
def font(name=MPC_FONT, style="Regular", height=20.0):
    return obj(("version", 1), ("name", name), ("style", style), ("height", float(height)))


def text_style(lettering, colour=WHITE, justification="left verticallyCentred"):
    return obj(("version", 1), ("font", lettering), ("colour", colour), ("justification", justification), ("case", "Original"))


def fixed_words(text, style, handle="Invalid"):
    return obj(("version", 1), ("textStyle", style), ("type", "Text"), ("handleName", handle), ("text", text))


def live_words(kind, style, handle="Data"):
    """`kind` is "Name" or "Value": the parameter's name, or what it is set to right now."""
    return obj(("version", 1), ("textStyle", style), ("type", kind), ("handleName", handle))


CENTRED = "horizontallyCentred verticallyCentred"


def value_words():
    return live_words("Value", text_style(font(height=24.0), WHITE, CENTRED))


def name_words():
    return live_words("Name", text_style(font(height=24.0), WHITE, CENTRED))


# --------------------------------------------------------------------------------------------
# Pictures and colour
# --------------------------------------------------------------------------------------------
def picture_data(file, colour=NO_COLOUR, *guessed_settings):
    """`guessed_settings` are extra keys the reference does not list, tried to see whether the MPC acts on them."""
    return obj(("version", 2), ("imageType", "Regular"), ("colour", colour), ("image", file), *guessed_settings)


def picture(file, place, *guessed_settings):
    return shown(file, "Image", picture_data(file, NO_COLOUR, *guessed_settings), place)


def colour_block(colour, place):
    return shown("colour " + colour, "Image", picture_data("", colour), place)


# --------------------------------------------------------------------------------------------
# Knobs
# --------------------------------------------------------------------------------------------
def kit_strip(name, folder):
    """Draws one of the SDK kit's film strips into the folder and says what it holds."""
    make_ui_kit.render_kit(folder, KIT_THEME, only=[name], quiet=True)
    os.remove(os.path.join(folder, "kit.json"))
    _, (width, height), frames = make_ui_kit.asset_table()[name]
    return FilmStrip(name + ".png", frames, width, height)


def knob_component(strip, width, height):
    """A film-strip knob drawn at the given size, with its value written underneath."""
    parts = [
        entry("focus", "Focus", gen_ui.focus_data(ACCENT), bounds_of(box(0, 0, width, height + VALUE_HEIGHT), whenVisible="WhenFocussed")),
        shown("knob", "Knob", gen_ui.knob_filmstrip_data(strip.file, strip.frames), box(0, 0, width, height)),
        shown("value", "Label", value_words(), box(0, height, width, VALUE_HEIGHT)),
    ]
    return gen_ui.component_definition(gen_ui.control_actions(KNOB_OVERLAY), parts, "00000000")


def name_as_fixed_words(text):
    """Words that stand where a control's name would be. They are tied to the control so that the MPC draws them."""
    return fixed_words(text, text_style(font(height=24.0), WHITE, CENTRED), "Data")


def named_knob_component(strip, width, fixed_name=None):
    """A film-strip knob in the middle of a wider space, with its name and then its value underneath.

    The name is the parameter's own, or with `fixed_name` those words whatever the parameter is called.
    """
    height = strip.height + NAME_HEIGHT + VALUE_HEIGHT
    name = name_as_fixed_words(fixed_name) if fixed_name else name_words()
    parts = [
        entry("focus", "Focus", gen_ui.focus_data(ACCENT), bounds_of(box(0, 0, width, height), whenVisible="WhenFocussed")),
        shown("knob", "Knob", gen_ui.knob_filmstrip_data(strip.file, strip.frames),
              box((width - strip.width) / 2, 0, strip.width, strip.height)),
        shown("name", "Label", name, box(0, strip.height, width, NAME_HEIGHT)),
        shown("value", "Label", value_words(), box(0, strip.height + NAME_HEIGHT, width, VALUE_HEIGHT)),
    ]
    return gen_ui.component_definition(gen_ui.control_actions(KNOB_OVERLAY), parts, "00000000")


def strip_component(strip):
    """A film strip that shows a value. The MPC draws these with its Knob widget, as other plugins' meters do."""
    frames = shown("strip", "Knob", gen_ui.knob_filmstrip_data(strip.file, strip.frames), box(0, 0, strip.width, strip.height))
    return untouchable_component([frames])


def display(component, parameter_index, place):
    """One of our own components showing a parameter, without taking the hardware focus."""
    tied_to = [("Data", parameter(parameter_index))]
    return entry(parameter(parameter_index), component, obj(("version", 1), ("handleName", "Data")), bounds_of(place), tied_to)


def picture_strip(strip, parameter_index, place, loops_by_itself=False):
    """A film strip drawn by the Image widget. It follows the parameter, or with `loops_by_itself` just cycles."""
    data = obj(("version", 2), ("imageType", "FilmStrip"), ("colour", NO_COLOUR), ("image", strip.file),
               ("numFrames", strip.frames), ("handleName", parameter(parameter_index)), ("animated", loops_by_itself))
    return shown(strip.file, "Image", data, place)


def untouchable_component(parts):
    """A see-through group of parts that does nothing when touched."""
    return gen_ui.component_definition([], parts, "00000000")


def knob_overlay_component():
    return gen_ui.overlay_component("KnobOverlay", gen_ui.knob_overlay_data(), PAGE_COLOUR)


# --------------------------------------------------------------------------------------------
# The two files
# --------------------------------------------------------------------------------------------
def tab(page, key_index):
    return obj(("version", 1), ("tabName", page.name), ("fnKeyIndex", key_index), ("fnKeySubIndex", 0),
               ("componentDefinition", gen_ui.component_definition([], list(page.entries), PAGE_COLOUR)),
               ("qlinkBoundsData", list(page.group_boxes)))


def whole_description():
    return obj(("version", 1), ("type", "CompleteDescription"))


def screen_file(pages, components):
    named_components = [obj(("key", name), ("value", component)) for name, component in components.items()]
    return obj(("pageData", obj(
        ("version", 1),
        ("componentDefinitions", obj(("version", 2), ("importFiles", []), ("localComponentDefinitions", named_components))),
        ("replacementStrings", []),
        ("info", whole_description()),
        ("tabs", [tab(page, key_index) for key_index, page in enumerate(pages)]))))


def knob_numbers(parameter_for_knob):
    return Obj(("Q-Link %d" % knob, parameter_for_knob.get(knob, NO_PARAMETER)) for knob in range(1, KNOB_COUNT + 1))


def knobs_on(page, tab_number):
    parameter_for_knob = dict(zip(KNOBS_DOWN_THE_COLUMNS, page.turned_by_knobs))
    return obj(("Tab", tab_number), ("SubTab", 1), ("Bank Direction", "Column"), ("Q-Links", knob_numbers(parameter_for_knob)))


def knob_file(pages):
    first_sixteen = {knob: knob - 1 for knob in range(1, KNOB_COUNT + 1)}
    return obj(("version", 4),
               ("info", whole_description()),
               ("Screen Mode Q-Links", obj(("version", 4), ("map", [knobs_on(page, number) for number, page in enumerate(pages, 1)]))),
               ("Program Mode Q-Links", knob_numbers(first_sixteen)))


def pictures_named_in(value):
    """Every picture file a screen refers to, wherever in the tree it is named."""
    if isinstance(value, str):
        return {value} if value.endswith(PICTURE_ENDINGS) else set()
    if isinstance(value, Obj):
        return set().union(*(pictures_named_in(inner) for _, inner in value))
    if isinstance(value, (list, tuple)):
        return set().union(*(pictures_named_in(inner) for inner in value))
    return set()


def write_json(value, path):
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write(gen_ui.dump_json(value) + "\n")


def write_skin(folder, pages, components):
    """Writes the two files the MPC reads and returns the pictures they refer to."""
    screen = screen_file(pages, components)
    write_json(screen, os.path.join(folder, "TUI.json"))
    write_json(knob_file(pages), os.path.join(folder, "Q-Links.json"))
    return pictures_named_in(screen)
