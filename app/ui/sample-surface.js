// The MPC Sample's surface as a Sample app meets it: the screen it draws on, and every control by the name the
// app is told when the musician touches it.
//
// A place is where a control sits in the pixel art of the device, counted in the art's own dots on a body 270
// across and 328 down. The art is to scale: the device is 19.4 cm across and 23.6 cm from back to front, and
// its screen measures 2.4 inches from corner to corner, which is a quarter of the way across the body. So an
// app's picture is shown small on it, as it is on the device, and can be made bigger to read.

(function () {
  "use strict";

  const SCREEN = { width: 320, height: 240, x: 100, y: 25, w: 68, h: 51 };
  // Under the controls the body ends in a plain strip, where a hand rests.
  const BODY = { width: 270, height: 328, topStrip: 82, restStrip: 274 };

  // The computer keys that play the pads, laid out as the pads are: pad 1 at the bottom left, 16 at the top right.
  const PAD_KEYS = ["z", "x", "c", "v", "a", "s", "d", "f", "q", "w", "e", "r", "1", "2", "3", "4"];
  const PADS_ACROSS = 4;
  const PAD = { size: 29, gap: 5, left: 68, top: 134 };
  const HARDEST_HIT = 127;

  function padAt(index) {
    const column = index % PADS_ACROSS;
    const rowFromTheBottom = Math.floor(index / PADS_ACROSS);
    const step = PAD.size + PAD.gap;
    return {
      number: index + 1, key: PAD_KEYS[index],
      x: PAD.left + column * step, y: PAD.top + (PADS_ACROSS - 1 - rowFromTheBottom) * step, w: PAD.size, h: PAD.size,
    };
  }

  const PADS = PAD_KEYS.map((key, index) => padAt(index));

  const KNOBS = [1, 2, 3].map((number) => ({ number, words: `K${number}`, x: 20 + number * 51, y: 97, w: 25, h: 25 }));
  const FADER = { x: 26, y: 166, w: 16, h: 70 };
  const ENCODER = { x: 216, y: 166, w: 33, h: 33 };
  const MAIN_VOLUME = { x: 21, y: 32, w: 27, h: 27 };

  // A button, with the words printed on it. `look` picks its colour: the device groups its buttons by colour.
  function button(name, look, x, y, size) {
    return { name, words: name, look, x, y, w: (size && size.w) || 19, h: (size && size.h) || 12 };
  }

  const LEFT = 12;
  const NEXT_TO_LEFT = 38;
  const RIGHT = 210;
  const NEXT_TO_RIGHT = 237;
  const SMALL = { h: 11 };
  const TALL = { h: 16 };
  // Tall enough for two lines of words over the coloured bar some buttons carry.
  const WITH_A_BAR = { h: 15 };
  const ABOVE_THE_SCREEN = { w: 16, h: 8 };

  const BUTTONS = [
    { ...button("B1", "soft", 102, 10, ABOVE_THE_SCREEN), words: "" },
    { ...button("B2", "soft", 126, 10, ABOVE_THE_SCREEN), words: "" },
    { ...button("B3", "soft", 150, 10, ABOVE_THE_SCREEN), words: "" },
    button("SAMPLE", "grey", LEFT, 95),
    button("SEQ", "grey", NEXT_TO_LEFT, 95),
    button("PAD FX", "orange", LEFT, 114),
    button("KNOB FX", "orange", NEXT_TO_LEFT, 114),
    button("SHIFT", "dark", LEFT, 137, TALL),
    button("PAD BANK", "grey", NEXT_TO_LEFT, 137),
    button("ERASE", "grey", LEFT, 251),
    button("NOTE REPEAT", "grey", NEXT_TO_LEFT, 251),
    button("CHOP", "blue", RIGHT, 95),
    button("MUTE", "blue", NEXT_TO_RIGHT, 95),
    button("LOOP", "blue", RIGHT, 114),
    button("16 LEVELS", "blue", NEXT_TO_RIGHT, 114),
    button("SAMPLE SELECT", "grey", RIGHT, 137),
    button("TAP TEMPO", "grey", NEXT_TO_RIGHT, 137),
    { ...button("MINUS", "grey", RIGHT, 206, SMALL), words: "-" },
    { ...button("PLUS", "grey", NEXT_TO_RIGHT, 206, SMALL), words: "+" },
    button("SAMPLE RECORD", "record", RIGHT, 227, WITH_A_BAR),
    button("SEQ RECORD", "record", NEXT_TO_RIGHT, 227, WITH_A_BAR),
    button("STOP", "grey", RIGHT, 249, WITH_A_BAR),
    button("PLAY", "play", NEXT_TO_RIGHT, 249, WITH_A_BAR),
  ];

  // What the app is told. A pad's `value` is how hard it was hit, and 0 when it is let go. A knob and the fader
  // say where they stand, from 0 to 1. The encoder says how many clicks it turned, and which way. A button says
  // 1 when pressed and 0 when let go; pressing the encoder is the button `ENCODER`.
  const told = {
    padHit: (number, velocity) => ({ control: "pad", number, value: Math.max(1, Math.min(HARDEST_HIT, Math.round(velocity))) }),
    padLetGo: (number) => ({ control: "pad", number, value: 0 }),
    knobTurned: (number, value) => ({ control: "knob", number, value }),
    faderMoved: (value) => ({ control: "fader", value }),
    encoderTurned: (clicks) => ({ control: "encoder", value: clicks }),
    button: (name, isPressed) => ({ control: "button", name, value: isPressed ? 1 : 0 }),
  };

  // What an app shows arrives as bytes, always the same number: the picture, four bytes a dot; then three
  // bytes for the colour of each pad; then a byte for each button, in the order MP Forge's kit counts them,
  // that is 1 when the button is lit; and last a byte that is 1 while the app waits for the musician to pick a
  // sound file on the computer.
  const BUTTONS_AS_THE_KIT_COUNTS = [...BUTTONS.map((made) => made.name), "ENCODER"];
  const BYTES_OF_PICTURE = SCREEN.width * SCREEN.height * 4;
  const BYTES_A_PAD = 3;
  const BYTES_OF_PADS = PADS.length * BYTES_A_PAD;
  const BYTES_OF_BUTTONS = BUTTONS_AS_THE_KIT_COUNTS.length;
  // The two strips of lights beside the screen, the left and the right: a byte each, for how many are lit.
  const LEVEL_STRIPS = 2;
  const WHERE_THE_LEVELS_START = BYTES_OF_PICTURE + BYTES_OF_PADS + BYTES_OF_BUTTONS;
  const BYTES_SHOWN = WHERE_THE_LEVELS_START + LEVEL_STRIPS + 1;

  function colourAt(bytes, place) {
    return `rgb(${bytes[place]}, ${bytes[place + 1]}, ${bytes[place + 2]})`;
  }

  // The picture, the colour of each pad, the names of the lit buttons, how many lights of each strip beside the
  // screen are lit, and whether the app waits for a file, read from the bytes an app showed. Nothing when the
  // bytes are not a whole showing.
  function readShown(arrived) {
    const bytes = new Uint8Array(arrived);
    if (bytes.length !== BYTES_SHOWN) return null;
    const lit = bytes.subarray(BYTES_OF_PICTURE + BYTES_OF_PADS);
    return {
      pixels: new Uint8ClampedArray(bytes.buffer, bytes.byteOffset, BYTES_OF_PICTURE),
      padColours: PADS.map((pad, index) => colourAt(bytes, BYTES_OF_PICTURE + index * BYTES_A_PAD)),
      litButtons: BUTTONS_AS_THE_KIT_COUNTS.filter((name, index) => lit[index] === 1),
      levelLights: Array.from(bytes.subarray(WHERE_THE_LEVELS_START, WHERE_THE_LEVELS_START + LEVEL_STRIPS)),
      waitsForAFile: bytes[BYTES_SHOWN - 1] === 1,
    };
  }

  // The same the other way round, for the demo's stand-in app: `padColours` are red, green and blue each.
  function writeShown({ pixels, padColours, litButtons, levelLights = [0, 0], waitsForAFile }) {
    const bytes = new Uint8Array(BYTES_SHOWN);
    bytes.set(pixels, 0);
    padColours.forEach((colour, index) => bytes.set(colour, BYTES_OF_PICTURE + index * BYTES_A_PAD));
    BUTTONS_AS_THE_KIT_COUNTS.forEach((name, index) => { bytes[BYTES_OF_PICTURE + BYTES_OF_PADS + index] = litButtons.includes(name) ? 1 : 0; });
    bytes.set(levelLights, WHERE_THE_LEVELS_START);
    bytes[BYTES_SHOWN - 1] = waitsForAFile ? 1 : 0;
    return bytes.buffer;
  }

  window.sampleSurface = { SCREEN, BODY, PADS, KNOBS, FADER, ENCODER, MAIN_VOLUME, BUTTONS, HARDEST_HIT, told, readShown, writeShown };
})();
