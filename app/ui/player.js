// The live player: the real, built plugin running beside the app, so its controls read their true values
// and it can be heard through the computer's speakers.
//
// One player runs at a time. It is started the first time it is needed for a plugin's current build, and
// stopped when another plugin is opened or the plugin is built again.

(function () {
  "use strict";

  const { ask, on } = window.backend;

  let listeners;
  let running = null;
  let runsStarted = 0;

  function isFor(plugin) {
    return running !== null && running.name === plugin.summary.name && running.builtAt === plugin.builtAt;
  }

  function stop() {
    if (!running) return;
    running = null;
    ask("player_stop").catch(() => {});
    listeners.onSoundChanged(false);
  }

  // The player says "ready" once the plugin is loaded. Until then, requests wait on this promise.
  function start(plugin) {
    let settle;
    const ready = new Promise((resolve, reject) => { settle = { resolve, reject }; });
    ready.catch(() => {});
    runsStarted += 1;
    running = { run: runsStarted, name: plugin.summary.name, builtAt: plugin.builtAt, ready, settle, loaded: false, sounding: false, instrument: false };
    ask("player_start", { name: running.name, run: running.run }).catch(settle.reject);
    return ready;
  }

  function ensureRunningFor(plugin) {
    if (isFor(plugin)) return running.ready;
    stop();
    return start(plugin);
  }

  function tell(plugin, request) {
    return ensureRunningFor(plugin).then(() => ask("player_tell", { request }));
  }

  function becameReady(ready) {
    running.loaded = true;
    running.instrument = ready.instrument;
    running.settle.resolve(ready);
    listeners.onReady(ready);
  }

  function soundChanged(sounding) {
    running.sounding = sounding;
    listeners.onSoundChanged(sounding);
  }

  // A player that ends without being stopped was most likely brought down by the plugin.
  function endedOnItsOwn() {
    running.settle.reject("The plugin could not be played.");
    running = null;
    listeners.onSoundChanged(false);
    listeners.onEndedOnItsOwn();
  }

  function heard(event) {
    if (!running || event.run !== running.run) return;
    const said = event.said;
    if (said.ready) becameReady(said.ready);
    if (said.value) listeners.onValue(said.value);
    if (said.moved) listeners.onMovedByThePlugin(said.moved);
    if (said.preset) listeners.onPreset(said.preset);
    if (said.sound !== undefined) soundChanged(said.sound);
    if (said.fed !== undefined) listeners.onFed(said.fed);
    if (said.problem) listeners.onProblem(said.problem);
    if (said.ended) endedOnItsOwn();
  }

  // Stops the player when the plugin on screen is no longer the one, or the build, it is playing.
  function follow(plugin) {
    if (running && !(plugin && isFor(plugin))) stop();
  }

  function turn(plugin, index, amount) {
    tell(plugin, { turn: { index, amount } }).catch(() => {});
  }

  // Loads the plugin without making a sound, and answers with what the player said of it once it was ready:
  // whether it is an instrument, and the presets built into it.
  function prepare(plugin) {
    return ensureRunningFor(plugin);
  }

  // Switches the plugin to one of its presets. The player answers with where every control then stands.
  function choosePreset(plugin, index) {
    return tell(plugin, { preset: index });
  }

  function setSound(plugin, sounding) {
    return tell(plugin, { sound: sounding });
  }

  function tellWhileSounding(request) {
    if (isSounding()) ask("player_tell", { request }).catch(() => {});
  }

  function note(pitch, pressed) {
    tellWhileSounding({ note: { pitch, pressed } });
  }

  // Gives an effect the sound file at `file` to work on, or the built-in beat again when there is none. A player
  // that has loaded its plugin takes it whether or not it is sounding yet, so a long file is read before it is
  // wanted.
  function feed(file) {
    if (hasLoaded()) ask("player_tell", { request: { feed: file } }).catch(() => {});
  }

  // Hears the effect, or passes it by so that what it is given is heard as it was.
  function hearEffect(heard) {
    tellWhileSounding({ effect: heard });
  }

  // How loud everything plays, from 0 for silence to 1 for full. A player that is still loading is told once
  // it is ready, and no player is started for it.
  function setVolume(amount) {
    if (running) running.ready.then(() => ask("player_tell", { request: { volume: amount } })).catch(() => {});
  }

  function isSounding() {
    return running !== null && running.sounding;
  }

  function hasLoaded() {
    return running !== null && running.loaded;
  }

  // Which start of the player this is, counted from one, or none when no player runs. A player keeps what it
  // was told for as long as it runs, and one that has just started knows none of it.
  function runNumber() {
    return running === null ? null : running.run;
  }

  function isPlayingAnInstrument() {
    return isSounding() && running.instrument;
  }

  function begin(pageListeners) {
    listeners = pageListeners;
    on("player-event", heard);
  }

  window.livePlayer = { begin, follow, prepare, turn, choosePreset, setSound, setVolume, note, feed, hearEffect, isSounding, hasLoaded, isPlayingAnInstrument, runNumber };
})();
