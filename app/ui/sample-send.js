// Putting a Sample app on the musician's MPC Sample, where it then runs by itself with no computer.
//
// There are two ways, and both put the same folder on the device's SD card. By cable, MP Forge Link on the
// device takes the app, writes it to the card that stays in the device, and starts it. To a card, the folder
// is written onto a card in this computer. Either way the app passes its safety check first, and one that
// does not pass goes nowhere.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element } = window.helpers;
  const { asked } = window.sheets;

  const BY_CABLE = "cable";
  const TO_A_CARD = "card";
  const ABOUT_THE_CABLE = "By cable: start MP Forge Link on your MPC Sample, then send. The card stays in it.";
  const ABOUT_A_CARD = "To a card: put the card in this computer and choose it.";
  const WHAT_IT_NEEDS = "Needs the boot menu SampleBoot on the card. Tried on firmware 1.3.0.12 only. On the MPC Sample, hold SHIFT and STOP to leave the app.";
  const WHILE_IT_IS_SENT = "Sending it to your MPC Sample. If the MPC Sample asks, push its dial to allow it.";

  function whatIsAsked(appName) {
    const question = element("div", "", `${appName} then runs on your MPC Sample by itself.`);
    question.append(element("span", "experimental", "EXPERIMENTAL"));
    return question;
  }

  // Answers with the way the musician chose, or with nothing.
  function askWhichWay(sheet, appName) {
    const holds = [whatIsAsked(appName), element("div", "soft small", ABOUT_THE_CABLE), element("div", "soft small", ABOUT_A_CARD), element("div", "soft small", WHAT_IT_NEEDS)];
    const toACard = { words: "WRITE TO A CARD", answerOf: () => TO_A_CARD };
    return asked(sheet, { title: "PUT ON YOUR MPC SAMPLE", confirmWords: "SEND BY CABLE", alsoOffers: [toACard], holds, wide: true }, () => BY_CABLE);
  }

  function wordsAboutTheCard(appName, made) {
    return made.isWhereTheBootMenuLooks
      ? `**${appName} is on the card**, in ${made.folder}. Put the card back in your MPC Sample and pick ${appName} in the boot menu.`
      : `**${appName} was made** in ${made.folder}. No boot menu was found there, so copy the folder ${appName} into autorun/mods on a card that has SampleBoot.`;
  }

  function wordsAboutTheCable(appName) {
    return `**${appName} is on your MPC Sample** and running there. Hold SHIFT and STOP on it to leave the app. It is in the boot menu from now on.`;
  }

  // Each way answers with the safety check's report and, when the app got there, the words that say so.
  // `say` is told what the musician should know while they wait.
  async function sendByCable(appName, say) {
    say(WHILE_IT_IS_SENT);
    const outcome = await ask("send_sample_app_by_link", { name: appName });
    return { report: outcome.report, words: outcome.done ? wordsAboutTheCable(appName) : "" };
  }

  async function writeToACard(appName) {
    const outcome = await ask("put_sample_app_on_card", { name: appName });
    return { report: outcome.report, words: outcome.done ? wordsAboutTheCard(appName, outcome.made) : "" };
  }

  // Whether the way can be gone: a card has first to be chosen, and choosing none is deciding against it.
  function isReadyToGo(way) {
    return way === TO_A_CARD ? ask("choose_card_for_sample_app") : Promise.resolve(true);
  }

  function put(appName, way, say) {
    return way === TO_A_CARD ? writeToACard(appName) : sendByCable(appName, say);
  }

  window.sampleSend = { askWhichWay, isReadyToGo, put };
})();
