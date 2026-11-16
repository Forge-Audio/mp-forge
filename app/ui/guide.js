// The guide that shows a new musician round: a few cards, each pointing at one part of the screen.
//
// It starts on the form for a new plugin, then opens a made-up example plugin to point at what a built plugin
// offers, and ends back on the form. While it shows, the app underneath cannot be clicked or typed into.
// It is offered once, the first time the app starts, and can be asked for again from Settings.

(function () {
  "use strict";

  const { remember, recall } = window.helpers;

  const SEEN = "guideSeen";
  const GAP_FROM_ITS_SPOT = 14;
  const GAP_FROM_THE_EDGE = 10;
  const ROOM_ROUND_THE_SPOT = 6;
  // The plugin's view slides in beside the conversation. The card and its ring follow it for as long as it moves.
  const PART_THAT_SLIDES = "#conversation";
  const PARTS_OF_THE_APP = [".bar", ".desk"];
  // Each step points at the part of the screen named by `at`. A step with none, or whose part is not showing,
  // is put in the middle of the window. A step marked `onExample` is shown with the example plugin open.
  const STEPS = [
    {
      title: "WELCOME TO MP FORGE",
      words: "You describe a plugin in plain words, an AI builds it, and MP Forge checks that it is safe before it goes anywhere near your MPC. Showing you round takes a minute.",
    },
    {
      at: "#ai-menu-button", title: "PICK YOUR AI",
      words: "This is the AI that builds your plugins. Open it to pick a model. A service you have not used before asks you to sign in or add a key the first time. Models that run on this computer need no account.",
    },
    {
      at: "#new-plugin-fields", title: "NAME IT",
      words: "Give the plugin a name and say who made it. Both show on your MPC. Then choose its kind: an effect changes sound played through it, an instrument makes sound when you play notes.",
    },
    {
      at: ".message-box", title: "DESCRIBE IT",
      words: "Say what you want as you would to a friend: \u201ca worn tape echo that gets seasick when I turn it up\u201d. The + adds a picture or a sound file. Then press BUILD IT.",
    },
    {
      at: "#starters", title: "NO IDEA YET?",
      words: "I\u2019M FEELING LUCKY rolls the dice for a theme and builds a surprise from something it finds. REMIX A PLUGIN starts from one in the Open MPC catalog. Tick the plugin architect above to be asked a few questions first.",
    },
    {
      at: "#mpc-screen", title: "YOUR PLUGIN\u2019S SCREEN", onExample: true,
      words: "This one is only an example. Once a plugin is built, its screen shows here as it will look on the MPC. Drag a control to turn it, and press a page name along the bottom to change page.",
    },
    {
      at: "#play", title: "HEAR IT", onExample: true,
      words: "Play runs the real plugin through this computer\u2019s speakers. An instrument gets a piano to play. An effect is given a beat to work on, or a sound file of your own, and can be switched off to compare. The volume knob is just below.",
    },
    {
      at: ".message-box", title: "ASK FOR CHANGES", onExample: true,
      words: "Say what to change, here in the conversation. Every change keeps the version before it, and VERSIONS at the top steps back through them.",
    },
    {
      at: "#edit-look", title: "EDIT THE LOOK", onExample: true,
      words: "Move, resize and restyle the controls by hand, add your own artwork, or pick a part of the screen and ask MP Forge to change just that.",
    },
    {
      at: ".hardware", title: "THE HARDWARE KNOBS", onExample: true,
      words: "The MPC\u2019s 16 knobs turn the controls of the page that is showing. This says which knob turns what.",
    },
    {
      at: "#status-button", title: "THE SAFETY CHECK", onExample: true,
      words: "Every plugin is built and played through a simulator before it can be sent. This says SAFE TO SEND or BLOCKED. Nothing reaches your MPC without passing.",
    },
    {
      at: "#send-to-mpc", title: "SEND IT TO YOUR MPC", onExample: true,
      words: "Once it is safe, this sends the plugin over your network to a USB drive or SD card in your MPC. It never touches the MPC\u2019s built-in storage.",
    },
    {
      at: "#settings-button", title: "SETTINGS", onExample: true,
      words: "Your keys and sign-ins, your MPC\u2019s address, where your plugins are kept, and this guide again. That is everything. Next you are back at the start, ready to make your first plugin.",
    },
  ];

  let parts;
  let views;
  let stepShown = 0;
  let exampleIsShown = false;
  const slideWatcher = new ResizeObserver(() => place());

  function partPointedAt(step) {
    const part = step.at && document.querySelector(step.at);
    const isShowing = part && part.getClientRects().length > 0 && getComputedStyle(part).visibility !== "hidden";
    return isShowing ? part : null;
  }

  function within(lowest, wanted, highest) {
    return Math.max(lowest, Math.min(wanted, highest));
  }

  function middleOfTheWindow(card) {
    return { left: (window.innerWidth - card.width) / 2, top: (window.innerHeight - card.height) / 2 };
  }

  function fitsInTheWindow(place, card) {
    const fitsAcross = place.left >= GAP_FROM_THE_EDGE && place.left + card.width <= window.innerWidth - GAP_FROM_THE_EDGE;
    const fitsDown = place.top >= GAP_FROM_THE_EDGE && place.top + card.height <= window.innerHeight - GAP_FROM_THE_EDGE;
    return fitsAcross && fitsDown;
  }

  // The places a card can go beside the part it points at, best first: under it, over it, left of it, right of it.
  // Under and over, it is slid sideways as far as it takes to stay in the window.
  function placesBeside(spot, card) {
    const mostLeft = window.innerWidth - card.width - GAP_FROM_THE_EDGE;
    const centredAcross = within(GAP_FROM_THE_EDGE, spot.left + spot.width / 2 - card.width / 2, mostLeft);
    const centredDown = spot.top + spot.height / 2 - card.height / 2;
    return [
      { left: centredAcross, top: spot.bottom + GAP_FROM_ITS_SPOT },
      { left: centredAcross, top: spot.top - GAP_FROM_ITS_SPOT - card.height },
      { left: spot.left - GAP_FROM_ITS_SPOT - card.width, top: centredDown },
      { left: spot.right + GAP_FROM_ITS_SPOT, top: centredDown },
    ];
  }

  function moveCardTo(place) {
    parts.card.style.left = `${place.left}px`;
    parts.card.style.top = `${place.top}px`;
  }

  function ringRound(spot) {
    parts.spot.style.top = `${spot.top - ROOM_ROUND_THE_SPOT}px`;
    parts.spot.style.left = `${spot.left - ROOM_ROUND_THE_SPOT}px`;
    parts.spot.style.width = `${spot.width + 2 * ROOM_ROUND_THE_SPOT}px`;
    parts.spot.style.height = `${spot.height + 2 * ROOM_ROUND_THE_SPOT}px`;
  }

  // A card with no room beside its part goes in the middle of the window, over the part if need be.
  function place() {
    const part = partPointedAt(STEPS[stepShown]);
    const card = parts.card.getBoundingClientRect();
    parts.guide.classList.toggle("points-at-nothing", !part);
    if (!part) return moveCardTo(middleOfTheWindow(card));
    const spot = part.getBoundingClientRect();
    ringRound(spot);
    return moveCardTo(placesBeside(spot, card).find((beside) => fitsInTheWindow(beside, card)) || middleOfTheWindow(card));
  }

  function draw() {
    const step = STEPS[stepShown];
    const isLast = stepShown === STEPS.length - 1;
    parts.title.textContent = step.title;
    parts.words.textContent = step.words;
    parts.count.textContent = `${stepShown + 1} OF ${STEPS.length}`;
    parts.back.hidden = stepShown === 0;
    parts.skip.hidden = isLast;
    parts.next.textContent = isLast ? "DONE" : stepShown === 0 ? "SHOW ME ROUND" : "NEXT";
    parts.card.setAttribute("aria-label", step.title);
    place();
    parts.next.focus();
  }

  // Brings up the view a step is shown on, when it is not the one showing.
  async function bringUpTheViewFor(step) {
    const wantsTheExample = Boolean(step.onExample);
    if (wantsTheExample === exampleIsShown) return;
    exampleIsShown = wantsTheExample;
    await (wantsTheExample ? views.showExample() : views.showNewPlugin());
  }

  async function goTo(step) {
    stepShown = within(0, step, STEPS.length - 1);
    await bringUpTheViewFor(STEPS[stepShown]);
    draw();
  }

  function stepWithKeys(event) {
    if (event.key === "Escape") close();
    if (event.key === "ArrowRight") goTo(stepShown + 1);
    if (event.key === "ArrowLeft") goTo(stepShown - 1);
  }

  function setTheAppAside(aside) {
    PARTS_OF_THE_APP.forEach((part) => { document.querySelector(part).inert = aside; });
  }

  function close() {
    parts.guide.hidden = true;
    remember(SEEN, "yes");
    document.removeEventListener("keydown", stepWithKeys);
    window.removeEventListener("resize", place);
    slideWatcher.disconnect();
    setTheAppAside(false);
    exampleIsShown = false;
    views.showNewPlugin();
  }

  function show() {
    parts.guide.hidden = false;
    document.addEventListener("keydown", stepWithKeys);
    window.addEventListener("resize", place);
    slideWatcher.observe(document.querySelector(PART_THAT_SLIDES));
    setTheAppAside(true);
    goTo(0);
  }

  function showOnTheFirstStart() {
    if (recall(SEEN) !== "yes") show();
  }

  // `guideViews.showExample()` opens the example plugin, and `guideViews.showNewPlugin()` goes back to the form
  // for a new plugin and lets the example go.
  function start(guideParts, guideViews) {
    parts = guideParts;
    views = guideViews;
    parts.skip.addEventListener("click", close);
    parts.back.addEventListener("click", () => goTo(stepShown - 1));
    parts.next.addEventListener("click", () => (stepShown === STEPS.length - 1 ? close() : goTo(stepShown + 1)));
  }

  window.guide = { start, show, showOnTheFirstStart };
})();
