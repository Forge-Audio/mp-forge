// The MPC's page, measured in the MPC's own pixels: 1280 wide and 628 tall, with a 4 by 4 grid of slots.
//
// Everything on the plugin's screen is placed by these measurements, which come from the SDK's gen_ui.py.
// The style sheet shrinks the whole page to fit, so nothing here knows how big it is drawn.

(function () {
  "use strict";

  const PAGE = { x: 0, y: 0, width: 1280, height: 628 };
  const SLOT_LEFTS = [23, 343, 663, 983];
  const SLOT_TOPS = [45, 196, 346, 497];
  const SLOT = { width: 278, height: 107 };
  const SLOTS_PER_ROW = SLOT_LEFTS.length;

  function placeAt(made, area) {
    made.classList.add("at");
    made.style.setProperty("--x", String(area.x));
    made.style.setProperty("--y", String(area.y));
    made.style.setProperty("--w", String(area.width));
    made.style.setProperty("--h", String(area.height));
    return made;
  }

  // The slot at a position counted from zero, left to right and then top to bottom.
  function areaOfSlot(position) {
    return { x: SLOT_LEFTS[position % SLOTS_PER_ROW], y: SLOT_TOPS[Math.floor(position / SLOTS_PER_ROW)], ...SLOT };
  }

  // A block of slots as pictures.json names it: rows and columns counted from one.
  function areaOfBlock(block) {
    const x = SLOT_LEFTS[block.column - 1];
    const y = SLOT_TOPS[block.row - 1];
    const right = SLOT_LEFTS[block.column + block.columns - 2] + SLOT.width;
    const bottom = SLOT_TOPS[block.row + block.rows - 2] + SLOT.height;
    return { x, y, width: right - x, height: bottom - y };
  }

  window.mpcStage = { PAGE, SLOT, SLOTS_PER_ROW, placeAt, areaOfSlot, areaOfBlock };
})();
