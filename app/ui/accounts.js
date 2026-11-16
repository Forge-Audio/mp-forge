// The musician's accounts with AI services: which have a key, the models each offers today, and adding or
// taking away a key. A key is typed straight into a sheet and handed to the app, which keeps it in the computer's own store for secrets.
// This page never sees a key again after that.
//
// Some services need no pasting: one is signed in to on its own web page, which hands the app a key, and the
// models that run on this computer have no account at all.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element, remember, recall, thisComputer } = window.helpers;
  const { asked, askForSecret, button } = window.sheets;

  const ABOUT_KEYS = `It is kept in this computer's ${thisComputer.secretStore}. It is never written into a plugin's files, and the AI is never shown it.`;
  const ONLY_FOR_THE_DIRECT_CONNECTION = "Claude Code needs no key. This one is only for the Claude API connection.";
  const HOW_TO_SIGN_IN = [
    "This uses your ChatGPT plan through Codex, OpenAI's own tool, so there is no key to paste.",
    "1. Install Codex. OpenAI's instructions are at developers.openai.com/codex.",
    "2. Open a terminal, type  codex login  and sign in with your ChatGPT account in the browser window it opens.",
    "3. Come back here and press CHECK AGAIN.",
    "You sign in with OpenAI yourself. MP Forge never sees your password. It only asks Codex whether it is signed in.",
  ];
  const HOW_THE_BROWSER_SIGN_IN_GOES = [
    "MP Forge opens the service's own page in your browser. Sign in there and approve MP Forge, and the service hands it a key of your own.",
    `You never type your password into MP Forge. The key is kept in this computer's ${thisComputer.secretStore}, and what you use is billed to your own account with the service.`,
  ];
  const ABOUT_USING_THE_PLAN = [
    "What MP Forge asks of the AI now uses your ChatGPT plan, while a ChatGPT model is picked.",
    "You can see how much it has used, and set a limit for MP Forge, in ChatGPT's settings. MANAGE USAGE under the message box opens them.",
  ];
  const SEEN_THE_PLAN_NOTICE = "seenChatGptPlanNotice";
  const WAITING_FOR_THE_BROWSER = "Finish signing in on the page that opened in your browser. This closes by itself when you are done.";
  const WHY_NOT_SIGNED_IN = { "not-installed": "Codex was not found on this computer yet.", "not-signed-in": "Codex is installed but not signed in yet." };

  let sheet;
  let known = [];
  const modelLists = new Map();
  const listeners = [];

  function all() {
    return known;
  }

  function called(word) {
    return known.find((account) => account.word === word);
  }

  function onChange(listener) {
    listeners.push(listener);
  }

  async function refresh() {
    known = await ask("ai_accounts").catch(() => known);
    listeners.forEach((tell) => tell());
  }

  // The list is asked of the service once and kept while the app is open. A failed asking is tried again next time.
  function modelsOf(word) {
    if (!modelLists.has(word)) {
      const asking = ask("models_offered", { provider: word });
      asking.catch(() => modelLists.delete(word));
      modelLists.set(word, asking);
    }
    return modelLists.get(word);
  }

  // A list that may have changed since it was asked for, such as the models on this computer, is asked for again.
  function forgetModelsOf(word) {
    modelLists.delete(word);
  }

  function canListModels(account) {
    return account.hasKey || account.listsModelsWithoutAKey;
  }

  // Models on this computer need no key. Every service does.
  function canBeUsed(account) {
    return account.hasKey || Boolean(account.runsOnThisComputer);
  }

  function wordsAboutKeyOf(account) {
    const whereToMakeOne = `Make a key at ${account.keysAreMadeAt}, then paste it here.`;
    const itsOwnNote = account.word === "anthropic" ? ` ${ONLY_FOR_THE_DIRECT_CONNECTION}` : "";
    return `${whereToMakeOne} ${ABOUT_KEYS}${itsOwnNote}`;
  }

  // Shows how to sign in to a service that is reached through its own tool, and checks again when asked to.
  // Says whether the musician is signed in by the end.
  async function signIn(word, problem) {
    const holds = [...HOW_TO_SIGN_IN.map((words) => element("div", "", words)), ...(problem ? [element("div", "sheet-problem", problem)] : [])];
    const checkAgain = await asked(sheet, { title: `SIGN IN TO ${called(word).name.toUpperCase()}`, confirmWords: "CHECK AGAIN", cancelWords: "CLOSE", wide: true, holds }, () => true);
    if (!checkAgain) return false;
    modelLists.delete(word);
    await refresh();
    return called(word).hasKey || signIn(word, WHY_NOT_SIGNED_IN[called(word).signIn]);
  }

  // Offers to sign in on the service's own page, or to paste a key after all. Answers "browser", "paste" or nothing.
  async function wayToSignIn(account, problem) {
    let confirm;
    let wantsToPaste = false;
    const paste = button("item-button", "PASTE A KEY INSTEAD");
    paste.addEventListener("click", () => { wantsToPaste = true; confirm.click(); });
    const words = (account.aboutSigningIn || HOW_THE_BROWSER_SIGN_IN_GOES).map((line) => element("div", "", line));
    const holds = [...words, ...(account.takesNoKey ? [] : [paste]), ...(problem ? [element("div", "sheet-problem", problem)] : [])];
    const details = { title: `CONNECT ${account.name.toUpperCase()}`, confirmWords: account.signInButton || "SIGN IN IN YOUR BROWSER", wide: true, holds, onMade: (made) => { confirm = made; } };
    return asked(sheet, details, () => (wantsToPaste ? "paste" : "browser"));
  }

  // Waits on a sheet while the musician signs in in their browser. The sheet closes itself when the app has the
  // key, or says why it has none. Answers with nothing when they stopped waiting.
  async function signedInInTheBrowser(account) {
    let closeSheet;
    const signingIn = ask("sign_in_with_browser", { account: account.word }).then(() => ({ signedIn: true }), (refusal) => ({ problem: String(refusal) }));
    const details = { title: `WAITING FOR ${account.name.toUpperCase()}`, cancelWords: "STOP WAITING", confirmWords: "", holds: [element("div", "", WAITING_FOR_THE_BROWSER)], onMade: (confirm) => { confirm.hidden = true; closeSheet = () => confirm.click(); } };
    let isWaiting = true;
    signingIn.then(() => { if (isWaiting) closeSheet(); });
    const finished = (await asked(sheet, details, () => true)) === true;
    isWaiting = false;
    if (finished) return signingIn;
    ask("stop_signing_in").catch(() => {});
    return null;
  }

  // Signs in on the service's own page and keeps the key it hands over. Says whether a key was kept.
  async function signInWithBrowser(word, problem) {
    const account = called(word);
    const way = await wayToSignIn(account, problem);
    if (way === "paste") return pasteKey(word);
    const outcome = way && (await signedInInTheBrowser(account));
    if (!outcome) return false;
    if (outcome.problem) return signInWithBrowser(word, outcome.problem);
    modelLists.delete(word);
    await refresh();
    if (account.usesChatGptPlan) await sayThePlanIsInUseOnce();
    return true;
  }

  // Said the first time the musician signs in with their ChatGPT plan, and never again.
  async function sayThePlanIsInUseOnce() {
    if (recall(SEEN_THE_PLAN_NOTICE)) return;
    remember(SEEN_THE_PLAN_NOTICE, "yes");
    await tell("YOU'RE USING YOUR CHATGPT PLAN", ABOUT_USING_THE_PLAN, "GOT IT");
  }

  // Shows the musician something to read, with one button to go on.
  function tell(title, lines, goOnWords) {
    const details = { title, confirmWords: goOnWords, cancelWords: "CLOSE", wide: true, holds: lines.map((words) => element("div", "", words)) };
    return asked(sheet, details, () => true);
  }

  // Connects a service in the way it is connected: its own tool's sign-in, its own web page, or a pasted key.
  // Says whether the musician can use it by the end.
  function addKey(word) {
    const account = called(word);
    if (account.signsIn) return signIn(word, WHY_NOT_SIGNED_IN[account.signIn]);
    return account.signsInWithBrowser ? signInWithBrowser(word) : pasteKey(word);
  }

  // Asks for the key and keeps it. A key the app would not keep is asked for again, with the reason shown.
  // Says whether a key was kept.
  async function pasteKey(word, problem) {
    const account = called(word);
    const details = { title: `ADD YOUR ${account.name.toUpperCase()} KEY`, confirmWords: "KEEP KEY", placeholder: "Paste the key here", words: wordsAboutKeyOf(account), problem };
    const key = await askForSecret(sheet, details);
    if (!key) return false;
    try {
      await ask("keep_key", { account: word, key });
    } catch (refusal) {
      return pasteKey(word, String(refusal));
    }
    modelLists.delete(word);
    await refresh();
    return true;
  }

  // Takes a key away, or signs out of a service that is signed in to. What could not be done fully is said.
  async function removeKey(word) {
    const notQuite = await ask("forget_key", { account: word }).then(() => null, (problem) => String(problem));
    modelLists.delete(word);
    await refresh();
    if (notQuite) await tell(called(word).name.toUpperCase(), [notQuite], "OK");
  }

  function start(sheetToAskOn) {
    sheet = sheetToAskOn;
    return refresh();
  }

  window.aiAccounts = { start, all, called, onChange, refresh, modelsOf, forgetModelsOf, canListModels, canBeUsed, addKey, removeKey };
})();
