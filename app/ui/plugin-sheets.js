// The questions asked before a plugin is renamed, deleted or sent to the MPC. Each is put on a sheet laid over
// the app, and answered by a promise.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { asked } = window.sheets;

  const LONGEST_NAME = 32;
  const LONGEST_ADDRESS = 253;
  const ABOUT_SENDING = "The safety check runs once more on the plugin as it is now, and only a plugin that passes is sent. Akai's own install tool does the sending. It goes into the VST2 Plugins folder of a USB drive or SD card in the MPC, never onto the MPC's built-in storage, so pulling that drive out always takes the plugin away again.";
  const ABOUT_RESTARTING = "Restart the MPC's app afterwards, so the plugin shows up. Anything unsaved on the MPC is lost.";
  const ABOUT_RENAMING = "The name is shown on the MPC. A plugin already sent to your MPC stays there under its old name until it is removed. Once renamed, the plugin is built and safety-checked again.";
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
  function askBeforeSending(sheet, pluginName, mpc) {
    const address = element("input", "item-input");
    address.type = "text";
    address.maxLength = LONGEST_ADDRESS;
    address.value = mpc.address;
    address.placeholder = "192.168.1.50";
    address.spellcheck = false;
    address.setAttribute("aria-label", "Your MPC's address");
    const restart = tick(ABOUT_RESTARTING);
    const holds = [element("div", "", `Send ${pluginName} to the MPC at this address?`), address, element("div", "soft small", ABOUT_SENDING), restart.label];
    const details = { title: "SEND TO YOUR MPC", confirmWords: "SEND", holds, focusOn: address };
    return asked(sheet, details, () => ({ address: address.value.trim(), restart: restart.box.checked })).then((answer) => (answer && answer.address ? answer : null));
  }

  window.pluginSheets = { askForNewName, askBeforeDeleting, askBeforeSending };
})();
