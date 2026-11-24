// A game controller plugged into this computer, standing in for one plugged into the MPC Sample.
//
// The browser's own list of controllers is looked at as often as the screen is drawn, and whatever changed on
// the first of them is told in the words the app's player knows: a button by where it sits, a stick from -1 to
// 1 with up as more, a trigger from 0 to 1. A browser shows a controller only once a button on it has been
// pressed, so nothing is heard of one before that.

(function () {
  "use strict";

  // The buttons of a controller laid out the usual way, in the browser's order, by the names the kit gives them.
  const BUTTONS = ["SOUTH", "EAST", "WEST", "NORTH", "LEFT SHOULDER", "RIGHT SHOULDER", "LEFT TRIGGER", "RIGHT TRIGGER", "SELECT", "START", "LEFT STICK", "RIGHT STICK", "UP", "DOWN", "LEFT", "RIGHT", "HOME"];
  // The sticks' directions in the browser's order. The browser counts down as more, and the kit up.
  const STICKS = [{ name: "LEFT X", turned: 1 }, { name: "LEFT Y", turned: -1 }, { name: "RIGHT X", turned: 1 }, { name: "RIGHT Y", turned: -1 }];
  // A trigger is one of the buttons, which also says how far it is pulled.
  const TRIGGERS = [{ name: "LEFT TRIGGER", button: 6 }, { name: "RIGHT TRIGGER", button: 7 }];
  // A stick at rest is never quite still, so the middle counts as rest. A stick or a trigger is said again
  // only once it has moved this far.
  const COUNTS_AS_REST = 0.12;
  const SMALLEST_MOVE_WORTH_SAYING = 0.01;

  const held = new Map();
  const stands = new Map();
  let tell = () => {};

  function theFirstPluggedIn() {
    const listed = navigator.getGamepads ? Array.from(navigator.getGamepads()) : [];
    return listed.find((controller) => controller && controller.connected) || null;
  }

  function sayAButton(name, isPressed) {
    if (Boolean(held.get(name)) === isPressed) return;
    held.set(name, isPressed);
    tell({ control: "gamepad button", name, value: isPressed ? 1 : 0 });
  }

  function sayWhereItStands(name, value) {
    const lastSaid = stands.get(name) || 0;
    const isWorthSaying = Math.abs(value - lastSaid) >= SMALLEST_MOVE_WORTH_SAYING || (value === 0 && lastSaid !== 0);
    if (!isWorthSaying) return;
    stands.set(name, value);
    tell({ control: "gamepad axis", name, value });
  }

  function acrossAStick(value) {
    return Math.abs(value) < COUNTS_AS_REST ? 0 : value;
  }

  function sayWhatItDoes(controller) {
    BUTTONS.forEach((name, place) => sayAButton(name, Boolean(controller.buttons[place] && controller.buttons[place].pressed)));
    STICKS.forEach((stick, place) => sayWhereItStands(stick.name, acrossAStick((controller.axes[place] || 0) * stick.turned)));
    TRIGGERS.forEach((trigger) => sayWhereItStands(trigger.name, controller.buttons[trigger.button] ? controller.buttons[trigger.button].value : 0));
  }

  // The app is told that everything held on a controller that has gone was let go, so nothing stays held.
  function sayItWasAllLetGo() {
    held.forEach((isHeld, name) => sayAButton(name, false));
    stands.forEach((value, name) => sayWhereItStands(name, 0));
  }

  function look() {
    const controller = theFirstPluggedIn();
    if (controller) sayWhatItDoes(controller);
    else sayItWasAllLetGo();
    requestAnimationFrame(look);
  }

  // What a controller does is told to `heard` from now on.
  function playInto(heard) {
    tell = heard;
    requestAnimationFrame(look);
  }

  window.gamepad = { playInto };
})();
