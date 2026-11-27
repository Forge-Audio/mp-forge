// The six knobs an effect is played with on an MPC Sample, chosen by hand.
//
// An MPC Sample shows none of an effect's pages. It has three knobs, and the same three while SHIFT is held,
// each with a short label. The sheet here shows which control sits on which knob, as kept with the effect or
// as MP Forge's first guess, and lets the musician change it. It keeps what they chose and draws nothing else.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element } = window.helpers;
  const { asked } = window.sheets;

  const LONGEST_SHORT_NAME = 12;
  const LONGEST_LABEL = 9;
  const TURNS_NOTHING = "";
  const ROWS = [
    { key: "knobs", words: "The three knobs" },
    { key: "shiftKnobs", words: "The same knobs while SHIFT is held" },
  ];
  const WHAT_THEY_ARE = "An MPC Sample shows none of this effect's pages. These six controls are all of it there.";
  const A_FIRST_GUESS = "Nobody has chosen yet. This is MP Forge's first guess.";
  const NEEDS_A_BUILD = "The knobs can be chosen once the effect has been built. Run the safety check first.";

  function startOf(words, longest) {
    return words.slice(0, longest).trim();
  }

  function boxFor(words, longest, describedAs) {
    const box = element("input", "item-input");
    box.maxLength = longest;
    box.value = words || "";
    box.spellcheck = false;
    box.setAttribute("aria-label", describedAs);
    return box;
  }

  function listOf(controls, chosen, describedAs) {
    const list = element("select", "item-input");
    list.setAttribute("aria-label", describedAs);
    [TURNS_NOTHING, ...controls].forEach((control) => {
      const row = element("option", "", control || "Nothing");
      row.value = control;
      list.append(row);
    });
    list.value = controls.includes(chosen) ? chosen : TURNS_NOTHING;
    return list;
  }

  // One knob: the control it turns and the label it shows. A label the musician has not written themselves
  // follows the control.
  function seatFor(kept, calledAs, controls) {
    const control = listOf(controls, kept.control, `${calledAs} turns`);
    const label = boxFor(kept.label, LONGEST_LABEL, `${calledAs} is labelled`);
    let followed = kept.control || TURNS_NOTHING;
    control.addEventListener("change", () => {
      if (label.value === "" || label.value === startOf(followed, LONGEST_LABEL)) label.value = startOf(control.value, LONGEST_LABEL);
      followed = control.value;
    });
    const shown = element("div", "six-knobs-seat");
    shown.append(element("div", "field-label", calledAs), control, label);
    return { shown, chosen: () => (control.value ? { control: control.value, label: label.value.trim() } : {}) };
  }

  function rowFor(row, kept, controls) {
    const seats = [0, 1, 2].map((position) => seatFor((kept[row.key] || [])[position] || {}, `K${position + 1}`, controls));
    const shown = element("div", "six-knobs-row");
    shown.append(...seats.map((seat) => seat.shown));
    return { heading: element("div", "field-label", row.words), shown, chosen: () => seats.map((seat) => seat.chosen()) };
  }

  // Shows the sheet for the effect and keeps what the musician chose. Answers with whether anything was kept.
  // An effect that has not been built has no controls to choose from yet, and that is said in its place.
  async function choose(sheet, effectName) {
    const kept = await ask("six_knobs", { name: effectName });
    if (!kept.controls.length) throw NEEDS_A_BUILD;
    const shortName = boxFor(kept.shortName, LONGEST_SHORT_NAME, "Short name");
    const rows = ROWS.map((row) => rowFor(row, kept, kept.controls));
    const holds = [
      element("div", "soft small", WHAT_THEY_ARE),
      ...(kept.firstGuess ? [element("div", "soft small", A_FIRST_GUESS)] : []),
      element("div", "field-label", `Short name, at most ${LONGEST_SHORT_NAME} letters`), shortName,
      ...rows.flatMap((row) => [row.heading, row.shown]),
    ];
    const whatWasChosen = () => ({ shortName: shortName.value.trim() || undefined, knobs: rows[0].chosen(), shiftKnobs: rows[1].chosen() });
    const chosen = await asked(sheet, { title: `THE SIX KNOBS OF ${effectName.toUpperCase()}`, confirmWords: "KEEP THESE", holds, wide: true }, whatWasChosen);
    if (!chosen) return false;
    await ask("choose_six_knobs", { name: effectName, knobs: chosen });
    return true;
  }

  window.sixKnobs = { choose };
})();
