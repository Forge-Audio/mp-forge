// Hearing an effect, or passing it by to hear the sound it is given as it was.
//
// The switch sits in the row that shows while an effect plays. The effect goes on working while it is passed
// by, so its echoes are where they should be when it is heard again. Every time the sound is started the
// effect is heard, so a plugin never seems to do nothing because of a switch left off.

(function () {
  "use strict";

  const livePlayer = window.livePlayer;

  let parts;
  let effectIsHeard = true;

  function draw() {
    parts.on.setAttribute("aria-pressed", String(effectIsHeard));
    parts.off.setAttribute("aria-pressed", String(!effectIsHeard));
  }

  function hear(effect) {
    effectIsHeard = effect;
    livePlayer.hearEffect(effect);
    draw();
  }

  // A player keeps what it was last told while its sound is stopped, so it is told again each time it starts.
  function follow(anEffectIsPlaying) {
    hear(effectIsHeard || !anEffectIsPlaying);
  }

  function start(switchParts) {
    parts = switchParts;
    parts.on.addEventListener("click", () => hear(true));
    parts.off.addEventListener("click", () => hear(false));
    draw();
  }

  window.effectSwitch = { start, follow };
})();
