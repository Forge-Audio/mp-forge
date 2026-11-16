// What covers the MPC view while a plugin is being built or checked: a blacksmith in a fitted cap, hammering an MPC
// on his anvil. He is one strip of frames in blacksmith.png, shown one frame at a time.

(function () {
  "use strict";

  const { element } = window.helpers;

  const FRAMES = 15;
  const FRAME_LASTS_MS = 83;
  const FRAME_WHEN_STILL = 1;
  const AT_WORK = "A blacksmith hammering your plugin into shape.";

  function prefersStillness() {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  // The frame is read off the clock, so the swing never starts over when the view behind it is drawn again.
  function frameNow() {
    return prefersStillness() ? FRAME_WHEN_STILL : Math.floor(performance.now() / FRAME_LASTS_MS) % FRAMES;
  }

  function showFrame(figure, frame) {
    figure.style.backgroundPositionX = `${(frame / (FRAMES - 1)) * 100}%`;
  }

  // The strip is as many figures wide as it has frames, so one frame fills the figure.
  function fitTheStripTo(figure) {
    figure.style.backgroundSize = `${FRAMES * 100}% 100%`;
  }

  // Puts the blacksmith in `veil`, the cover laid over the MPC view. `show(words)` brings him out with what is
  // being done written underneath, and `hide()` sends him away.
  function standIn(veil) {
    const figure = element("div", "smith-figure");
    const words = element("div", "smith-words");
    let swinging;
    figure.setAttribute("role", "img");
    figure.setAttribute("aria-label", AT_WORK);
    fitTheStripTo(figure);
    veil.append(figure, words);

    function show(saying) {
      words.textContent = saying;
      showFrame(figure, frameNow());
      veil.hidden = false;
      if (swinging === undefined) swinging = setInterval(() => showFrame(figure, frameNow()), FRAME_LASTS_MS);
    }

    function hide() {
      veil.hidden = true;
      clearInterval(swinging);
      swinging = undefined;
    }

    return { show, hide };
  }

  window.blacksmith = { standIn };
})();
