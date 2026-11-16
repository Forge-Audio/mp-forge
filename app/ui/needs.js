// The part of the settings that says what MP Forge needs on this computer besides itself, whether each thing is
// there, and how to get one that is not. Akai's build tools it can fetch itself.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element } = window.helpers;

  const LOOKING = "Looking…";
  const FETCHING = "Fetching Akai's build tools. This takes a minute or so, and needs the network.";
  const COULD_NOT_LOOK = "MP Forge could not look just now.";
  const MARK = { there: "OK", requiredAndMissing: "MISSING", missing: "NOT FOUND" };

  let parts;

  function markOf(need) {
    if (need.isThere) return MARK.there;
    return need.isRequired ? MARK.requiredAndMissing : MARK.missing;
  }

  // Only what a plugin cannot be made without stands out when it is missing.
  function standingOf(need) {
    if (need.isThere) return "is-there";
    return need.isRequired ? "is-missing" : "is-not-found";
  }

  // What is there is only said to be there. What is missing is followed by the way to get it.
  function row(need) {
    const made = element("div", `need ${standingOf(need)}`);
    const words = element("div", "need-words");
    words.append(element("div", "", need.name), element("div", "soft small", need.isThere ? need.usedFor : `${need.usedFor} ${need.howToGet}`));
    made.append(element("span", "need-mark", markOf(need)), words);
    return made;
  }

  function say(words) {
    parts.said.textContent = words;
    parts.said.hidden = !words;
  }

  function draw(needs) {
    parts.list.replaceChildren(...needs.map(row));
    parts.fetch.hidden = !needs.some((need) => need.canBeFetched && !need.isThere);
  }

  function describe(problem) {
    return typeof problem === "string" ? problem : COULD_NOT_LOOK;
  }

  // Answers with what a plugin cannot be made without and this computer still lacks.
  async function look() {
    say(LOOKING);
    try {
      const needs = await ask("what_is_needed");
      draw(needs);
      say("");
      return needs.filter((need) => need.isRequired && !need.isThere);
    } catch (problem) {
      say(describe(problem));
      return [];
    }
  }

  async function fetchTheBuildTools() {
    parts.fetch.disabled = true;
    say(FETCHING);
    const problem = await ask("fetch_build_tools").then(() => "", describe);
    parts.fetch.disabled = false;
    await look();
    say(problem);
  }

  // `tellOfMissing` hears, once, what was found missing when the app started.
  function start(pageParts, tellOfMissing) {
    parts = pageParts;
    parts.check.addEventListener("click", look);
    parts.fetch.addEventListener("click", fetchTheBuildTools);
    look().then((missing) => { if (missing.length) tellOfMissing(missing); });
  }

  window.whatIsNeeded = { start };
})();
