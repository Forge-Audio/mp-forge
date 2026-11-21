// Which letters the app is written in: its own pixel letters, or the computer's own font, which is easier to read.
//
// A plugin's screen is not the app's to restyle, so it keeps the MPC's letters either way.

(function () {
  "use strict";

  const { recall } = window.helpers;

  const PIXEL = "pixel";
  const PLAIN = "plain";
  const KEPT_AS = "letters";
  const MARK_OF_PLAIN_LETTERS = "plain-letters";

  function arePlain() {
    return recall(KEPT_AS) === PLAIN;
  }

  // The styles use the computer's own font wherever the page carries this mark.
  function markThePage() {
    document.body.classList.toggle(MARK_OF_PLAIN_LETTERS, arePlain());
  }

  // The setting as the settings page draws it. The first choice is the one in force until another is picked.
  const SETTING = { key: KEPT_AS, choices: [[PIXEL, "PIXEL"], [PLAIN, "PLAIN"]], whenPicked: markThePage };

  markThePage();

  window.letters = { SETTING, arePlain };
})();
