// The part of the settings that keeps MP Forge up to date. GitHub is asked which version is the newest when the
// app opens and whenever CHECK FOR UPDATES is pressed. A newer one is fetched and installed only when the musician
// says yes. One found when the app opens is told of quietly, by a note in the bar at the top that leads here.

(function () {
  "use strict";

  const { ask, on } = window.backend;
  const { button } = window.sheets;

  const CAN_UPDATE_ITSELF = "byItself";
  const BUILT_FROM_SOURCE = "fromSource";
  const HOW_A_COPY_UPDATES = {
    [CAN_UPDATE_ITSELF]: "This copy can update itself.",
    asAPackage: "This copy was installed as a package, so your computer asks for an administrator's password when it updates itself.",
    [BUILT_FROM_SOURCE]: "This copy was built from source, so it cannot update itself. Fetch the newer source and build it again, or get an installer from the download page.",
  };
  const NEWEST = "This is the newest version.";
  const NOTHING_FOR_THIS_COPY = "No update could be found for this copy. The newest version is always on the download page.";
  const WHAT_UPDATING_DOES = "Updating fetches it, checks that it is MP Forge's own, installs it and opens MP Forge again.";

  let parts;
  let howThisCopyUpdates = CAN_UPDATE_ITSELF;
  let newerVersion = null;

  function say(words) {
    parts.said.textContent = words;
    parts.said.hidden = !words;
  }

  // What is on offer, or how far it has come, with the buttons that answer it.
  function putToTheMusician(words, answers) {
    const question = document.createElement("div");
    question.textContent = words;
    const row = document.createElement("div");
    row.className = "folder-answers";
    row.append(...answers);
    parts.asking.replaceChildren(question, row);
    parts.asking.hidden = false;
  }

  function stopAsking() {
    parts.asking.replaceChildren();
    parts.asking.hidden = true;
  }

  function answer(words, className, onPressed) {
    const made = button(className, words);
    made.addEventListener("click", onPressed);
    return made;
  }

  // The note in the bar is shown for as long as a newer version is on offer and has not been turned down.
  function drawNote() {
    parts.note.textContent = newerVersion ? `VERSION ${newerVersion} IS OUT` : "";
    parts.note.title = newerVersion ? `Version ${newerVersion} of MP Forge is out. Press to see it in Settings.` : "";
    parts.note.hidden = !newerVersion;
  }

  function notNow() {
    newerVersion = null;
    drawNote();
    stopAsking();
  }

  function offerTheNewerVersion() {
    if (howThisCopyUpdates === BUILT_FROM_SOURCE) return say(`Version ${newerVersion} is out. ${HOW_A_COPY_UPDATES[BUILT_FROM_SOURCE]}`);
    const whatItTakes = howThisCopyUpdates === CAN_UPDATE_ITSELF ? "" : " Your computer will ask for an administrator's password.";
    putToTheMusician(`Version ${newerVersion} is out. ${WHAT_UPDATING_DOES}${whatItTakes}`, [
      answer("UPDATE AND RESTART", "item-button done", install),
      answer("NOT NOW", "item-button", notNow),
    ]);
  }

  // While the update is fetched and installed there is nothing to answer, only how far it has come.
  function sayHowFar(words) {
    putToTheMusician(words, []);
  }

  function progressed(progress) {
    if (!newerVersion) return;
    sayHowFar(progress.installing ? `Installing version ${newerVersion}. MP Forge opens again in a moment.` : `Fetching version ${newerVersion}: ${progress.fetched}%`);
  }

  // The app opens again once the update is in, so this only ever comes back when it could not be installed.
  async function install() {
    say("");
    sayHowFar(`Fetching version ${newerVersion}.`);
    parts.check.disabled = true;
    try {
      await ask("install_update");
    } catch (refusal) {
      offerTheNewerVersion();
      say(String(refusal));
    } finally {
      parts.check.disabled = false;
    }
  }

  async function lookForANewerVersion() {
    const found = await ask("newest_version");
    newerVersion = found.standing === "newer" ? found.version : null;
    drawNote();
    return found.standing;
  }

  function tellWhatWasFound(standing) {
    if (newerVersion) offerTheNewerVersion();
    else say(standing === "newest" ? NEWEST : NOTHING_FOR_THIS_COPY);
  }

  async function check() {
    say("");
    stopAsking();
    parts.check.disabled = true;
    try {
      tellWhatWasFound(await lookForANewerVersion());
    } catch (refusal) {
      say(String(refusal));
    } finally {
      parts.check.disabled = false;
    }
  }

  // When the app opens, only a newer version is worth a word. Anything else, a computer that is offline included,
  // is passed over in silence.
  async function lookQuietly() {
    await lookForANewerVersion().catch(() => null);
    if (newerVersion) offerTheNewerVersion();
  }

  async function sayHowThisCopyUpdates() {
    const copy = await ask("this_copy").catch(() => null);
    if (copy && HOW_A_COPY_UPDATES[copy.updating]) howThisCopyUpdates = copy.updating;
    parts.how.textContent = HOW_A_COPY_UPDATES[howThisCopyUpdates];
  }

  // `pageParts` are the updates' own corner of the settings and the note in the bar. `openSettingsHere` shows
  // that corner.
  async function start(pageParts, openSettingsHere) {
    parts = pageParts;
    parts.check.addEventListener("click", check);
    parts.page.addEventListener("click", () => ask("open_download_page").catch((refusal) => say(String(refusal))));
    parts.note.addEventListener("click", openSettingsHere);
    on("update-progress", progressed);
    await sayHowThisCopyUpdates();
    await lookQuietly();
  }

  window.updates = { start };
})();
