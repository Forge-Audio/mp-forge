// Letters made of dots, three across and five down: capitals, digits and a few marks. They are what the pixel
// art of the MPC Sample is lettered in, and what the demo's stand-in app writes on its screen.

(function () {
  "use strict";

  const DOTS_ACROSS = 3;
  const ROWS = 5;
  const FROM_ONE_LETTER_TO_THE_NEXT = DOTS_ACROSS + 1;

  // Each letter is five rows of three dots, a row written as one number.
  const GLYPHS = {
    A: [2, 5, 7, 5, 5], B: [6, 5, 6, 5, 6], C: [3, 4, 4, 4, 3], D: [6, 5, 5, 5, 6], E: [7, 4, 6, 4, 7], F: [7, 4, 6, 4, 4], G: [3, 4, 5, 5, 3],
    H: [5, 5, 7, 5, 5], I: [7, 2, 2, 2, 7], J: [1, 1, 1, 5, 2], K: [5, 5, 6, 5, 5], L: [4, 4, 4, 4, 7], M: [5, 7, 7, 5, 5], N: [6, 5, 5, 5, 5],
    O: [2, 5, 5, 5, 2], P: [6, 5, 6, 4, 4], Q: [2, 5, 5, 6, 3], R: [6, 5, 6, 5, 5], S: [3, 4, 2, 1, 6], T: [7, 2, 2, 2, 2], U: [5, 5, 5, 5, 7],
    V: [5, 5, 5, 5, 2], W: [5, 5, 7, 7, 5], X: [5, 5, 2, 5, 5], Y: [5, 5, 2, 2, 2], Z: [7, 1, 2, 4, 7],
    0: [7, 5, 5, 5, 7], 1: [2, 6, 2, 2, 7], 2: [6, 1, 2, 4, 7], 3: [6, 1, 2, 1, 6], 4: [5, 5, 7, 1, 1],
    5: [7, 4, 6, 1, 6], 6: [3, 4, 6, 5, 2], 7: [7, 1, 2, 2, 2], 8: [2, 5, 2, 5, 2], 9: [2, 5, 3, 1, 6],
    "-": [0, 0, 7, 0, 0], "+": [0, 2, 7, 2, 0], ":": [0, 2, 0, 2, 0], ".": [0, 0, 0, 0, 2],
  };

  // Where every dot of `words` falls, as [across, down] from the top left of the first letter. Small letters
  // are written as capitals, and anything the letters do not cover is left as a gap.
  function dotsOf(words) {
    const dots = [];
    [...String(words).toUpperCase()].forEach((character, place) => {
      (GLYPHS[character] || []).forEach((row, down) => {
        for (let across = 0; across < DOTS_ACROSS; across += 1) {
          if (row & (4 >> across)) dots.push([place * FROM_ONE_LETTER_TO_THE_NEXT + across, down]);
        }
      });
    });
    return dots;
  }

  function widthOf(words) {
    return Math.max(0, String(words).length * FROM_ONE_LETTER_TO_THE_NEXT - 1);
  }

  window.pixelLetters = { dotsOf, widthOf, HEIGHT: ROWS };
})();
