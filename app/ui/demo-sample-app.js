// A stand-in for a Sample app, so the drawing of the MPC Sample has something to show in the browser demo.
//
// In the app itself a Sample app is the musician's own program, built and run by MP Forge. This one only draws:
// the pads as they are hit, the knobs and the fader where they stand, and a row of steps that runs while PLAY
// is lit. It makes no sound.

(function () {
  "use strict";

  const { SCREEN, PADS } = window.sampleSurface;
  const { dotsOf } = window.pixelLetters;

  const BYTES_A_PIXEL = 4;

  const INK = [236, 235, 242];
  const SOFT = [120, 117, 135];
  const DARK = [16, 16, 24];
  const WELL = [38, 37, 48];
  const ACCENT = [181, 39, 31];
  const ROW_COLOURS = [[240, 96, 48], [236, 188, 44], [64, 200, 124], [64, 140, 240]];
  const TILE = { size: 40, gap: 6, left: 10, top: 24 };
  const BAR = { width: 14, height: 96, top: 36 };
  const STEPS = 16;
  const STEPS_A_SECOND = 8;
  const GLOW_FADES_IN_SECONDS = 0.6;

  // The picture, as the device takes it: four bytes a pixel, row after row.
  function canvas() {
    const pixels = new Uint8ClampedArray(SCREEN.width * SCREEN.height * BYTES_A_PIXEL);
    const fill = (x, y, width, height, [red, green, blue]) => {
      for (let row = Math.max(0, y); row < Math.min(SCREEN.height, y + height); row += 1) {
        for (let column = Math.max(0, x); column < Math.min(SCREEN.width, x + width); column += 1) {
          pixels.set([red, green, blue, 255], (row * SCREEN.width + column) * BYTES_A_PIXEL);
        }
      }
    };
    const write = (words, x, y, dot, colour) => dotsOf(words).forEach(([across, down]) => fill(x + across * dot, y + down * dot, dot, dot, colour));
    return { pixels, fill, write };
  }

  function between(from, to, howFar) {
    return from.map((part, index) => Math.round(part + (to[index] - part) * howFar));
  }

  function colourOfPad(index, glow) {
    const bright = ROW_COLOURS[Math.floor(index / 4)];
    return between(between(DARK, bright, 0.28), between(bright, INK, 0.35), glow);
  }

  function make() {
    const stands = { knobs: [0.5, 0.5, 0.5], fader: 0.75, encoder: 0, lastTouched: "", isPlaying: false, playedFor: 0, glows: PADS.map(() => 0), lastSeconds: 0 };

    const taken = {
      pad: (input) => { if (input.value) stands.glows[input.number - 1] = input.value / 127; },
      knob: (input) => { stands.knobs[input.number - 1] = input.value; },
      fader: (input) => { stands.fader = input.value; },
      encoder: (input) => { stands.encoder += input.value; },
      button: (input) => {
        if (!input.value) return;
        stands.lastTouched = input.name;
        if (input.name === "PLAY") stands.isPlaying = true;
        if (input.name === "STOP") stands.isPlaying = false;
      },
    };

    function take(input) {
      if (taken[input.control]) taken[input.control](input);
    }

    function letTimePass(seconds) {
      const passed = Math.max(0, seconds - stands.lastSeconds);
      stands.lastSeconds = seconds;
      stands.glows = stands.glows.map((glow) => Math.max(0, glow - passed / GLOW_FADES_IN_SECONDS));
      if (stands.isPlaying) stands.playedFor += passed;
    }

    function drawPads(drawn) {
      PADS.forEach((pad, index) => {
        const step = TILE.size + TILE.gap;
        const x = TILE.left + (index % 4) * step;
        const y = TILE.top + (3 - Math.floor(index / 4)) * step;
        drawn.fill(x, y, TILE.size, TILE.size, colourOfPad(index, stands.glows[index]));
        drawn.write(pad.number, x + 3, y + 3, 1, DARK);
      });
    }

    function drawBar(drawn, x, words, value) {
      const filled = Math.round(BAR.height * value);
      drawn.fill(x, BAR.top, BAR.width, BAR.height, WELL);
      drawn.fill(x, BAR.top + BAR.height - filled, BAR.width, filled, INK);
      drawn.write(words, x + 1, BAR.top + BAR.height + 4, 2, SOFT);
    }

    function drawSteps(drawn) {
      const running = Math.floor(stands.playedFor * STEPS_A_SECOND) % STEPS;
      for (let step = 0; step < STEPS; step += 1) {
        const isOn = stands.isPlaying && step === running;
        drawn.fill(10 + step * 19, 216, 15, 12, isOn ? ACCENT : step % 4 === 0 ? SOFT : WELL);
      }
    }

    function picture() {
      const drawn = canvas();
      drawn.fill(0, 0, SCREEN.width, SCREEN.height, DARK);
      drawn.fill(0, 0, SCREEN.width, 16, ACCENT);
      drawn.write("POCKET PADS", 6, 3, 2, INK);
      drawn.write(stands.isPlaying ? "PLAY" : "STOP", 280, 3, 2, INK);
      drawPads(drawn);
      stands.knobs.forEach((value, index) => drawBar(drawn, 204 + index * 28, `K${index + 1}`, value));
      drawBar(drawn, 292, "F", stands.fader);
      drawn.write(`DIAL ${stands.encoder}`, 204, 160, 2, INK);
      drawn.write(stands.lastTouched, 204, 178, 2, SOFT);
      drawSteps(drawn);
      return drawn.pixels;
    }

    // What the app shows `seconds` after it started: its picture, the colour of each pad (red, green and blue)
    // and the buttons it lights.
    function frame(seconds) {
      letTimePass(seconds);
      return {
        pixels: picture(),
        padColours: stands.glows.map((glow, index) => colourOfPad(index, glow)),
        litButtons: stands.isPlaying ? ["PLAY"] : [],
      };
    }

    return { take, frame };
  }

  window.demoSampleApp = { make };
})();
