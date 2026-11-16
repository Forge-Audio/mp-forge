// What an effect is given to work on while it plays: the built-in beat, or a sound file of the musician's own.
//
// The file stays where it is on the computer. Only where it is is remembered, and the live player reads it each
// time it is asked to play it. It is never copied into a plugin and never goes to the MPC.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { remember, recall } = window.helpers;
  const livePlayer = window.livePlayer;

  const CHOSEN = "testSound";
  const IN_USE = "testSoundInUse";
  const LONGEST_NAME_SHOWN = 28;

  let parts;
  let listeners;
  let chosen = null;
  let ownIsInUse = false;
  let runGivenTheOwnSound = null;

  function recallWhatWasChosen() {
    try {
      const kept = JSON.parse(recall(CHOSEN));
      chosen = kept && kept.path && kept.name ? kept : null;
    } catch (nothingKept) {
      chosen = null;
    }
    ownIsInUse = Boolean(chosen) && recall(IN_USE) === "yes";
  }

  function nameShown() {
    return chosen.name.length > LONGEST_NAME_SHOWN ? `${chosen.name.slice(0, LONGEST_NAME_SHOWN)}\u2026` : chosen.name;
  }

  function draw() {
    parts.beat.setAttribute("aria-pressed", String(!ownIsInUse));
    parts.own.hidden = !chosen;
    parts.own.textContent = chosen ? `YOUR SOUND: ${nameShown()}` : "";
    parts.own.title = chosen ? chosen.path : "";
    parts.own.setAttribute("aria-pressed", String(ownIsInUse));
    parts.choose.textContent = chosen ? "CHOOSE ANOTHER\u2026" : "CHOOSE A SOUND OF YOUR OWN\u2026";
  }

  // Only a player that has loaded its plugin takes a sound, so only then is it counted as having the musician's own.
  function feedThePlayer() {
    runGivenTheOwnSound = ownIsInUse && livePlayer.hasLoaded() ? livePlayer.runNumber() : null;
    livePlayer.feed(ownIsInUse ? chosen.path : null);
  }

  function use(own) {
    ownIsInUse = own;
    remember(IN_USE, own ? "yes" : "no");
    feedThePlayer();
    draw();
  }

  async function chooseASound() {
    const picked = await ask("choose_test_sound").catch((problem) => { listeners.onProblem(String(problem)); return null; });
    if (!picked) return;
    chosen = picked;
    remember(CHOSEN, JSON.stringify(picked));
    use(true);
  }

  // A player that has only just loaded an effect would start on the beat, so it is given the musician's own
  // sound at once when that is the one in use. The file is then read before the musician presses play.
  function playerBecameReady(ready) {
    if (!ready.instrument && ownIsInUse) feedThePlayer();
  }

  // The row shows while an effect plays. A player that somehow lacks the musician's own sound is given it. One
  // that already has it is left alone: reading a long file again takes a while, and it would start the sound
  // from its beginning.
  function follow(anEffectIsPlaying) {
    parts.row.hidden = !anEffectIsPlaying;
    const playerLacksTheOwnSound = runGivenTheOwnSound !== livePlayer.runNumber();
    if (anEffectIsPlaying && ownIsInUse && playerLacksTheOwnSound) feedThePlayer();
  }

  // The player says whether it took the sound it was last given. One it could not play leaves it on the beat.
  function playerSaid(wasFed) {
    if (wasFed || !ownIsInUse) return;
    ownIsInUse = false;
    remember(IN_USE, "no");
    feedThePlayer();
    draw();
  }

  function start(rowParts, pageListeners) {
    parts = rowParts;
    listeners = pageListeners;
    recallWhatWasChosen();
    parts.beat.addEventListener("click", () => use(false));
    parts.own.addEventListener("click", () => use(true));
    parts.choose.addEventListener("click", chooseASound);
    draw();
  }

  window.testSound = { start, follow, playerBecameReady, playerSaid };
})();
