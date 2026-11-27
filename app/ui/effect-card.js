// Putting an effect on the card of an MPC Sample, where the device loads it as a Knob FX.
//
// An MPC Sample takes plugins only from the folder `VST2 Plugins` on its card. The musician picks the card, and
// MP Forge copies the effect there once it has passed its safety check. Nothing else on the card is touched.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element } = window.helpers;
  const { asked } = window.sheets;

  const TO_THE_CARD = "card";
  const THE_KNOBS_FIRST = "knobs";
  const WHAT_IT_NEEDS = "Needs firmware 1.4.0 or later on the MPC Sample. On it, open MIDI Config and switch on Dev Enhancements (Beta), then Custom Plug-In Support.";
  const HOW_THE_CARD_SHOWS = "Connect the MPC Sample by USB and enter SD Card Access mode from its Project screen, or put its card in this computer. Then choose the card.";
  const NOT_TRIED_YET = "Not yet tried on a device. It follows what Akai has written about Knob FX.";

  function whatIsAsked(effectName) {
    const question = element("div", "", `${effectName} then loads on your MPC Sample as a Knob FX, played with its three knobs.`);
    question.append(element("span", "experimental", "EXPERIMENTAL"));
    return question;
  }

  // Answers with what the musician wants next: the card, the knobs first, or nothing.
  function askBeforeCopying(sheet, effectName) {
    const holds = [whatIsAsked(effectName), element("div", "soft small", HOW_THE_CARD_SHOWS), element("div", "soft small", WHAT_IT_NEEDS), element("div", "soft small", NOT_TRIED_YET)];
    const theKnobsFirst = { words: "THE SIX KNOBS", answerOf: () => THE_KNOBS_FIRST };
    return asked(sheet, { title: "EXPORT TO MPC SAMPLE", confirmWords: "CHOOSE THE CARD", alsoOffers: [theKnobsFirst], holds, wide: true }, () => TO_THE_CARD);
  }

  function wordsWhenOnTheCard(effectName, onTheCard) {
    const how = onTheCard.replaced ? "replaced the copy that was there" : "is on the card";
    return `**${effectName} ${how}**, in ${onTheCard.folder}. Eject the card on this computer first. Then leave SD Card Access mode on the MPC Sample, or put the card back in it. ${effectName} is at the bottom of its FX Select list.`;
  }

  // Lets the musician pick the card, then copies the effect there. Answers with the safety check's report and,
  // when the effect got there, the words that say so. Nothing when they picked no card.
  async function put(effectName) {
    if (!(await ask("choose_card_for_sample_app"))) return null;
    const outcome = await ask("put_effect_on_card", { name: effectName });
    return { report: outcome.report, words: outcome.done ? wordsWhenOnTheCard(effectName, outcome.onTheCard) : "" };
  }

  window.effectCard = { TO_THE_CARD, THE_KNOBS_FIRST, askBeforeCopying, put };
})();
