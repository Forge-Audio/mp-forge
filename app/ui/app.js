// The workbench screen: which plugin is open, the conversation about it, and what the buttons do.

(function () {
  "use strict";

  const { ask, on } = window.backend;
  const { drawScreen, drawHardware, knobsOnPage, pageNamesOf, valueArrived, pluginMoved, controlsStandAt, turnedSettings } = window.pluginScreen;
  const knobChoosing = window.knobChoosing;
  const { withKnobGiven } = window.lookEdits;
  const blacksmith = window.blacksmith;
  const { askForNewName, askForAnotherName, askWhatToImport, askAboutTheGuide, askAboutListing, askBeforeDeleting, askBeforeSending } = window.pluginSheets;
  const versionTimeline = window.versionTimeline;
  const pluginArchitect = window.pluginArchitect;
  const appSettings = window.appSettings;
  const appMenu = window.appMenu;
  const aiAccounts = window.aiAccounts;
  const livePlayer = window.livePlayer;
  const samplePlayer = window.samplePlayer;
  const sampleSend = window.sampleSend;
  const lookEditor = window.lookEditor;
  const { element, remember, recall } = window.helpers;
  const { addressOf } = window.fetchedImages;
  const aiMenu = window.aiMenu;
  const sampleShelf = window.sampleShelf;
  const pictureTray = window.pictureTray;
  const offeredOptions = window.offeredOptions;
  const timeTaken = window.timeTaken;
  const tabStrip = window.tabStrip;
  const remixPicker = window.remixPicker;
  const guide = window.guide;
  const guideExample = window.guideExample;
  const testSound = window.testSound;
  const effectSwitch = window.effectSwitch;
  const outputVolume = window.outputVolume;

  const NEW_PLUGIN = null;
  const STATUS = { noPlugin: "NO PLUGIN YET", unchecked: "NOT CHECKED YET", working: "WORKING", building: "BUILDING", checking: "CHECKING", sending: "SENDING", listing: "GETTING READY", stopping: "STOPPING", safe: "SAFE TO SEND", blocked: "BLOCKED" };
  // Whatever the musician did not say themselves is said by MP Forge, whichever AI did the work.
  const SPEAKER = { you: "You", mpForge: "MP Forge" };
  const WAITS_ITS_TURN = `${SPEAKER.you} · waiting its turn`;
  const NO_PICTURES = { names: [], addresses: [], handed: [] };
  const WORKING_WORDS = { [STATUS.checking]: "Its safety check is running.", [STATUS.sending]: "It is being sent to your MPC.", [STATUS.listing]: "It is being got ready for Open MPC." };
  const WIDEN_THE_CONVERSATION = "Give the conversation the whole window.";
  const SHOW_THE_PLUGIN = "Bring the plugin's view back beside the conversation.";
  const ASK_FOR_A_PLUGIN = "Describe your plugin";
  const ASK_FOR_AN_APP = "Describe your app for the MPC Sample";
  const KIND_OF_A_SAMPLE_APP = "sample-app";
  const MADE_FOR_THE_SAMPLE = "sample";
  const NAME_FOR_EXAMPLE = { plugin: "Dusty Tape", app: "Night Bus" };
  const ASK_FOR_A_CHANGE = "Ask for a change";
  const ASK_WHAT_IS_DIFFERENT = "Say what should be different in your version";
  const OPENING_QUESTION = "What do you want to make?";
  const SCOUT_IS_LOOKING = "Looking for something new and interesting…";
  const LUCKY_NEEDS_A_MAKER = "Say who the maker is first. It is shown on the MPC and cannot be changed later.";
  const LUCKY_WITHOUT_THE_WEB = "Web lookups are off for this model, so the find comes from what it already knows. Switch them on in the model menu for fresh finds.";
  const LONGEST_PLUGIN_NAME = 32;
  const FINISHED_WITHOUT_WORDS = "Done.";
  const ARCHITECT_IS_THINKING = "The architect is thinking of questions…";
  const ARCHITECTS = { plugin: "Plugin architect", os: "OS architect" };
  // Each plugin being built has an AI, a compiler and a safety check of its own at work on this computer.
  const MOST_BUILT_AT_ONCE = 5;
  const AI_HAS_NO_ROOM = "The AI is already building five plugins. Build this one once one of them has finished.";
  const ABOUT_THE_CONTEXT = "How much the AI is holding in mind for this request, in tokens: your words, the files it has read and the pictures it was shown. It starts again with every request. Settings has the limit at which older material is dropped to make room.";
  const TOKENS_IN_A_THOUSAND = 1000;
  const A_GUIDE_NEEDS_A_BUILD = "A guide can be made once the plugin has been built. Run its safety check first.";
  const AI_IS_WRITING_THE_GUIDE = "Asking the AI to write the guide. This takes a minute or so, and the guide opens in your browser when it is done.";
  const ABOUT_A_MADE_GUIDE = "Print it or save it as a PDF from there. Its words are kept with the plugin and go along when you export it.";
  const A_LISTING_NEEDS_A_BUILD = "Only a built plugin can be listed. Run its safety check first.";
  const ABOUT_A_LISTING_MADE = "Nothing has been sent anywhere. Open START HERE in that folder: it has the three steps to do on GitHub.";
  const GOES_ONLINE_FOR = "This plugin goes online. On your MPC and when you play it here, it fetches sound from:";
  const WHAT_AN_EXPORT_HOLDS = "The file holds the plugin's code, its screen and look, its pictures and its sounds. Your conversation with the AI and the plugin's earlier versions are not in it. Whoever you give it to brings it in with IMPORT A PLUGIN.";
  const MARK_OF_A_SAMPLE_APP = "on-the-sample";
  const KINDS_IN_WORDS = { effect: "Effect", instrument: "Instrument", "sample-app": "MPC Sample app" };
  const IMPORTED_AND_NOT_RUN = "**Brought in.** Nothing of it has been run yet. Run the safety check to build it, then you can see its screen, play it and change it.";
  const NO_SUCH_VERSION = "That version could not be found, so nothing was changed.";
  const NOT_SENT_UNTIL_SAFE = "A plugin has to pass its safety check before it can be sent to your MPC. Press the status at the top to run the check.";
  const NOT_PUT_ON_THE_SAMPLE_UNTIL_SAFE = "An app has to pass its safety check before it can go onto your MPC Sample. Press the status at the top to run the check.";
  const WHERE_AN_OPEN_PLUGIN_GOES = { plugin: "Send to your MPC", app: "Put on your MPC Sample" };
  const SHOWS_AFTER_A_RESTART = "It shows in the MPC's plugin list once the MPC's app has next been restarted.";
  // The steps of the AI's work that change what the plugin's screen shows, by how their words begin.
  const STEPS_THAT_CHANGE_THE_SCREEN = ["writing ", "styling the screen"];
  const WRITING_THE_SCREEN = "writing ui.json";
  const STEP_THAT_BUILDS = "running the safety check";
  const SAFETY_CHECK_OFFERED = "It builds the plugin and plays it through the simulator, which takes a little while. Run it now?";
  const BEAT_WORDS = [
    "Chopping the sample", "Digging in the crates", "Laying down the drums", "Finding the pocket", "Swinging the hi-hats",
    "Tuning the kick", "Looping the break", "Layering the snare", "Flipping the sample", "Riding the fader",
    "Nudging the swing", "Tapping the pads", "Filtering the bass", "Counting it in", "Dusting off the vinyl", "Bouncing the mix",
  ];
  const BEAT_WORDS_CHANGE_EVERY_MS = 3000;
  const NOT_PLAYED_UNTIL_SAFE = "This plugin did not pass its safety check, so it is not played. Ask for a fix first.";
  const PLAYER_ENDED = "The live player stopped on its own. The plugin most likely crashed it. Run the safety check to find out why.";
  // The keys A to K play one octave from middle C, laid out like a piano: the row above holds the black notes.

  const page = {
    tabs: document.getElementById("plugin-tabs"),
    newPluginTab: document.getElementById("new-plugin-tab"),
    aiMenuButton: document.getElementById("ai-menu-button"),
    aiMenu: document.getElementById("ai-menu"),
    conversation: document.getElementById("conversation"),
    appMenu: document.getElementById("app-menu"),
    sendToMpc: document.getElementById("send-to-mpc"),
    messages: document.getElementById("messages"),
    goBack: document.getElementById("go-back"),
    widenChat: document.getElementById("widen-chat"),
    contextReadout: document.getElementById("context-readout"),
    planNote: document.getElementById("plan-note"),
    manageUsage: document.getElementById("manage-usage"),
    architect: document.getElementById("architect"),
    starters: document.getElementById("starters"),
    feelLucky: document.getElementById("feel-lucky"),
    remix: document.getElementById("remix"),
    importPlugin: document.getElementById("import-plugin"),
    remixOf: document.getElementById("remix-of"),
    remixOfWords: document.getElementById("remix-of-words"),
    remixOfClear: document.getElementById("remix-of-clear"),
    showGuide: document.getElementById("show-guide"),
    presets: document.getElementById("presets"),
    presetList: document.getElementById("preset-list"),
    presetMenu: document.getElementById("preset-menu"),
    presetBefore: document.getElementById("preset-before"),
    presetAfter: document.getElementById("preset-after"),
    editLook: document.getElementById("edit-look"),
    status: document.getElementById("status"),
    statusButton: document.getElementById("status-button"),
    statusMenu: document.getElementById("status-menu"),
    settingsButton: document.getElementById("settings-button"),
    settings: document.getElementById("settings"),
    settingsDone: document.getElementById("settings-done"),
    moreButton: document.getElementById("more-button"),
    moreMenu: document.getElementById("more-menu"),
    guidePlugin: document.getElementById("guide-plugin"),
    exportPlugin: document.getElementById("export-plugin"),
    listPlugin: document.getElementById("list-plugin"),
    renamePlugin: document.getElementById("rename-plugin"),
    deletePlugin: document.getElementById("delete-plugin"),
    appSheet: document.getElementById("app-sheet"),
    composer: document.getElementById("composer"),
    newPluginFields: document.getElementById("new-plugin-fields"),
    pluginName: document.getElementById("plugin-name"),
    makerName: document.getElementById("maker-name"),
    message: document.getElementById("message"),
    send: document.getElementById("send"),
    addButton: document.getElementById("add-button"),
    addMenu: document.getElementById("add-menu"),
    addSamples: document.getElementById("add-samples"),
    samplePicker: document.getElementById("sample-picker"),
    sampleList: document.getElementById("sample-list"),
    addPictures: document.getElementById("add-pictures"),
    picturePicker: document.getElementById("picture-picker"),
    pictureList: document.getElementById("picture-list"),
    title: document.getElementById("plugin-title"),
    subtitle: document.getElementById("plugin-subtitle"),
    screen: document.getElementById("mpc-screen"),
    sampleDevice: document.getElementById("sample-device"),
    sampleProblem: document.getElementById("sample-problem"),
    sampleBeingWritten: document.getElementById("sample-being-written"),
    hardwareGrid: document.getElementById("hardware-grid"),
    hardwareNote: document.getElementById("hardware-note"),
    editKnobs: document.getElementById("edit-knobs"),
    checkSummary: document.getElementById("check-summary"),
    check: document.getElementById("check"),
    play: document.getElementById("play"),
    piano: document.getElementById("piano"),
  };

  const working = workingBlock();
  const smith = blacksmith.standIn(document.getElementById("smith"));
  const chatEnd = window.endFollower(page.messages);
  const presetList = window.pickList(page.presetList, page.presetMenu, choosePreset);
  const piano = window.pixelPiano.standIn(page.piano, (pitch, pressed) => livePlayer.note(pitch, pressed));

  // Pressing the status opens the offer to run the safety check.
  const statusMenu = window.popupMenu(page.statusButton, page.statusMenu);

  const state = {
    plugins: [],
    openName: NEW_PLUGIN,
    openPlugin: null,
    screenPage: 0,
    // The AI's work in progress, by the name of the plugin it is on. Each job holds its status in the status's
    // own words, whether it can still be stopped, the moment it began, the steps taken so far, and whether it is
    // a new plugin whose own screen the AI has not written yet.
    aiJobs: new Map(),
    // The plugins being checked or sent to the MPC, each with the status word for which. These run side by side,
    // and beside the AI's work on other plugins.
    checking: new Map(),
    // What was asked for while the AI was at work on a plugin, by the plugin's name, the oldest first. Each
    // waits its turn with the model the menu showed as it was typed.
    waitingToBeSent: new Map(),
    beatWords: 0,
    beatWordsTimer: undefined,
    conversations: new Map(),
    reports: new Map(),
    // Plugins whose look was edited by hand since their last safety check.
    editedSinceCheck: new Set(),
    // Whether the conversation has been given the whole window, with the plugin's view put away.
    chatIsWide: false,
    // The idea the plugin architect is asking about, before a new plugin is built: the request as first typed,
    // what has been answered so far, the card of questions on screen, and whether more are being thought of.
    architect: null,
    // Whether the scout is out looking for a find, after the dice were rolled for "I'm feeling lucky".
    scouting: false,
    // The catalog plugin whose source is on its way, while one is being taken in as it is.
    fetchingSourceOf: null,
    // The plugin of the Open MPC catalog the new plugin is to be a remix of, when one was picked.
    remixOf: null,
    remixMaterial: null,
    // The name of the made-up plugin the guide is showing, while it shows one.
    guidesExample: undefined,
    // How many tokens the AI's context held when last heard of, for each plugin it has worked on.
    contextOf: new Map(),
    // The preset last chosen in the preview, with the plugin and the build it was chosen for.
    presetChosen: null,
  };

  // --- Small helpers -------------------------------------------------------------------------

  // The pads, the beat-making words and the AI's steps. It is the last thing in the conversation while the AI works.
  function workingBlock() {
    const block = document.getElementById("working-template").content.firstElementChild.cloneNode(true);
    const part = (name) => block.querySelector(`.working-${name}`);
    return { block, words: part("words"), fold: part("fold"), count: part("count"), latest: part("latest"), steps: part("steps") };
  }

  // The AI works on a few plugins at once. With that many in hand, one more has to wait for one of them.
  function aiHasNoRoom() {
    return state.aiJobs.size >= MOST_BUILT_AT_ONCE;
  }

  function aiJobOn(pluginName) {
    return state.aiJobs.get(pluginName);
  }

  // A plugin is out of reach while the AI works on it or its safety check runs. Any other plugin can still be
  // built, changed, edited by hand, played and checked.
  function isBeingWorkedOn(pluginName) {
    return state.aiJobs.has(pluginName) || state.checking.has(pluginName);
  }

  function architectIsThinking() {
    return Boolean(state.architect && state.architect.thinking);
  }

  // The scout is out looking for a plugin to build, or a catalog plugin's source is on its way.
  function isWaitingOnTheWeb() {
    return state.scouting || Boolean(state.fetchingSourceOf);
  }

  function somethingIsBeingWorkedOn() {
    return state.aiJobs.size > 0 || state.checking.size > 0 || architectIsThinking() || isWaitingOnTheWeb();
  }

  // The AI's work can be stopped from the conversation of the plugin it is working on.
  function canBeStoppedHere() {
    const job = aiJobOn(state.openName);
    return Boolean(job && job.canBeStopped);
  }

  function describeProblem(problem) {
    return typeof problem === "string" ? problem : (problem && problem.message) || "Something went wrong.";
  }

  // --- Conversations --------------------------------------------------------------------------

  function conversationKey(pluginName) {
    return pluginName === NEW_PLUGIN ? "" : pluginName;
  }

  // A plugin that is still only an idea has no folder, so nothing was kept of it.
  function wordsKeptOf(pluginName) {
    return pluginName === NEW_PLUGIN ? Promise.resolve([]) : ask("conversation", { name: pluginName }).catch(() => []);
  }

  // A conversation is its messages and `readBack`, which settles once what was kept before the app was last closed
  // stands in front of them.
  function conversationOf(messages, kept) {
    return { messages, readBack: kept.then((earlier) => { messages.unshift(...earlier); }) };
  }

  function conversationOnlyOf(messages) {
    return conversationOf(messages, Promise.resolve([]));
  }

  // The first that is heard of a plugin begins the reading back of what was kept. Words said meanwhile come after
  // what was kept, and never take its place.
  function conversationWith(pluginName) {
    const key = conversationKey(pluginName);
    if (!state.conversations.has(key)) state.conversations.set(key, conversationOf([], wordsKeptOf(pluginName)));
    return state.conversations.get(key);
  }

  function conversationAbout(pluginName) {
    return conversationWith(pluginName).messages;
  }

  // What is kept of a message: its words, the names of its pictures, and the steps the AI took. A picture's
  // address only lasts while the app is open, so it is left out.
  function asKept(message) {
    return { kind: message.kind, text: message.text, pictureFiles: message.pictureFiles || [], steps: message.steps || [], tookSeconds: message.tookSeconds };
  }

  // Nothing is kept until what was kept before has been read back, so the two are never kept one without the other.
  // A plugin that is still being made has no folder to keep its conversation in yet, so that attempt is let go.
  function keepConversation(pluginName) {
    if (pluginName === NEW_PLUGIN) return;
    const { messages, readBack } = conversationWith(pluginName);
    readBack.then(() => ask("keep_conversation", { name: pluginName, messages: messages.map(asKept) })).catch(() => {});
  }

  // The musician's own words always bring the conversation back to its end.
  function addMessage(pluginName, message) {
    conversationAbout(pluginName).push(message);
    keepConversation(pluginName);
    if (message.kind === "you") chatEnd.followAgain();
    if (pluginName === state.openName) drawMessages();
  }

  // Shows **bold** the way Claude writes it, without ever treating its words as markup.
  function appendWithBold(parent, text) {
    String(text).split(/\*\*(.+?)\*\*/g).forEach((piece, index) => {
      parent.append(index % 2 ? element("strong", "", piece) : document.createTextNode(piece));
    });
  }

  // A picture sent while the app has been open still has its address. One from an earlier day is fetched by name.
  function pictureAddresses(message) {
    if (message.pictures && message.pictures.length) return message.pictures.map((address) => Promise.resolve(address));
    return (message.pictureFiles || []).map((file) => addressOf("attachment_image", { name: state.openName, file }, 0));
  }

  function picturesSent(addresses) {
    const row = element("div", "message-pictures");
    addresses.forEach((address, index) => {
      const image = element("img", "message-picture");
      image.alt = `Picture ${index + 1} sent with this message`;
      image.addEventListener("load", chatEnd.glide);
      address.then((url) => { image.src = url; }, () => image.remove());
      row.append(image);
    });
    return row;
  }

  // Only the AI offers choices. Anyone else's words are shown exactly as written.
  function wordsAndOptionsOf(message) {
    return message.kind === "said" ? offeredOptions.readFrom(message.text) : { words: message.text, options: [] };
  }

  function wordsOfStep(step) {
    return offeredOptions.readFrom(step.text).words;
  }

  function stepLine(tag, step) {
    return element(tag, step.kind === "said" ? "step said" : "step", wordsOfStep(step));
  }

  function countOfSteps(steps) {
    return `${steps.length} step${steps.length === 1 ? "" : "s"}`;
  }

  // The steps the AI took, folded away under its closing words for anyone who wants to look.
  function stepsTaken(steps) {
    const folded = element("details", "steps");
    const list = element("ol", "steps-list");
    list.append(...steps.map((step) => stepLine("li", step)));
    folded.append(element("summary", "", `What it did (${countOfSteps(steps)})`), list);
    return folded;
  }

  function optionButton(label) {
    const button = element("button", "option-button", label);
    button.type = "button";
    button.addEventListener("click", () => changeOpenPlugin(label));
    return button;
  }

  // Each choice the AI offered is a button. Pressing one sends it as the musician's next message.
  function optionsToPick(labels) {
    const row = element("div", "options");
    row.setAttribute("role", "group");
    row.setAttribute("aria-label", "Pick one");
    row.append(...labels.map(optionButton));
    return row;
  }

  function speakerOf(message) {
    return message.kind === "you" ? SPEAKER.you : SPEAKER.mpForge;
  }

  // Only a message that ends a piece of work says how long that work took.
  function wasTimed(message) {
    return message.tookSeconds !== undefined;
  }

  function howLongItTook(message) {
    return element("div", "message-took", `Took ${timeTaken.inWords(message.tookSeconds)}`);
  }

  function spokenMessage(message, canBeAnswered) {
    const box = element("div", `message ${message.kind}`);
    const text = element("div", "message-text");
    const { words, options } = wordsAndOptionsOf(message);
    const pictures = pictureAddresses(message);
    appendWithBold(text, words);
    box.append(element("div", "field-label", speakerOf(message)), text);
    if (pictures.length) box.append(picturesSent(pictures));
    if (canBeAnswered && options.length) box.append(optionsToPick(options));
    if (message.steps && message.steps.length) box.append(stepsTaken(message.steps));
    if (wasTimed(message)) box.append(howLongItTook(message));
    return box;
  }

  function pressable(words, whenPressed) {
    const button = element("button", "item-button", words);
    button.type = "button";
    button.addEventListener("click", whenPressed);
    return button;
  }

  // A request that waits its turn is shown under the work it waits for, with the two things that can be done
  // about it meanwhile.
  function waitingMessage(waited) {
    const pluginName = state.openName;
    const box = element("div", "message you waiting");
    const answers = element("div", "waiting-answers");
    answers.append(pressable("SEND NOW", () => sendNow(pluginName, waited)), pressable("REMOVE", () => stopWaiting(pluginName, waited)));
    box.append(element("div", "field-label", WAITS_ITS_TURN), element("div", "message-text", waited.words), answers);
    return box;
  }

  // Choices can be picked only from the newest message, and only while the AI has room for what is picked.
  function drawMessages() {
    const conversation = conversationAbout(state.openName);
    const newest = conversation.length - 1;
    const drawn = conversation.map((message, index) => spokenMessage(message, index === newest && !aiHasNoRoom()));
    if (!drawn.length || nothingSaidYet()) drawn.unshift(...openingLines());
    if (state.openName === NEW_PLUGIN) drawn.push(...whatIsBeingThoughtOver());
    if (isBeingWorkedOn(state.openName)) drawn.push(working.block);
    drawn.push(...waitingFor(state.openName).map(waitingMessage));
    page.messages.replaceChildren(...drawn);
    document.body.classList.toggle("nothing-said-yet", nothingSaidYet());
    chatEnd.glide();
  }

  // The steps are the AI's. A plugin that is only having its safety check run has none to show. They are kept
  // folded away, with only the newest showing beside how many there have been, until the musician opens them.
  function drawSteps() {
    const job = aiJobOn(state.openName);
    const steps = job ? job.steps : [];
    working.fold.hidden = steps.length === 0;
    working.count.textContent = countOfSteps(steps);
    working.latest.textContent = steps.length ? wordsOfStep(steps[steps.length - 1]) : "";
    working.steps.replaceChildren(...steps.map((step) => stepLine("div", step)));
    chatEnd.glide();
  }

  // A new plugin opens on a question, with its form in the middle of the window. The form moves down to make
  // room for the conversation as soon as there is one.
  function nothingSaidYet() {
    const saysSomething = (message) => message.kind === "you" || message.kind === "note";
    return state.openName === NEW_PLUGIN && !state.architect && !isWaitingOnTheWeb() && !conversationAbout(NEW_PLUGIN).some(saysSomething);
  }

  function openingLines() {
    if (state.openName !== NEW_PLUGIN) return [element("div", "soft", "Say what you would like changed. The current version is kept, so nothing is lost.")];
    return [element("div", "opening-question", OPENING_QUESTION), element("div", "soft", "Give the plugin a name, say who made it, then describe the sound or tool you want.")];
  }

  // --- The open plugin ------------------------------------------------------------------------

  // A tab shows a pulsing mark while its plugin is being built or checked, whichever plugin is open.
  function markAsBeingWorkedOn(tab, pluginName) {
    tab.classList.add("working");
    tab.title = WORKING_WORDS[state.checking.get(pluginName)] || "The AI is working on it.";
  }

  function openFromItsTab(pluginName) {
    if (!lookEditor.isOpen()) open(pluginName);
  }

  function showAsOpen(tab, isOpen) {
    if (isOpen) tab.setAttribute("aria-current", "page"); else tab.removeAttribute("aria-current");
  }

  function tabOf(plugin) {
    const tab = element("button", "tab", plugin.name);
    tab.type = "button";
    tab.dataset.plugin = plugin.name;
    showAsOpen(tab, plugin.name === state.openName);
    if (isBeingWorkedOn(plugin.name)) markAsBeingWorkedOn(tab, plugin.name);
    tab.addEventListener("click", () => openFromItsTab(plugin.name));
    return tab;
  }

  // The plugins' tabs, in the order the musician put them in. The tab for a new plugin stands after them and never
  // scrolls out of sight. While a tab is being dragged to another place the tabs are left as they are.
  function drawTabs() {
    if (tabStrip.isBeingRearranged()) return;
    page.tabs.replaceChildren(...tabStrip.inOrder(state.plugins).map(tabOf));
    showAsOpen(page.newPluginTab, state.openName === NEW_PLUGIN);
    tabStrip.settle(state.openName);
  }

  function summaryLine(plugin) {
    const kind = KINDS_IN_WORDS[plugin.summary.kind] || KINDS_IN_WORDS.effect;
    const kept = plugin.versionsKept ? ` · ${plugin.versionsKept} earlier version${plugin.versionsKept === 1 ? "" : "s"} kept` : "";
    return `${kind} by ${plugin.summary.maker}${kept}${streamsOf(plugin).length ? " · ONLINE" : ""}`;
  }

  // The addresses the plugin brings sound in from over the internet. Most plugins have none.
  function streamsOf(plugin) {
    return (plugin && plugin.streams) || [];
  }

  // What a plugin that goes online fetches, said in full for whoever rests the pointer on its summary.
  function whereItGoesOnline(plugin) {
    const addresses = streamsOf(plugin).map((stream) => `${stream.name}: ${stream.address}`);
    return addresses.length ? `${GOES_ONLINE_FOR} ${addresses.join(", ")}` : "";
  }

  function whereItWasBlocked(report) {
    const blocked = report.checks.find((check) => check.outcome === "BLOCK");
    return blocked ? blocked.name : "an early step";
  }

  function checkSummary() {
    if (state.openName === NEW_PLUGIN) return "No plugin open.";
    const report = state.reports.get(state.openName);
    if (state.editedSinceCheck.has(state.openName)) return "Safety check: not run since the look was edited.";
    if (!report) return "Safety check: not run yet.";
    return report.safe ? `Safety check: passed all ${report.checks.length} checks.` : `Safety check: blocked at "${whereItWasBlocked(report)}".`;
  }

  // A new plugin starts from a stock layout that is not its own, so that is never shown.
  function isAwaitingItsScreen(plugin) {
    const job = aiJobOn(state.openName);
    return Boolean(plugin && job && job.awaitsItsScreen);
  }

  function drawEmptyScreen() {
    page.screen.style.background = "";
    page.screen.replaceChildren(element("div", "mpc-stage"));
  }

  function drawMpcScreen(plugin) {
    if (isAwaitingItsScreen(plugin)) return drawEmptyScreen();
    const onControlPicked = isGivingAKnobAway() ? giveChosenKnobTo : undefined;
    drawScreen(page.screen, plugin, state.screenPage, { onPageChosen: showScreenPage, onTurned: controlTurned, onControlPicked });
  }

  // Something new for the MPC Sample is shown on the drawing of the device before it exists, as a Sample app is.
  function isShownOnTheSample(plugin) {
    return plugin ? samplePlayer.isASampleApp(plugin) : isNewForTheSample();
  }

  // An app runs on the drawing except while it is worked on, when its code is changing under it. The
  // blacksmith is then shown on the device's screen in its place, on the drawing and on a real MPC Sample.
  function showOnTheSample(plugin) {
    const isOutOfReach = isBeingWorkedOn(plugin.summary.name);
    samplePlayer.drawnIn(page.sampleDevice);
    samplePlayer.follow(plugin, page.sampleDevice, isOutOfReach);
    page.sampleBeingWritten.hidden = !isOutOfReach;
  }

  function drawMirror() {
    const plugin = state.openPlugin;
    const runsOnTheSample = isShownOnTheSample(plugin);
    page.title.textContent = plugin ? plugin.summary.name : isNewForTheSample() ? "New app" : "New plugin";
    page.subtitle.textContent = plugin ? summaryLine(plugin) : "";
    page.subtitle.title = whereItGoesOnline(plugin);
    showTheDeviceItRunsOn(runsOnTheSample);
    if (plugin && runsOnTheSample) showOnTheSample(plugin);
    else samplePlayer.follow(null, page.sampleDevice);
    livePlayer.follow(!runsOnTheSample && plugin && plugin.builtAt ? plugin : null);
    if (!runsOnTheSample) drawAsOnAnMpc(plugin);
    drawStatus();
  }

  // A Sample app is shown on the drawing of the MPC Sample. Anything else is shown as an MPC's touch screen,
  // with its presets and the hardware knobs. The styles show the one or the other by this mark on the page.
  function showTheDeviceItRunsOn(runsOnTheSample) {
    document.body.classList.toggle(MARK_OF_A_SAMPLE_APP, runsOnTheSample);
  }

  function showWhyTheSampleAppIsNotRunning(problem) {
    page.sampleProblem.textContent = problem;
    page.sampleProblem.hidden = !problem;
  }

  function drawAsOnAnMpc(plugin) {
    drawMpcScreen(plugin);
    drawHardwareKnobs();
    readPresets();
  }

  function drawHardwareKnobs() {
    const choosing = isChoosingKnobs() ? { chosenKnob: knobChoosing.chosenKnob(), onKnobChosen: knobPressed } : null;
    drawHardware(page.hardwareGrid, page.hardwareNote, state.openPlugin, state.screenPage, choosing);
    page.editKnobs.setAttribute("aria-pressed", String(isChoosingKnobs()));
    if (choosing) page.hardwareNote.textContent = knobChoosing.whatToDoNext();
  }

  function controlsOfOpenPlugin() {
    const plugin = state.openPlugin;
    return (plugin && plugin.parameters && plugin.parameters.params) || [];
  }

  // A turned control is passed to the live player, which answers with what the control now reads.
  function tellPlayerOfTurn(name, amount) {
    const control = controlsOfOpenPlugin().find((known) => known.name.toLowerCase() === name.toLowerCase());
    if (control && state.openPlugin.builtAt) livePlayer.turn(state.openPlugin, control.index, amount);
  }

  function controlTurned(name, amount) {
    drawHardwareKnobs();
    tellPlayerOfTurn(name, amount);
  }

  function showScreenPage(index) {
    state.screenPage = index;
    drawMirror();
  }

  function canTakeNoRequest() {
    const startingNew = state.openName === NEW_PLUGIN;
    return aiHasNoRoom() || isBeingWorkedOn(state.openName) || (startingNew && (architectIsThinking() || isWaitingOnTheWeb()));
  }

  // While the AI works on the open plugin, what is typed waits its turn. With nothing typed, the button stops the work.
  function typedWaitsItsTurn() {
    return state.aiJobs.has(state.openName) && page.message.value.trim() !== "";
  }

  function wordsOnTheRequestButton() {
    if (typedWaitsItsTurn()) return "QUEUE IT";
    if (canBeStoppedHere()) return "STOP";
    return state.openName === NEW_PLUGIN ? "BUILD IT" : "DO IT";
  }

  function drawRequestButton() {
    page.send.textContent = wordsOnTheRequestButton();
    page.send.disabled = canTakeNoRequest() && !canBeStoppedHere() && !typedWaitsItsTurn();
  }

  // A plugin being worked on cannot be played, and the blacksmith is at work for as long as that lasts.
  // A sound that is playing can always be stopped.
  function playCannotBePressed() {
    return state.openName === NEW_PLUGIN || (isBeingWorkedOn(state.openName) && !livePlayer.isSounding());
  }

  function drawComposer() {
    const startingNew = state.openName === NEW_PLUGIN;
    const askedOfSomethingNew = isNewForTheSample() ? ASK_FOR_AN_APP : state.remixOf ? ASK_WHAT_IS_DIFFERENT : ASK_FOR_A_PLUGIN;
    const asked = startingNew ? askedOfSomethingNew : ASK_FOR_A_CHANGE;
    page.newPluginFields.hidden = !startingNew;
    page.pluginName.placeholder = isNewForTheSample() ? NAME_FOR_EXAMPLE.app : NAME_FOR_EXAMPLE.plugin;
    page.message.placeholder = asked;
    page.message.setAttribute("aria-label", asked);
    const cannotTakeARequest = canTakeNoRequest();
    page.addButton.disabled = cannotTakeARequest;
    page.starters.hidden = !startingNew || Boolean(state.architect);
    page.feelLucky.disabled = cannotTakeARequest;
    page.remix.disabled = cannotTakeARequest;
    drawRemixNote(startingNew);
    drawRequestButton();
    page.goBack.hidden = startingNew;
    page.goBack.disabled = isBeingWorkedOn(state.openName) || !(state.openPlugin && state.openPlugin.versionsKept);
    page.editLook.disabled = isBeingWorkedOn(state.openName) || !hasPagesToArrange(state.openPlugin);
    page.editKnobs.disabled = !knobsCanBeChosen();
    page.play.disabled = playCannotBePressed();
    drawSendButton();
    page.moreButton.disabled = isBeingWorkedOn(state.openName) || startingNew;
    sampleShelf.setBusy(cannotTakeARequest);
    pictureTray.setBusy(cannotTakeARequest);
  }

  // Above the message box, while a new plugin is to be a remix: which plugin it is a remix of.
  function drawRemixNote(startingNew) {
    const original = state.remixOf;
    page.remixOf.hidden = !startingNew || !original;
    page.remixOfWords.textContent = original ? `Remixing ${original.name} by ${original.author}` : "";
  }

  // The send button is held until the open plugin has passed its safety check. Pressing it before then says why.
  function canBeSent() {
    return statusOf(state.openName) === STATUS.safe;
  }

  function whereTheOpenPluginGoes() {
    return samplePlayer.isASampleApp(state.openPlugin) ? WHERE_AN_OPEN_PLUGIN_GOES.app : WHERE_AN_OPEN_PLUGIN_GOES.plugin;
  }

  function drawSendButton() {
    const words = canBeSent() ? whereTheOpenPluginGoes() : `${whereTheOpenPluginGoes()}. It has to pass its safety check first.`;
    page.sendToMpc.disabled = state.openName === NEW_PLUGIN || isBeingWorkedOn(state.openName);
    page.sendToMpc.classList.toggle("is-held", !canBeSent());
    page.sendToMpc.title = words;
    page.sendToMpc.setAttribute("aria-label", words);
  }

  function hasPagesToArrange(plugin) {
    return Boolean(plugin && plugin.screen && plugin.screen.pages && plugin.screen.pages.length);
  }

  // --- Giving the hardware knobs other controls ------------------------------------------------

  // The knobs are the musician's to give out while nothing else is changing the plugin.
  function knobsCanBeChosen() {
    return !isBeingWorkedOn(state.openName) && hasPagesToArrange(state.openPlugin);
  }

  function isChoosingKnobs() {
    return knobChoosing.isOn() && knobsCanBeChosen();
  }

  // A knob has been picked, and the next control pressed on the screen is the one it will turn.
  function isGivingAKnobAway() {
    return isChoosingKnobs() && knobChoosing.chosenKnob() !== null;
  }

  function drawKnobChoosing() {
    drawMpcScreen(state.openPlugin);
    drawHardwareKnobs();
  }

  function toggleKnobChoosing() {
    if (knobChoosing.isOn()) knobChoosing.end();
    else knobChoosing.begin();
    drawKnobChoosing();
  }

  function knobPressed(number) {
    knobChoosing.knobPressed(number);
    drawKnobChoosing();
  }

  async function keepAVersionBeforeTheFirstKnob(pluginName) {
    if (!knobChoosing.stillNeedsAVersionKept()) return;
    await ask("begin_editing", { name: pluginName });
    knobChoosing.versionWasKept();
  }

  // The last safety check was of the plugin before this knob was given, so it no longer counts.
  function knobWasGiven(pluginName, look, kept) {
    knobChoosing.knobWasGiven();
    state.reports.delete(pluginName);
    state.editedSinceCheck.add(pluginName);
    if (state.openName === pluginName) state.openPlugin = { ...state.openPlugin, look, arrangement: kept.arrangement, screen: kept.screen };
  }

  // The control takes the chosen knob, and whatever that knob turned before takes the one the control gives up.
  async function giveChosenKnobTo(controlName) {
    const pluginName = state.openName;
    const plugin = state.openPlugin;
    const pageName = pageNamesOf(plugin)[state.screenPage];
    const look = withKnobGiven(plugin.look || { pages: [] }, pageName, knobsOnPage(plugin, state.screenPage), controlName, knobChoosing.chosenKnob());
    try {
      await keepAVersionBeforeTheFirstKnob(pluginName);
      knobWasGiven(pluginName, look, await ask("keep_look", { name: pluginName, look }));
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
    drawEverything();
  }

  function drawSamples() {
    sampleShelf.show(state.openName, state.openPlugin && state.openPlugin.samples);
  }

  // After a sound file is added or removed, the open plugin is read again so the list is what the app really holds.
  // Reading takes a moment, and what was read is shown only if the musician has not opened another meanwhile.
  async function reloadOpenPlugin() {
    const pluginName = state.openName;
    if (pluginName === NEW_PLUGIN) return;
    const read = await loadPlugin(pluginName);
    if (state.openName !== pluginName) return;
    state.openPlugin = read;
    drawSamples();
  }

  // A Sample app is handed its sound files when it starts, so it is started again when they change.
  async function soundFilesChanged() {
    const pluginName = state.openName;
    await reloadOpenPlugin();
    if (state.openName !== pluginName || !samplePlayer.isASampleApp(state.openPlugin)) return;
    samplePlayer.stopIfItIs(pluginName);
    showOnTheSample(state.openPlugin);
  }

  // The box grows with what is typed, up to the tallest the page allows, so it is small while it is empty.
  // Measuring it means shrinking it for a moment, so the conversation above is held in place meanwhile.
  function fitMessageBox() {
    chatEnd.keepPlaceWhile(() => {
      page.message.style.height = "auto";
      page.message.style.height = `${page.message.scrollHeight}px`;
    });
  }

  function emptyMessageBox() {
    page.message.value = "";
    fitMessageBox();
    drawRequestButton();
  }

  // Until there is a plugin to look at, the conversation has the whole window. As soon as one is opened or begun,
  // the conversation narrows and the MPC view slides in beside it. The page's styling does the moving.
  // The musician can also give the conversation the whole window while a plugin is open, and bring its view back.
  function drawLayout() {
    const startingNew = state.openName === NEW_PLUGIN;
    document.body.classList.toggle("chat-only", startingNew || state.chatIsWide);
    document.body.classList.toggle("starting-new", startingNew);
    page.widenChat.hidden = startingNew;
    const words = state.chatIsWide ? SHOW_THE_PLUGIN : WIDEN_THE_CONVERSATION;
    page.widenChat.textContent = state.chatIsWide ? "<-|" : "|->";
    page.widenChat.title = `${words} (Tab)`;
    page.widenChat.setAttribute("aria-label", words);
    page.widenChat.setAttribute("aria-pressed", String(state.chatIsWide));
  }

  // Tab flips between the two, as long as the musician is typing a message or has nothing picked out. Anywhere
  // else Tab still steps from one button to the next, and Shift with Tab always does.
  function isTabToFlipTheView(event) {
    const plainTab = event.key === "Tab" && !event.shiftKey && !event.altKey && !event.ctrlKey && !event.metaKey;
    const restsOn = document.activeElement;
    return plainTab && !page.widenChat.hidden && (restsOn === document.body || restsOn === page.message);
  }

  function flipTheViewOnTab(event) {
    if (!isTabToFlipTheView(event)) return;
    event.preventDefault();
    toggleWideChat();
  }

  // Flipping the view always comes to rest on the latest words, wherever the conversation was being read.
  function toggleWideChat() {
    state.chatIsWide = !state.chatIsWide;
    chatEnd.followAgain();
    drawLayout();
    chatEnd.jump();
  }

  // --- How full the AI's context is -------------------------------------------------------------

  function inThousands(tokens) {
    return tokens < TOKENS_IN_A_THOUSAND ? String(tokens) : `${Math.round(tokens / TOKENS_IN_A_THOUSAND)}K`;
  }

  // The context is measured against the limit set for compaction, or failing that against all the model can hold.
  // While a model on the musician's ChatGPT plan is picked, the screen says so, with the way to manage its use.
  function drawPlanNote() {
    page.planNote.hidden = !aiMenu.usesChatGptPlan();
  }

  function drawContext() {
    const held = state.contextOf.get(state.openName);
    const room = appSettings.compactAt() || aiMenu.contextWindow();
    page.contextReadout.hidden = state.openName === NEW_PLUGIN;
    page.contextReadout.textContent = held === undefined ? "CONTEXT –" : `CONTEXT ${inThousands(held)}${room ? ` / ${inThousands(room)}` : ""}`;
    page.contextReadout.title = ABOUT_THE_CONTEXT;
    page.contextReadout.classList.toggle("is-nearly-full", Boolean(room) && held !== undefined && held >= room * 0.8);
  }

  function contextHeard(heard) {
    if (!aiJobOn(heard.plugin)) return;
    state.contextOf.set(heard.plugin, heard.tokens);
    drawContext();
  }

  function drawEverything() {
    drawLayout();
    drawContext();
    drawTabs();
    drawStatus();
    drawMessages();
    drawMirror();
    drawComposer();
    drawSamples();
  }

  // A plugin that is still being created has no files to read yet, so it opens as a bare summary.
  async function loadPlugin(pluginName) {
    try {
      return await ask("open_plugin", { name: pluginName });
    } catch (notThereYet) {
      const listed = state.plugins.find((plugin) => plugin.name === pluginName);
      return listed ? { summary: listed, screen: null, parameters: null, versionsKept: 0 } : null;
    }
  }

  // A conversation that is opened starts at its newest words. One that is only drawn again stays where it was being read.
  // Reading a plugin takes a moment. Until it is read, the one open before is not shown under its name, and if the
  // musician opens another meanwhile, what was read is let go.
  async function open(pluginName) {
    const isAnotherConversation = pluginName !== state.openName;
    state.openName = pluginName;
    if (isAnotherConversation) knobChoosing.end();
    aiMenu.showModelOf(pluginName === NEW_PLUGIN ? null : pluginName);
    appSettings.noteOpened(pluginName);
    state.screenPage = 0;
    if (isAnotherConversation) state.openPlugin = null;
    const read = pluginName === NEW_PLUGIN ? null : await loadPlugin(pluginName);
    await conversationWith(pluginName).readBack;
    if (state.openName !== pluginName) return;
    state.openPlugin = read;
    if (isAnotherConversation) chatEnd.arriveWith(drawEverything);
    else drawEverything();
  }

  async function refreshLibrary() {
    const library = await ask("library");
    state.plugins = library.plugins;
  }

  // --- Work that takes a while ----------------------------------------------------------------

  function safetyOf(pluginName) {
    if (pluginName === NEW_PLUGIN) return STATUS.noPlugin;
    const report = state.reports.get(pluginName);
    if (!report) return STATUS.unchecked;
    return report.safe ? STATUS.safe : STATUS.blocked;
  }

  // The safety check can be asked for only when the open plugin has not had one since it last changed.
  function canBeChecked() {
    return !isBeingWorkedOn(state.openName) && safetyOf(state.openName) === STATUS.unchecked;
  }

  // What is happening to a plugin, or failing that whether it has passed its safety check.
  function statusOf(pluginName) {
    if (state.checking.has(pluginName)) return state.checking.get(pluginName);
    if (aiJobOn(pluginName)) return aiJobOn(pluginName).status;
    return safetyOf(pluginName);
  }

  // The status is the open plugin's. When that plugin has not been checked, pressing the status offers to check it.
  // Work on any other plugin shows on that plugin's tab.
  // The blacksmith covers the plugin's view for as long as the plugin is out of reach, with what is being done
  // written under him. That is exactly as long as its Play button is held: from the moment the AI starts on it,
  // through building and checking, and while it is sent or got ready for a catalog.
  function smithIsAtWork() {
    return isBeingWorkedOn(state.openName);
  }

  function drawSmith() {
    keepTheSampleInStep();
    if (smithIsAtWork()) smith.show(`${statusOf(state.openName)}…`);
    else smith.hide();
  }

  // An app for the MPC Sample has him on the device's own screen, from the moment the work on it starts until
  // the moment it ends, when the app is back on it.
  function keepTheSampleInStep() {
    const plugin = state.openPlugin;
    if (!samplePlayer.isASampleApp(plugin) || samplePlayer.showsTheSmith() === isBeingWorkedOn(plugin.summary.name)) return;
    showOnTheSample(plugin);
  }

  function drawStatus() {
    page.status.textContent = statusOf(state.openName);
    drawSmith();
    page.statusButton.disabled = !canBeChecked();
    drawSendButton();
    page.statusButton.parentElement.title = canBeChecked() ? "Press to run the safety check." : checkSummary();
    page.checkSummary.textContent = `${checkSummary()} ${SAFETY_CHECK_OFFERED}`;
    if (!canBeChecked()) statusMenu.close();
  }

  function showNextBeatWords() {
    state.beatWords = (state.beatWords + 1) % BEAT_WORDS.length;
    working.words.textContent = `${BEAT_WORDS[state.beatWords]}…`;
  }

  function startBeatWords() {
    state.beatWords = Math.floor(Math.random() * BEAT_WORDS.length);
    showNextBeatWords();
    state.beatWordsTimer = setInterval(showNextBeatWords, BEAT_WORDS_CHANGE_EVERY_MS);
  }

  function stopBeatWords() {
    clearInterval(state.beatWordsTimer);
    state.beatWordsTimer = undefined;
  }

  // The pads wave beside beat-making words for as long as anything is being worked on.
  function keepBeatWordsInStep() {
    const areGoing = state.beatWordsTimer !== undefined;
    if (somethingIsBeingWorkedOn() && !areGoing) startBeatWords();
    if (!somethingIsBeingWorkedOn() && areGoing) stopBeatWords();
  }

  // Shows everywhere that work on some plugin has begun or ended: on its tab, in the status, and in the conversation.
  function drawWork() {
    keepBeatWordsInStep();
    drawTabs();
    drawStatus();
    drawSteps();
    drawComposer();
    drawMessages();
  }

  // Only the AI's work can be stopped.
  function startAiWork(pluginName) {
    state.aiJobs.set(pluginName, { status: STATUS.working, canBeStopped: true, beganAt: timeTaken.now(), steps: [], awaitsItsScreen: false });
    state.contextOf.set(pluginName, 0);
    samplePlayer.stopIfItIs(pluginName);
    drawContext();
    working.fold.open = false;
    drawWork();
  }

  function endAiWork(pluginName) {
    state.aiJobs.delete(pluginName);
    drawWork();
  }

  function blockedMessage(report) {
    return { kind: "problem", text: `**Not safe yet.** It was stopped at "${whereItWasBlocked(report)}". Ask for a fix and it will try again.` };
  }

  // A safe plugin is announced by the status in the top bar alone. Only a blocked one is explained in the conversation.
  function showReport(pluginName, report) {
    state.reports.set(pluginName, report);
    state.editedSinceCheck.delete(pluginName);
    if (!report.safe) addMessage(pluginName, blockedMessage(report));
  }

  // Work that ended without a safety check leaves the plugin unchecked, whatever an earlier check said of it.
  function showProblem(pluginName, message) {
    state.reports.delete(pluginName);
    addMessage(pluginName, message);
  }

  // What of the original goes along with a remix: its README when it has one, and the picture of its screen
  // for as long as the musician leaves it among the pictures to send.
  function sentWithTheRemix() {
    const material = state.remixMaterial || {};
    return { readme: material.readme, itsPicture: Boolean(material.picture) && pictureTray.isWaiting(material.picture) };
  }

  // A remix is described by what is known of the original, then by what was typed. Its `shown` is how the
  // request reads in the conversation, which leaves out the long README.
  // Whether what is being started is an app for the MPC Sample, and not a plugin for the MPCs with a touch screen.
  function isNewForTheSample() {
    return page.composer.querySelector('input[name="made-for"]:checked').value === MADE_FOR_THE_SAMPLE;
  }

  function kindOfWhatIsNew() {
    return isNewForTheSample() ? KIND_OF_A_SAMPLE_APP : page.composer.querySelector('input[name="kind"]:checked').value;
  }

  // Nothing is remixed into an app for the MPC Sample: the plugins there are to remix are for other MPCs.
  function newPluginRequest() {
    const typed = page.message.value.trim();
    const said = { name: page.pluginName.value.trim(), maker: page.makerName.value.trim(), kind: kindOfWhatIsNew() };
    if (!state.remixOf || isNewForTheSample()) return { ...said, description: typed };
    const sent = sentWithTheRemix();
    return { ...said, description: remixPicker.describedWith(state.remixOf, typed, sent), shown: remixPicker.shownWith(state.remixOf, typed, sent) };
  }

  function whatIsMissing(request) {
    const called = request.kind === KIND_OF_A_SAMPLE_APP ? "app" : "plugin";
    if (!request.name) return `Give the ${called} a name first.`;
    if (!request.maker) return "Say who the maker is. It cannot be changed later.";
    if (!request.description) return `Describe what you want the ${called} to do.`;
    return null;
  }

  // Starts a new plugin from what is in the form. With the architect switched on, the idea goes to it first.
  function startNewPlugin() {
    if (state.architect) return buildWhatWasPlanned();
    const request = newPluginRequest();
    const missing = whatIsMissing(request);
    if (missing) return addMessage(NEW_PLUGIN, { kind: "problem", text: missing });
    return page.architect.checked ? beginWithTheArchitect(request) : build(request);
  }

  // The switch between the MPCs with a touch screen and the MPC Sample was thrown: the form and the drawing
  // beside it change to what is now being started.
  function madeForChanged() {
    remember("madeFor", isNewForTheSample() ? MADE_FOR_THE_SAMPLE : "mpc");
    drawComposer();
    drawMirror();
  }

  function putInTheMessageBox(words) {
    page.message.value = words;
    page.message.dispatchEvent(new Event("input", { bubbles: true }));
    page.message.focus();
  }

  // A request reads in the conversation as the builder is sent it, unless it says how else it is to be shown.
  function asShown(request) {
    return request.shown || request.description;
  }

  // The builder is sent the request without how it reads in the conversation.
  function asBuilt(request) {
    const { shown, ...sent } = request;
    return sent;
  }

  // `foreword` is what MP Forge has to say before the request, such as what the dice rolled.
  async function build(request, foreword = []) {
    remember("maker", request.maker);
    let soundFiles = [];
    let pictures;
    try {
      soundFiles = await sampleShelf.storeWaitingAs(request.name);
      pictures = await pictureTray.handOverFor(request.name);
      await ask("start_plugin", { request: { ...asBuilt(request), pictures: pictures.names }, ...aiMenu.chosen() });
    } catch (problem) {
      sampleShelf.discard(request.name, soundFiles);
      return addMessage(NEW_PLUGIN, { kind: "problem", text: describeProblem(problem) });
    }
    pictureTray.letGo(pictures);
    forgetTheNewPluginTalk();
    listWhileBeingMade(request);
    foreword.forEach((message) => addMessage(request.name, message));
    addMessage(request.name, { kind: "you", text: asShown(request), pictures: pictures.addresses, pictureFiles: pictures.names });
    startAiWork(request.name);
    aiJobOn(request.name).awaitsItsScreen = true;
    page.pluginName.value = "";
    emptyMessageBox();
    await open(request.name);
  }

  // Once a plugin is being built, what was said while it was only an idea has done its work.
  function forgetTheNewPluginTalk() {
    state.architect = null;
    state.remixOf = null;
    state.remixMaterial = null;
    state.conversations.delete(conversationKey(NEW_PLUGIN));
  }

  // --- The plugin architect ---------------------------------------------------------------------

  // The line that says the scout is out looking, or that a catalog plugin's source is on its way. Otherwise the
  // architect's card of questions, or the working line while it thinks of them.
  function whatIsBeingThoughtOver() {
    const planning = state.architect;
    if (state.scouting) return [element("div", "architect-thinking soft", SCOUT_IS_LOOKING)];
    if (state.fetchingSourceOf) return [element("div", "architect-thinking soft", fetchingLine(state.fetchingSourceOf))];
    if (!planning) return [];
    if (planning.thinking) return [element("div", "architect-thinking soft", ARCHITECT_IS_THINKING)];
    return planning.card ? [planning.card.made] : [];
  }

  function beginWithTheArchitect(request) {
    state.architect = { request, answered: [], card: null, thinking: false };
    addMessage(NEW_PLUGIN, { kind: "you", text: asShown(request) });
    emptyMessageBox();
    askTheArchitect();
  }

  // Asks for a round of questions. If the architect cannot be reached, the idea is kept, so it can be asked
  // again or built without it.
  async function askTheArchitect() {
    const planning = state.architect;
    const idea = { name: planning.request.name, kind: planning.request.kind, description: planning.request.description, answered: planning.answered };
    planning.thinking = true;
    planning.card = null;
    drawWork();
    try {
      const questions = await ask("architect_questions", { idea, ...aiMenu.chosen() });
      const called = planning.request.kind === KIND_OF_A_SAMPLE_APP ? ARCHITECTS.os : ARCHITECTS.plugin;
      planning.card = pluginArchitect.card(questions, { called, onBuild: buildWhatWasPlanned, onMore: askForMoreQuestions, onStartOver: startOver });
    } catch (problem) {
      if (state.architect === planning) addMessage(NEW_PLUGIN, { kind: "problem", text: `${describeProblem(problem)} Press BUILD IT to build from what you have said so far.` });
    }
    planning.thinking = false;
    drawWork();
  }

  // What was answered on the card joins what was answered before, and shows as the musician's own words.
  function noteAnswers() {
    const planning = state.architect;
    const answers = planning.card ? planning.card.answers() : [];
    planning.card = null;
    if (!answers.length) return;
    planning.answered.push(...answers);
    addMessage(NEW_PLUGIN, { kind: "you", text: pluginArchitect.wordsOf(answers) });
  }

  function askForMoreQuestions() {
    noteAnswers();
    askTheArchitect();
  }

  function startOver() {
    putInMessageBox(state.architect.request.description);
    forgetTheNewPluginTalk();
    drawWork();
  }

  // Builds from the first idea, everything answered, and whatever was typed in the message box since.
  // The name, the maker and the kind are read from the form again, in case they were changed meanwhile.
  function buildWhatWasPlanned() {
    if (architectIsThinking()) return undefined;
    if (aiHasNoRoom()) return addMessage(NEW_PLUGIN, { kind: "problem", text: AI_HAS_NO_ROOM });
    noteAnswers();
    const planning = state.architect;
    const planned = (firstIdea) => pluginArchitect.describedWith(firstIdea, planning.answered, page.message.value.trim());
    const request = { ...newPluginRequest(), description: planned(planning.request.description), shown: planned(asShown(planning.request)) };
    const missing = whatIsMissing(request);
    if (missing) return addMessage(NEW_PLUGIN, { kind: "problem", text: missing });
    return build(request);
  }

  // --- I'm feeling lucky, and remixing ------------------------------------------------------------

  function rollNote(roll) {
    const withoutTheWeb = aiMenu.looksThingsUp() ? "" : ` ${LUCKY_WITHOUT_THE_WEB}`;
    return { kind: "note", text: `The dice rolled **${roll.number}: ${roll.theme}**. It will be an ${roll.kind}.${withoutTheWeb}` };
  }

  function findNote(find) {
    const foundAt = find.source ? ` Found at ${find.source}` : "";
    return { kind: "note", text: `**${find.name}.** ${find.story}${foundAt}` };
  }

  function isTaken(name) {
    return state.plugins.some((plugin) => plugin.name.toLowerCase() === name.toLowerCase());
  }

  // A name no plugin has yet: the one wanted, or that with the first free number after it.
  function nameNotTaken(wanted) {
    const numbered = (number) => `${wanted.slice(0, LONGEST_PLUGIN_NAME - 3).trim()} ${number}`;
    let number = 2;
    while (isTaken(numbered(number))) number += 1;
    return isTaken(wanted) ? numbered(number) : wanted;
  }

  function huntFor(roll) {
    return { number: roll.number, kind: roll.kind, alreadyMade: state.plugins.map((plugin) => plugin.name) };
  }

  // The request a find becomes: the scout's brief, with the find it came from.
  function requestFrom(find, roll, maker) {
    const description = `${find.description}\n\nIt is based on this find: ${find.story}`;
    return { name: nameNotTaken(find.name), maker, kind: roll.kind, description };
  }

  // Rolls the dice, sends the scout out for a find in the theme rolled, and builds a plugin from what it brings back.
  async function rollAndBuild(maker) {
    const roll = await ask("roll_the_dice");
    const rolled = rollNote(roll);
    addMessage(NEW_PLUGIN, rolled);
    const find = await ask("lucky_find", { hunt: huntFor(roll), ...aiMenu.chosen() });
    state.scouting = false;
    await build(requestFrom(find, roll, maker), [rolled, findNote(find)]);
  }

  async function feelLucky() {
    const maker = page.makerName.value.trim();
    if (!maker) return addMessage(NEW_PLUGIN, { kind: "problem", text: LUCKY_NEEDS_A_MAKER });
    if (aiHasNoRoom()) return addMessage(NEW_PLUGIN, { kind: "problem", text: AI_HAS_NO_ROOM });
    state.scouting = true;
    drawWork();
    await rollAndBuild(maker).catch((problem) => addMessage(NEW_PLUGIN, { kind: "problem", text: describeProblem(problem) }));
    state.scouting = false;
    return drawWork();
  }

  // The original's README is kept for the builder, and the picture of its screen joins the pictures to send,
  // where the musician can see it and take it out again.
  async function bringWhatGoesWith(original) {
    const material = await remixPicker.materialOf(original);
    const picture = material.screenshot ? await pictureTray.add(material.screenshot) : null;
    state.remixMaterial = { readme: material.readme, picture };
  }

  // A plugin picked from the catalog is either taken in as its makers wrote it, or remixed.
  async function chooseFromTheCatalog() {
    const picked = await remixPicker.pick(page.appSheet);
    if (!picked) return;
    return picked.asItIs ? importFromTheCatalog(picked.original) : remix(picked.original);
  }

  // The plugin to remix fills in the form: a name for the remix and its kind. Both can be changed.
  async function remix(original) {
    forgetTheRemix();
    await bringWhatGoesWith(original);
    state.remixOf = original;
    page.pluginName.value = nameNotTaken(original.remixName);
    page.composer.querySelector(`input[name="kind"][value="${original.kind}"]`).checked = true;
    drawComposer();
    page.message.focus();
  }

  // The picture of the original's screen leaves with the remix, unless it has been sent or taken out already.
  function forgetTheRemix() {
    if (state.remixMaterial && state.remixMaterial.picture) pictureTray.stopWaiting(state.remixMaterial.picture);
    state.remixOf = null;
    state.remixMaterial = null;
    drawComposer();
  }

  function isTheGuidesExample(plugin) {
    return plugin.name === state.guidesExample;
  }

  // Opens the made-up plugin the guide points at. It gets a tab, a conversation and a passed safety check to
  // show, and none of it is kept: nothing is asked of the workbench about it.
  function showTheGuidesExample() {
    const name = nameNotTaken(guideExample.NAME);
    const example = guideExample.plugin(name, page.makerName.value.trim() || "You");
    state.guidesExample = name;
    state.plugins = [...state.plugins, example.summary];
    state.conversations.set(conversationKey(name), conversationOnlyOf([...guideExample.TALK]));
    state.reports.set(name, guideExample.REPORT);
    state.openName = name;
    state.openPlugin = example;
    state.screenPage = 0;
    drawEverything();
  }

  function letTheGuidesExampleGo() {
    state.plugins = state.plugins.filter((plugin) => !isTheGuidesExample(plugin));
    state.conversations.delete(conversationKey(state.guidesExample));
    state.reports.delete(state.guidesExample);
    state.guidesExample = undefined;
  }

  function backToANewPlugin() {
    if (state.guidesExample) letTheGuidesExampleGo();
    return open(NEW_PLUGIN);
  }

  async function showTheGuide() {
    showSettings(false);
    await open(NEW_PLUGIN);
    guide.show();
  }

  // The plugin gets its tab straight away, before its files exist on disk.
  function listWhileBeingMade(request) {
    const alreadyListed = state.plugins.some((plugin) => plugin.name === request.name);
    if (!alreadyListed) state.plugins.push({ name: request.name, maker: request.maker, kind: request.kind });
  }

  // A request made now goes out with the pictures in the tray and the model the menu shows.
  function requestMadeNow(words) {
    return { words, picturesFor: pictureTray.handOverFor, sentWith: aiMenu.chosen() };
  }

  // One that waited goes out with the model the menu showed as it was typed. It has no pictures: none can be
  // added while the AI works, and the tray may by now hold another plugin's.
  function requestThatWaited(waited) {
    return { words: waited.words, picturesFor: async () => NO_PICTURES, sentWith: waited.sentWith };
  }

  // Asks the AI for a change to a plugin. Says whether the request went out.
  async function askForAChange(pluginName, request) {
    if (!request.words || aiHasNoRoom() || isBeingWorkedOn(pluginName) || pluginName === NEW_PLUGIN) return false;
    let pictures;
    try {
      pictures = await request.picturesFor(pluginName);
      await ask("change_plugin", { name: pluginName, change: { words: request.words, pictures: pictures.names }, ...request.sentWith });
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
      return false;
    }
    pictureTray.letGo(pictures);
    addMessage(pluginName, { kind: "you", text: request.words, pictures: pictures.addresses, pictureFiles: pictures.names });
    startAiWork(pluginName);
    return true;
  }

  // Asks for a change to the open plugin, in words that were typed or picked from the AI's choices.
  function changeOpenPlugin(words) {
    return askForAChange(state.openName, requestMadeNow(words));
  }

  // --- Requests that wait their turn ------------------------------------------------------------

  // What is asked for while the AI works on a plugin waits its turn. It goes out once that work has ended well,
  // the oldest first, or as soon as the musician says SEND NOW, which ends the work in hand.
  function waitingFor(pluginName) {
    if (!state.waitingToBeSent.has(pluginName)) state.waitingToBeSent.set(pluginName, []);
    return state.waitingToBeSent.get(pluginName);
  }

  function letTypedWaitItsTurn() {
    waitingFor(state.openName).push({ words: page.message.value.trim(), sentWith: aiMenu.chosen() });
    emptyMessageBox();
    chatEnd.followAgain();
    drawMessages();
  }

  function stopWaiting(pluginName, waited) {
    const waiting = waitingFor(pluginName);
    waiting.splice(waiting.indexOf(waited), 1);
    if (pluginName === state.openName) drawMessages();
  }

  // Sends the request that has waited longest. One that could not go out goes on waiting.
  async function sendWhatWaitedLongest(pluginName) {
    const [next] = waitingFor(pluginName);
    if (next && (await askForAChange(pluginName, requestThatWaited(next)))) stopWaiting(pluginName, next);
  }

  function putFirst(pluginName, waited) {
    const waiting = waitingFor(pluginName);
    waiting.unshift(...waiting.splice(waiting.indexOf(waited), 1));
  }

  // The request goes out next. Work the AI has in hand on the plugin is ended first, and the request follows
  // as soon as that work has stopped.
  function sendNow(pluginName, waited) {
    putFirst(pluginName, waited);
    const job = aiJobOn(pluginName);
    if (!job) return sendWhatWaitedLongest(pluginName);
    job.makesWayForWhatWaits = true;
    if (job.canBeStopped) stopWorkOn(pluginName);
    return drawMessages();
  }

  // What was typed stays in the box until the request has gone out, so nothing is lost when it could not.
  async function sendTypedChange() {
    if (await changeOpenPlugin(page.message.value.trim())) emptyMessageBox();
  }

  // The AI's steps show under the working line as it takes them. When it finishes, they fold away under its closing words.
  // A step is heard of as it starts, so the plugin is being built from the step that builds it until the next one.
  function statusDuring(job, line) {
    if (job.status === STATUS.stopping) return STATUS.stopping;
    return line.kind !== "said" && line.text.startsWith(STEP_THAT_BUILDS) ? STATUS.building : STATUS.working;
  }

  function workProgressed(line) {
    const job = aiJobOn(line.plugin);
    if (!job) return;
    const itsScreenJustAppeared = job.awaitsItsScreen && hasWrittenItsScreen(job);
    if (itsScreenJustAppeared) job.awaitsItsScreen = false;
    job.steps.push({ kind: line.kind, text: line.text });
    job.status = statusDuring(job, line);
    drawStatus();
    drawSteps();
    if (itsScreenJustAppeared || changesTheScreen(line)) showLatestScreen(line.plugin);
  }

  function changesTheScreen(line) {
    return line.kind !== "said" && STEPS_THAT_CHANGE_THE_SCREEN.some((start) => line.text.startsWith(start));
  }

  // A step is heard of as it starts. So the screen's layout is known to be written once that step is no longer the newest.
  function hasWrittenItsScreen(job) {
    return job.steps.some((step) => step.text === WRITING_THE_SCREEN);
  }

  // The AI's work on a plugin gives up at its next step, and the workbench then says it was stopped.
  function stopWorkOn(pluginName) {
    const job = aiJobOn(pluginName);
    job.canBeStopped = false;
    job.status = STATUS.stopping;
    drawStatus();
    drawComposer();
    ask("stop_work", { name: pluginName }).catch(() => {});
  }

  // While the AI works on the open plugin, the MPC view follows what it has written so far.
  async function showLatestScreen(pluginName) {
    if (state.openName !== pluginName) return;
    const latest = await loadPlugin(state.openName);
    if (!latest || state.openName !== latest.summary.name) return;
    state.openPlugin = latest;
    const lastPage = Math.max(0, ((latest.screen && latest.screen.pages) || []).length - 1);
    state.screenPage = Math.min(state.screenPage, lastPage);
    drawMirror();
    drawSmith();
  }

  async function workFinished(result) {
    const job = aiJobOn(result.plugin);
    const howItWent = { steps: job.steps, tookSeconds: timeTaken.secondsSince(job.beganAt) };
    if (result.ok) {
      addMessage(result.plugin, { kind: "said", text: result.closingWords || FINISHED_WITHOUT_WORDS, ...howItWent });
      showReport(result.plugin, result.report);
    } else {
      showProblem(result.plugin, { kind: "problem", text: result.error, ...howItWent });
    }
    endAiWork(result.plugin);
    await refreshLibrary();
    const stillExists = state.plugins.some((plugin) => plugin.name === result.plugin);
    if (stillExists) await foldOldVersions(result.plugin);
    if (!stillExists) moveConversation(result.plugin, NEW_PLUGIN);
    if (state.openName === result.plugin) await open(stillExists ? result.plugin : NEW_PLUGIN);
    else drawTabs();
    if (!stillExists) state.waitingToBeSent.delete(result.plugin);
    else if (result.ok || job.makesWayForWhatWaits) await sendWhatWaitedLongest(result.plugin);
  }

  // When nothing could be made, what was said about it belongs with the empty "new plugin" form.
  function moveConversation(fromPlugin, toPlugin) {
    conversationAbout(toPlugin).push(...conversationAbout(fromPlugin));
    state.conversations.delete(conversationKey(fromPlugin));
  }

  // Checks one plugin. Others can be checked at the same time, and the AI can be at work on another meanwhile.
  // The check builds the plugin afresh, so if it is still the one open it is read again to show what was built.
  async function runSafetyCheck(pluginName) {
    state.checking.set(pluginName, STATUS.checking);
    drawWork();
    try {
      showReport(pluginName, await ask("check_plugin", { name: pluginName }));
    } catch (problem) {
      showProblem(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
    state.checking.delete(pluginName);
    drawWork();
    if (state.openName === pluginName) await showAsBuilt(pluginName);
  }

  async function showAsBuilt(pluginName) {
    const built = await loadPlugin(pluginName);
    if (!built || state.openName !== pluginName) return;
    state.openPlugin = built;
    drawMirror();
  }

  function checkOpenPlugin() {
    statusMenu.close();
    runSafetyCheck(state.openName);
  }

  // --- Settings --------------------------------------------------------------------------------

  // The settings take the place of the conversation and the MPC view until DONE is pressed.
  function showSettings(isShown) {
    page.settings.hidden = !isShown;
    document.body.classList.toggle("in-settings", isShown);
    page.settingsButton.setAttribute("aria-pressed", String(isShown));
    if (!isShown) drawContext();
  }

  // --- Renaming and deleting a plugin -----------------------------------------------------------

  // What the workbench remembers of a plugin under its name: what was said, and how its last check went.
  function forget(pluginName) {
    state.conversations.delete(conversationKey(pluginName));
    state.reports.delete(pluginName);
    state.editedSinceCheck.delete(pluginName);
    state.waitingToBeSent.delete(pluginName);
  }

  // Asks the app to do something to the open plugin, with the live player stopped first so it holds none of its files.
  // Says whether it was done. A refusal is explained in the plugin's conversation.
  async function doToOpenPlugin(command, details) {
    const pluginName = state.openName;
    livePlayer.follow(null);
    try {
      await ask(command, { name: pluginName, ...details });
      return true;
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
      return false;
    }
  }

  // The conversation is kept with the plugin's files, so it comes along under the new name. Everything built under
  // the old name is thrown away by the rename, so the plugin is built and checked again at once.
  async function renameOpenPlugin() {
    const oldName = state.openName;
    const newName = await askForNewName(page.appSheet, oldName);
    if (!newName || newName === oldName) return;
    if (!(await doToOpenPlugin("rename_plugin", { newName }))) return;
    forget(oldName);
    aiMenu.pluginWasRenamed(oldName, newName);
    tabStrip.renamed(oldName, newName);
    await refreshLibrary();
    await open(newName);
    runSafetyCheck(newName);
  }

  // --- A guide to the plugin -------------------------------------------------------------------

  // Asks whether to make the guide, and whether the AI is to write its words. Answers with whether it is to write
  // them again and whether it will be asked at all, or with nothing when no guide is to be made.
  async function guideWanted(pluginName) {
    const guide = await ask("guide_state", { name: pluginName });
    if (!guide.built) {
      addMessage(pluginName, { kind: "problem", text: A_GUIDE_NEEDS_A_BUILD });
      return null;
    }
    const wanted = await askAboutTheGuide(page.appSheet, guide.written);
    return wanted && { writeAgain: wanted.writeAgain, asksTheAi: wanted.writeAgain || !guide.written };
  }

  async function makeGuideOfOpenPlugin() {
    const pluginName = state.openName;
    try {
      const wanted = await guideWanted(pluginName);
      if (!wanted) return;
      if (wanted.asksTheAi) addMessage(pluginName, { kind: "note", text: AI_IS_WRITING_THE_GUIDE });
      const made = await ask("make_guide", { name: pluginName, writeAgain: wanted.writeAgain, ...aiMenu.chosen() });
      addMessage(pluginName, { kind: "note", text: `**The guide is open in your browser.** It is saved as ${made.page}. ${ABOUT_A_MADE_GUIDE}` });
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
  }

  // --- Listing a plugin in the Open MPC catalog -------------------------------------------------

  // The app runs the safety check once more before it makes the listing's folder, so a plugin that no longer
  // passes is held back and shown as blocked, the same as after any check.
  async function getReadyForOpenMpc(pluginName, listing) {
    state.checking.set(pluginName, STATUS.listing);
    drawWork();
    try {
      const outcome = await ask("list_on_open_mpc", { name: pluginName, listing });
      if (outcome) showReport(pluginName, outcome.report);
      if (outcome && outcome.ready) addMessage(pluginName, { kind: "note", text: `**Ready for Open MPC.** Everything is in ${outcome.folder}. ${ABOUT_A_LISTING_MADE}` });
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
    state.checking.delete(pluginName);
    drawWork();
  }

  async function listOpenPluginOnOpenMpc() {
    const pluginName = state.openName;
    try {
      const listed = await ask("open_mpc_listing_state", { name: pluginName });
      if (!listed.built) return addMessage(pluginName, { kind: "problem", text: A_LISTING_NEEDS_A_BUILD });
      const listing = await askAboutListing(page.appSheet, pluginName, listed);
      if (listing) await getReadyForOpenMpc(pluginName, listing);
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
  }

  // --- Handing a plugin to somebody else, and bringing one in ----------------------------------

  async function exportOpenPlugin() {
    const pluginName = state.openName;
    try {
      const savedAt = await ask("export_plugin", { name: pluginName });
      if (savedAt) addMessage(pluginName, { kind: "note", text: exportedNote(savedAt) });
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
  }

  function exportedNote(savedAt) {
    return `**Exported** to ${savedAt}. ${WHAT_AN_EXPORT_HOLDS}`;
  }

  // Takes the picked plugin in, asking for another name for as long as the one it would have is taken.
  // Answers with the name it came in under, or with nothing when the musician gave up.
  async function takeIn(from, named) {
    const taken = await ask("import_plugin", { from, named: named || null });
    if (taken.name) return taken.name;
    const anotherName = await askForAnotherName(page.appSheet, taken.nameTaken);
    return anotherName ? takeIn(from, anotherName) : null;
  }

  async function showImported(pluginName, credit) {
    await refreshLibrary();
    await open(pluginName);
    addMessage(pluginName, { kind: "note", text: [IMPORTED_AND_NOT_RUN, credit, whereItGoesOnline(state.openPlugin)].filter(Boolean).join(" ") });
  }

  // Takes a catalog plugin in as its makers wrote it, asking for another name for as long as its own is taken.
  // Answers with what came in, or with nothing when the musician gave up.
  async function takeInFromTheCatalog(original, named) {
    const taken = await ask("import_from_open_mpc", { original: whereItComesFrom(original), named: named || null });
    if (taken.name) return taken;
    const anotherName = await askForAnotherName(page.appSheet, taken.nameTaken);
    return anotherName ? takeInFromTheCatalog(original, anotherName) : null;
  }

  function whereItComesFrom(original) {
    return { name: original.name, author: original.author, source: original.source, license: original.license };
  }

  function fetchingLine(original) {
    return `Fetching the source of ${original.name} by ${original.author}…`;
  }

  async function importFromTheCatalog(original) {
    state.fetchingSourceOf = original;
    drawWork();
    try {
      const taken = await takeInFromTheCatalog(original);
      if (taken) await showImported(taken.name, `${taken.credit} ${original.termsForTakingItIn}`);
    } catch (problem) {
      addMessage(NEW_PLUGIN, { kind: "problem", text: describeProblem(problem) });
    }
    state.fetchingSourceOf = null;
    return drawWork();
  }

  async function importAPlugin() {
    const what = await askWhatToImport(page.appSheet);
    if (!what) return;
    try {
      const from = await ask("choose_plugin_to_import", { asAFolder: what.asAFolder });
      const pluginName = from && (await takeIn(from));
      if (pluginName) await showImported(pluginName);
    } catch (problem) {
      addMessage(state.openName, { kind: "problem", text: describeProblem(problem) });
    }
  }

  async function deleteOpenPlugin() {
    const pluginName = state.openName;
    if (!(await askBeforeDeleting(page.appSheet, pluginName))) return;
    if (!(await doToOpenPlugin("delete_plugin", {}))) return;
    forget(pluginName);
    aiMenu.pluginWasDeleted(pluginName);
    await refreshLibrary();
    await open(NEW_PLUGIN);
  }

  // --- Arranging the screen by hand ------------------------------------------------------------

  // What the musician asked of the AI about something in the editor goes out as a request of its own.
  // When it cannot go out now, it waits in the message box, so nothing they wrote is lost.
  async function askFromTheEditor(request) {
    if (await changeOpenPlugin(request)) return;
    page.message.value = request;
    fitMessageBox();
  }

  function editTheLook() {
    lookEditor.open(state.openPlugin, state.screenPage);
  }

  // What was arranged is already kept. The plugin is read again so the preview shows it, on the page that was being edited.
  // The last safety check was of the screen before the edits, so it no longer counts.
  async function doneEditing(visit) {
    const pluginName = state.openName;
    if (visit.madeChanges) {
      state.reports.delete(pluginName);
      state.editedSinceCheck.add(pluginName);
    }
    await foldOldVersions(pluginName);
    state.openPlugin = await loadPlugin(pluginName);
    state.screenPage = visit.pageIndex;
    drawEverything();
    if (visit.request) askFromTheEditor(visit.request);
  }

  // --- Stepping through the versions -------------------------------------------------------------

  // Shows the plugin's versions as a timeline. Picking one puts the plugin back as that version had it. Versions
  // can be thrown away from the timeline too, so the plugin is read again afterwards to count the ones left.
  async function showVersions() {
    const pluginName = state.openName;
    const forget = (number) => ask("forget_version", { name: pluginName, number });
    try {
      const versions = await ask("versions", { name: pluginName });
      const number = await versionTimeline.pick(page.appSheet, pluginName, versions, forget);
      if (number !== null) await goToVersion(pluginName, number);
      else if (state.openName === pluginName) await showVersionsLeft(pluginName);
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
  }

  async function showVersionsLeft(pluginName) {
    const read = await loadPlugin(pluginName);
    if (!read || state.openName !== pluginName) return;
    state.openPlugin = read;
    page.subtitle.textContent = summaryLine(read);
    drawComposer();
  }

  // With a limit set in the settings, a plugin's oldest versions are thrown away once it has more than that.
  // A plugin that is busy just now is folded the next time round.
  async function foldOldVersions(pluginName) {
    const most = appSettings.mostVersionsKept();
    if (most) await ask("fold_old_versions", { name: pluginName, most }).catch(() => {});
  }

  // The files are put back, and the plugin is built and checked again so the preview and the sound match them.
  async function goToVersion(pluginName, number) {
    livePlayer.follow(null);
    samplePlayer.stopIfItIs(pluginName);
    const went = await ask("go_to_version", { name: pluginName, number });
    addMessage(pluginName, went ? { kind: "note", text: `Went back to version ${number}. How it was before that is kept as a version too.` } : { kind: "problem", text: NO_SUCH_VERSION });
    if (went) await foldOldVersions(pluginName);
    if (went) await runSafetyCheck(pluginName);
    if (state.openName === pluginName) await open(pluginName);
  }

  // --- Sending the plugin to the MPC ------------------------------------------------------------

  function wordsAboutSending(pluginName, address, outcome) {
    const sent = `**Sent ${pluginName} to your MPC** at ${address}, onto its drive ${outcome.drive}.`;
    if (outcome.restarted) return `${sent} The MPC's app was restarted, so it is in the plugin list now.`;
    if (outcome.restartProblem) return `${sent} The MPC's app could not be restarted, though: ${outcome.restartProblem}`;
    return `${sent} ${SHOWS_AFTER_A_RESTART}`;
  }

  // An address that reached an MPC is remembered for next time.
  function noteSent(pluginName, address, outcome) {
    appSettings.rememberMpcAddress(address);
    addMessage(pluginName, { kind: "note", text: wordsAboutSending(pluginName, address, outcome) });
  }

  // The app runs the safety check once more before it sends, so a plugin that no longer passes is held back
  // and shown as blocked, the same as after any check.
  async function send(pluginName, mpc) {
    state.checking.set(pluginName, STATUS.sending);
    drawWork();
    try {
      const outcome = await ask("send_to_mpc", { name: pluginName, mpc });
      showReport(pluginName, outcome.report);
      if (outcome.sent) noteSent(pluginName, mpc.address, outcome);
    } catch (problem) {
      addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    }
    state.checking.delete(pluginName);
    drawWork();
  }

  // An app for the MPC Sample goes onto the device's card, by its cable or by a card in this computer. The
  // safety check is run once more first, as before any sending.
  async function putOpenAppOnTheSample() {
    const appName = state.openName;
    if (!canBeSent()) return addMessage(appName, { kind: "problem", text: NOT_PUT_ON_THE_SAMPLE_UNTIL_SAFE });
    const way = await sampleSend.askWhichWay(page.appSheet, appName);
    if (!way || !(await sampleSend.isReadyToGo(way).catch(() => false))) return;
    state.checking.set(appName, STATUS.sending);
    drawWork();
    try {
      const outcome = await sampleSend.put(appName, way, (words) => addMessage(appName, { kind: "note", text: words }));
      showReport(appName, outcome.report);
      if (outcome.words) addMessage(appName, { kind: "note", text: outcome.words });
    } catch (problem) {
      addMessage(appName, { kind: "problem", text: describeProblem(problem) });
    }
    state.checking.delete(appName);
    drawWork();
  }

  async function sendOpenPluginToMpc() {
    const pluginName = state.openName;
    if (samplePlayer.isASampleApp(state.openPlugin)) return putOpenAppOnTheSample();
    if (!canBeSent()) return addMessage(pluginName, { kind: "problem", text: NOT_SENT_UNTIL_SAFE });
    const asked = await askBeforeSending(page.appSheet, pluginName, appSettings.mpc(), whereItGoesOnline(state.openPlugin));
    if (!asked) return;
    await send(pluginName, { address: asked.address, drive: appSettings.mpc().drive || null, restart: asked.restart });
  }

  // --- The right-click menu -----------------------------------------------------------------------

  function copyWords(words) {
    navigator.clipboard.writeText(words).catch(() => {});
  }

  function putInMessageBox(words) {
    page.message.value = words;
    fitMessageBox();
    page.message.focus();
  }

  function rowsForAMessage(shown) {
    const words = shown.querySelector(".message-text").textContent;
    const askAgain = shown.classList.contains("you") ? [appMenu.row("Put these words in the message box", () => putInMessageBox(words))] : [];
    return [appMenu.row("Copy these words", () => copyWords(words)), ...askAgain];
  }

  // What can be done to the open plugin, with whatever cannot be done right now switched off.
  function rowsForTheOpenPlugin() {
    const cannotBeTouched = state.openName === NEW_PLUGIN || isBeingWorkedOn(state.openName);
    return [
      appMenu.row(livePlayer.isSounding() ? "Stop playing" : "Play plugin", toggleLiveSound, page.play.disabled),
      appMenu.row(`${whereTheOpenPluginGoes()}…`, sendOpenPluginToMpc, cannotBeTouched || !canBeSent()),
      appMenu.row("Edit the look", editTheLook, page.editLook.disabled),
      appMenu.row("Versions…", showVersions, page.goBack.disabled),
      appMenu.row("Run the safety check", checkOpenPlugin, !canBeChecked()),
      appMenu.DIVIDER,
      appMenu.row("Guide…", makeGuideOfOpenPlugin, cannotBeTouched),
      appMenu.row("Export…", exportOpenPlugin, cannotBeTouched),
      appMenu.row("List on Open MPC…", listOpenPluginOnOpenMpc, cannotBeTouched),
      appMenu.row("Rename…", renameOpenPlugin, cannotBeTouched),
      appMenu.row("Delete…", deleteOpenPlugin, cannotBeTouched),
    ];
  }

  function rowsForATab(pluginName) {
    return pluginName === state.openName ? rowsForTheOpenPlugin() : [appMenu.row(`Open ${pluginName}`, () => open(pluginName))];
  }

  function rowsForTheConversation() {
    const startingNew = state.openName === NEW_PLUGIN;
    return [
      appMenu.row("Add a picture…", () => page.addPictures.click(), page.addButton.disabled),
      appMenu.row("Add a sound file…", () => page.addSamples.click(), page.addButton.disabled),
      appMenu.row("Versions…", showVersions, startingNew || page.goBack.disabled),
      appMenu.row(state.chatIsWide ? "Show the plugin beside the conversation" : "Give the conversation the whole window", toggleWideChat, startingNew),
    ];
  }

  const ROWS_FOR_ANYWHERE = [appMenu.row("New plugin", () => open(NEW_PLUGIN)), appMenu.row("Import a plugin…", importAPlugin), appMenu.row("Settings", () => showSettings(true)), appMenu.row("Show me round", showTheGuide)];

  // What the right-click menu offers depends on what was clicked. The editor has a menu of its own for its page.
  function rowsForWhatWasClicked(target) {
    if (lookEditor.isOpen() || !page.settings.hidden) return [];
    const tab = target.closest(".tab[data-plugin]");
    const message = target.closest(".message");
    if (tab) return rowsForATab(tab.dataset.plugin);
    if (message) return rowsForAMessage(message);
    if (target.closest(".mirror")) return rowsForTheOpenPlugin();
    if (target.closest(".conversation")) return rowsForTheConversation();
    return ROWS_FOR_ANYWHERE;
  }

  // --- Playing the plugin live ----------------------------------------------------------------

  function isKnownToBeBlocked(pluginName) {
    const report = state.reports.get(pluginName);
    return Boolean(report) && !report.safe;
  }

  function toggleLiveSound() {
    const pluginName = state.openName;
    const sayWhy = (problem) => addMessage(pluginName, { kind: "problem", text: describeProblem(problem) });
    if (livePlayer.isSounding()) return livePlayer.setSound(state.openPlugin, false).catch(sayWhy);
    if (isKnownToBeBlocked(pluginName)) return sayWhy(NOT_PLAYED_UNTIL_SAFE);
    return livePlayer.setSound(state.openPlugin, true).catch(sayWhy);
  }

  function soundChanged(sounding) {
    const words = sounding ? "Stop playing" : "Play plugin";
    page.play.title = words;
    page.play.setAttribute("aria-label", words);
    page.play.setAttribute("aria-pressed", String(sounding));
    page.play.disabled = playCannotBePressed();
    page.piano.hidden = !livePlayer.isPlayingAnInstrument();
    const anEffectIsPlaying = sounding && !livePlayer.isPlayingAnInstrument();
    testSound.follow(anEffectIsPlaying);
    effectSwitch.follow(anEffectIsPlaying);
    if (!sounding) piano.letGo();
  }

  // A player that has just started is told how loud to play, and where every control has already been turned to.
  function playerBecameReady(ready) {
    outputVolume.tellThePlayer();
    testSound.playerBecameReady(ready);
    turnedSettings().forEach((setting) => tellPlayerOfTurn(setting.name, setting.amount));
  }

  function playerToldAValue(value) {
    const control = controlsOfOpenPlugin().find((known) => known.index === value.index);
    if (control) valueArrived(control.name, value.amount, value.text);
  }

  // Controls the plugin moved by itself, such as lamps it lights for the notes held.
  function playerToldOfMoves(moves) {
    moves.forEach((move) => {
      const control = controlsOfOpenPlugin().find((known) => known.index === move.index);
      if (control) pluginMoved(control.name, move.amount);
    });
  }

  // --- The presets built into the plugin ----------------------------------------------------------

  function isSameBuild(plugin, noted) {
    return Boolean(plugin && noted) && noted.name === plugin.summary.name && noted.builtAt === plugin.builtAt;
  }

  // A plugin's presets are read from the plugin itself, so only one that is built and not being worked on has them
  // to show. One that is known to be blocked is never loaded.
  function canShowPresets(plugin) {
    return Boolean(plugin && plugin.builtAt) && !isBeingWorkedOn(plugin.summary.name) && !isKnownToBeBlocked(plugin.summary.name);
  }

  function presetName(name, index) {
    return name || `Preset ${index + 1}`;
  }

  // A plugin with a single preset has nothing to step through, so the row stays put away.
  function showPresets(plugin, ready) {
    if (state.openPlugin !== plugin) return;
    const names = ready.presets || [];
    const chosen = isSameBuild(plugin, state.presetChosen) ? state.presetChosen.index : ready.preset || 0;
    presetList.show(names.map(presetName), chosen);
    page.presets.hidden = names.length < 2;
  }

  function readPresets() {
    const plugin = state.openPlugin;
    page.presets.hidden = true;
    if (canShowPresets(plugin)) livePlayer.prepare(plugin).then((ready) => showPresets(plugin, ready), () => {});
  }

  function choosePreset(index) {
    const plugin = state.openPlugin;
    const count = presetList.count();
    if (!plugin || !count) return;
    const wrapped = (index + count) % count;
    state.presetChosen = { name: plugin.summary.name, builtAt: plugin.builtAt, index: wrapped };
    presetList.choose(wrapped);
    livePlayer.choosePreset(plugin, wrapped).catch(() => {});
  }

  function stepPreset(distance) {
    choosePreset(presetList.picked() + distance);
  }

  // The player says where the preset put every control, and the screen and the hardware knobs follow.
  function playerChosePreset(preset) {
    const named = (value) => ({ control: controlsOfOpenPlugin().find((known) => known.index === value.index), value });
    const settings = preset.values.map(named).filter((found) => found.control);
    controlsStandAt(settings.map(({ control, value }) => ({ name: control.name, amount: value.amount, text: value.text })));
    drawHardwareKnobs();
  }

  function isTypingSomewhere(event) {
    return event.target instanceof HTMLElement && event.target.matches("input, textarea, select");
  }

  // A note typed on the computer's keyboard is played through the piano, so its key shows as held.
  function playNoteKey(event, pressed) {
    const pitch = window.pixelPiano.noteTypedWith(event.key);
    const plainPress = !event.repeat && !event.metaKey && !event.ctrlKey && !event.altKey;
    if (pitch === null || !plainPress || isTypingSomewhere(event) || lookEditor.isOpen() || !livePlayer.isPlayingAnInstrument()) return;
    event.preventDefault();
    piano.press(pitch, pressed);
  }

  function testSoundParts() {
    const part = (name) => document.getElementById(`test-sound${name}`);
    return { row: part(""), beat: part("-beat"), own: part("-own"), choose: part("-choose") };
  }

  function startLivePlayer() {
    testSound.start(testSoundParts(), { onProblem: (text) => addMessage(state.openName, { kind: "problem", text }) });
    effectSwitch.start({ on: document.getElementById("effect-on"), off: document.getElementById("effect-off") });
    outputVolume.start(document.getElementById("output-volume"));
    document.addEventListener("keydown", (event) => playNoteKey(event, true));
    document.addEventListener("keyup", (event) => playNoteKey(event, false));
    samplePlayer.listen({ onProblem: showWhyTheSampleAppIsNotRunning });
    livePlayer.begin({
      onReady: playerBecameReady,
      onValue: playerToldAValue,
      onMovedByThePlugin: playerToldOfMoves,
      onPreset: playerChosePreset,
      onSoundChanged: soundChanged,
      onFed: testSound.playerSaid,
      onProblem: (text) => addMessage(state.openName, { kind: "problem", text }),
      onEndedOnItsOwn: () => addMessage(state.openName, { kind: "problem", text: PLAYER_ENDED }),
    });
  }

  // --- Starting up ----------------------------------------------------------------------------

  // Enter sends the message. Shift with Enter starts a new line. While the AI works on the open plugin, Enter
  // has what was typed wait its turn, and does nothing else, so typing can never stop the work in progress.
  function sendOnEnter(event) {
    if (event.key !== "Enter" || event.shiftKey || event.isComposing) return;
    event.preventDefault();
    if (typedWaitsItsTurn()) return letTypedWaitItsTurn();
    if (isBeingWorkedOn(state.openName)) return;
    page.composer.requestSubmit();
  }

  function connectButtons() {
    page.check.addEventListener("click", checkOpenPlugin);
    page.play.addEventListener("click", toggleLiveSound);
    page.sendToMpc.addEventListener("click", sendOpenPluginToMpc);
    page.goBack.addEventListener("click", showVersions);
    page.widenChat.addEventListener("click", toggleWideChat);
    document.addEventListener("keydown", flipTheViewOnTab);
    page.presetBefore.addEventListener("click", () => stepPreset(-1));
    page.presetAfter.addEventListener("click", () => stepPreset(1));
    page.architect.addEventListener("change", () => remember("architect", page.architect.checked ? "on" : "off"));
    page.composer.querySelectorAll('input[name="made-for"]').forEach((side) => side.addEventListener("change", madeForChanged));
    page.composer.querySelectorAll("[data-idea]").forEach((idea) => idea.addEventListener("click", () => putInTheMessageBox(idea.dataset.idea)));
    page.feelLucky.addEventListener("click", feelLucky);
    page.remix.addEventListener("click", chooseFromTheCatalog);
    page.remixOfClear.addEventListener("click", forgetTheRemix);
    page.showGuide.addEventListener("click", showTheGuide);
    page.editLook.addEventListener("click", editTheLook);
    page.editKnobs.addEventListener("click", toggleKnobChoosing);
    page.settingsButton.addEventListener("click", () => showSettings(page.settings.hidden));
    page.settingsDone.addEventListener("click", () => showSettings(false));
    page.guidePlugin.addEventListener("click", makeGuideOfOpenPlugin);
    page.exportPlugin.addEventListener("click", exportOpenPlugin);
    page.listPlugin.addEventListener("click", listOpenPluginOnOpenMpc);
    page.importPlugin.addEventListener("click", importAPlugin);
    page.renamePlugin.addEventListener("click", renameOpenPlugin);
    page.deletePlugin.addEventListener("click", deleteOpenPlugin);
    page.message.addEventListener("keydown", sendOnEnter);
    page.message.addEventListener("input", fitMessageBox);
    page.message.addEventListener("input", drawRequestButton);
    page.composer.addEventListener("submit", (event) => {
      event.preventDefault();
      if (typedWaitsItsTurn()) return letTypedWaitItsTurn();
      if (canBeStoppedHere()) return stopWorkOn(state.openName);
      if (aiHasNoRoom() || isBeingWorkedOn(state.openName)) return;
      if (state.openName === NEW_PLUGIN) startNewPlugin(); else sendTypedChange();
    });
  }

  // The menu for the open plugin closes as soon as one of its rows is picked.
  function connectMoreMenu() {
    const moreMenu = window.popupMenu(page.moreButton, page.moreMenu);
    [page.guidePlugin, page.exportPlugin, page.listPlugin, page.renamePlugin, page.deletePlugin].forEach((row) => row.addEventListener("click", moreMenu.close));
  }

  // One plus button offers both things that can be added. Picking either closes the menu and opens its file picker.
  function connectAddMenu() {
    const whatToAdd = [page.addPictures, page.addSamples];
    const addMenu = window.popupMenu(page.addButton, page.addMenu, (isOpen) => { if (isOpen) whatToAdd[0].focus(); });
    whatToAdd.forEach((row) => row.addEventListener("click", addMenu.close));
  }

  function editorParts() {
    const part = (name) => document.getElementById(`editor-${name}`);
    return {
      window: document.getElementById("editor"),
      title: part("title"), pages: part("pages"), copyDesign: part("copy-design"), undo: part("undo"), redo: part("redo"), done: part("done"),
      options: part("options"), palette: part("palette"), view: part("view"), canvas: part("canvas"),
      layers: part("layers"), item: part("item"), shelf: part("shelf"), picker: part("picker"), hint: part("hint"), zoom: part("zoom"),
      menu: part("menu"), sheet: part("sheet"),
      comment: { about: part("comment-about"), words: part("comment-words"), send: part("comment-send") },
    };
  }

  function settingsParts() {
    const part = (id) => document.getElementById(id);
    return {
      sheet: page.appSheet, modeDark: part("mode-dark"), modeLight: part("mode-light"), startsNew: part("starts-new"), startsLast: part("starts-last"),
      versionsKept: part("versions-kept"), compactAt: part("compact-at"), motion: part("motion"), letters: part("letters"), onlinePlugins: part("online-plugins"), thisCopy: part("this-copy"),
      mpcAddress: part("mpc-address"), mpcDrive: part("mpc-drive"), mpcPasswordState: part("mpc-password-state"), mpcPassword: part("mpc-password"),
      accounts: part("ai-accounts"),
    };
  }

  function guideParts() {
    const part = (name) => document.getElementById(`guide-${name}`);
    return { guide: document.getElementById("guide"), spot: part("spot"), card: part("card"), title: part("title"), words: part("words"), count: part("count"), skip: part("skip"), back: part("back"), next: part("next") };
  }

  function whatIsNeededParts() {
    const part = (name) => document.getElementById(`needs-${name}`);
    return { list: part("list"), check: part("check"), fetch: part("fetch"), said: part("said") };
  }

  function namesOf(needs) {
    return needs.map((need) => need.name).join(" and ");
  }

  // A computer that cannot build a plugin yet is made ready without the musician doing anything, and they are
  // told what is going on. Only what MP Forge could not fetch is left to them, with where to look.
  const gettingThisComputerReady = {
    fetching(needs) {
      addMessage(NEW_PLUGIN, { kind: "note", text: `**Getting this computer ready.** MP Forge is fetching ${namesOf(needs)}, which it needs to build a plugin. This happens once, takes a minute or two and needs the network. You can describe your plugin meanwhile.` });
    },
    ready() {
      addMessage(NEW_PLUGIN, { kind: "note", text: "**This computer is ready.** Everything MP Forge needs to build a plugin is in place." });
    },
    stillMissing(needs, problem) {
      const whatWentWrong = problem ? ` ${problem}` : "";
      addMessage(NEW_PLUGIN, { kind: "problem", text: `**This computer cannot build a plugin yet.** It still needs ${namesOf(needs)}.${whatWentWrong} Open Settings to try again or to see how to get ${needs.length === 1 ? "it" : "them"}.` });
    },
  };

  function updatesParts() {
    const part = (name) => document.getElementById(`updates-${name}`);
    return { how: part("how"), check: part("check"), page: part("page"), asking: part("asking"), said: part("said"), note: document.getElementById("update-note") };
  }

  function showUpdatesInSettings() {
    showSettings(true);
    window.settingsGroups.showTheGroupOf(document.getElementById("updates-check"));
  }

  function sampleLinkParts() {
    const part = (name) => document.getElementById(`sample-link-${name}`);
    return { make: part("make"), made: part("made"), said: part("said") };
  }

  function pluginsFolderParts() {
    const part = (name) => document.getElementById(`plugins-folder-${name}`);
    return { path: part("path"), state: part("state"), show: part("show"), choose: part("choose"), asking: part("asking"), said: part("said") };
  }

  // Another folder holds other plugins, or the same ones read afresh. What was known of the old folder's is let go.
  async function pluginsAreKeptElsewhere() {
    const newPluginTalk = conversationWith(NEW_PLUGIN);
    state.conversations.clear();
    state.conversations.set(conversationKey(NEW_PLUGIN), newPluginTalk);
    state.reports.clear();
    state.editedSinceCheck.clear();
    state.contextOf.clear();
    await refreshLibrary();
    const stillThere = state.plugins.some((plugin) => plugin.name === state.openName);
    await open(stillThere ? state.openName : NEW_PLUGIN);
  }

  // The plugin the settings say to open first, if it is still there. Otherwise the app starts on a new plugin.
  function pluginToOpenFirst() {
    const wanted = appSettings.pluginToOpenFirst();
    return state.plugins.some((plugin) => plugin.name === wanted) ? wanted : NEW_PLUGIN;
  }

  async function start() {
    await aiAccounts.start(page.appSheet);
    appSettings.start(settingsParts());
    window.settingsGroups.start(document.getElementById("settings-groups"), document.getElementById("settings-shown"));
    window.pluginsFolder.start(pluginsFolderParts(), pluginsAreKeptElsewhere);
    window.sampleLink.start(sampleLinkParts());
    window.updates.start(updatesParts(), showUpdatesInSettings);
    tabStrip.start({ strip: page.tabs, earlier: document.getElementById("tabs-earlier"), later: document.getElementById("tabs-later") }, drawTabs);
    page.newPluginTab.addEventListener("click", () => openFromItsTab(NEW_PLUGIN));
    window.whatIsNeeded.start(whatIsNeededParts(), gettingThisComputerReady);
    appMenu.start(page.appMenu, rowsForWhatWasClicked);
    lookEditor.start(editorParts(), { onDone: doneEditing });
    guide.start(guideParts(), { showExample: showTheGuidesExample, showNewPlugin: backToANewPlugin });
    aiMenu.start(page.aiMenuButton, page.aiMenu);
    aiMenu.onPicked(drawPlanNote);
    page.manageUsage.addEventListener("click", () => ask("open_chatgpt_usage").catch(() => {}));
    drawPlanNote();
    sampleShelf.start(
      { button: page.addSamples, picker: page.samplePicker, list: page.sampleList },
      { onProblem: (text) => addMessage(state.openName, { kind: "problem", text }), onChanged: soundFilesChanged },
    );
    pictureTray.start(
      { button: page.addPictures, picker: page.picturePicker, list: page.pictureList, message: page.message, dropZone: page.conversation },
      { onProblem: (text) => addMessage(state.openName, { kind: "problem", text }) },
    );
    page.makerName.value = recall("maker") || "";
    page.architect.checked = recall("architect") === "on";
    page.composer.querySelector(`input[name="made-for"][value="${recall("madeFor") || "mpc"}"]`).checked = true;
    connectButtons();
    connectAddMenu();
    connectMoreMenu();
    startLivePlayer();
    on("work-progress", workProgressed);
    on("work-finished", workFinished);
    on("work-context", contextHeard);
    await refreshLibrary();
    await open(pluginToOpenFirst());
    if (state.openName === NEW_PLUGIN) guide.showOnTheFirstStart();
  }

  start();
})();
