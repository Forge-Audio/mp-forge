// Says what something is once the pointer has rested on it for a second, in a small label right by the pointer.
// Anything with a `title` gets one. The title is taken over, so the computer's own tooltip never shows as well.

(function () {
  "use strict";

  const { element, thisComputer } = window.helpers;

  const RESTS_FOR_MS = 1000;
  const GAP = 6;
  // Room left for the pointer's own arrow, so the label never sits under it.
  const POINTER_REACHES = 16;

  const label = element("div", "hover-name");
  let resting;
  let restedOn = null;
  let pointer = { x: 0, y: 0 };

  const KEY_A_MAC_HAS = /\bCmd\b/g;

  // Shortcuts are written the way a Mac names their keys. On Windows and Linux, Ctrl does what Cmd does there.
  function inThisComputersKeys(words) {
    return words.replace(KEY_A_MAC_HAS, thisComputer.commandKey);
  }

  // Someone who cannot see the label still hears what it says, unless the thing's own label already says it.
  function takeOverTitle(thing) {
    if (!thing.title) return;
    const said = inThisComputersKeys(thing.title);
    thing.dataset.hoverName = said;
    if (thing.getAttribute("aria-label") !== thing.title) thing.setAttribute("aria-description", said);
    thing.removeAttribute("title");
  }

  function namedThingAt(target) {
    const thing = target.closest ? target.closest("[title], [data-hover-name]") : null;
    if (thing) takeOverTitle(thing);
    return thing;
  }

  function keptInside(start, length, room) {
    return Math.round(Math.max(GAP, Math.min(start, room - length - GAP)));
  }

  // Centred on the pointer and just under it: under the thing itself when that is near, and never further away
  // than the pointer's own arrow reaches. Over the pointer instead when there is no room underneath.
  function placeByThePointer(thing) {
    const box = thing.getBoundingClientRect();
    const under = Math.min(box.bottom, pointer.y + POINTER_REACHES) + GAP;
    const over = Math.max(box.top, pointer.y - POINTER_REACHES) - GAP - label.offsetHeight;
    const fitsUnder = under + label.offsetHeight + GAP <= window.innerHeight;
    label.style.left = `${keptInside(pointer.x - label.offsetWidth / 2, label.offsetWidth, window.innerWidth)}px`;
    label.style.top = `${Math.round(fitsUnder ? under : over)}px`;
  }

  function show(thing) {
    if (!thing.isConnected) return;
    label.textContent = thing.dataset.hoverName;
    label.hidden = false;
    placeByThePointer(thing);
  }

  function hide() {
    clearTimeout(resting);
    restedOn = null;
    label.hidden = true;
  }

  function pointerMoved(event) {
    pointer = { x: event.clientX, y: event.clientY };
  }

  function pointerIsOver(event) {
    pointerMoved(event);
    const thing = namedThingAt(event.target);
    if (thing === restedOn) return;
    hide();
    if (!thing) return;
    restedOn = thing;
    resting = setTimeout(() => show(thing), RESTS_FOR_MS);
  }

  function start() {
    label.hidden = true;
    label.setAttribute("role", "tooltip");
    document.body.append(label);
    document.addEventListener("pointerover", pointerIsOver);
    document.addEventListener("pointermove", pointerMoved, { passive: true });
    document.addEventListener("pointerdown", hide);
    document.addEventListener("keydown", hide);
    document.addEventListener("scroll", hide, true);
    document.documentElement.addEventListener("pointerleave", hide);
  }

  start();
})();
