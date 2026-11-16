"""Draws images of Akai's control kit again at other sizes, with Akai's own drawing code.

The MPC never resizes a knob's or fader's image, so a control the musician resized needs its image
drawn at exactly the new size. Each drawing is kept in a cache, because drawing one takes seconds.

Arguments: the SDK's tools folder, params.json, ui.json, the folder to draw into, the cache folder,
then one "image:width:height:frames:file name" for each drawing.
"""

import os
import shutil
import sys

tools_folder, params_path, layout_path, into_folder, cache_folder = sys.argv[1:6]
sys.path.insert(0, tools_folder)

import gen_ui  # noqa: E402  (Akai's generator, found through the path added above)
import make_ui_kit  # noqa: E402


def kit_of_the_layout():
    table = gen_ui.ParamTable(gen_ui.load_params(params_path))
    kit = gen_ui.load_ui(layout_path, table)[4]
    if not kit:
        sys.exit("ui.json uses no control kit, so there is nothing to draw at another size.")
    return kit


def drawn(image, width, height, frames, theme):
    centre_zero = image.endswith("_bi")
    if image.startswith("knob"):
        return make_ui_kit.render_knob(width, frames, theme, centre_zero)
    return make_ui_kit.render_fader(width, height, frames, theme, centre_zero, False)


def draw_each(drawings):
    kit = kit_of_the_layout()
    theme = make_ui_kit.make_theme(kit["theme"], kit["accent"])
    cache = os.path.join(cache_folder, "%s-%s" % (kit["theme"], kit["accent"] or "own"))
    for drawing in drawings:
        image, width, height, frames, file_name = drawing.split(":")
        kept = os.path.join(cache, file_name)
        if not os.path.isfile(kept):
            make_ui_kit.write_png(kept, int(width), drawn(image, int(width), int(height), int(frames), theme))
        shutil.copyfile(kept, os.path.join(into_folder, file_name))


try:
    draw_each(sys.argv[6:])
except gen_ui.UiError as problem:
    sys.exit("ui.json could not be read: %s" % problem)
