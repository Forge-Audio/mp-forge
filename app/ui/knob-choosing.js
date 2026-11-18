// Giving a control another of the MPC's hardware knobs. The musician presses the pencil beside the knobs, picks a
// knob, then presses the control on the plugin's screen that the knob should turn.
//
// This keeps how far along they are, and the words that say what to do next. It draws nothing and keeps nothing.

(function () {
  "use strict";

  const PICK_A_KNOB = "Pick a knob to give another control to.";
  const pickItsControl = (knob) => `Knob ${knob}: now press the control on the screen it should turn.`;

  let choosing = null;

  function begin() {
    choosing = { knob: null, versionKept: false };
  }

  function end() {
    choosing = null;
  }

  function isOn() {
    return choosing !== null;
  }

  function chosenKnob() {
    return choosing ? choosing.knob : null;
  }

  // Pressing the chosen knob again lets go of it.
  function knobPressed(number) {
    choosing.knob = choosing.knob === number ? null : number;
  }

  function knobWasGiven() {
    choosing.knob = null;
  }

  // The first change keeps a version of the plugin, so GO BACK undoes every knob given since the pencil was pressed.
  function stillNeedsAVersionKept() {
    return !choosing.versionKept;
  }

  function versionWasKept() {
    choosing.versionKept = true;
  }

  function whatToDoNext() {
    return choosing.knob === null ? PICK_A_KNOB : pickItsControl(choosing.knob);
  }

  window.knobChoosing = { begin, end, isOn, chosenKnob, knobPressed, knobWasGiven, stillNeedsAVersionKept, versionWasKept, whatToDoNext };
})();
