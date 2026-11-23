#!/bin/sh
# Starts an app made with MP Forge on the MPC Sample. The boot menu starts it, or MP Forge Link does, right
# after taking the app from the computer. When the app is left, Akai's own app is started again, unless
# MP Forge Link started it: the link then takes the device back itself.

HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SAID="$HERE/last-run.txt"
[ "$1" = @STARTED_BY_THE_LINK@ ] || trap 'systemctl start ac50' EXIT

# The program is run from memory, so nothing is written to the MPC Sample's own storage.
PROGRAM=/run/mp-forge-app
cp "$HERE/@PROGRAM@" "$PROGRAM" && chmod +x "$PROGRAM" || { echo "The program could not be copied to $PROGRAM." >"$SAID"; exit 1; }

# The controls and the sound are sound cards of the device, whichever numbers they have today.
card_named() { awk -v name="$1" 'index($0, "[" name) { print $1; exit }' /proc/asound/cards; }
CONTROLS=$(card_named Surface)
SOUND=$(card_named I2S)
{ echo "The sound cards of this MPC Sample:"; cat /proc/asound/cards; } >"$SAID"
[ -n "$CONTROLS" ] || { echo "No controls were found among them." >>"$SAID"; exit 1; }
[ -n "$SOUND" ] || echo "No sound card called I2S was found among them, so the app runs without sound." >>"$SAID"

CARD=$(CDPATH= cd -- "$HERE/../../.." && pwd)
"$PROGRAM" /dev/fb0 "/dev/snd/midiC${CONTROLS}D0" "/dev/snd/pcmC${SOUND}D0p" "/dev/snd/controlC${SOUND}" "$HERE/@SOUNDS@" "$CARD" 2>>"$SAID"
