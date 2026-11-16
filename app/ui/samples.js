// The strip under the message box where the musician adds sound files for the plugin to play.
//
// A plugin that exists keeps its files in the app straight away. For a plugin that has not been made yet,
// the picked files wait here and are handed over just before it is built.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { ask, send } = window.backend;

  const IS_A_SOUND_FILE = /\.(wav|mp3)$/i;
  const NOT_A_SOUND_FILE = "Only WAV and MP3 files can be added. Convert other formats to WAV first.";

  let parts;
  let listeners;
  let pluginName = null;
  let keptFiles = [];
  let waitingFiles = [];
  let busy = false;

  function hasPlugin() {
    return pluginName !== null;
  }

  function lengthOf(sample) {
    return `${sample.seconds.toFixed(1)} s`;
  }

  // The file's bytes are the message. Its names ride along as labels, encoded so any character survives.
  function store(inPlugin, file) {
    return file.arrayBuffer().then((contents) => send("add_sample", new Uint8Array(contents), {
      "plugin-name": encodeURIComponent(inPlugin),
      "file-name": encodeURIComponent(file.name),
    }));
  }

  async function storeEach(inPlugin, files) {
    const stored = [];
    for (const file of files) {
      const kept = await store(inPlugin, file);
      stored.push(kept.fileName);
    }
    return stored;
  }

  function reportProblem(problem) {
    listeners.onProblem(typeof problem === "string" ? problem : (problem && problem.message) || "That file could not be added.");
  }

  async function addToPlugin(files) {
    try {
      await storeEach(pluginName, files);
    } catch (problem) {
      reportProblem(problem);
    }
    listeners.onChanged();
  }

  function picked(files) {
    const soundFiles = files.filter((file) => IS_A_SOUND_FILE.test(file.name));
    if (soundFiles.length < files.length) reportProblem(NOT_A_SOUND_FILE);
    if (hasPlugin()) return addToPlugin(soundFiles);
    waitingFiles.push(...soundFiles);
    draw();
  }

  async function removeFromPlugin(fileName) {
    try {
      await ask("remove_sample", { name: pluginName, fileName });
    } catch (problem) {
      reportProblem(problem);
    }
    listeners.onChanged();
  }

  function stopWaiting(file) {
    waitingFiles = waitingFiles.filter((waiting) => waiting !== file);
    draw();
  }

  function chip(label, remove) {
    const made = element("span", "sample");
    const removeButton = element("button", "sample-remove", "x");
    removeButton.type = "button";
    removeButton.disabled = busy;
    removeButton.setAttribute("aria-label", `Remove ${label}`);
    removeButton.addEventListener("click", remove);
    made.append(element("span", "sample-name", label), removeButton);
    return made;
  }

  function draw() {
    parts.button.disabled = busy;
    const kept = keptFiles.map((sample) => chip(`${sample.fileName} · ${lengthOf(sample)}`, () => removeFromPlugin(sample.fileName)));
    const waiting = waitingFiles.map((file) => chip(file.name, () => stopWaiting(file)));
    parts.list.replaceChildren(...kept, ...waiting);
  }

  // Shows the sound files of the open plugin. A name of null means a plugin that has not been made yet.
  function show(openPluginName, samples) {
    pluginName = openPluginName;
    keptFiles = samples || [];
    draw();
  }

  function setBusy(isBusy) {
    busy = isBusy;
    draw();
  }

  // Hands the waiting files to the app under the new plugin's name. Returns the names they were kept under.
  async function storeWaitingAs(newPluginName) {
    const stored = await storeEach(newPluginName, waitingFiles);
    waitingFiles = [];
    return stored;
  }

  function discard(fromPlugin, fileNames) {
    return Promise.allSettled(fileNames.map((fileName) => ask("remove_sample", { name: fromPlugin, fileName })));
  }

  function start(pageParts, pageListeners) {
    parts = pageParts;
    listeners = pageListeners;
    parts.button.addEventListener("click", () => parts.picker.click());
    parts.picker.addEventListener("change", () => {
      picked(Array.from(parts.picker.files));
      parts.picker.value = "";
    });
    draw();
  }

  window.sampleShelf = { start, show, setBusy, storeWaitingAs, discard };
})();
