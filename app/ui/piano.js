// The piano under an instrument's screen while it plays: two octaves of pixel-art keys that play notes when
// pressed with the mouse or a finger, and light up for every note held, however it was played.
//
// The octave from middle C up is also played from the computer's keyboard, and each of its keys shows its letter.

(function () {
  "use strict";

  const DRAWING = "http://www.w3.org/2000/svg";
  const LOWEST_NOTE = 48;
  const HIGHEST_NOTE = 72;
  const MIDDLE_C = 60;
  const NOTES_IN_AN_OCTAVE = 12;
  const BLACK_NOTES = [1, 3, 6, 8, 10];
  // The computer keys that play the octave from middle C up, a semitone apart, as on any music program.
  const TYPING_KEYS = "awsedftgyhujk";
  // Sizes in the drawing's own pixels. Neighbouring white keys share the line between them.
  const WHITE = { width: 12, height: 40, lip: 4 };
  const BLACK = { width: 8, height: 25, lip: 3 };
  const LINE = 1;

  function drawn(tag, attributes) {
    const made = document.createElementNS(DRAWING, tag);
    Object.entries(attributes).forEach(([name, value]) => made.setAttribute(name, value));
    return made;
  }

  function isBlack(pitch) {
    return BLACK_NOTES.includes(pitch % NOTES_IN_AN_OCTAVE);
  }

  function pitches() {
    return Array.from({ length: HIGHEST_NOTE - LOWEST_NOTE + 1 }, (unused, place) => LOWEST_NOTE + place);
  }

  function whiteKeysBelow(pitch) {
    return pitches().filter((lower) => lower < pitch && !isBlack(lower)).length;
  }

  // Where a key sits. A black key straddles the line between the white keys either side of it.
  function placeOf(pitch) {
    const left = whiteKeysBelow(pitch) * WHITE.width;
    return isBlack(pitch) ? { left: left - BLACK.width / 2, ...BLACK, width: BLACK.width + LINE } : { left, ...WHITE, width: WHITE.width + LINE };
  }

  function letterOn(pitch) {
    return TYPING_KEYS[pitch - MIDDLE_C] || "";
  }

  function letterDrawn(pitch, place) {
    const letter = drawn("text", { class: "piano-letter", x: place.left + place.width / 2, y: place.height - place.lip - 2, "text-anchor": "middle" });
    letter.textContent = letterOn(pitch).toUpperCase();
    return letter;
  }

  // One key: its outline, its face, and the lip along its front edge that goes when the key is down.
  function keyDrawn(pitch) {
    const place = placeOf(pitch);
    const inside = { x: place.left + LINE, width: place.width - 2 * LINE };
    const key = drawn("g", { class: `piano-key ${isBlack(pitch) ? "black" : "white"}`, "data-pitch": pitch });
    key.append(
      drawn("rect", { class: "piano-edge", x: place.left, y: 0, width: place.width, height: place.height }),
      drawn("rect", { class: "piano-face", ...inside, y: LINE, height: place.height - 2 * LINE }),
      drawn("rect", { class: "piano-lip", ...inside, y: place.height - LINE - place.lip, height: place.lip }),
    );
    if (letterOn(pitch)) key.append(letterDrawn(pitch, place));
    return key;
  }

  function keyboardDrawn() {
    const whiteKeys = pitches().filter((pitch) => !isBlack(pitch));
    const keyboard = drawn("svg", { class: "piano-keys", viewBox: `0 0 ${whiteKeys.length * WHITE.width + LINE} ${WHITE.height}`, "shape-rendering": "crispEdges", role: "group", "aria-label": "Piano keys that play notes" });
    const blackKeysOnTop = [...whiteKeys, ...pitches().filter(isBlack)];
    keyboard.append(...blackKeysOnTop.map(keyDrawn));
    return keyboard;
  }

  // Puts the piano in `holder`. `onNote(pitch, pressed)` is told of every key that goes down or comes up.
  // `press` plays a note from anywhere else, such as the computer's keyboard, and shows it on its key.
  // `letGo` lifts every key that is down.
  function standIn(holder, onNote) {
    const keyboard = keyboardDrawn();
    const keys = new Map([...keyboard.querySelectorAll(".piano-key")].map((key) => [Number(key.dataset.pitch), key]));
    let isSliding = false;

    function press(pitch, pressed) {
      const key = keys.get(pitch);
      if (!key || key.classList.contains("down") === pressed) return;
      key.classList.toggle("down", pressed);
      onNote(pitch, pressed);
    }

    function letGo() {
      keys.forEach((key, pitch) => press(pitch, false));
    }

    // A finger or the mouse can slide along the keys with the button held, playing each one it crosses.
    function followThePointer(key, pitch) {
      key.addEventListener("pointerdown", (event) => {
        if (key.hasPointerCapture(event.pointerId)) key.releasePointerCapture(event.pointerId);
        isSliding = true;
        press(pitch, true);
      });
      key.addEventListener("pointerenter", () => { if (isSliding) press(pitch, true); });
      ["pointerup", "pointerleave", "pointercancel"].forEach((ending) => key.addEventListener(ending, () => press(pitch, false)));
    }

    keys.forEach(followThePointer);
    window.addEventListener("pointerup", () => { isSliding = false; });
    window.addEventListener("pointercancel", () => { isSliding = false; });
    holder.replaceChildren(keyboard);
    return { press, letGo };
  }

  // The note a computer key plays, or nothing when it plays none.
  function noteTypedWith(letter) {
    const place = TYPING_KEYS.indexOf(letter.toLowerCase());
    return place < 0 ? null : MIDDLE_C + place;
  }

  window.pixelPiano = { standIn, noteTypedWith };
})();
