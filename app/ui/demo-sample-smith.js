// A stand-in for the blacksmith on the MPC Sample's screen, so the browser demo shows him where the app itself
// draws him: in the middle of the device's screen, on a dark ground, with every light dark.

(function () {
  "use strict";

  const { SCREEN, PADS } = window.sampleSurface;

  const FRAMES = 15;
  const FRAME_LASTS_SECONDS = 0.083;
  const TIMES_BIGGER = 3;
  const GROUND = "rgb(17, 18, 20)";
  const DARK = [0, 0, 0];

  function make() {
    const strip = new Image();
    const canvas = document.createElement("canvas");
    const drawing = canvas.getContext("2d", { willReadFrequently: true });
    canvas.width = SCREEN.width;
    canvas.height = SCREEN.height;
    drawing.imageSmoothingEnabled = false;
    strip.src = "blacksmith.png";

    function drawFrame(frame) {
      const across = strip.naturalWidth / FRAMES;
      const [width, height] = [across * TIMES_BIGGER, strip.naturalHeight * TIMES_BIGGER];
      drawing.drawImage(strip, frame * across, 0, across, strip.naturalHeight, (SCREEN.width - width) / 2, (SCREEN.height - height) / 2, width, height);
    }

    function frame(seconds) {
      drawing.fillStyle = GROUND;
      drawing.fillRect(0, 0, SCREEN.width, SCREEN.height);
      if (strip.complete && strip.naturalWidth) drawFrame(Math.floor(seconds / FRAME_LASTS_SECONDS) % FRAMES);
      return { pixels: drawing.getImageData(0, 0, SCREEN.width, SCREEN.height).data, padColours: PADS.map(() => DARK), litButtons: [] };
    }

    return { frame, take: () => {} };
  }

  window.demoSampleSmith = { make };
})();
