// The part of the settings that says where the plugins are kept on this computer, opens that folder, and lets the
// musician choose another one. Before anything moves they are asked whether their plugins should come along.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { button } = window.sheets;

  const NOT_FOUND = "This folder cannot be found just now. If it is on a drive, plug the drive in.";
  const SAID_FOR_MS = 12000;

  let parts;
  let tellOfChange;
  let kept;

  function countOf(plugins) {
    return plugins === 1 ? "1 plugin" : `${plugins} plugins`;
  }

  function drawFolder() {
    parts.path.textContent = kept.path;
    parts.state.textContent = kept.found ? `It holds ${countOf(kept.plugins)}.` : NOT_FOUND;
    parts.show.disabled = !kept.found;
  }

  async function readFolder() {
    kept = await ask("plugins_folder");
    drawFolder();
  }

  function say(words) {
    parts.said.textContent = words;
    parts.said.hidden = !words;
    if (words) setTimeout(() => { if (parts.said.textContent === words) say(""); }, SAID_FOR_MS);
  }

  function stopAsking() {
    parts.asking.replaceChildren();
    parts.asking.hidden = true;
  }

  function whatHappened(outcome) {
    const moved = outcome.moved.length ? `Moved ${countOf(outcome.moved.length)}.` : "";
    const left = outcome.leftBehind.length ? ` ${outcome.leftBehind.join(", ")} stayed in the old folder, because the new one already holds a plugin of that name.` : "";
    return `Your plugins are now kept in ${outcome.folder.path}. ${moved}${left}`.trim();
  }

  async function keepIn(picked, bringAlong) {
    stopAsking();
    try {
      const outcome = await ask("keep_plugins_in", { folder: picked.path, bringAlong });
      kept = outcome.folder;
      drawFolder();
      say(whatHappened(outcome));
      await tellOfChange();
    } catch (refusal) {
      say(String(refusal));
      await readFolder();
    }
  }

  function answer(words, className, onPressed) {
    const made = button(className, words);
    made.addEventListener("click", onPressed);
    return made;
  }

  // Asked only when there are plugins to move. With none, the folder is simply used.
  function askAboutThePlugins(picked) {
    const alreadyThere = picked.plugins ? ` It already holds ${countOf(picked.plugins)}.` : "";
    const question = document.createElement("div");
    question.textContent = `Keep your plugins in ${picked.path} from now on?${alreadyThere}`;
    const answers = document.createElement("div");
    answers.className = "folder-answers";
    answers.append(
      answer(`MOVE MY ${countOf(kept.plugins).toUpperCase()} THERE`, "item-button done", () => keepIn(picked, true)),
      answer("LEAVE THEM WHERE THEY ARE", "item-button", () => keepIn(picked, false)),
      answer("CANCEL", "item-button", stopAsking),
    );
    parts.asking.replaceChildren(question, answers);
    parts.asking.hidden = false;
  }

  async function chooseAnother() {
    say("");
    stopAsking();
    try {
      const picked = await ask("choose_plugins_folder");
      if (!picked) return;
      if (kept.found && kept.plugins) askAboutThePlugins(picked);
      else await keepIn(picked, false);
    } catch (refusal) {
      say(String(refusal));
    }
  }

  // `pageParts` are the folder's own corner of the settings. `onChange` is told once the plugins are kept elsewhere.
  function start(pageParts, onChange) {
    parts = pageParts;
    tellOfChange = onChange;
    parts.show.addEventListener("click", () => ask("show_plugins_folder").catch((refusal) => say(String(refusal))));
    parts.choose.addEventListener("click", chooseAnother);
    return readFolder();
  }

  window.pluginsFolder = { start };
})();
