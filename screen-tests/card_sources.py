"""Writes each test card's C++ file and README from its plugin.json, so the name and uid live in one place only."""

import json
import os
from collections import namedtuple

# `parameters` is how many parameters the shared plugin declares.
SharedPlugin = namedtuple("SharedPlugin", "header shared_class parameters")
KNOBS_THAT_HOLD_A_VALUE = SharedPlugin("test_card_plugin.h", "TestCardPlugin", 16)
PICTURES_THAT_MOVE = SharedPlugin("live_card_plugin.h", "LiveCardPlugin", 28)
PICTURES_AT_FOUR_SPEEDS = SharedPlugin("fast_card_plugin.h", "FastCardPlugin", 6)

SOURCE = """// {name} - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/{header}.

#include "../../shared/{header}"

class {class_name} : public screen_test::{shared_class}
{{
public:
    {class_name}() : {shared_class}("{name}", MPC_FOURCC({uid_letters})) {{}}   // matches plugin.json "uid"
}};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
MPC_PLUGIN_ENTRY({class_name})
"""

README = """# {name}

A screen test card for Akai MPC standalone hardware, by Workbench Tests. It is not a real effect.

- **Sound:** passes the input through. Volume (parameter 0) is the only control that changes the sound.
- **Other parameters:** they give the test screens something to show. See `../shared/` for the plugin's code.
- **Screen:** hand-written files in `skins/`, produced by `../make_test_cards.py`. Do not edit them by hand.
- **CPU:** next to nothing.

What each page tests and what to look for is in `../CHECKLIST.md`.

Needs MPC OS 3.10.1 or later, with External Plugin Support enabled on the MPC
(Preferences, Enhancements (Beta), Enable Enhancements and External Plugin Support).
"""


def identity_of(card_folder):
    with open(os.path.join(card_folder, "plugin.json"), encoding="utf-8") as file:
        return json.load(file)


def write_text(text, path):
    with open(path, "w", encoding="utf-8", newline="\n") as file:
        file.write(text)


def class_name_for(card_name):
    return card_name.replace(" ", "")


def quoted_letters(uid):
    return ", ".join("'%s'" % letter for letter in uid)


def write_source(card_folder, shared_plugin):
    identity = identity_of(card_folder)
    class_name = class_name_for(identity["name"])
    source = SOURCE.format(name=identity["name"], class_name=class_name, uid_letters=quoted_letters(identity["uid"]),
                           **shared_plugin._asdict())
    os.makedirs(os.path.join(card_folder, "src"), exist_ok=True)
    write_text(source, os.path.join(card_folder, "src", class_name + ".cpp"))


def write_readme(card_folder):
    write_text(README.format(name=identity_of(card_folder)["name"]), os.path.join(card_folder, "README.md"))
