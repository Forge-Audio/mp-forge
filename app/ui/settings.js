// The settings page: the colour mode, the font the app is written in, whether the app moves, what the app opens
// when it starts, how many versions are kept, when the AI's context is compacted, where the musician's MPC is, and
// their accounts with AI services.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element, remember, recall, thisComputer } = window.helpers;
  const { askForSecret, button } = window.sheets;
  const aiAccounts = window.aiAccounts;

  const OPENS_WITH_NEW = "new";
  const OPENS_WITH_LAST = "last";
  const MPC_ACCOUNT = "mpc";
  const ABOUT_THE_MPC_PASSWORD = `Only needed when your MPC asks for one. It is kept in this computer's ${thisComputer.secretStore} and handed to Akai's install tool when a plugin is sent.`;

  const KEEP_EVERY_VERSION = "all";
  const VERSIONS_KEPT = { key: "versionsKept", choices: [[KEEP_EVERY_VERSION, "ALL OF THEM"], ["50", "THE NEWEST 50"], ["25", "THE NEWEST 25"], ["10", "THE NEWEST 10"]] };
  const MODELS_OWN_LIMIT = "model";
  const COMPACT_AT = { key: "compactAt", choices: [[MODELS_OWN_LIMIT, "THE MODEL'S OWN LIMIT"], ["400000", "400K TOKENS"], ["200000", "200K TOKENS"], ["100000", "100K TOKENS"]] };

  // Off until the musician switches it on. The app is told, because its safety check has to know.
  const ONLINE_PLUGINS_OFF = "off";
  const ONLINE_PLUGINS = { key: "onlinePlugins", choices: [[ONLINE_PLUGINS_OFF, "OFF"], ["on", "ON"]], whenPicked: tellTheAppAboutOnlinePlugins };

  let parts;
  let mpcHasPassword = false;

  // --- A setting picked from a few choices -------------------------------------------------------

  // What was picked for a setting. Until something is, it is the first of its choices.
  function picked(setting) {
    const remembered = recall(setting.key);
    return setting.choices.some(([value]) => value === remembered) ? remembered : setting.choices[0][0];
  }

  function drawChoices(holder, setting) {
    const choiceButton = ([value, words]) => {
      const made = button("mode-button", words);
      made.setAttribute("aria-pressed", String(picked(setting) === value));
      made.addEventListener("click", () => { remember(setting.key, value); drawChoices(holder, setting); setting.whenPicked?.(); });
      return made;
    };
    holder.replaceChildren(...setting.choices.map(choiceButton));
  }

  // The most versions to keep of a plugin, or nothing when every one is kept.
  function mostVersionsKept() {
    return picked(VERSIONS_KEPT) === KEEP_EVERY_VERSION ? null : Number(picked(VERSIONS_KEPT));
  }

  // How many tokens the AI's context may reach before older material is dropped, or nothing to leave it to the model.
  function compactAt() {
    return picked(COMPACT_AT) === MODELS_OWN_LIMIT ? null : Number(picked(COMPACT_AT));
  }

  // --- Colour mode ----------------------------------------------------------------------------

  function useMode(mode) {
    document.body.dataset.theme = mode;
    parts.modeDark.setAttribute("aria-pressed", String(mode === "dark"));
    parts.modeLight.setAttribute("aria-pressed", String(mode === "light"));
    remember("mode", mode);
  }

  // --- What the app opens when it starts -------------------------------------------------------

  function opensWith() {
    return recall("startup") === OPENS_WITH_LAST ? OPENS_WITH_LAST : OPENS_WITH_NEW;
  }

  function openWith(choice) {
    remember("startup", choice);
    parts.startsNew.setAttribute("aria-pressed", String(choice === OPENS_WITH_NEW));
    parts.startsLast.setAttribute("aria-pressed", String(choice === OPENS_WITH_LAST));
  }

  function noteOpened(pluginName) {
    remember("lastOpen", pluginName || "");
  }

  // The plugin to open first, or nothing when the app is to start on a new plugin.
  function pluginToOpenFirst() {
    return opensWith() === OPENS_WITH_LAST ? recall("lastOpen") || null : null;
  }

  // --- The musician's MPC ----------------------------------------------------------------------

  function mpc() {
    return { address: recall("mpcAddress") || "", drive: recall("mpcDrive") || "" };
  }

  function rememberMpcAddress(address) {
    remember("mpcAddress", address);
    parts.mpcAddress.value = address;
  }

  function drawMpcPassword() {
    parts.mpcPasswordState.textContent = mpcHasPassword ? `Kept in the ${thisComputer.secretStore}` : "None kept";
    parts.mpcPassword.textContent = mpcHasPassword ? "REMOVE" : "ADD PASSWORD";
  }

  async function readMpcPassword() {
    mpcHasPassword = await ask("mpc_password_is_kept").catch(() => false);
    drawMpcPassword();
  }

  async function addMpcPassword(problem) {
    const details = { title: "YOUR MPC'S PASSWORD", confirmWords: "KEEP IT", placeholder: "Type the password here", words: ABOUT_THE_MPC_PASSWORD, problem };
    const password = await askForSecret(parts.sheet, details);
    if (!password) return;
    try {
      await ask("keep_key", { account: MPC_ACCOUNT, key: password });
    } catch (refusal) {
      return addMpcPassword(String(refusal));
    }
    await readMpcPassword();
  }

  async function changeMpcPassword() {
    if (!mpcHasPassword) return addMpcPassword();
    await ask("forget_key", { account: MPC_ACCOUNT }).catch(() => {});
    await readMpcPassword();
  }

  // --- AI accounts -----------------------------------------------------------------------------

  // A service that is signed in to through its own tool has no key here. Signing out is done in that tool.
  function signInRow(account) {
    const row = element("div", "account");
    const check = button("item-button", account.hasKey ? "CHECK AGAIN" : "HOW TO SIGN IN");
    check.addEventListener("click", () => (account.hasKey ? aiAccounts.refresh() : aiAccounts.addKey(account.word)));
    row.append(element("span", "account-name", account.name), element("span", "soft", account.hasKey ? "Signed in" : "Not signed in"), check);
    return row;
  }

  // Models that run on this computer have no account, so there is nothing to add or take away.
  function localRow(account) {
    const row = element("div", "account");
    row.append(element("span", "account-name", account.name), element("span", "soft", "No account or key needed"));
    return row;
  }

  // A service signed in to on its own page has no key to speak of: it is signed in to, and signed out of.
  function wordsToChange(account) {
    if (account.hasKey) return account.takesNoKey ? "SIGN OUT" : "REMOVE KEY";
    return account.signsInWithBrowser ? account.signInButton || "SIGN IN" : "ADD KEY";
  }

  function standingOf(account) {
    if (account.takesNoKey) return account.hasKey ? "Signed in" : "Not signed in";
    return account.hasKey ? "Key kept" : "No key";
  }

  function accountRow(account) {
    if (account.signsIn) return signInRow(account);
    if (account.runsOnThisComputer) return localRow(account);
    const row = element("div", "account");
    const change = button("item-button", wordsToChange(account));
    change.addEventListener("click", () => (account.hasKey ? aiAccounts.removeKey(account.word) : aiAccounts.addKey(account.word)));
    row.append(element("span", "account-name", account.name), element("span", "soft", standingOf(account)), change);
    return row;
  }

  function drawAccounts() {
    parts.accounts.replaceChildren(...aiAccounts.all().map(accountRow));
  }

  // --- Which copy this is -----------------------------------------------------------------------

  // Copies of one version can differ, so the commit a copy was built from is said beside its version.
  function wordsFor(copy) {
    return copy.builtFrom ? `Version ${copy.version} · build ${copy.builtFrom}` : `Version ${copy.version}`;
  }

  async function sayWhichCopyThisIs() {
    const copy = await ask("this_copy").catch(() => null);
    if (!copy) return;
    parts.thisCopy.textContent = wordsFor(copy);
    parts.thisCopy.hidden = false;
  }

  function onlinePluginsAreOn() {
    return picked(ONLINE_PLUGINS) !== ONLINE_PLUGINS_OFF;
  }

  function tellTheAppAboutOnlinePlugins() {
    return ask("allow_online_plugins", { allowed: onlinePluginsAreOn() });
  }

  // --- Starting up ----------------------------------------------------------------------------

  function start(pageParts) {
    parts = pageParts;
    useMode(recall("mode") === "light" ? "light" : "dark");
    openWith(opensWith());
    drawChoices(parts.versionsKept, VERSIONS_KEPT);
    drawChoices(parts.compactAt, COMPACT_AT);
    drawChoices(parts.motion, window.motion.SETTING);
    drawChoices(parts.letters, window.letters.SETTING);
    drawChoices(parts.onlinePlugins, ONLINE_PLUGINS);
    tellTheAppAboutOnlinePlugins();
    parts.mpcAddress.value = mpc().address;
    parts.mpcDrive.value = mpc().drive;
    parts.modeDark.addEventListener("click", () => useMode("dark"));
    parts.modeLight.addEventListener("click", () => useMode("light"));
    parts.startsNew.addEventListener("click", () => openWith(OPENS_WITH_NEW));
    parts.startsLast.addEventListener("click", () => openWith(OPENS_WITH_LAST));
    parts.mpcAddress.addEventListener("change", () => remember("mpcAddress", parts.mpcAddress.value.trim()));
    parts.mpcDrive.addEventListener("change", () => remember("mpcDrive", parts.mpcDrive.value.trim()));
    parts.mpcPassword.addEventListener("click", changeMpcPassword);
    aiAccounts.onChange(drawAccounts);
    drawAccounts();
    readMpcPassword();
    sayWhichCopyThisIs();
  }

  window.appSettings = { start, mpc, rememberMpcAddress, noteOpened, pluginToOpenFirst, mostVersionsKept, compactAt, onlinePluginsAreOn };
})();
