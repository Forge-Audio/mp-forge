// An effect tried on the drawing of the MPC Sample, the way the device would play it as a Knob FX.
//
// The drawn knobs turn the effect's six controls while the live player plays it on the computer. On the
// device SHIFT is held to reach the other three. A mouse cannot hold SHIFT and drag a knob at once, so on the
// drawing SHIFT is pressed once to reach them and again to come back. The drawn screen shows each knob's
// label and what it reads.
//
// That screen is MP Forge's own plain stand-in and says so. It is not a copy of what Akai's software shows.

(function () {
  "use strict";

  const { SCREEN, PADS } = window.sampleSurface;

  const GROUND = "#0c0c0c";
  const INK = "#f0f0f0";
  const SOFT = "#8c8c8c";
  const LIT = "#ff9f1c";
  const UNLIT_PAD = "rgb(24, 24, 24)";
  const TURNS_NOTHING = "-";
  const COLUMNS = [54, 160, 266];
  const BAR = { width: 76, height: 6, top: 138 };
  const STAND_IN_WORDS = "MP Forge's stand-in, not Akai's screen";
  const LETTERS = "ui-monospace, Menlo, Consolas, monospace";

  let device = null;
  // What is shown now: the effect, its six knobs, its controls, and who is told when a knob turns one.
  let showing = null;
  let shiftIsOn = false;
  // What each control reads and where it stands, by its number in the plugin.
  const readings = new Map();

  function sameName(one, other) {
    return one.toLowerCase() === other.toLowerCase();
  }

  function controlTurnedBy(seat) {
    return seat && seat.control ? showing.controls.find((control) => sameName(control.name, seat.control)) : undefined;
  }

  function seatsInReach() {
    return (shiftIsOn ? showing.knobs.shiftKnobs : showing.knobs.knobs) || [];
  }

  function hasAKnobWithShift() {
    return (showing.knobs.shiftKnobs || []).some((seat) => seat.control);
  }

  function readingAsBuilt(control) {
    return { amount: control.default, text: `${control.display} ${control.label || ""}`.trim() };
  }

  function readingOf(control) {
    return readings.get(control.index) || readingAsBuilt(control);
  }

  function write(drawing, words, x, y, { size, colour, bold = false, align = "center" }) {
    drawing.font = `${bold ? "bold " : ""}${size}px ${LETTERS}`;
    drawing.fillStyle = colour;
    drawing.textAlign = align;
    drawing.fillText(words, x, y);
  }

  function drawSeat(drawing, seat, middle) {
    const control = controlTurnedBy(seat);
    if (!control) return write(drawing, TURNS_NOTHING, middle, 118, { size: 16, colour: SOFT });
    const reading = readingOf(control);
    write(drawing, seat.label || control.name, middle, 100, { size: 13, colour: SOFT });
    write(drawing, reading.text, middle, 124, { size: 15, colour: INK, bold: true });
    drawing.fillStyle = SOFT;
    drawing.fillRect(middle - BAR.width / 2, BAR.top, BAR.width, BAR.height);
    drawing.fillStyle = LIT;
    drawing.fillRect(middle - BAR.width / 2, BAR.top, Math.round(BAR.width * reading.amount), BAR.height);
  }

  function drawShiftMark(drawing) {
    if (!hasAKnobWithShift()) return;
    write(drawing, "SHIFT", 12, 196, { size: 11, colour: shiftIsOn ? LIT : SOFT, bold: true, align: "left" });
  }

  function pictureOfTheScreen() {
    const canvas = document.createElement("canvas");
    canvas.width = SCREEN.width;
    canvas.height = SCREEN.height;
    const drawing = canvas.getContext("2d");
    drawing.fillStyle = GROUND;
    drawing.fillRect(0, 0, SCREEN.width, SCREEN.height);
    write(drawing, showing.knobs.shortName || showing.name, 12, 34, { size: 18, colour: INK, bold: true, align: "left" });
    write(drawing, "KNOB FX", SCREEN.width - 12, 34, { size: 11, colour: LIT, bold: true, align: "right" });
    COLUMNS.forEach((middle, position) => drawSeat(drawing, seatsInReach()[position], middle));
    drawShiftMark(drawing);
    write(drawing, STAND_IN_WORDS, SCREEN.width / 2, SCREEN.height - 10, { size: 9, colour: SOFT });
    return drawing.getImageData(0, 0, SCREEN.width, SCREEN.height).data;
  }

  function paint() {
    if (!showing) return;
    const litButtons = shiftIsOn ? ["KNOB FX", "SHIFT"] : ["KNOB FX"];
    device.show({ pixels: pictureOfTheScreen(), padColours: PADS.map(() => UNLIT_PAD), litButtons, levelLights: [0, 0] });
  }

  function knobTurned(number, amount) {
    const control = controlTurnedBy(seatsInReach()[number - 1]);
    if (!control) return;
    readings.set(control.index, { ...readingOf(control), amount });
    showing.onTurned(control.name, amount);
    paint();
  }

  // Everything done on the drawing arrives here, in the words a Sample app would be told it in.
  function heard(input) {
    if (!showing) return;
    if (input.control === "button" && input.name === "SHIFT" && input.value === 1) {
      shiftIsOn = !shiftIsOn;
      paint();
    }
    if (input.control === "knob") knobTurned(input.number, input.value);
  }

  function isShowing(plugin) {
    return Boolean(showing && plugin && showing.name === plugin.summary.name && showing.builtAt === plugin.builtAt);
  }

  // Shows `plugin` on the drawing in `holder` with these six `knobs`. `onTurned` is told the name of a control
  // a drawn knob turned and how far.
  function show(holder, plugin, knobs, onTurned) {
    device = device || window.sampleDevice.make(holder, heard);
    if (!isShowing(plugin)) readings.clear();
    showing = { name: plugin.summary.name, builtAt: plugin.builtAt, knobs, controls: plugin.parameters.params, onTurned };
    shiftIsOn = false;
    paint();
  }

  function hide() {
    showing = null;
  }

  // The live player's word on what a control reads at an amount.
  function valueArrived(index, amount, text) {
    readings.set(index, { amount, text });
    paint();
  }

  window.sampleEffect = { show, hide, isShowing, valueArrived };
})();
