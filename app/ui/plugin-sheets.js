// The questions asked before a plugin is renamed, deleted or sent to the MPC. Each is put on a sheet laid over
// the app, and answered by a promise.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { asked, button } = window.sheets;

  const LONGEST_NAME = 32;
  const LONGEST_ADDRESS = 253;
  const ABOUT_SENDING = "The safety check runs once more on the plugin as it is now, and only a plugin that passes is sent. Akai's own install tool does the sending. It goes into the VST2 Plugins folder of a USB drive or SD card in the MPC, never onto the MPC's built-in storage, so pulling that drive out always takes the plugin away again.";
  const ABOUT_RESTARTING = "Restart the MPC's app afterwards, so the plugin shows up. Anything unsaved on the MPC is lost.";
  const ABOUT_RENAMING = "The name is shown on the MPC. A plugin already sent to your MPC stays there under its old name until it is removed. Once renamed, the plugin is built and safety-checked again.";
  const ABOUT_ANOTHER_NAME = "The name is shown on the MPC. To the MPC a plugin is known by a hidden four-letter id, which stays the same, so if both are copies of one plugin the MPC will take them for the same one.";
  const ABOUT_IMPORTING = "Bring in a plugin somebody handed you: a file saved with Export, or the folder of a plugin's project, such as one written with Akai's SDK. It comes in as a new plugin. None of yours is replaced.";
  const ONLY_FROM_PEOPLE_YOU_TRUST = "Only bring in plugins from people you trust. A plugin is a program: its safety check builds its code and runs it on this computer. The check reads the code first and stops what it knows to be harmful, but it cannot catch everything.";
  const ABOUT_THE_GUIDE = "One page on how to use this plugin, to keep or to hand to others: a picture of its screen, what every control does, where it starts and which hardware knob turns it, its presets, and things to try. It is saved in the plugin's folder and opens in your browser, where it can be printed or saved as a PDF.";
  const GUIDE_ASKS_THE_AI = "Its words have not been written yet. Your AI is asked for them once, which uses a little of your AI credit and takes a minute or so. They are then kept with the plugin.";
  const WRITE_THE_GUIDE_AGAIN = "Have the AI write its words again. This uses a little of your AI credit.";
  const ABOUT_DELETING = "Its folder is moved to the Trash on this computer, with its sound, its screen, its pictures and its conversation. A copy already on your MPC is not touched.";

  // Answers with the name typed, or nothing when the musician thought better of it.
  function askForNewName(sheet, pluginName) {
    const name = element("input", "item-input");
    name.type = "text";
    name.maxLength = LONGEST_NAME;
    name.value = pluginName;
    name.setAttribute("aria-label", "The plugin's new name");
    const details = { title: "RENAME THIS PLUGIN", confirmWords: "RENAME", holds: [name, element("div", "soft small", ABOUT_RENAMING)], focusOn: name };
    const renaming = asked(sheet, details, () => name.value.trim());
    name.select();
    return renaming;
  }

  // Answers with another name for a plugin being brought in, because the one it has is taken, or with nothing.
  function askForAnotherName(sheet, takenName) {
    const name = element("input", "item-input");
    name.type = "text";
    name.maxLength = LONGEST_NAME;
    name.value = takenName;
    name.setAttribute("aria-label", "A name for the plugin being brought in");
    const holds = [element("div", "", `You already have a plugin called ${takenName}. Give this one another name.`), name, element("div", "soft small", ABOUT_ANOTHER_NAME)];
    const naming = asked(sheet, { title: "IMPORT A PLUGIN", confirmWords: "IMPORT", holds, focusOn: name }, () => name.value.trim());
    name.select();
    return naming;
  }

  // Two buttons of which one is always pressed. `isSecond()` says whether it is the second.
  function eitherOf(firstWords, secondWords) {
    const choices = [button("mode-button", firstWords), button("mode-button", secondWords)];
    const press = (pressed) => choices.forEach((choice) => choice.setAttribute("aria-pressed", String(choice === pressed)));
    choices.forEach((choice) => choice.addEventListener("click", () => press(choice)));
    press(choices[0]);
    const row = element("div", "sheet-choices");
    row.append(...choices);
    return { row, isSecond: () => choices[1].getAttribute("aria-pressed") === "true" };
  }

  // Answers with whether a folder is to be picked and not a file, or with nothing when the musician thought better of it.
  function askWhatToImport(sheet) {
    const kind = eitherOf("A PLUGIN FILE", "A PLUGIN'S FOLDER");
    const holds = [element("div", "", ABOUT_IMPORTING), kind.row, element("div", "sheet-problem", ONLY_FROM_PEOPLE_YOU_TRUST)];
    return asked(sheet, { title: "IMPORT A PLUGIN", confirmWords: "CHOOSE…", holds }, () => ({ asAFolder: kind.isSecond() }));
  }

  // Answers with whether the AI is to write the guide's words again, or with nothing when the musician thought
  // better of it. Words that are not written yet are always asked for, so there is nothing to tick.
  function askAboutTheGuide(sheet, wordsAreWritten) {
    const again = tick(WRITE_THE_GUIDE_AGAIN);
    const holds = [element("div", "", ABOUT_THE_GUIDE), wordsAreWritten ? again.label : element("div", "soft small", GUIDE_ASKS_THE_AI)];
    const details = { title: "A GUIDE TO THIS PLUGIN", confirmWords: wordsAreWritten ? "OPEN IT" : "WRITE IT", holds };
    return asked(sheet, details, () => ({ writeAgain: wordsAreWritten && again.box.checked }));
  }

  // Answers true only when the musician pressed DELETE.
  function askBeforeDeleting(sheet, pluginName) {
    const question = element("div", "", `Delete ${pluginName}?`);
    const details = { title: "DELETE THIS PLUGIN", confirmWords: "DELETE", holds: [question, element("div", "soft small", ABOUT_DELETING)] };
    return asked(sheet, details, () => true).then(Boolean);
  }

  function tick(words) {
    const box = element("input");
    box.type = "checkbox";
    const label = element("label", "sheet-tick");
    label.append(box, element("span", "", words));
    return { label, box };
  }

  // Answers with where to send the plugin and whether to restart the MPC's app afterwards, or with nothing.
  // The restart is asked afresh every time, and is never ticked to begin with.
  // `goingOnline` says what the plugin fetches from the internet once it is on the MPC, when it fetches anything.
  function askBeforeSending(sheet, pluginName, mpc, goingOnline) {
    const address = element("input", "item-input");
    address.type = "text";
    address.maxLength = LONGEST_ADDRESS;
    address.value = mpc.address;
    address.placeholder = "192.168.1.50";
    address.spellcheck = false;
    address.setAttribute("aria-label", "Your MPC's address");
    const restart = tick(ABOUT_RESTARTING);
    const online = goingOnline ? [element("div", "sheet-problem", goingOnline)] : [];
    const holds = [element("div", "", `Send ${pluginName} to the MPC at this address?`), address, element("div", "soft small", ABOUT_SENDING), ...online, restart.label];
    const details = { title: "SEND TO YOUR MPC", confirmWords: "SEND", holds, focusOn: address };
    return asked(sheet, details, () => ({ address: address.value.trim(), restart: restart.box.checked })).then((answer) => (answer && answer.address ? answer : null));
  }

  window.pluginSheets = { askForNewName, askForAnotherName, askWhatToImport, askAboutTheGuide, askBeforeDeleting, askBeforeSending };
})();
