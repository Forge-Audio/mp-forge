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

  // What differs between a Mac, Windows and Linux in what the screen says and shows: which key does what Cmd
  // does on a Mac, what the computer calls the place it keeps secrets in, and its scroll bars. A Mac shows a
  // thin dark one only while something is scrolled. The others always show theirs, wide and white, so there
  // the app draws its own.
  const SYSTEMS = [
    { runsOn: /Mac|iPhone|iPad/, commandKey: "Cmd", secretStore: "keychain", scrollBarsNeedDrawing: false },
    { runsOn: /Win/, commandKey: "Ctrl", secretStore: "Credential Manager", scrollBarsNeedDrawing: true },
    { runsOn: /./, commandKey: "Ctrl", secretStore: "keyring", scrollBarsNeedDrawing: true },
  ];
  const thisComputer = SYSTEMS.find((system) => system.runsOn.test(navigator.platform || "")) || SYSTEMS[0];
  document.body.classList.toggle("draws-scroll-bars", thisComputer.scrollBarsNeedDrawing);

  window.helpers = { element, remember, recall, thisComputer };
})();
