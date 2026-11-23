#!/bin/sh
# Starts MP Forge Link from the boot menu. The MPC Sample becomes the screen and the controls of an app that
# runs in MP Forge on a computer, and takes apps from MP Forge onto the card. When the program ends, Akai's own
# app is started again.

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SAID="$HERE/last-run.txt"
trap 'systemctl start ac50' EXIT

# The program is run from memory, so nothing is written to the MPC Sample's own storage.
PROGRAM=/run/mp-forge-link
cp "$HERE/mp-forge-link" "$PROGRAM" && chmod +x "$PROGRAM" || { echo "The program could not be copied to $PROGRAM." >"$SAID"; exit 1; }

# The controls are the sound card named Surface, whichever number it has today.
CARD=$(awk '/\[Surface/ { print $1; exit }' /proc/asound/cards)
[ -n "$CARD" ] || { echo "No controls were found among these:" >"$SAID"; cat /proc/asound/cards >>"$SAID"; exit 1; }

# Apps the computer sends go beside this folder, where the boot menu looks, and never onto the MPC Sample's own storage.
"$PROGRAM" /dev/fb0 "/dev/snd/midiC${CARD}D0" @PORT@ @PORT_FOR_APPS@ "$(dirname -- "$HERE")" / 2>"$SAID"
