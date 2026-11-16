// Choices the AI offers the musician to pick from.
//
// The AI ends its message with one line per choice, each starting with "[option]". Those lines are taken
// out of the words that are shown and become buttons instead.

(function () {
  "use strict";

  const OPTION_LINE = /^\s*\[option\]\s*(.*)$/i;
  const MOST_OPTIONS_SHOWN = 4;

  function labelOn(line) {
    const found = OPTION_LINE.exec(line);
    return found ? found[1].trim() : null;
  }

  // Splits a message into the words to show and the labels of the choices it offers.
  function readFrom(text) {
    const lines = String(text).split("\n");
    const words = lines.filter((line) => labelOn(line) === null).join("\n").trim();
    const options = lines.map(labelOn).filter(Boolean).slice(0, MOST_OPTIONS_SHOWN);
    return { words, options };
  }

  window.offeredOptions = { readFrom };
})();
