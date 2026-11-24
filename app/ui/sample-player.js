// Runs the open Sample app on the drawing of the MPC Sample: what the musician does on the drawing is passed to
// the app, and what the app shows is fetched and drawn, about as often as a screen is drawn again.
//
// One app runs at a time. It is built and started when its plugin is opened, and stopped when another is.
// While an app is out of reach, because it is being written, built or checked, the blacksmith is shown on the
// drawing in its place, the controls go gray and nothing touched does anything.
//
// Whatever the musician would set on the device, they set in the app, on its own pages, and the computer
// follows: its sound input, its MIDI devices and a game controller stand in for what is plugged into the device. A computer has
// no SD card, so when the app opens its page for picking a file from one, a file is picked on the computer.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { readShown } = window.sampleSurface;

  const KIND = "sample-app";
  const FETCHES_EVERY = 33;

  let device = null;
  // What shows on the drawing now: which app, and whether it is the blacksmith in its place.
  let running = null;
  let stopping = Promise.resolve();
  let waitedForAFile = false;
  let listeners = { onProblem: () => {} };

  function isASampleApp(plugin) {
    return Boolean(plugin && plugin.summary.kind === KIND);
  }

  function tellTheApp(input) {
    if (running && !running.showsTheSmith) ask("sample_app_tell", { name: running.name, input }).catch(() => {});
  }

  function drawnIn(holder) {
    device = device || window.sampleDevice.make(holder, tellTheApp);
    return device;
  }

  function stop() {
    if (!running) return;
    running = null;
    stopping = ask("sample_app_stop").catch(() => {});
  }

  // The musician is asked for a file once each time the app starts to wait for one. The app's player is told
  // which they picked, or that they picked none, without the file passing through here.
  function letAFileBePickedFor(name, waitsForAFile) {
    if (waitsForAFile && !waitedForAFile) ask("sample_app_pick_sound", { name }).catch(() => {});
    waitedForAFile = waitsForAFile;
  }

  // Fetches what `run` shows for as long as it is what runs. One that has stopped says so once.
  async function keepShowing(run) {
    while (running === run) {
      const shown = readShown(await ask("sample_app_shown", { name: run.name }));
      if (running !== run) return;
      if (shown) device.show(shown);
      if (shown) letAFileBePickedFor(run.name, shown.waitsForAFile);
      await new Promise((resume) => setTimeout(resume, FETCHES_EVERY));
    }
  }

  function begin(run) {
    if (run.showsTheSmith) return ask("sample_smith_start", { name: run.name, keepsStill: window.motion.keepsStill() });
    return ask("sample_app_start", { name: run.name });
  }

  // The drawing is made before anything starts. What ran before has stopped by the time `run` begins, so the
  // one never ends the other. Once an app runs it is told where the knobs and the fader stand, since it was
  // not there to hear them moved.
  async function start(run, holder) {
    const drawn = drawnIn(holder);
    running = run;
    waitedForAFile = false;
    listeners.onProblem("");
    drawn.putOutOfReach(run.showsTheSmith);
    await stopping;
    if (running !== run) return;
    await begin(run);
    if (running !== run) return;
    if (!run.showsTheSmith) drawn.sayWhereThingsStand();
    await keepShowing(run);
  }

  function gaveUp(run, problem) {
    if (running !== run) return;
    running = null;
    listeners.onProblem(String(problem));
  }

  function isWhatRuns(wanted) {
    if (!wanted || !running) return wanted === running;
    return wanted.name === running.name && wanted.showsTheSmith === running.showsTheSmith;
  }

  // Runs `plugin` on the drawing in `holder` when it is a Sample app, and stops whatever ran before. An app
  // that `isOutOfReach` is not run: the blacksmith is shown in its place.
  function follow(plugin, holder, isOutOfReach = false) {
    const wanted = isASampleApp(plugin) ? { name: plugin.summary.name, showsTheSmith: isOutOfReach } : null;
    if (isWhatRuns(wanted)) return;
    stop();
    if (wanted) start(wanted, holder).catch((problem) => gaveUp(wanted, problem));
  }

  // Stops `name` when it is the app that runs, because its code is about to change. It is started again the
  // next time it is followed.
  function stopIfItIs(name) {
    if (running && running.name === name) stop();
  }

  function showsTheSmith() {
    return Boolean(running && running.showsTheSmith);
  }

  // `onProblem` is told why the app is not running, in words for the musician, and with no words once it is.
  function listen(heard) {
    listeners = heard;
  }

  window.gamepad.playInto(tellTheApp);

  window.samplePlayer = { isASampleApp, drawnIn, follow, stopIfItIs, showsTheSmith, listen };
})();
