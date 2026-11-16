// How loud MP Forge plays, set with one knob beside the plugin's presets.
//
// It is the same for every plugin and is remembered. Only this computer's speakers follow it: it is no part
// of any plugin and never goes to the MPC.

(function () {
  "use strict";

  const { element, remember, recall } = window.helpers;
  const { pixelKnob, letItBeTurned } = window.pluginScreen;
  const livePlayer = window.livePlayer;

  const KEPT_AS = "outputVolume";
  const USUAL = 0.9;

  let holder;
  let amount;

  function withinTheKnob(wanted) {
    return Math.min(1, Math.max(0, wanted));
  }

  function recalled() {
    const kept = Number.parseFloat(recall(KEPT_AS));
    return Number.isFinite(kept) ? withinTheKnob(kept) : USUAL;
  }

  function draw() {
    const percent = Math.round(amount * 100);
    holder.replaceChildren(pixelKnob(amount), element("span", "volume-reading", String(percent)));
    holder.title = `Volume ${percent}%. Drag up or down to turn it. Double-click puts it back.`;
    holder.setAttribute("aria-valuenow", String(percent));
    holder.setAttribute("aria-valuetext", `${percent}%`);
  }

  function tellThePlayer() {
    livePlayer.setVolume(amount);
  }

  function turnTo(wanted) {
    amount = withinTheKnob(wanted);
    remember(KEPT_AS, String(amount));
    tellThePlayer();
    draw();
  }

  function start(knobHolder) {
    holder = knobHolder;
    amount = recalled();
    letItBeTurned(holder, { name: "Volume", amount: () => amount, turnTo, resting: USUAL }, { handle: holder });
    draw();
  }

  window.outputVolume = { start, tellThePlayer };
})();
