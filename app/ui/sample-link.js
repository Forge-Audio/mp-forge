// The part of the settings that puts MP Forge Link on an MPC Sample's SD card. MP Forge Link is the small program
// that makes a real MPC Sample the screen and the controls of an app that runs on this computer. The musician
// picks the card, and the program is built and written there. Linking itself needs no switch: MP Forge looks for
// the device whenever an app is open.

(function () {
  "use strict";

  const { ask } = window.backend;

  const WHILE_IT_IS_MADE = "Choose the card. MP Forge Link is then made, which takes a moment.";

  let parts;

  function show(part, words) {
    part.textContent = words;
    part.hidden = !words;
  }

  function wordsFor(made) {
    return made.isWhereTheBootMenuLooks
      ? `MP Forge Link is on the card, in ${made.folder}. Put the card back in your MPC Sample and pick MP Forge Link in the boot menu.`
      : `MP Forge Link was made in ${made.folder}. No boot menu was found there, so copy the folder MP Forge Link into autorun/mods on a card that has SampleBoot.`;
  }

  // Answers with nothing when the musician picked no card, and then nothing is said.
  async function make() {
    show(parts.said, "");
    show(parts.made, WHILE_IT_IS_MADE);
    parts.make.disabled = true;
    try {
      const made = await ask("make_link_for_mpc_sample");
      show(parts.made, made ? wordsFor(made) : "");
    } catch (refusal) {
      show(parts.made, "");
      show(parts.said, String(refusal));
    } finally {
      parts.make.disabled = false;
    }
  }

  function start(itsParts) {
    parts = itsParts;
    parts.make.addEventListener("click", make);
  }

  window.sampleLink = { start };
})();
