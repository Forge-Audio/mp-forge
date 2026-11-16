// The part of the settings that says what MP Forge needs on this computer besides itself, whether each thing is
// there, and how to get one that is not. Python and Akai's build tools it fetches itself: when the app opens and
// either is missing, it is fetched without the musician being asked to do anything.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element } = window.helpers;

  const LOOKING = "Looking…";
  const FETCHING = "Fetching what is missing. This takes a minute or two, and needs the network.";
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

  // Answers with what is still missing afterwards, and with what went wrong if anything did.
  async function fetchWhatIsMissing() {
    parts.fetch.disabled = true;
    say(FETCHING);
    const problem = await ask("fetch_what_is_missing").then(() => "", describe);
    parts.fetch.disabled = false;
    const stillMissing = await look();
    say(problem);
    return { stillMissing, problem };
  }

  // What is missing when the app opens is fetched there and then. `hear` is told as it goes: what is being
  // fetched, and then either that the computer is ready or what it still lacks.
  async function makeThisComputerReady(hear) {
    const missing = await look();
    const toFetch = missing.filter((need) => need.canBeFetched);
    if (!missing.length) return;
    if (!toFetch.length) return hear.stillMissing(missing, "");
    hear.fetching(toFetch);
    const { stillMissing, problem } = await fetchWhatIsMissing();
    if (stillMissing.length) hear.stillMissing(stillMissing, problem); else hear.ready();
  }

  function start(pageParts, hear) {
    parts = pageParts;
    parts.check.addEventListener("click", look);
    parts.fetch.addEventListener("click", fetchWhatIsMissing);
    makeThisComputerReady(hear);
  }

  window.whatIsNeeded = { start };
})();
