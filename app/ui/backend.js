// How the screen talks to the app.
//
// Inside the real app this calls the Rust side. Opened in a plain browser there is no Rust side, so a
// demo stands in with made-up plugins. The demo exists only so the screen can be looked at on its own.

(function () {
  "use strict";

  function realBackend(tauri) {
    return {
      isDemo: false,
      ask: (command, details) => tauri.core.invoke(command, details),
      send: (command, bytes, labels) => tauri.core.invoke(command, bytes, { headers: labels }),
      on: (eventName, handler) => tauri.event.listen(eventName, (event) => handler(event.payload)),
    };
  }

  const DEMO_SCREENS = {
    "Dusty Tape": {
      kit: "cyan",
      pages: [
        { name: "MAIN", controls: [
          { param: "Time", widget: "knob" }, { param: "Repeats", widget: "knob" }, { param: "Tone", widget: "knob" }, { param: "Mix", widget: "fader" },
          { param: "Wobble", widget: "knob" }, { param: "Flutter", widget: "knob" }, { param: "Dirt", widget: "knob" }, { param: "Hiss", widget: "fader" },
        ] },
        { name: "MORE", controls: [
          { param: "Sync", widget: "switch" }, { param: "Division", widget: "slider", overlay: "menu" }, { param: "Low Cut", widget: "knob" }, { param: "Volume", widget: "fader" },
        ] },
      ],
    },
    "Glass Pad": {
      kit: "amber",
      pages: [
        { name: "MAIN", controls: ["Bloom", "Shimmer", "Tone", "Volume",
          { param: "Attack", widget: "fader" }, { param: "Decay", widget: "fader" }, { param: "Sustain", widget: "fader" }, { param: "Release", widget: "fader" }] },
        { name: "MOTION", controls: [{ widget: "text", text: "Movement" }, { param: "Motion", widget: "knob" }, { param: "Chorus", widget: "knob" }, "empty"] },
      ],
    },
  };

  const DEMO_PICTURES = {
    "Dusty Tape": [
      { name: "reels", page: "MAIN", slots: { row: 3, column: 1, rows: 2, columns: 4 }, frames: 12, motion: "loops", follows: null, loopSeconds: 2, version: 1 },
      { name: "dirt-lamp", page: "MORE", slots: { row: 2, column: 1, rows: 1, columns: 1 }, frames: 8, motion: "follows", follows: "Low Cut", version: 1 },
    ],
  };

  function drawDemoFrame(pen, frame, frames) {
    const turn = (frame / frames) * Math.PI / 2;
    pen.fillStyle = "#1b1815";
    pen.fillRect(0, 0, 320, 80);
    [90, 230].forEach((centre) => {
      pen.strokeStyle = "#d8c9a8";
      pen.lineWidth = 4;
      pen.beginPath();
      pen.arc(centre, 40, 28, 0, 2 * Math.PI);
      pen.moveTo(centre - 24 * Math.cos(turn), 40 - 24 * Math.sin(turn));
      pen.lineTo(centre + 24 * Math.cos(turn), 40 + 24 * Math.sin(turn));
      pen.stroke();
    });
    pen.fillStyle = `rgba(255, 90, 31, ${(frame + 1) / frames})`;
    pen.fillRect(150, 30, 20, 20);
  }

  // A stand-in film strip: every frame of a made-up picture, stacked top to bottom.
  function demoPictureImage(frames) {
    const strip = document.createElement("canvas");
    strip.width = 320;
    strip.height = 80 * frames;
    const pen = strip.getContext("2d");
    for (let frame = 0; frame < frames; frame += 1) {
      pen.setTransform(1, 0, 0, 1, 0, 80 * frame);
      drawDemoFrame(pen, frame, frames);
    }
    return new Promise((resolve) => strip.toBlob(resolve, "image/png"));
  }

  function demoParameter(name, index) {
    const amount = ((index * 37) % 90 + 10) / 100;
    const isSwitch = name === "Sync";
    return { name, default: isSwitch ? 1 : amount, display: isSwitch ? "On" : String(Math.round(amount * 100)), label: isSwitch ? "" : "%", kind: isSwitch ? "bool" : "float", choices: isSwitch ? ["Off", "On"] : [] };
  }

  function demoParameters(screen) {
    const names = screen.pages.flatMap((page) => page.controls).map((control) => (typeof control === "string" ? control : control.param)).filter((name) => name && name !== "empty");
    return { params: names.map((name, index) => ({ ...demoParameter(name, index), index })) };
  }

  // --- The demo's stand-in for the app's own artwork drawing ------------------------------------

  const DEMO_PAGE = { x: 0, y: 0, width: 1280, height: 628 };
  const DEMO_FONTS = ["Arial", "Courier New", "Georgia", "Helvetica Neue", "Impact", "Times New Roman", "Verdana"];
  const demoArt = new Map();

  function demoLettering(layer) {
    return `${layer.bold ? 700 : 400} ${layer.size}px "${layer.font}"`;
  }

  // Where a layer sits. Words are measured in their font, as the app measures them.
  function demoAreaOf(layer) {
    if (layer.kind !== "words") return layer.area;
    const pen = document.createElement("canvas").getContext("2d");
    pen.font = demoLettering(layer);
    return { ...layer.place, width: Math.max(1, Math.ceil(pen.measureText(layer.text).width)), height: Math.ceil(layer.size * 1.25) };
  }

  function demoFill(pen, layer, area) {
    if (!layer.fade) return layer.colour;
    const across = (hundredths) => area.x + (area.width * hundredths) / 100;
    const down = (hundredths) => area.y + (area.height * hundredths) / 100;
    const fade = pen.createLinearGradient(across(layer.fade.from.x), down(layer.fade.from.y), across(layer.fade.to.x), down(layer.fade.to.y));
    fade.addColorStop(0, layer.colour);
    fade.addColorStop(1, layer.fade.colour);
    return fade;
  }

  function demoOutline(pen, layer, area, inset) {
    const inside = { x: area.x + inset, y: area.y + inset, width: Math.max(0, area.width - inset * 2), height: Math.max(0, area.height - inset * 2) };
    pen.beginPath();
    if (layer.shape === "ellipse") pen.ellipse(inside.x + inside.width / 2, inside.y + inside.height / 2, inside.width / 2, inside.height / 2, 0, 0, 2 * Math.PI);
    else pen.roundRect(inside.x, inside.y, inside.width, inside.height, Math.max(0, (layer.corner || 0) - inset));
  }

  function drawDemoBlock(pen, layer, area) {
    pen.fillStyle = demoFill(pen, layer, area);
    demoOutline(pen, layer, area, 0);
    pen.fill();
    if (!layer.edge) return;
    pen.strokeStyle = layer.edge.colour;
    pen.lineWidth = layer.edge.width;
    demoOutline(pen, layer, area, layer.edge.width / 2);
    pen.stroke();
  }

  function drawDemoWords(pen, layer, area) {
    pen.font = demoLettering(layer);
    pen.fillStyle = layer.colour;
    pen.fillText(layer.text, area.x, area.y + layer.size * 0.95);
  }

  function drawDemoArt(pen, layer, area) {
    const kept = demoArt.get(layer.file);
    if (!kept) return;
    const crop = layer.crop || { x: 0, y: 0, width: kept.width, height: kept.height };
    pen.drawImage(kept.image, crop.x, crop.y, crop.width, crop.height, area.x, area.y, area.width, area.height);
  }

  function drawDemoLayer(pen, layer) {
    const draw = { block: drawDemoBlock, words: drawDemoWords, picture: drawDemoArt }[layer.kind];
    pen.globalAlpha = (layer.opacity === undefined ? 100 : layer.opacity) / 100;
    draw(pen, layer, demoAreaOf(layer));
    pen.globalAlpha = 1;
  }

  function demoDrawing(size, layers) {
    const canvas = Object.assign(document.createElement("canvas"), { width: size.width, height: size.height });
    layers.forEach((layer) => drawDemoLayer(canvas.getContext("2d"), layer));
    return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
  }

  async function keepDemoArt(bytes) {
    const image = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
    const file = `art-${demoArt.size + 1}.png`;
    demoArt.set(file, { image, width: image.width, height: image.height });
    return { file, width: image.width, height: image.height };
  }

  // --- The demo's stand-in for the app's own arranging -----------------------------------------

  const guidesWritten = new Set();

  // The plugin the demo pretends somebody handed over.
  const DEMO_IMPORT = "Glass Pad";

  function demoPageLook(look, pageName) {
    const page = look.pages.find((known) => known.page === pageName) || {};
    return { items: page.items || [], artwork: page.artwork || [], added: page.added || [], removed: page.removed || [], knobs: page.knobs || [] };
  }

  function isDemoEmpty(entry) {
    return entry === undefined || entry === "empty" || (entry && entry.widget === "empty");
  }

  // A page's controls once what the musician added and took off is applied, as the app writes them into the layout.
  function demoControlsOf(page, pageLook) {
    const controls = page.controls.slice();
    pageLook.removed.forEach((removed) => { controls[removed.slot] = "empty"; });
    pageLook.added.forEach((added) => {
      const free = controls.findIndex(isDemoEmpty);
      controls[free < 0 ? controls.length : free] = { param: added.control, widget: added.widget };
    });
    return controls;
  }

  function demoScreenWith(screen, look) {
    return { ...screen, pages: screen.pages.map((page) => ({ ...page, controls: demoControlsOf(page, demoPageLook(look, page.name)) })) };
  }

  function demoSizeOfParts(parts) {
    const shown = [parts.art, parts.name && !parts.name.hidden && parts.name.area, parts.value && !parts.value.hidden && parts.value.area].filter(Boolean);
    return { width: Math.max(...shown.map((area) => area.x + area.width)), height: Math.max(...shown.map((area) => area.y + area.height)) };
  }

  // Each thing sits where its look puts it, at the size its look gives it, or else in its slot.
  // The hardware knob that turns a control: the one the musician chose, or the one beside its slot.
  function demoKnobOf(pageLook, controlName, slot) {
    const chosen = (pageLook.knobs || []).find((choice) => choice.control === controlName);
    return chosen ? chosen.knob : 13 + (slot % 4) - 4 * Math.floor(slot / 4);
  }

  function demoArrangement(screen, pictures, look) {
    const { areaOfSlot, areaOfBlock, PAGE } = window.mpcStage;
    const changeOf = (pageLook, kind, name) => pageLook.items.find((change) => change.kind === kind && change.name === name) || {};
    const placed = (area, change) => ({ ...area, ...(change.size || {}), ...(change.place || {}) });
    const itemIn = (pageLook, entry, slot) => {
      const isHeading = entry && entry.widget === "text";
      const kind = isHeading ? "heading" : "control";
      const name = typeof entry === "string" ? entry : isHeading ? entry.text : entry.param;
      const change = changeOf(pageLook, kind, name);
      const sized = change.parts ? { ...areaOfSlot(slot), ...demoSizeOfParts(change.parts) } : areaOfSlot(slot);
      return { kind, name, slot, knob: isHeading ? null : demoKnobOf(pageLook, name, slot), area: placed(sized, change), shownName: change.shownName || null, moved: Boolean(change.place), parts: change.parts || null };
    };
    const pictureOn = (pageLook, picture) => {
      const ownArea = picture.slots ? areaOfBlock(picture.slots) : PAGE;
      return { name: picture.name, area: placed(ownArea, changeOf(pageLook, "picture", picture.name)) };
    };
    const pageOf = (page) => {
      const pageLook = demoPageLook(look, page.name);
      return {
        name: page.name,
        items: page.controls.map((entry, slot) => (isDemoEmpty(entry) ? null : itemIn(pageLook, entry, slot))).filter(Boolean),
        pictures: pictures.filter((picture) => picture.page === page.name).map((picture) => pictureOn(pageLook, picture)),
        artwork: pageLook.artwork.map((layer) => ({ id: layer.id, area: demoAreaOf(layer), hidden: Boolean(layer.hidden) })),
        groupBoxes: [],
      };
    };
    return { pages: screen.pages.map(pageOf) };
  }

  // The notes the demo's silent player was asked to play, kept so they can be looked at.
  const demoNotes = [];
  window.demoNotes = demoNotes;
  // How loud the demo's silent player was last told to play, whether it was to pass its effect by, and every
  // sound it was given to work on.
  const demoListening = { volume: null, effect: true, soundsFed: [] };
  window.demoListening = demoListening;
  const DEMO_README = {
    shown: "# Tape Echo\n\nA worn tape echo for the MPC.\n\n- Wow and flutter that grow with the TIME knob\n- A tone control on the repeats\n- Freeze switch\n\nBuilt with the MPC plugin builder SDK.",
    toldToTheBuilder: "What its makers wrote about it in their README follows, with `>` at the start of every line of it.\n\n> # Tape Echo\n> \n> A worn tape echo for the MPC.",
  };
  const DEMO_ROLL = { number: 4, theme: "Space", kind: "instrument" };
  const DEMO_FIND = {
    name: "Pulsar Gate",
    story: "A dead star 2,000 light years away ticks 1.4 times a second, steadier than most clocks. Its pulse makes a ready-made rhythm.",
    description: "An instrument whose notes are gated by a pulsar's tick. Controls: Spin, Drift, Glow, Depth. A dark screen with one star that flashes with each pulse.",
    source: "https://example.org/pulsars",
  };
  const DEMO_CATALOG = [
    { id: "tape-echo", name: "Tape Echo", author: "somebody", kind: "effect", summary: "A worn tape echo with wow, flutter and a tone control.", style: "delay", license: "MIT", source: "https://github.com/somebody/tape-echo", screenshot: "blacksmith.png", remixName: "Tape Echo Remix", brief: "Make a remix of \"Tape Echo\" by somebody, an effect (delay) from the Open MPC catalog.", aboutItsPicture: "One of the attached pictures is the original's screen." },
    { id: "tr-6w6", name: "6W6", author: "athousanddetails", kind: "instrument", summary: "A drum machine voice set fitted against real hardware recordings.", style: "drum-machine", license: "GPL-3.0-only", source: null, screenshot: null, remixName: "Remix 6W6", brief: "Make a remix of \"6W6\" by athousanddetails, an instrument (drum-machine) from the Open MPC catalog.", aboutItsPicture: "One of the attached pictures is the original's screen." },
    { id: "glass", name: "Glass Hall", author: "another", kind: "effect", summary: "A bright, long reverb.", style: "reverb", license: null, source: null, screenshot: "a-picture-that-is-not-there.png", remixName: "Glass Hall Remix", brief: "Make a remix of \"Glass Hall\" by another, an effect (reverb) from the Open MPC catalog.", aboutItsPicture: "One of the attached pictures is the original's screen." },
  ];
  const DEMO_SERVICES = [
    { word: "anthropic", name: "Anthropic", keysAreMadeAt: "console.anthropic.com/settings/keys", listsModelsWithoutAKey: false },
    { word: "openai", name: "OpenAI", keysAreMadeAt: "platform.openai.com/api-keys", listsModelsWithoutAKey: false },
    {
      word: "chatgpt", name: "ChatGPT (your plan)", keysAreMadeAt: "", listsModelsWithoutAKey: false, signsInWithBrowser: true, takesNoKey: true, usesChatGptPlan: true,
      signInButton: "CONTINUE WITH CHATGPT", aboutSigningIn: ["Use the ChatGPT plan you already have.", "The demo signs you in by itself after a moment."],
    },
    { word: "google", name: "Google", keysAreMadeAt: "aistudio.google.com/apikey", listsModelsWithoutAKey: false },
    { word: "openrouter", name: "OpenRouter", keysAreMadeAt: "openrouter.ai/settings/keys", listsModelsWithoutAKey: true, signsInWithBrowser: true, sellsWebLookups: true },
    { word: "ollama", name: "Ollama (on this computer)", keysAreMadeAt: "", listsModelsWithoutAKey: true, runsOnThisComputer: true },
  ];

  // Made-up names, so the demo never passes for a list of real models.
  const DEMO_MODELS = {
    openai: [{ id: "demo-large", name: "demo-large", seesPictures: true }, { id: "demo-small", name: "demo-small", seesPictures: true }],
    google: [{ id: "demo-pro", name: "demo-pro", seesPictures: true }],
    // Every other made-up model can be told how hard to think, and only at some levels.
    openrouter: Array.from({ length: 90 }, (unused, index) => ({ id: `lab-${index % 9}/model-${index}`, name: `Lab ${index % 9}: Model ${index}`, seesPictures: index % 2 === 0, efforts: index % 2 === 0 ? ["low", "medium", "high"] : [] })),
    ollama: [{ id: "demo-local:8b", name: "demo-local:8b", seesPictures: false }],
    chatgpt: [{ id: "demo-plan-large", name: "Demo Plan Large", seesPictures: true, contextWindow: 400000 }],
    codex: [{ id: "demo-chat-large", name: "Demo Chat Large", seesPictures: true, contextWindow: 272000 }],
  };
  // The demo's ChatGPT sign-in is found signed in the second time it is checked, to show both ways the sheet reads.
  const DEMO_SIGN_IN_CHECKS = ["not-installed", "not-signed-in", "signed-in"];
  let demoSignInChecks = 0;

  function demoSignInAccount() {
    const signIn = DEMO_SIGN_IN_CHECKS[Math.min(demoSignInChecks, DEMO_SIGN_IN_CHECKS.length - 1)];
    demoSignInChecks += 1;
    return { word: "codex", name: "ChatGPT (Codex sign-in)", hasKey: signIn === "signed-in", signsIn: true, signIn, keysAreMadeAt: "", listsModelsWithoutAKey: false };
  }

  // The demo starts without the build tools, so fetching them can be tried.
  const DEMO_NEEDS = [
    { name: "Python 3", usedFor: "Runs Akai's tools that build and check a plugin.", isThere: true, isRequired: true, howToGet: "Choose GET WHAT IS MISSING. MP Forge fetches a Python of its own, about 30 MB, and installs nothing on your computer.", canBeFetched: true },
    { name: "Akai's build tools", usedFor: "The compiler that turns a plugin's code into what the MPC runs.", isThere: false, isRequired: true, howToGet: "Choose GET WHAT IS MISSING. Akai's build script fetches its compiler once, about 50 MB.", canBeFetched: true },
    { name: "SSH", usedFor: "Sends a plugin to your MPC over the network.", isThere: true, isRequired: false, howToGet: "", canBeFetched: false },
    { name: "Claude Code", usedFor: "Lets Claude do the work through your own Claude sign-in, without a key.", isThere: false, isRequired: false, howToGet: "Install Claude Code from claude.com/claude-code and sign in to it. Not needed when you use a key or another service.", canBeFetched: false },
  ];
  let demoToolsFetched = false;

  const DEMO_REPORT = {
    safe: true,
    checks: ["Code stays inside the plugin", "Builds for every MPC", "Survives everyday use", "Survives a full chord"].map((name) => ({ name, outcome: "PASS", details: [] })),
  };

  function demoBackend() {
    const handlers = {};
    const emit = (eventName, payload) => (handlers[eventName] || []).forEach((handler) => handler(payload));
    const plugins = [{ name: "Dusty Tape", maker: "Demo", kind: "effect" }, { name: "Glass Pad", maker: "Demo", kind: "instrument" }];

    const DEMO_CLOSING_WORDS = "This is the demo, so nothing was really built.\n\n**Controls:** Time, Repeats and Tone.";
    const DEMO_CHOICES = "This is the demo, so here are three made-up ideas: a reverb knob, a tape stop switch, or a stereo spread knob.\n\n[option] Add a reverb knob\n[option] Add a tape stop switch\n[option] Add a stereo spread knob";

    // Asking the demo for options shows how the AI's choices look. Anything else gets the plain closing words.
    function demoClosingWords(asked) {
      return /option|idea/i.test(asked) ? DEMO_CHOICES : DEMO_CLOSING_WORDS;
    }

    // As in the real app, a plugin that has only just been begun has the starter's screen, which is not its
    // own, until the AI has written one.
    const withoutAScreenYet = new Set();
    const NO_SCREEN_YET = { pages: [{ name: "MAIN", controls: ["Gain"] }] };

    // The steps the stand-in has yet to take for each plugin it pretends to work on.
    const stepsToCome = new Map();
    const DEMO_STOPPED = "Stopped. Whatever was changed up to then has been kept, and GO BACK returns to the version before.";

    function pretendToWork(pluginName, asked) {
      const lines = [
        { kind: "step", text: "reading docs/overview.md" },
        { kind: "said", text: "I have read the documentation. Writing the screen and the audio code now." },
        { kind: "step", text: "writing ui.json" },
        { kind: "step", text: "writing src/Plugin.cpp" },
        { kind: "step", text: "running the safety check (this builds and tests the plugin)" },
      ];
      const later = (what, afterSteps) => stepsToCome.get(pluginName).push(setTimeout(what, 500 * afterSteps));
      stepsToCome.set(pluginName, []);
      lines.forEach((line, index) => later(() => {
        if (line.text === "writing src/Plugin.cpp") withoutAScreenYet.delete(pluginName);
        emit("work-progress", { ...line, plugin: pluginName });
      }, index + 1));
      lines.forEach((_, index) => later(() => emit("work-context", { plugin: pluginName, tokens: 21000 + 58000 * index }), index + 1));
      const closingWords = demoClosingWords(asked);
      later(() => emit("work-finished", { ok: true, plugin: pluginName, closingWords, report: DEMO_REPORT }), lines.length + 1);
    }

    // As in the real app, work that is stopped takes no further step and says that it was stopped.
    function stopPretendingToWork(pluginName) {
      (stepsToCome.get(pluginName) || []).forEach(clearTimeout);
      stepsToCome.delete(pluginName);
      withoutAScreenYet.delete(pluginName);
      setTimeout(() => emit("work-finished", { ok: false, plugin: pluginName, error: DEMO_STOPPED }), 200);
    }

    const samples = {};
    const samplesOf = (pluginName) => (samples[pluginName] = samples[pluginName] || []);

    function keepDemoSample(bytes, labels) {
      const kept = { fileName: decodeURIComponent(labels["file-name"]), seconds: bytes.length / 176400, channels: 2, sampleRate: 44100 };
      samplesOf(decodeURIComponent(labels["plugin-name"])).push(kept);
      return kept;
    }

    const looks = {};
    const lookOf = (pluginName) => looks[pluginName] || { pages: [] };
    const screenOf = (pluginName) => demoScreenWith(DEMO_SCREENS[pluginName] || DEMO_SCREENS["Dusty Tape"], lookOf(pluginName));
    const arrangementOf = (pluginName) => demoArrangement(screenOf(pluginName), DEMO_PICTURES[pluginName] || [], lookOf(pluginName));
    const artworkOn = (pluginName, pageName) => demoPageLook(lookOf(pluginName), pageName).artwork.filter((layer) => !layer.hidden);

    const demoKeys = new Set();
    const demoVersions = [
      { number: 1, keptAt: 1794500000, keptBefore: "the change \u201cmake the repeats darker\u201d", isHowItIsNow: false, bytes: 41200 },
      { number: 2, keptAt: 1794503600, keptBefore: "the look was edited by hand", isHowItIsNow: false, bytes: 43900 },
      { number: 3, keptAt: 1794590000, keptBefore: "the change \u201cadd a wobble knob and a hiss fader\u201d", isHowItIsNow: false, bytes: 1310000 },
    ];

    // The demo's architect asks the same made-up questions every time, and one more the second time round.
    const DEMO_QUESTIONS = [
      { asks: "How worn should it sound?", choices: ["Fresh off the reel", "Left in a hot car", "Chewed by the machine"], pickSeveral: false },
      { asks: "What should surprise you when you play it?", choices: ["It freezes on loud hits", "The repeats drift out of tune", "It answers back in reverse"], pickSeveral: false },
      { asks: "Which extras do you want under your hands?", choices: ["A big wobble knob", "A tape stop switch", "A hiss fader", "A stereo spread knob"], pickSeveral: true },
    ];
    const DEMO_LATER_QUESTION = { asks: "How should its screen feel?", choices: ["A dusty studio shelf", "Neon on black", "Plain and bold"], pickSeveral: false };
    const DEMO_PRESETS = ["Init", "Deep Space", "Slapback", "Worn Out"];
    let demoPreset = 0;
    let demoRun = 0;

    function demoPresetValues(chosen) {
      return [0, 1, 2].map((index) => ({ index, amount: ((chosen + 1) * (index + 2) * 0.13) % 1, text: `${Math.round((((chosen + 1) * (index + 2) * 0.13) % 1) * 100)} %` }));
    }

    let demoPicturesKept = 0;
    const keepDemoPicture = () => `picture-${(demoPicturesKept += 1)}.png`;

    // The demo's plugins folder. Choosing another only changes the path that is shown.
    const demoFolder = (path) => ({ path, found: true, plugins: plugins.length });
    let demoFolderPath = "/Users/you/MP Forge/plugins";
    const DEMO_FOLDER_PICKED = "/Users/you/Music/My MPC plugins";

    const answers = {
      library: () => ({ plugins: plugins.slice() }),
      plugins_folder: () => demoFolder(demoFolderPath),
      show_plugins_folder: () => null,
      choose_plugins_folder: () => ({ path: DEMO_FOLDER_PICKED, found: true, plugins: 0 }),
      keep_plugins_in: ({ folder, bringAlong }) => {
        demoFolderPath = folder;
        return { moved: bringAlong ? plugins.map((plugin) => plugin.name) : [], leftBehind: [], folder: demoFolder(folder) };
      },
      open_plugin: ({ name }) => {
        const screen = withoutAScreenYet.has(name) ? NO_SCREEN_YET : screenOf(name);
        const summary = plugins.find((plugin) => plugin.name === name) || { name, maker: "Demo", kind: "effect" };
        const arranged = { arrangement: arrangementOf(name), look: lookOf(name) };
        return { summary, screen, parameters: demoParameters(screen), versionsKept: 2, samples: samplesOf(name).slice(), pictures: DEMO_PICTURES[name] || [], builtAt: 1, ...arranged };
      },
      begin_editing: () => {},
      keep_look: ({ name, look }) => {
        looks[name] = JSON.parse(JSON.stringify(look));
        return { arrangement: arrangementOf(name), screen: screenOf(name) };
      },
      layer_image: ({ layer }) => demoDrawing(demoAreaOf(layer), [layer]),
      page_art_image: ({ name, page }) => demoDrawing(DEMO_PAGE, artworkOn(name, page)),
      art_size: ({ file }) => ({ file, width: demoArt.get(file).width, height: demoArt.get(file).height }),
      font_names: () => DEMO_FONTS,
      // The demo has no built artwork of its own. A folder of it can be put beside the page to see the real thing.
      skin_image: ({ file }) => fetch(`demo-skin/${file}`).then((reply) => (reply.ok ? reply.blob() : Promise.reject(new Error("no artwork")))),
      picture_image: ({ name, picture }) => demoPictureImage(DEMO_PICTURES[name].find((known) => known.name === picture).frames),
      remove_sample: ({ name, fileName }) => { samples[name] = samplesOf(name).filter((sample) => sample.fileName !== fileName); },
      start_plugin: ({ request }) => {
        plugins.push({ name: request.name, maker: request.maker, kind: request.kind });
        withoutAScreenYet.add(request.name);
        pretendToWork(request.name, request.description);
      },
      change_plugin: ({ name, change }) => pretendToWork(name, change.words),
      check_plugin: () => DEMO_REPORT,
      preview_plugin: () => { throw "The demo has no sound. Open the real app to hear a plugin."; },
      conversation: () => [],
      what_is_needed: () => DEMO_NEEDS.map((need) => ({ ...need, isThere: need.isThere || demoToolsFetched })),
      fetch_what_is_missing: () => new Promise((resolve) => setTimeout(() => { demoToolsFetched = true; resolve(); }, 1500)),
      keep_conversation: () => {},
      versions: () => demoVersions.slice(),
      forget_version: ({ number }) => {
        demoVersions.splice(demoVersions.findIndex((version) => version.number === number), 1);
        return demoVersions.slice();
      },
      fold_old_versions: ({ most }) => demoVersions.splice(0, Math.max(0, demoVersions.length - most)).length,
      choose_test_sound: () => ({ path: "/Users/demo/Music/My Drum Loop.wav", name: "My Drum Loop.wav" }),
      roll_the_dice: () => DEMO_ROLL,
      lucky_find: () => new Promise((resolve) => setTimeout(() => resolve(DEMO_FIND), 1500)),
      open_mpc_catalog: () => new Promise((resolve) => setTimeout(() => resolve(DEMO_CATALOG), 600)),
      remix_readme: () => new Promise((resolve) => setTimeout(() => resolve(DEMO_README), 400)),
      remix_screenshot: () => fetch("blacksmith.png").then((answer) => answer.arrayBuffer()),
      architect_questions: ({ idea }) => new Promise((resolve) => setTimeout(() => resolve(idea.answered.length ? [DEMO_LATER_QUESTION] : DEMO_QUESTIONS), 900)),
      go_to_version: ({ number }) => {
        if (!demoVersions.some((version) => version.isHowItIsNow)) demoVersions.push({ number: demoVersions.length + 1, keptAt: Date.now() / 1000, keptBefore: `going back to version ${number}`, isHowItIsNow: false });
        demoVersions.forEach((version) => { version.isHowItIsNow = version.number === number; });
        return true;
      },
      send_to_mpc: ({ mpc }) => {
        if (!/^[A-Za-z0-9][A-Za-z0-9.-]*$/.test(mpc.address)) throw `\`${mpc.address}\` is not an address an MPC can have.`;
        return { sent: true, report: DEMO_REPORT, drive: mpc.drive || "SD CARD", restarted: mpc.restart, restartProblem: null };
      },
      clipboard_words: () => navigator.clipboard.readText(),
      // The stand-in's Claude Code is ready. Keeping another word under "demoClaudeCode" shows how the menu reads then.
      this_copy: () => ({ version: "0.1.4", builtFrom: "the stand-in" }),
      claude_code_standing: () => window.helpers.recall("demoClaudeCode") || "ready",
      ai_accounts: () => [...DEMO_SERVICES.map((service) => ({ ...service, hasKey: demoKeys.has(service.word) })), demoSignInAccount()],
      keep_key: ({ account, key }) => {
        if (/\s/.test(key)) throw "That does not look like a key. A key is one unbroken run of letters, digits and marks, with no spaces.";
        demoKeys.add(account);
      },
      forget_key: ({ account }) => { demoKeys.delete(account); },
      // The demo signs in by itself after a moment, as if the musician had approved it in their browser.
      sign_in_with_browser: ({ account }) => new Promise((resolve) => setTimeout(() => { demoKeys.add(account); resolve(); }, 1200)),
      stop_signing_in: () => {},
      open_chatgpt_usage: () => {},
      mpc_password_is_kept: () => demoKeys.has("mpc"),
      models_offered: ({ provider }) => {
        if (!DEMO_MODELS[provider]) throw "The demo has no list for this service.";
        return DEMO_MODELS[provider];
      },
      // The demo's screens and pictures are found by name, so a renamed plugin takes its old ones along.
      rename_plugin: ({ name, newName }) => {
        if (plugins.some((plugin) => plugin.name === newName)) throw `There is already a plugin called \`${newName}\`. Pick another name.`;
        plugins.find((plugin) => plugin.name === name).name = newName;
        [DEMO_SCREENS, DEMO_PICTURES, looks, samples].forEach((known) => { known[newName] = known[name]; delete known[name]; });
      },
      delete_plugin: ({ name }) => { plugins.splice(plugins.findIndex((plugin) => plugin.name === name), 1); },
      export_plugin: ({ name }) => `/Users/you/Desktop/${name}.mpforge`,
      allow_online_plugins: () => {},
      open_mpc_listing_state: ({ name }) => ({ built: true, id: name.toLowerCase().replace(/[^a-z0-9]+/g, "-"), author: "Demo", version: "1.0.0", summary: guidesWritten.has(name) ? "A worn tape echo with wow, flutter and a tone control." : "", guideWritten: guidesWritten.has(name) }),
      list_on_open_mpc: ({ name, listing }) => {
        if (!/^[\w.-]+\/[\w.-]+$/.test(listing.repository)) return Promise.reject("The repository has to be written as owner/name, such as yourname/tape-echo.");
        return new Promise((resolve) => setTimeout(() => resolve({ ready: true, report: DEMO_REPORT, folder: `/Users/you/Desktop/${name} for Open MPC` }), 700));
      },
      // The demo keeps no guide's words until one has been made, and takes a moment to pretend the AI is writing.
      guide_state: ({ name }) => ({ built: true, written: guidesWritten.has(name) }),
      make_guide: ({ name, writeAgain }) => {
        const asked = writeAgain || !guidesWritten.has(name);
        guidesWritten.add(name);
        return new Promise((resolve) => setTimeout(() => resolve({ page: `/Users/you/MP Forge/${name}/guide.html`, asked }), asked ? 900 : 50));
      },
      choose_plugin_to_import: ({ asAFolder }) => (asAFolder ? `/Users/you/Downloads/${DEMO_IMPORT}` : `/Users/you/Downloads/${DEMO_IMPORT}.mpforge`),
      // The demo brings in a second Glass Pad, so the question asked when a name is taken can be tried.
      import_plugin: ({ named }) => {
        const name = named || DEMO_IMPORT;
        if (plugins.some((plugin) => plugin.name === name)) return { nameTaken: name };
        plugins.push({ ...plugins.find((plugin) => plugin.name === DEMO_IMPORT), name });
        [DEMO_SCREENS, DEMO_PICTURES, looks, samples].forEach((known) => { known[name] = known[DEMO_IMPORT]; });
        return { name };
      },
      stop_work: ({ name }) => stopPretendingToWork(name),
      attachment_image: () => { throw "The demo keeps no pictures."; },
      // The demo's player makes no sound, but it plays in silence, so the piano and the presets can be tried.
      player_start: ({ name, run }) => {
        demoRun = run;
        const instrument = plugins.some((plugin) => plugin.name === name && plugin.kind === "instrument");
        setTimeout(() => emit("player-event", { run, said: { ready: { controls: 3, instrument, presets: DEMO_PRESETS, preset: demoPreset } } }), 50);
      },
      player_tell: ({ request }) => {
        if (request.sound !== undefined) setTimeout(() => emit("player-event", { run: demoRun, said: { sound: request.sound } }), 30);
        if (request.note) demoNotes.push(request.note);
        if (request.volume !== undefined) demoListening.volume = request.volume;
        if (request.effect !== undefined) demoListening.effect = request.effect;
        if (request.feed !== undefined) demoListening.soundsFed.push(request.feed);
        if (request.feed !== undefined) setTimeout(() => emit("player-event", { run: demoRun, said: { fed: true } }), 30);
        if (request.preset === undefined) return;
        demoPreset = request.preset;
        setTimeout(() => emit("player-event", { run: demoRun, said: { preset: { chosen: demoPreset, values: demoPresetValues(demoPreset) } } }), 30);
      },
      player_stop: () => {},
    };

    return {
      isDemo: true,
      ask: (command, details) => new Promise((resolve, reject) => {
        try { resolve(answers[command](details || {})); } catch (problem) { reject(problem); }
      }),
      send: (command, bytes, labels) => {
        if (command === "add_artwork") return keepDemoArt(bytes);
        return Promise.resolve(command === "add_attachment" ? keepDemoPicture() : keepDemoSample(bytes, labels));
      },
      on: (eventName, handler) => { (handlers[eventName] = handlers[eventName] || []).push(handler); },
    };
  }

  window.backend = window.__TAURI__ ? realBackend(window.__TAURI__) : demoBackend();
})();
