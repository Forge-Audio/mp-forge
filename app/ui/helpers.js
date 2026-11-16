// Small things every part of the workbench screen needs.

(function () {
  "use strict";

  // Page elements are always built this way, never from markup text, so no words can run as code.
  function element(tag, className, text) {
    const made = document.createElement(tag);
    if (className) made.className = className;
    if (text !== undefined) made.textContent = text;
    return made;
  }

  function remember(key, value) {
    try { localStorage.setItem(key, value); } catch (unavailable) { /* the choice just is not kept */ }
  }

  function recall(key) {
    try { return localStorage.getItem(key); } catch (unavailable) { return null; }
  }

  // What differs between a Mac, Windows and Linux in what the screen says: which key does what Cmd does on a
  // Mac, and what the computer calls the place it keeps secrets in.
  const SYSTEMS = [
    { runsOn: /Mac|iPhone|iPad/, commandKey: "Cmd", secretStore: "keychain" },
    { runsOn: /Win/, commandKey: "Ctrl", secretStore: "Credential Manager" },
    { runsOn: /./, commandKey: "Ctrl", secretStore: "keyring" },
  ];
  const thisComputer = SYSTEMS.find((system) => system.runsOn.test(navigator.platform || "")) || SYSTEMS[0];

  window.helpers = { element, remember, recall, thisComputer };
})();
