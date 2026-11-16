"""Runs one of Akai's tools so that a folder on another drive does not stop it.

Akai's tools name a folder by the way to it from the folder they were started in. Windows has no such way
from one drive to another, and Python stops the tool there. Run through here, such a folder is named by its
whole path instead, which is what the tools themselves fall back on when the way would lead upwards.

Arguments: the tool's script, then the tool's own arguments.
"""

import os
import runpy
import sys

way_between = os.path.relpath


def way_or_whole_path(path, start=os.curdir):
    try:
        return way_between(path, start)
    except ValueError:
        return os.path.abspath(path)


def run_as_if_started_directly(script):
    sys.path.insert(0, os.path.dirname(os.path.abspath(script)))
    runpy.run_path(script, run_name="__main__")


os.path.relpath = way_or_whole_path
sys.argv = sys.argv[1:]
run_as_if_started_directly(sys.argv[0])
