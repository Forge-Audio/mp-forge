// The made-up plugin the guide shows a new musician round, so there is something to point at before they
// have made a plugin of their own. It exists only on the screen: no files, no sound, nothing to send.

(function () {
  "use strict";

  const { areaOfSlot } = window.mpcStage;

  const SCREEN = {
    kit: null,
    pages: [
      { name: "MAIN", controls: ["Time", "Repeats", "Tone", "Mix", "Wobble", "Flutter", "Dirt", "Hiss"] },
      { name: "MORE", controls: ["Low Cut", "Width", "Volume"] },
    ],
  };
  const TALK = [
    { kind: "you", text: "A worn tape echo that gets seasick when I turn it up." },
    { kind: "said", text: "**Built.** Time and Repeats set the echo, Wobble and Flutter make it seasick, and Dirt and Hiss wear the tape. It passed its safety check." },
  ];
  const REPORT = { safe: true, checks: [] };

  function controlNames() {
    return SCREEN.pages.flatMap((page) => page.controls);
  }

  // Where each control stands to begin with: spread out, so the knobs do not all point the same way.
  function parameter(name, index) {
    const amount = ((index * 37) % 80 + 10) / 100;
    return { name, index, default: amount, display: String(Math.round(amount * 100)), label: "%", kind: "float", choices: [] };
  }

  function arrangedPage(page) {
    const items = page.controls.map((name, slot) => ({ kind: "control", name, slot, area: areaOfSlot(slot), shownName: null, moved: false, parts: null }));
    return { name: page.name, items, pictures: [], artwork: [], groupBoxes: [] };
  }

  // The example as an opened plugin. It carries no build, so it is never played and has no presets to read.
  function plugin(name, maker) {
    return {
      summary: { name, maker, kind: "effect" },
      screen: SCREEN,
      parameters: { params: controlNames().map(parameter) },
      arrangement: { pages: SCREEN.pages.map(arrangedPage) },
      look: { pages: [] },
      versionsKept: 2, samples: [], pictures: [],
    };
  }

  window.guideExample = { NAME: "Example Echo", TALK, REPORT, plugin };
})();
