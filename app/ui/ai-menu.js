// The one menu in the top bar: which model builds the plugin, how hard it thinks, how the workbench reaches it,
// and whether it may look things up online.
//
// Models are grouped by the service that offers them and can be searched across all of them. Each service's list
// is asked of the service itself, so it is always today's. Picking from a service the musician has no key for
// asks for the key first.

(function () {
  "use strict";

  const { element, remember, recall } = window.helpers;
  const aiAccounts = window.aiAccounts;

  const ANTHROPIC = "anthropic";
  // How many tokens the context of each of these Claude models holds, as Anthropic documents it.
  const CLAUDE_CONTEXT_WINDOW = 1000000;
  const CLAUDE_MODELS = [
    { id: "fable", name: "Fable 5.2", contextWindow: CLAUDE_CONTEXT_WINDOW },
    { id: "opus", name: "Opus 5.5", contextWindow: CLAUDE_CONTEXT_WINDOW },
    { id: "sonnet", name: "Sonnet 5.5", contextWindow: CLAUDE_CONTEXT_WINDOW },
    { id: "haiku", name: "Haiku 5.5", contextWindow: CLAUDE_CONTEXT_WINDOW },
  ];
  // The ChatGPT sign-in runs through Codex, which takes a reasoning level and web lookups the way Claude does.
  const CODEX = "codex";
  const EFFORT = {
    key: "effort",
    title: "Effort",
    worksWith: [ANTHROPIC, CODEX],
    choices: [
      { value: "default", label: "Default" },
      { value: "low", label: "Low" },
      { value: "medium", label: "Medium" },
      { value: "high", label: "High" },
      { value: "xhigh", label: "Extra high" },
      { value: "max", label: "Max" },
    ],
  };
  const CONNECTION = {
    key: "ai",
    title: "Connection",
    worksWith: [ANTHROPIC],
    choices: [
      { value: "claude-code", label: "Claude Code" },
      { value: "api", label: "Claude API" },
    ],
  };
  const WEB_LOOKUPS = {
    key: "web",
    title: "Web lookups",
    worksWith: [ANTHROPIC, CODEX],
    choices: [
      { value: "on", label: "On" },
      { value: "off", label: "Off" },
    ],
  };
  // Lookups through a service that charges for each one. They are a setting of their own that starts off, so
  // having lookups on for Claude never switches on ones that cost money.
  const PAID_WEB_LOOKUPS = {
    key: "paidWeb",
    title: "Web lookups",
    worksWith: [],
    choices: [
      { value: "off", label: "Off" },
      { value: "on", label: "On" },
    ],
    note: "This service charges for each lookup, about a cent, on top of what the model costs.",
  };
  const EVERY_SETTING = [EFFORT, CONNECTION, WEB_LOOKUPS, PAID_WEB_LOOKUPS];
  const WEB_CLOSED = "off";
  const UNCHOSEN = "default";
  const DIRECT_CONNECTION = "api";
  const MOST_MATCHES_SHOWN = 60;
  const NOT_FOR_THIS_MODEL = "Not for this model";
  // Models on this computer forget their instructions unless the program that runs them is given room for them.
  const ROOM_LOCAL_MODELS_NEED = "These run on this computer, with no account and nothing sent away. In Ollama's settings, give them a context length of 64000 or more, or they lose track of what they were asked.";
  const NO_LOCAL_MODELS = "No model here can use tools yet. Get one in Ollama that can, such as qwen3, then open this list again.";

  const picked = {};
  // What is known of each service's models: nothing yet, a list, or why the list could not be had.
  const listings = new Map();
  // The Claude model the workbench starts with. It is named, so the musician always sees which model builds.
  const MODEL_TO_START_WITH = { provider: ANTHROPIC, ...CLAUDE_MODELS.find((model) => model.id === "opus"), seesPictures: true };
  // The model the menu shows, which is the open plugin's.
  let pickedModel = MODEL_TO_START_WITH;
  // The model picked last, whichever plugin it was picked for. A new plugin starts with it.
  let pickedLast = MODEL_TO_START_WITH;
  // Each plugin keeps a model of its own, so one can be built by Claude while another is built by a model of
  // another service. They are kept together under this word, by the plugin's name.
  const KEPT_FOR_EACH_PLUGIN = "modelOfEachPlugin";
  // The plugin whose model the menu shows. None while a new plugin is being described.
  let pluginShown = null;
  const pickListeners = [];
  let openService = ANTHROPIC;
  let opener;
  let menu;
  let popup;
  let search;
  let modelList;

  // --- What was picked --------------------------------------------------------------------------

  function isOffered(setting, value) {
    return setting.choices.some((choice) => choice.value === value);
  }

  function restore(setting) {
    const remembered = recall(setting.key);
    picked[setting.key] = isOffered(setting, remembered) ? remembered : setting.choices[0].value;
  }

  // A Claude model that is no longer offered by name, such as the old "Default", gives way to the one started with.
  // So does a model of a service MP Forge no longer reaches.
  function isStillOffered(model) {
    if (model.provider === ANTHROPIC) return CLAUDE_MODELS.some((claude) => claude.id === model.id);
    return aiAccounts.all().length === 0 || Boolean(aiAccounts.called(model.provider));
  }

  function letGoOfAModelNoLongerOffered() {
    if (isStillOffered(pickedModel)) return;
    pickedModel = MODEL_TO_START_WITH;
    openService = ANTHROPIC;
  }

  // A Claude model's context is known here. Any other model's is whatever its service's list said when it was picked.
  function contextWindowOf(model) {
    const claude = model.provider === ANTHROPIC && CLAUDE_MODELS.find((known) => known.id === model.id);
    return claude ? claude.contextWindow : model.contextWindow || null;
  }

  // A model picked before other services were offered was kept as a bare word, which is a Claude model's.
  function restoreModel() {
    try {
      const kept = JSON.parse(recall("pickedModel"));
      if (kept && kept.provider && kept.id && isStillOffered(kept)) pickedModel = kept;
    } catch (notKeptThatWay) {
      const claude = CLAUDE_MODELS.find((model) => model.id === recall("model"));
      if (claude) pickedModel = { provider: ANTHROPIC, ...claude, seesPictures: true };
    }
    pickedModel = { ...pickedModel, contextWindow: contextWindowOf(pickedModel) };
    pickedLast = pickedModel;
    openService = pickedModel.provider;
  }

  // --- A model for each plugin ------------------------------------------------------------------

  function modelsKeptForPlugins() {
    try {
      return JSON.parse(recall(KEPT_FOR_EACH_PLUGIN)) || {};
    } catch (notKeptThatWay) {
      return {};
    }
  }

  function keepForPlugins(models) {
    remember(KEPT_FOR_EACH_PLUGIN, JSON.stringify(models));
  }

  function keepForThePluginShown() {
    if (pluginShown) keepForPlugins({ ...modelsKeptForPlugins(), [pluginShown]: pickedModel });
  }

  // The model for a new plugin is the one picked last. An open plugin has the model it was given, and one that
  // was given none takes the model picked last and keeps it from then on.
  function modelFor(pluginName) {
    const kept = pluginName && modelsKeptForPlugins()[pluginName];
    return kept && kept.provider && kept.id && isStillOffered(kept) ? kept : pickedLast;
  }

  function showModelOf(pluginName) {
    pluginShown = pluginName;
    pickedModel = modelFor(pluginName);
    keepForThePluginShown();
    openService = pickedModel.provider;
    draw(!menu.hidden);
    pickListeners.forEach((tell) => tell());
  }

  function pluginWasRenamed(oldName, newName) {
    const { [oldName]: itsModel, ...others } = modelsKeptForPlugins();
    keepForPlugins(itsModel ? { ...others, [newName]: itsModel } : others);
  }

  function pluginWasDeleted(pluginName) {
    const { [pluginName]: gone, ...others } = modelsKeptForPlugins();
    keepForPlugins(others);
  }

  // The effort levels the picked model's own service says it accepts. A Claude or Codex model takes them all.
  function effortsOfPickedModel() {
    return pickedModel.efforts || [];
  }

  // What a setting offers for the picked model. A model that names its effort levels is offered only those.
  function choicesFor(setting) {
    const namesItsLevels = setting === EFFORT && !setting.worksWith.includes(pickedModel.provider);
    if (!namesItsLevels) return setting.choices;
    return setting.choices.filter((choice) => choice.value === UNCHOSEN || effortsOfPickedModel().includes(choice.value));
  }

  // What is picked for a setting, or its first choice when the picked model does not offer what was picked.
  function choiceInForce(setting) {
    const offered = choicesFor(setting);
    return offered.find((choice) => choice.value === picked[setting.key]) || offered[0];
  }

  function labelOf(setting) {
    return choiceInForce(setting).label;
  }

  function pickedModelIsSoldLookups() {
    const service = aiAccounts.called(pickedModel.provider);
    return Boolean(service && service.sellsWebLookups);
  }

  // The web lookups setting the picked model goes by.
  function webLookups() {
    return pickedModelIsSoldLookups() ? PAID_WEB_LOOKUPS : WEB_LOOKUPS;
  }

  function appliesToPickedModel(setting) {
    if (setting === PAID_WEB_LOOKUPS) return pickedModelIsSoldLookups();
    if (setting === EFFORT && effortsOfPickedModel().length > 0) return true;
    return setting.worksWith.includes(pickedModel.provider);
  }

  // Whether the picked model will really be let onto the web.
  function webLookupsInForce() {
    const setting = webLookups();
    return appliesToPickedModel(setting) ? choiceInForce(setting).value : WEB_CLOSED;
  }

  // The effort the picked model will really be asked for.
  function effortInForce() {
    return appliesToPickedModel(EFFORT) ? choiceInForce(EFFORT).value : UNCHOSEN;
  }

  function summary() {
    return effortInForce() !== UNCHOSEN ? `${pickedModel.name} · ${labelOf(EFFORT)}` : pickedModel.name;
  }

  function closeAndReturn() {
    popup.close();
    opener.focus();
  }

  function pick(setting, value) {
    picked[setting.key] = value;
    remember(setting.key, value);
    closeAndReturn();
  }

  // The direct connection needs a key. Without one the musician is asked for it, and saying no keeps Claude Code.
  async function pickConnection(value) {
    const needsAKey = value === DIRECT_CONNECTION && !aiAccounts.called(ANTHROPIC).hasKey;
    closeAndReturn();
    if (needsAKey && !(await aiAccounts.addKey(ANTHROPIC))) return;
    pick(CONNECTION, value);
    opener.textContent = summary();
  }

  function keepModel(service, model) {
    pickedModel = { provider: service.word, id: model.id, name: model.name, seesPictures: Boolean(model.seesPictures), contextWindow: model.contextWindow || null, efforts: model.efforts || [] };
    pickedLast = pickedModel;
    remember("pickedModel", JSON.stringify(pickedLast));
    keepForThePluginShown();
    openService = service.word;
    opener.textContent = summary();
    pickListeners.forEach((tell) => tell());
  }

  // Picking a model of a service the musician has no key for asks for the key first.
  async function pickModel(service, model) {
    closeAndReturn();
    const needsAKey = service.word !== ANTHROPIC && !aiAccounts.canBeUsed(service);
    if (needsAKey && !(await aiAccounts.addKey(service.word))) return;
    keepModel(service, model);
  }

  // --- Each service's models --------------------------------------------------------------------

  // Claude's models are named in this file, so Anthropic is never asked for a list.
  function isAskedForItsModels(service) {
    return service.word !== ANTHROPIC;
  }

  function load(service) {
    if (!isAskedForItsModels(service) || listings.has(service.word)) return;
    listings.set(service.word, { loading: true });
    aiAccounts.modelsOf(service.word).then(
      (models) => { listings.set(service.word, { models }); takeNewsOfThePickedModel(service, models); },
      (problem) => listings.set(service.word, { problem: String(problem) }),
    ).then(drawModels);
  }

  // A model picked on an earlier day is brought up to date with what its service says of it today, such as
  // the effort levels it accepts.
  // The menu is only drawn again when that changes what it offers, and never under someone who is searching.
  function takeNewsOfThePickedModel(service, models) {
    const asListedToday = pickedModel.provider === service.word && models.find((model) => model.id === pickedModel.id);
    if (!asListedToday) return;
    const effortsBefore = effortsOfPickedModel().join(" ");
    const isTheOnePickedLast = pickedLast.provider === pickedModel.provider && pickedLast.id === pickedModel.id;
    pickedModel = { ...pickedModel, efforts: asListedToday.efforts || [], contextWindow: asListedToday.contextWindow || pickedModel.contextWindow };
    keepForThePluginShown();
    if (isTheOnePickedLast) {
      pickedLast = pickedModel;
      remember("pickedModel", JSON.stringify(pickedLast));
    }
    if (effortsOfPickedModel().join(" ") !== effortsBefore && !search.value) draw(!menu.hidden);
  }

  function loadEveryListThatCanBeHad() {
    aiAccounts.all().filter(aiAccounts.canListModels).forEach(load);
  }

  function modelsOf(service) {
    if (!isAskedForItsModels(service)) return CLAUDE_MODELS;
    return (listings.get(service.word) || {}).models || [];
  }

  // A key that was added or taken away changes what a service will list, so what was known of it is let go.
  function forgetListings() {
    listings.clear();
  }

  // --- Rows -------------------------------------------------------------------------------------

  function menuRow(role, className) {
    const row = element("button", `menu-row ${className || ""}`);
    row.type = "button";
    row.setAttribute("role", role);
    return row;
  }

  function isPicked(service, model) {
    return pickedModel.provider === service.word && pickedModel.id === model.id;
  }

  function modelRow(service, model, words) {
    const row = menuRow("menuitemradio", "model-row");
    row.setAttribute("aria-checked", String(isPicked(service, model)));
    row.append(element("span", "model-name", words), element("span", isPicked(service, model) ? "menu-mark" : ""));
    row.addEventListener("click", () => pickModel(service, model));
    return row;
  }

  function wordsRow(words) {
    return element("div", "menu-words soft", words);
  }

  function whatLocalModelsShow(service) {
    const models = modelsOf(service).map((model) => modelRow(service, model, model.name));
    return models.length ? [...models, wordsRow(ROOM_LOCAL_MODELS_NEED)] : [wordsRow(NO_LOCAL_MODELS)];
  }

  function whatServiceShows(service) {
    const listing = listings.get(service.word) || {};
    if (listing.loading) return [wordsRow("Asking for its models…")];
    if (listing.problem) return [wordsRow(`Its models could not be listed: ${listing.problem}`)];
    if (service.runsOnThisComputer) return whatLocalModelsShow(service);
    return modelsOf(service).map((model) => modelRow(service, model, model.name));
  }

  function noteBeside(service) {
    if (service.runsOnThisComputer) return "NO KEY NEEDED";
    if (service.word === ANTHROPIC || service.hasKey) return "";
    return service.signsIn || service.signsInWithBrowser ? "SIGN IN" : "ADD KEY";
  }

  // The models on this computer change whenever one is fetched, and a list that failed may work now.
  function shouldBeAskedAgain(service) {
    return Boolean(service.runsOnThisComputer) || Boolean((listings.get(service.word) || {}).problem);
  }

  function askAgainFor(service) {
    listings.delete(service.word);
    aiAccounts.forgetModelsOf(service.word);
  }

  // Opening a service that will not list its models without a key asks for the key. Opening any other shows its models.
  async function openOrSignIn(service) {
    if (aiAccounts.canListModels(service) || !isAskedForItsModels(service)) {
      openService = openService === service.word ? null : service.word;
      if (openService && shouldBeAskedAgain(service)) askAgainFor(service);
      load(service);
      return drawModels();
    }
    closeAndReturn();
    if (!(await aiAccounts.addKey(service.word))) return;
    openService = service.word;
    opener.click();
  }

  function serviceHeading(service) {
    const isOpen = openService === service.word;
    const row = menuRow("menuitem", "service-row");
    row.setAttribute("aria-expanded", String(isOpen));
    row.append(element("span", "", `${isOpen ? "v" : ">"} ${service.name.toUpperCase()}`), element("span", "menu-value", noteBeside(service)));
    row.addEventListener("click", () => openOrSignIn(service));
    return row;
  }

  function serviceGroup(service) {
    const isOpen = openService === service.word;
    return [serviceHeading(service), ...(isOpen ? whatServiceShows(service) : [])];
  }

  // --- Searching --------------------------------------------------------------------------------

  function matches(sought, service, model) {
    const words = `${service.name} ${model.name} ${model.id}`.toLowerCase();
    return sought.split(/\s+/).every((word) => words.includes(word));
  }

  function everyMatch(sought) {
    return aiAccounts.all().flatMap((service) => modelsOf(service).filter((model) => matches(sought, service, model)).map((model) => ({ service, model })));
  }

  function matchRows(sought) {
    const found = everyMatch(sought);
    const shown = found.slice(0, MOST_MATCHES_SHOWN).map(({ service, model }) => modelRow(service, model, `${service.name} · ${model.name}`));
    if (!found.length) return [wordsRow("No model by that name. A service's models are searched once it has a key.")];
    const more = found.length - shown.length;
    return more > 0 ? [...shown, wordsRow(`${more} more. Keep typing to narrow them down.`)] : shown;
  }

  function drawModels() {
    if (!modelList) return;
    const sought = search.value.trim().toLowerCase();
    modelList.replaceChildren(...(sought ? matchRows(sought) : aiAccounts.all().flatMap(serviceGroup)));
  }

  function pickFirstMatch(event) {
    if (event.key !== "Enter") return;
    const first = modelList.querySelector(".model-row");
    if (first) first.click();
  }

  function searchBox() {
    const box = element("input", "menu-search");
    box.type = "search";
    box.placeholder = "Search every model";
    box.setAttribute("aria-label", "Search every model");
    box.autocomplete = "off";
    box.spellcheck = false;
    box.addEventListener("input", drawModels);
    box.addEventListener("keydown", pickFirstMatch);
    return box;
  }

  // --- Effort, connection and web lookups -------------------------------------------------------

  function choiceRow(setting, choice) {
    const isChosen = choiceInForce(setting) === choice;
    const row = menuRow("menuitemradio");
    row.setAttribute("aria-checked", String(isChosen));
    row.append(element("span", "", choice.label), element("span", isChosen ? "menu-mark" : ""));
    row.addEventListener("click", () => (setting === CONNECTION ? pickConnection(choice.value) : pick(setting, choice.value)));
    return row;
  }

  function showOnlySideMenu(wanted) {
    menu.querySelectorAll(".side-menu").forEach((sideMenu) => {
      sideMenu.hidden = sideMenu !== wanted;
      sideMenu.previousElementSibling.setAttribute("aria-expanded", String(sideMenu === wanted));
    });
  }

  function toggleSideMenu(sideMenu) {
    showOnlySideMenu(sideMenu.hidden ? sideMenu : null);
    if (!sideMenu.hidden) sideMenu.querySelector(".menu-row").focus();
  }

  function sideMenuOf(setting) {
    const sideMenu = element("div", "menu side-menu");
    sideMenu.setAttribute("role", "menu");
    sideMenu.setAttribute("aria-label", setting.title);
    sideMenu.hidden = true;
    sideMenu.append(...choicesFor(setting).map((choice) => choiceRow(setting, choice)));
    if (setting.note) sideMenu.append(wordsRow(setting.note));
    return sideMenu;
  }

  // A row such as "Effort   High >" that opens its choices beside the menu. A setting the picked model's
  // service has no use for is switched off.
  function branch(setting) {
    const applies = appliesToPickedModel(setting);
    const row = menuRow("menuitem");
    row.disabled = !applies;
    row.setAttribute("aria-haspopup", "menu");
    row.setAttribute("aria-expanded", "false");
    row.append(element("span", "", setting.title), element("span", "menu-value", applies ? `${labelOf(setting)} >` : NOT_FOR_THIS_MODEL));
    const sideMenu = sideMenuOf(setting);
    row.addEventListener("click", () => toggleSideMenu(sideMenu));
    const holder = element("div", "menu-branch");
    holder.append(row, sideMenu);
    return holder;
  }

  // --- The menu itself --------------------------------------------------------------------------

  // The rows are drawn afresh each time the menu opens or closes, so a side menu never stays open behind it.
  function draw(isOpen) {
    opener.textContent = summary();
    search = searchBox();
    modelList = element("div", "menu-models");
    menu.replaceChildren(search, modelList, element("div", "menu-divider"), ...[EFFORT, CONNECTION, webLookups()].map(branch));
    drawModels();
    if (!isOpen) return;
    loadEveryListThatCanBeHad();
    search.focus();
  }

  function start(openerButton, menuBox) {
    opener = openerButton;
    menu = menuBox;
    EVERY_SETTING.forEach(restore);
    restoreModel();
    popup = window.popupMenu(opener, menu, draw);
    aiAccounts.onChange(() => { forgetListings(); letGoOfAModelNoLongerOffered(); draw(!menu.hidden); pickListeners.forEach((tell) => tell()); });
    draw(false);
  }

  function looksThingsUp() {
    return webLookupsInForce() !== WEB_CLOSED;
  }

  // What the workbench is told when a plugin is started or changed.
  function chosen() {
    const picks = { provider: pickedModel.provider, model: pickedModel.id, seesPictures: pickedModel.seesPictures, effort: effortInForce(), web: webLookupsInForce(), compactAt: window.appSettings.compactAt() };
    return { ai: picked.ai, picks };
  }

  // How many tokens the picked model's context holds, or nothing when its service did not say.
  function contextWindow() {
    return pickedModel.contextWindow || null;
  }

  // Whether the picked model runs on the musician's own ChatGPT plan, which the screen has to say while it does.
  // Signed out, nothing is using the plan.
  function usesChatGptPlan() {
    const service = aiAccounts.called(pickedModel.provider);
    return Boolean(service && service.usesChatGptPlan && service.hasKey);
  }

  window.aiMenu = {
    start, chosen, contextWindow, usesChatGptPlan, looksThingsUp, showModelOf, pluginWasRenamed, pluginWasDeleted,
    onPicked: (listener) => pickListeners.push(listener),
  };
})();
