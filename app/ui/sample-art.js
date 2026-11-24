// The MPC Sample as pixel art, painted dot by dot.
//
// It is drawn the way pixel art is: a few tones for each material, lit from the top left; two tones mixed dot
// by dot where one has to fade into the other; and a softer dot along a curved edge to take the stairs off it.
// What never changes is painted once and kept. The pads, the buttons, the knobs, the fader and the dial are
// painted over it afresh whenever one of them changes.
//
// The words printed on the device are finer than the art, two of their dots to one of the art's each way, the
// way the print on a real device is finer than its buttons.
//
// The art is our own and stands for the device without copying its look: no maker's name is on it.

(function () {
  "use strict";

  const { SCREEN, BODY, PADS, KNOBS, FADER, ENCODER, MAIN_VOLUME, BUTTONS } = window.sampleSurface;
  const { dotsOf, widthOf, HEIGHT: LETTER_HEIGHT } = window.pixelLetters;

  // Under its face the body shows a little of its side, and under that its shadow.
  const SIDE = 4;
  const SHADOW = 3;
  const CORNER = 8;
  const RIM = 2;
  const SWEEP_OF_A_KNOB = (270 * Math.PI) / 180;
  const TURN_OF_ONE_CLICK = (18 * Math.PI) / 180;
  const RIDGES_ROUND_THE_DIAL = 40;
  const ROOM_FOR_A_BAR = 3;
  const BEZEL = 3;
  // The light of a hit runs out over a pad for this many thousandths of a second, as far as the pad's corners,
  // and so many dots at a step. Its rings are each so far behind the front of it, and so wide.
  const RIPPLE_LASTS = 480;
  const RIPPLE_REACHES = 22;
  const STEP_OF_A_RIPPLE = 2;
  const RINGS_OF_A_RIPPLE = [{ behind: 0, wide: 2, brightness: 0.75 }, { behind: 6, wide: 2, brightness: 0.4 }];
  const DOTS_OF_PRINT_TO_ONE_OF_ART = 2;
  const PRINT_HEIGHT = LETTER_HEIGHT / DOTS_OF_PRINT_TO_ONE_OF_ART;
  const BETWEEN_LINES_OF_PRINT = 1;
  const FROM_LINE_TO_LINE_OF_PRINT = PRINT_HEIGHT + BETWEEN_LINES_OF_PRINT;

  const BLACK = [0, 0, 0];
  const WHITE = [246, 244, 238];
  const CASE = [11, 10, 15];
  const STRIP = [[58, 57, 68], [42, 41, 50], [30, 29, 37], [21, 20, 26]];
  const PANEL = { glint: [243, 239, 231], light: [230, 225, 215], face: [216, 211, 200], shade: [196, 191, 180], dark: [168, 163, 152], line: [127, 122, 112] };
  const METAL = [[199, 196, 207], [156, 153, 166], [123, 120, 132], [95, 92, 104], [69, 67, 77], [42, 41, 48], [23, 22, 27]];
  const RED = [213, 53, 44];
  const GREEN = [55, 181, 86];
  const PURPLE = [157, 111, 224];
  const UNLIT_PAD = [112, 109, 121];
  const FACES = { grey: [111, 108, 119], orange: [226, 90, 43], blue: [47, 127, 218], dark: [52, 51, 60], soft: [74, 72, 82], record: [111, 108, 119], play: [111, 108, 119] };
  // The two strips of lights beside the screen, from the top light down: as they look unlit, and lit.
  const METER_LIGHTS = [[122, 47, 43], [122, 109, 43], [47, 106, 58], [47, 106, 58], [47, 106, 58]];
  const LIT_METER_LIGHTS = [[255, 74, 60], [255, 214, 64], [84, 232, 118], [84, 232, 118], [84, 232, 118]];
  const METER_LIGHT = { width: 5, height: 7, gap: 2, top: 27 };
  const WHERE_THE_METERS_ARE = [184, 191];
  const BANKS = [["E", "F", "G", "H"], ["A", "B", "C", "D"]];

  function mixed(from, to, howFar) {
    return from.map((part, index) => Math.round(part + (to[index] - part) * howFar));
  }

  function ink([red, green, blue], strength) {
    return strength === undefined ? `rgb(${red}, ${green}, ${blue})` : `rgba(${red}, ${green}, ${blue}, ${strength})`;
  }

  // The five tones a material is painted in, from where the light catches it to where it is in shade.
  function tonesOf(face) {
    return { glint: mixed(face, WHITE, 0.55), light: mixed(face, WHITE, 0.28), face, shade: mixed(face, BLACK, 0.24), dark: mixed(face, BLACK, 0.48) };
  }

  function fromLightToDark(tones) {
    return [tones.glint, tones.light, tones.face, tones.shade, tones.dark];
  }

  // The tone for how much light falls on a dot, from 1 for full light to -1 for none. Where two tones meet they
  // are mixed dot by dot, like a chessboard, which is how pixel art fades one into the other.
  function toneFor(light, tones, x, y) {
    const place = ((1 - light) / 2) * (tones.length - 1);
    const lower = Math.floor(place);
    const isBetweenTwo = place - lower > 0.36 && place - lower < 0.64;
    const picked = isBetweenTwo ? lower + ((x + y) & 1) : Math.round(place);
    return tones[Math.max(0, Math.min(tones.length - 1, picked))];
  }

  // How far in from the side a rounded corner starts on this row, counted from the top of the corner.
  function cornerInset(row, radius) {
    if (row >= radius) return 0;
    const up = radius - row - 0.5;
    return Math.ceil(radius - Math.sqrt(radius * radius - up * up) - 0.25);
  }

  // --- The brushes -----------------------------------------------------------------------------

  function brushesOn(context) {
    const fine = (dots) => Math.round(dots * DOTS_OF_PRINT_TO_ONE_OF_ART);
    const block = (x, y, width, height, colour, strength) => {
      context.fillStyle = ink(colour, strength);
      context.fillRect(fine(x), fine(y), fine(width), fine(height));
    };
    // A dot with no colour named is left as it is, so a shape can be painted with gaps in it.
    const dot = (x, y, colour, strength) => { if (colour) block(x, y, 1, 1, colour, strength); };

    // Every other dot of a block, like the dark squares of a chessboard.
    const dither = (x, y, width, height, colour, strength) => {
      for (let row = y; row < y + height; row += 1) {
        for (let column = x + ((x + row) & 1); column < x + width; column += 2) dot(column, row, colour, strength);
      }
    };

    // Words as big as the art's own dots, for what is not print on the device but a help to whoever plays it.
    const write = (words, x, y, colour) => dotsOf(words).forEach(([across, down]) => dot(x + across, y + down, colour));
    const writeCentred = (words, middle, y, colour) => write(words, Math.round(middle - widthOf(words) / 2), y, colour);

    // Words in the fine print. They are placed in the art's dots, like everything else.
    const print = (words, x, y, colour) => {
      context.fillStyle = ink(colour);
      dotsOf(words).forEach(([across, down]) => context.fillRect(fine(x) + across, fine(y) + down, 1, 1));
    };
    const printCentred = (words, middle, y, colour) => print(words, middle - widthOf(words) / DOTS_OF_PRINT_TO_ONE_OF_ART / 2, y, colour);

    // A slab standing proud of what is behind it: lit along its top and left, in shade along its bottom and
    // right, its corners cut and softened towards `backdrop`.
    const slab = ({ x, y, w, h }, tones, backdrop) => {
      block(x + 1, y, w - 2, h, tones.face);
      block(x, y + 1, w, h - 2, tones.face);
      block(x + 1, y, w - 2, 1, tones.light);
      block(x, y + 1, 1, h - 2, tones.light);
      block(x + 1, y + h - 1, w - 2, 1, tones.shade);
      block(x + w - 1, y + 1, 1, h - 2, tones.shade);
      dot(x + 1, y + 1, tones.glint);
      [[x, y], [x + w - 1, y], [x, y + h - 1], [x + w - 1, y + h - 1]].forEach(([cornerX, cornerY]) => dot(cornerX, cornerY, mixed(tones.shade, backdrop, 0.6)));
    };

    // A hollow cut into what is around it: dark along its top and left, where no light reaches, and a line of
    // light caught on the lip below and to the right of it.
    const hollow = ({ x, y, w, h }, floor, lip) => {
      block(x, y, w, h, floor);
      block(x, y, w, 1, CASE);
      block(x, y, 1, h, CASE);
      block(x + 1, y + h, w, 1, lip);
      block(x + w, y + 1, 1, h, lip);
    };

    // A round part. `toneAt` gives the colour of a dot from how far it is from the middle, across and down.
    // The dots that the edge only half covers are painted softer, in `edge`.
    const disc = (middleX, middleY, radius, toneAt, edge) => {
      for (let y = Math.floor(middleY - radius - 1); y <= Math.ceil(middleY + radius); y += 1) {
        for (let x = Math.floor(middleX - radius - 1); x <= Math.ceil(middleX + radius); x += 1) {
          const across = x + 0.5 - middleX;
          const down = y + 0.5 - middleY;
          const out = Math.hypot(across, down);
          if (out <= radius - 0.5) dot(x, y, toneAt(across, down, out, x, y));
          else if (out <= radius + 0.25 && edge) dot(x, y, edge.colour, edge.strength);
        }
      }
    };

    // A dome lit from the top left, in tones from light to dark.
    const dome = (middleX, middleY, radius, tones, edge) => {
      const lightOn = (across, down) => (-across - down) / (radius * 1.2);
      disc(middleX, middleY, radius, (across, down, out, x, y) => toneFor(lightOn(across, down), tones, x, y), edge);
    };

    // The shadow a round part casts on the panel: down and to the right, its edge broken up dot by dot.
    const shadowOfADisc = (middleX, middleY, radius, colour) => {
      disc(middleX + 1.5, middleY + 2.5, radius, (across, down, out, x, y) => (out > radius - 1.5 && (x + y) & 1 ? null : colour), null);
    };

    // Lets `paint` reach only the dots whose middle is within `radius` of a middle, and no others.
    const onlyWithin = (middleX, middleY, radius, paint) => {
      context.save();
      context.beginPath();
      for (let y = Math.floor(middleY - radius); y <= Math.ceil(middleY + radius); y += 1) {
        const reach = Math.sqrt(Math.max(0, radius * radius - (y + 0.5 - middleY) ** 2));
        const from = Math.ceil(middleX - reach - 0.5);
        const to = Math.floor(middleX + reach - 0.5);
        if (to >= from) context.rect(fine(from), fine(y), fine(to - from + 1), fine(1));
      }
      context.clip();
      paint();
      context.restore();
    };

    return { block, dot, dither, write, writeCentred, print, printCentred, slab, hollow, disc, dome, shadowOfADisc, onlyWithin };
  }

  // --- What never changes ----------------------------------------------------------------------

  function bodyRow(row, height) {
    const inset = Math.max(cornerInset(row, CORNER), cornerInset(height - 1 - row, CORNER));
    return { from: inset, width: BODY.width - inset * 2 };
  }

  // The body's outline filled in one colour, starting `top` rows down: the face, or its side, or its shadow.
  function paintBodyShape(brushes, top, paintRow) {
    for (let row = 0; row < BODY.height; row += 1) {
      const { from, width } = bodyRow(row, BODY.height);
      paintRow(from, top + row, width);
    }
  }

  function toneOfStripRow(row) {
    if (row < 3) return [STRIP[0], STRIP[0]];
    if (row < 22) return [STRIP[1], STRIP[1]];
    if (row < 28) return [STRIP[1], STRIP[2]];
    if (row < 56) return [STRIP[2], STRIP[2]];
    if (row < 62) return [STRIP[2], STRIP[3]];
    return [STRIP[3], STRIP[3]];
  }

  function toneOfPanelRow(row) {
    if (row < BODY.topStrip + 2) return [PANEL.glint, PANEL.glint];
    if (row < BODY.topStrip + 4) return [PANEL.light, PANEL.light];
    if (row > BODY.height - 5) return [PANEL.shade, PANEL.shade];
    if (row > BODY.height - 7) return [PANEL.face, PANEL.shade];
    return [PANEL.face, PANEL.face];
  }

  function paintCase(brushes) {
    paintBodyShape(brushes, SIDE + SHADOW, (from, row, width) => brushes.dither(from + 2, row, width - 4, 1, BLACK, 0.5));
    paintBodyShape(brushes, SIDE, (from, row, width) => brushes.block(from, row, width, 1, STRIP[2]));
    paintBodyShape(brushes, SIDE - 1, (from, row, width) => brushes.block(from, row, width, 1, CASE));
    paintBodyShape(brushes, 0, (from, row, width) => {
      brushes.block(from, row, width, 1, CASE);
      if (from > 0) [from - 1, from + width].forEach((x) => brushes.dot(x, row, CASE, 0.45));
    });
  }

  // The face inside the case's rim: the dark strip across the top, and the pale panel under it.
  function paintFace(brushes) {
    for (let row = RIM; row < BODY.height - RIM; row += 1) {
      const inset = Math.max(cornerInset(row - RIM, CORNER - RIM), cornerInset(BODY.height - RIM - 1 - row, CORNER - RIM)) + RIM;
      const [tone, mixedWith] = row < BODY.topStrip ? toneOfStripRow(row) : toneOfPanelRow(row);
      brushes.block(inset, row, BODY.width - inset * 2, 1, tone);
      if (mixedWith !== tone) brushes.dither(inset, row, BODY.width - inset * 2, 1, mixedWith);
    }
    brushes.block(RIM, BODY.topStrip - 2, BODY.width - RIM * 2, 2, CASE);
    brushes.block(RIM, BODY.topStrip + 4, 1, BODY.height - BODY.topStrip - 14, PANEL.glint);
    brushes.block(BODY.width - RIM - 1, BODY.topStrip + 4, 1, BODY.height - BODY.topStrip - 14, PANEL.shade);
  }

  function paintBezel(brushes) {
    const bezel = { x: SCREEN.x - BEZEL, y: SCREEN.y - BEZEL, w: SCREEN.w + BEZEL * 2, h: SCREEN.h + BEZEL * 2 };
    brushes.hollow(bezel, [2, 2, 3], STRIP[0]);
    brushes.block(SCREEN.x - 1, SCREEN.y - 1, SCREEN.w + 2, SCREEN.h + 2, METAL[6]);
  }

  function paintSpeaker(brushes) {
    const grille = { x: 206, y: 24, w: 49, h: 49 };
    brushes.hollow(grille, STRIP[0], STRIP[0]);
    for (let row = 0; row * 4 + 4 < grille.h; row += 1) {
      for (let x = grille.x + 3 + (row & 1) * 2; x + 2 < grille.x + grille.w; x += 4) {
        brushes.block(x, grille.y + 3 + row * 4, 2, 2, [5, 4, 8]);
        brushes.dot(x + 1, grille.y + 5 + row * 4, METAL[3]);
      }
    }
  }

  function paintMeterLight(brushes, x, placeFromTheTop, colour) {
    const light = METER_LIGHT;
    const y = light.top + placeFromTheTop * (light.height + light.gap);
    brushes.block(x, y, light.width, light.height, colour);
    brushes.block(x, y, light.width, 1, mixed(colour, WHITE, 0.3));
    brushes.block(x, y + light.height - 1, light.width, 1, mixed(colour, BLACK, 0.4));
  }

  function paintMeter(brushes, x) {
    const light = METER_LIGHT;
    brushes.block(x - 1, light.top - 1, light.width + 2, METER_LIGHTS.length * (light.height + light.gap), [5, 4, 8]);
    METER_LIGHTS.forEach((colour, place) => paintMeterLight(brushes, x, place, colour));
  }

  // Lights as many of a strip's lights as `lit` says, counted from the bottom.
  function paintLitMeterLights(brushes, x, lit) {
    LIT_METER_LIGHTS.forEach((colour, placeFromTheTop) => {
      const isLit = LIT_METER_LIGHTS.length - placeFromTheTop <= lit;
      if (isLit) paintMeterLight(brushes, x, placeFromTheTop, colour);
    });
  }

  function paintMainVolume(brushes) {
    const middleX = MAIN_VOLUME.x + MAIN_VOLUME.w / 2;
    const middleY = MAIN_VOLUME.y + MAIN_VOLUME.h / 2;
    const outer = MAIN_VOLUME.w / 2;
    const underIt = MAIN_VOLUME.y + MAIN_VOLUME.h + 5;
    brushes.shadowOfADisc(middleX, middleY, outer, CASE);
    brushes.dome(middleX, middleY, outer, [METAL[4], METAL[5], METAL[5], METAL[6], CASE], { colour: CASE, strength: 0.6 });
    brushes.dome(middleX, middleY, outer - 3, [METAL[0], METAL[1], METAL[2], METAL[3], METAL[4]], { colour: METAL[6] });
    paintPointer(brushes, middleX, middleY, outer - 4, 0);
    brushes.printCentred("MAIN", middleX, underIt, METAL[2]);
    brushes.printCentred("VOLUME", middleX, underIt + FROM_LINE_TO_LINE_OF_PRINT, METAL[2]);
  }

  // The eight banks of pads, with the one in use lit, and under them the lamp that says the battery is good.
  function paintBanks(brushes) {
    BANKS.forEach((row, down) => row.forEach((bank, across) => {
      brushes.print(bank, 64 + across * 5, 41 + down * 6, bank === "A" ? WHITE : METAL[4]);
    }));
    brushes.block(64, 71, 5, 2, mixed(GREEN, BLACK, 0.25));
    brushes.block(64, 71, 5, 1, mixed(GREEN, WHITE, 0.25));
  }

  // A screw: a small bright dome with a slot across it.
  function paintScrew(brushes, x, y) {
    brushes.dome(x + 2.5, y + 2.5, 3, [PANEL.glint, PANEL.light, PANEL.dark, PANEL.line], { colour: PANEL.line, strength: 0.7 });
    [[1, 3], [2, 2], [3, 1]].forEach(([across, down]) => brushes.dot(x + across, y + down, PANEL.line));
  }

  function paintPadWell(brushes) {
    const first = PADS[12];
    const last = PADS[3];
    const well = { x: first.x - 3, y: first.y - 3, w: last.x + last.w - first.x + 6, h: last.y + last.h - first.y + 6 };
    brushes.hollow(well, STRIP[3], PANEL.glint);
    brushes.block(well.x + 1, well.y + 1, well.w - 1, 1, [5, 4, 8]);
  }

  function paintFaderSlot(brushes) {
    const lamp = { x: FADER.x + FADER.w + 7, y: FADER.y + Math.floor(FADER.h / 2) - 1 };
    brushes.hollow(FADER, [14, 13, 18], PANEL.glint);
    brushes.block(FADER.x + Math.floor(FADER.w / 2), FADER.y + 3, 1, FADER.h - 6, METAL[5]);
    for (let y = FADER.y + 4; y < FADER.y + FADER.h; y += 8) brushes.block(FADER.x - 6, y, 3, 1, PANEL.line);
    brushes.dither(lamp.x - 1, lamp.y - 1, 7, 5, mixed(PURPLE, PANEL.face, 0.7));
    brushes.block(lamp.x, lamp.y, 5, 3, PURPLE);
    brushes.block(lamp.x, lamp.y, 5, 1, mixed(PURPLE, WHITE, 0.45));
  }

  function paintMicrophone(brushes) {
    const grille = { x: ENCODER.x + ENCODER.w + 3, y: ENCODER.y - 8, w: 5, h: 13 };
    brushes.block(grille.x, grille.y + 1, grille.w, grille.h - 2, PANEL.shade);
    brushes.block(grille.x + 1, grille.y, grille.w - 2, grille.h, PANEL.shade);
    [2, 6, 10].forEach((down) => {
      brushes.block(grille.x + 1, grille.y + down, 2, 2, METAL[5]);
      brushes.dot(grille.x + 3, grille.y + down + 1, PANEL.glint);
    });
  }

  function paintRestStrip(brushes) {
    const rest = { x: 8, y: BODY.restStrip + 1, w: BODY.width - 16, h: BODY.height - BODY.restStrip - 7 };
    brushes.block(rest.x + 1, rest.y, rest.w - 2, rest.h, PANEL.shade);
    brushes.block(rest.x, rest.y + 1, rest.w, rest.h - 2, PANEL.shade);
    brushes.block(rest.x + 1, rest.y, rest.w - 2, 1, PANEL.line);
    brushes.dither(rest.x + 1, rest.y + 1, rest.w - 2, 2, PANEL.dark);
    brushes.block(rest.x + 1, rest.y + rest.h, rest.w - 2, 1, PANEL.glint);
  }

  function buttonNamed(name) {
    return BUTTONS.find((button) => button.name === name);
  }

  // What a pair of buttons side by side is for, printed over the middle of the pair.
  function paintCaptionOver(brushes, words, [left, right]) {
    brushes.printCentred(words, (left.x + right.x + right.w) / 2, left.y - PRINT_HEIGHT - 4, PANEL.line);
  }

  function paintCaptions(brushes) {
    paintCaptionOver(brushes, "MODE", ["SAMPLE", "SEQ"].map(buttonNamed));
    paintCaptionOver(brushes, "PAD PLAY", ["CHOP", "MUTE"].map(buttonNamed));
    KNOBS.forEach((knob) => brushes.printCentred(knob.words, knob.x + knob.w / 2, knob.y + knob.h + 4, PANEL.line));
  }

  function stillLife() {
    const kept = document.createElement("canvas");
    kept.width = BODY.width * DOTS_OF_PRINT_TO_ONE_OF_ART;
    kept.height = (BODY.height + SIDE + SHADOW) * DOTS_OF_PRINT_TO_ONE_OF_ART;
    const brushes = brushesOn(kept.getContext("2d"));
    paintCase(brushes);
    paintFace(brushes);
    paintBezel(brushes);
    paintSpeaker(brushes);
    WHERE_THE_METERS_ARE.forEach((x) => paintMeter(brushes, x));
    paintMainVolume(brushes);
    paintBanks(brushes);
    [[4, 87], [261, 87], [4, 266], [261, 266]].forEach(([x, y]) => paintScrew(brushes, x, y));
    paintPadWell(brushes);
    paintFaderSlot(brushes);
    paintMicrophone(brushes);
    paintRestStrip(brushes);
    paintCaptions(brushes);
    return kept;
  }

  // --- What the musician moves -----------------------------------------------------------------

  // The white line on a knob that says where it stands, `turned` round from straight up.
  function paintPointer(brushes, middleX, middleY, length, turned) {
    for (let out = 3; out <= length; out += 0.5) {
      brushes.dot(Math.floor(middleX + Math.sin(turned) * out), Math.floor(middleY - Math.cos(turned) * out), WHITE);
    }
  }

  function paintKnob(brushes, knob, stands) {
    const middleX = knob.x + knob.w / 2;
    const middleY = knob.y + knob.h / 2;
    const outer = knob.w / 2;
    brushes.shadowOfADisc(middleX, middleY, outer - 0.5, PANEL.dark);
    brushes.dome(middleX, middleY, outer, [METAL[4], METAL[5], METAL[6], METAL[6], CASE], { colour: METAL[5], strength: 0.55 });
    brushes.dome(middleX, middleY, outer - 2.5, [METAL[1], METAL[3], METAL[4], METAL[5], METAL[6]], { colour: CASE });
    paintPointer(brushes, middleX, middleY, outer - 3, (stands - 0.5) * SWEEP_OF_A_KNOB);
  }

  function ridgeAt(across, down) {
    return Math.floor(((Math.atan2(down, across) + Math.PI) / (Math.PI * 2)) * RIDGES_ROUND_THE_DIAL) & 1;
  }

  function paintEncoder(brushes, clicks) {
    const middleX = ENCODER.x + ENCODER.w / 2;
    const middleY = ENCODER.y + ENCODER.h / 2;
    const outer = ENCODER.w / 2;
    const turned = clicks * TURN_OF_ONE_CLICK;
    const ridged = (across, down) => {
      const isInShade = across + down > 5;
      const [raised, cut] = isInShade ? [METAL[3], METAL[5]] : [METAL[1], METAL[4]];
      return ridgeAt(across, down) ? raised : cut;
    };
    brushes.shadowOfADisc(middleX, middleY, outer - 0.5, PANEL.dark);
    brushes.disc(middleX, middleY, outer, ridged, { colour: METAL[4], strength: 0.6 });
    brushes.dome(middleX, middleY, outer - 4, [METAL[5], METAL[5], METAL[6], METAL[6]], null);
    brushes.dome(middleX, middleY, outer - 5, [METAL[0], METAL[1], METAL[2], METAL[3], METAL[4]], { colour: METAL[5] });
    brushes.dome(middleX + Math.sin(turned) * 7, middleY - Math.cos(turned) * 7, 2.5, [METAL[5], METAL[4], METAL[3], METAL[1]], { colour: METAL[3] });
  }

  // The fader's cap is ribbed, with a white line across its middle to show where it stands.
  function paintFaderCap(brushes, stands) {
    const tall = 21;
    const cap = { x: FADER.x - 2, y: Math.round(FADER.y + (FADER.h - tall) * (1 - stands)), w: FADER.w + 4, h: tall };
    const middleRow = (tall - 1) / 2;
    brushes.block(cap.x + 2, cap.y + cap.h, cap.w - 1, 2, BLACK, 0.35);
    brushes.slab(cap, tonesOf(METAL[2]), PANEL.face);
    for (let row = 2; row < cap.h - 2; row += 2) brushes.block(cap.x + 2, cap.y + row, cap.w - 4, 1, row === middleRow ? WHITE : METAL[4]);
  }

  // The words on a button: on one line when they fit across `room` dots of the art, and otherwise a word to
  // a line.
  function linesOf(words, room) {
    return widthOf(words) <= room * DOTS_OF_PRINT_TO_ONE_OF_ART ? [words] : words.split(" ");
  }

  // The coloured bar along the bottom of a button that has one, or nothing.
  function barOf(button) {
    return { record: RED, play: GREEN }[button.look];
  }

  // The words sit in the middle of the room there is for them, which is less where a bar takes the bottom.
  function paintWordsOn(brushes, button, top) {
    const lines = linesOf(button.words, button.w - 4);
    const tall = lines.length * FROM_LINE_TO_LINE_OF_PRINT - BETWEEN_LINES_OF_PRINT;
    const room = button.h - (barOf(button) ? ROOM_FOR_A_BAR : 0);
    const first = top + (room - tall) / 2;
    lines.forEach((line, place) => brushes.printCentred(line, button.x + button.w / 2, first + place * FROM_LINE_TO_LINE_OF_PRINT, WHITE));
  }

  function faceOf(button, isLit) {
    const face = button.look === "play" && isLit ? [47, 157, 74] : FACES[button.look];
    return isLit ? mixed(face, WHITE, 0.3) : face;
  }

  // A button stands on a skirt two dots tall and casts a shadow. Pressed, it sinks into the skirt.
  function paintButton(brushes, button, { isDown, isLit }) {
    const isOnTheStrip = button.y < BODY.topStrip;
    const backdrop = isOnTheStrip ? STRIP[1] : PANEL.face;
    const tones = tonesOf(faceOf(button, isLit));
    const sunk = isDown ? 2 : 0;
    const face = { x: button.x, y: button.y + sunk, w: button.w, h: button.h };
    if (!isDown) brushes.dither(button.x + 1, button.y + button.h + 2, button.w, 1, isOnTheStrip ? CASE : PANEL.dark);
    brushes.block(button.x + 1, button.y + button.h, button.w - 2, 2, tones.dark);
    if (isLit) brushes.dither(button.x - 1, button.y - 1 + sunk, button.w + 2, button.h + 2, tones.glint, 0.55);
    brushes.slab(face, tones, backdrop);
    paintWordsOn(brushes, button, face.y);
    if (barOf(button)) brushes.block(face.x + 4, face.y + face.h - ROOM_FOR_A_BAR, face.w - 8, 2, barOf(button));
  }

  // A pad is rubber, lit from inside in the colour the app gives it: brightest in the middle, where the two
  // tones are mixed dot by dot round a core of the lighter one. On the core is the computer key that plays it.
  function paintPad(brushes, pad, colour, isDown) {
    const tones = tonesOf(colour);
    const sunk = isDown ? 1 : 0;
    const face = { x: pad.x, y: pad.y + sunk, w: pad.w, h: pad.h - sunk };
    const middleX = pad.x + pad.w / 2;
    const middleY = face.y + pad.h / 2 - 2;
    brushes.slab(face, tones, STRIP[3]);
    if (!isDown) brushes.block(pad.x + 1, pad.y + pad.h - 2, pad.w - 2, 1, tones.shade);
    brushes.disc(middleX, middleY, 9, (across, down, out, x, y) => (out < 5 || (x + y) & 1 ? tones.light : null), null);
    brushes.writeCentred(pad.key, middleX, Math.round(middleY - LETTER_HEIGHT / 2), tones.dark);
    brushes.block(pad.x + 3, face.y + 2, 3, 1, tones.glint);
    brushes.print(pad.number, pad.x + 3, face.y + face.h - 6, tones.dark);
  }

  // How far the front of a ripple has run from the middle of its pad. It moves a step at a time, not smoothly.
  function frontOfARipple(age) {
    const steps = RIPPLE_REACHES / STEP_OF_A_RIPPLE;
    return Math.floor((age / RIPPLE_LASTS) * steps) * STEP_OF_A_RIPPLE;
  }

  // The ring of a ripple that a dot is on, `out` dots from the middle of the pad, or nothing.
  function ringAt(out, front) {
    return RINGS_OF_A_RIPPLE.find((ring) => out <= front - ring.behind && out > front - ring.behind - ring.wide);
  }

  // The rings of a ripple, which break up dot by dot as it tires.
  function paintRings(brushes, pad, { colour, age }) {
    const front = frontOfARipple(age);
    const isTiring = age > RIPPLE_LASTS / 2;
    for (let y = pad.y + 1; y < pad.y + pad.h - 1; y += 1) {
      for (let x = pad.x + 1; x < pad.x + pad.w - 1; x += 1) {
        const ring = ringAt(Math.hypot(x + 0.5 - pad.x - pad.w / 2, y + 0.5 - pad.y - pad.h / 2), front);
        if (ring && !(isTiring && (x + y) & 1)) brushes.dot(x, y, mixed(colour, WHITE, ring.brightness));
      }
    }
  }

  // A pad that was hit `age` thousandths of a second ago. The light the app gives it for the hit has not
  // reached all of it yet: it runs out from the middle behind the rings of a ripple, and ahead of them the
  // pad is still the colour it was `before`.
  function paintHitPad(brushes, pad, { colour, before, isDown, age }) {
    paintPad(brushes, pad, before, isDown);
    brushes.onlyWithin(pad.x + pad.w / 2, pad.y + pad.h / 2, frontOfARipple(age), () => paintPad(brushes, pad, colour, isDown));
    paintRings(brushes, pad, { colour, age });
  }

  function asAColour(written) {
    const parts = written && written.match(/\d+/g);
    return parts && parts.length >= 3 ? parts.slice(0, 3).map(Number) : UNLIT_PAD;
  }

  // --- The painting ----------------------------------------------------------------------------

  // Paints the device on `canvas`, and answers with the ways to say what has changed on it. It is painted
  // again at most once for each time the window is drawn, however much has changed since.
  function make(canvas) {
    const still = stillLife();
    const context = canvas.getContext("2d");
    const brushes = brushesOn(context);
    const looks = { knobs: KNOBS.map(() => 0.5), fader: 0.75, clicks: 0, padsDown: new Set(), buttonsDown: new Set(), padColours: [], litButtons: [], levelLights: [0, 0], hits: new Map() };
    let isWaitingToPaint = false;

    function paint() {
      isWaitingToPaint = false;
      context.clearRect(0, 0, canvas.width, canvas.height);
      context.drawImage(still, 0, 0);
      WHERE_THE_METERS_ARE.forEach((x, strip) => paintLitMeterLights(brushes, x, looks.levelLights[strip]));
      BUTTONS.forEach((button) => paintButton(brushes, button, { isDown: looks.buttonsDown.has(button.name), isLit: looks.litButtons.includes(button.name) }));
      KNOBS.forEach((knob, index) => paintKnob(brushes, knob, looks.knobs[index]));
      paintPads(performance.now());
      paintFaderCap(brushes, looks.fader);
      paintEncoder(brushes, looks.clicks);
    }

    function colourOfPad(number) {
      return asAColour(looks.padColours[number - 1]);
    }

    // Paints every pad, one that was hit a moment ago with its ripple, and goes on painting for as long as a
    // ripple is running.
    function paintPads(now) {
      looks.hits.forEach((hit, number) => { if (now - hit.at >= RIPPLE_LASTS) looks.hits.delete(number); });
      PADS.forEach((pad) => {
        const hit = looks.hits.get(pad.number);
        const light = { colour: colourOfPad(pad.number), isDown: looks.padsDown.has(pad.number) };
        if (hit) paintHitPad(brushes, pad, { ...light, before: hit.colourBefore, age: now - hit.at });
        else paintPad(brushes, pad, light.colour, light.isDown);
      });
      if (looks.hits.size) paintSoon();
    }

    function paintSoon() {
      if (isWaitingToPaint) return;
      isWaitingToPaint = true;
      requestAnimationFrame(paint);
    }

    function held(which, name, isDown) {
      if (isDown) which.add(name); else which.delete(name);
      paintSoon();
    }

    function lit({ padColours, litButtons, levelLights = [0, 0] }) {
      const isAsBefore = String(padColours) === String(looks.padColours) && String(litButtons) === String(looks.litButtons) && String(levelLights) === String(looks.levelLights);
      looks.padColours = padColours || [];
      looks.litButtons = litButtons || [];
      looks.levelLights = levelLights;
      if (!isAsBefore) paintSoon();
    }

    canvas.width = still.width;
    canvas.height = still.height;
    paint();
    return {
      knobTurned: (number, stands) => { looks.knobs[number - 1] = stands; paintSoon(); },
      faderMoved: (stands) => { looks.fader = stands; paintSoon(); },
      encoderTurned: (clicks) => { looks.clicks = clicks; paintSoon(); },
      padHeld: (number, isDown) => {
        if (isDown) looks.hits.set(number, { at: performance.now(), colourBefore: colourOfPad(number) });
        held(looks.padsDown, number, isDown);
      },
      buttonHeld: (name, isDown) => held(looks.buttonsDown, name, isDown),
      lit,
    };
  }

  window.sampleArt = { make, HEIGHT: BODY.height + SIDE + SHADOW };
})();
