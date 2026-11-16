// What is typed into the workbench stays as it was typed. The computer's own autocorrect, automatic capitals,
// spelling marks and suggestions are switched off for every box that takes words, on macOS, Windows and Linux alike,
// including boxes that are made later.

(function () {
  "use strict";

  const BOXES_FOR_WORDS = 'textarea, input:not([type]), input[type="text"], input[type="search"], input[type="password"], [contenteditable]';
  const LEFT_AS_TYPED = { autocorrect: "off", autocapitalize: "off", autocomplete: "off", spellcheck: "false" };

  function leaveAsTyped(box) {
    Object.entries(LEFT_AS_TYPED).forEach(([setting, value]) => box.setAttribute(setting, value));
  }

  function boxesIn(added) {
    if (!(added instanceof Element)) return [];
    return [...(added.matches(BOXES_FOR_WORDS) ? [added] : []), ...added.querySelectorAll(BOXES_FOR_WORDS)];
  }

  function leaveNewBoxesAsTyped(changes) {
    changes.forEach((change) => change.addedNodes.forEach((added) => boxesIn(added).forEach(leaveAsTyped)));
  }

  function start() {
    document.querySelectorAll(BOXES_FOR_WORDS).forEach(leaveAsTyped);
    new MutationObserver(leaveNewBoxesAsTyped).observe(document.body, { childList: true, subtree: true });
  }

  start();
})();
