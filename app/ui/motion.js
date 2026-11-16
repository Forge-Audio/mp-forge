// Whether the app moves: the conversation sliding aside, the blacksmith's swing, new words gliding into view.
//
// A computer can say that its user wants less motion. Many a Windows computer says so only because its own
// window animations were switched off to make it quicker, so the app moves unless the musician asks it to go
// by what the computer says. Then it keeps still whenever the computer asks for less motion.

(function () {
  "use strict";

  const { recall } = window.helpers;

  const AS_THE_COMPUTER_SAYS = "system";
  const ALWAYS = "always";
  const KEPT_AS = "motion";
  const MARK_OF_KEEPING_STILL = "keeps-still";
  const computerWantsLess = window.matchMedia("(prefers-reduced-motion: reduce)");

  function keepsStill() {
    return recall(KEPT_AS) === AS_THE_COMPUTER_SAYS && computerWantsLess.matches;
  }

  // The styles keep still wherever the page carries this mark.
  function markThePage() {
    document.body.classList.toggle(MARK_OF_KEEPING_STILL, keepsStill());
  }

  // The setting as the settings page draws it. The first choice is the one in force until another is picked.
  const SETTING = {
    key: KEPT_AS,
    choices: [[ALWAYS, "ALWAYS"], [AS_THE_COMPUTER_SAYS, "AS MY COMPUTER SAYS"]],
    whenPicked: markThePage,
  };

  computerWantsLess.addEventListener("change", markThePage);
  markThePage();

  window.motion = { SETTING, keepsStill };
})();
