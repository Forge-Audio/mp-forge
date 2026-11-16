// The app's own right-click menu, in place of the one a browser would show, and the browser's own shortcuts
// for reloading the page and opening its inspector switched off. Neither belongs in an app.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { openMenu } = window.editorMenu;

  const DIVIDER = null;
  // Letters that, with Command or Control, reload the page or open the browser's inspector.
  const RELOADS = ["KeyR"];
  const INSPECTS = ["KeyI", "KeyJ", "KeyC", "KeyU"];
  const KEYS_FOR_THE_SAME = ["F5", "F12"];

  function row(words, does, isOff) {
    return { words, does, isOff: Boolean(isOff) };
  }

  // --- A box for typing gets cut, copy, paste and select all -----------------------------------

  function isForTyping(target) {
    return target instanceof HTMLElement && target.matches("textarea, input[type='text'], input[type='search'], input[type='password'], input:not([type])") && !target.disabled;
  }

  function hasSelection(field) {
    return field.selectionStart !== field.selectionEnd;
  }

  function doToSelection(field, command) {
    field.focus();
    document.execCommand(command);
  }

  // What is pasted takes the place of what is selected, and the box is told, so it grows to fit.
  async function pasteInto(field) {
    const words = await ask("clipboard_words").catch(() => "");
    if (!words) return;
    field.focus();
    field.setRangeText(words, field.selectionStart, field.selectionEnd, "end");
    field.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function rowsForTyping(field) {
    const holdsASecret = field.type === "password";
    return [
      row("Cut", () => doToSelection(field, "cut"), !hasSelection(field) || holdsASecret),
      row("Copy", () => doToSelection(field, "copy"), !hasSelection(field) || holdsASecret),
      row("Paste", () => pasteInto(field)),
      DIVIDER,
      row("Select all", () => { field.focus(); field.select(); }, !field.value),
    ];
  }

  // --- The browser's own shortcuts -------------------------------------------------------------

  function wouldReloadOrInspect(event) {
    const withCommand = event.metaKey || event.ctrlKey;
    const reloads = withCommand && RELOADS.includes(event.code);
    const inspects = withCommand && (event.altKey || event.shiftKey) && INSPECTS.includes(event.code);
    return reloads || inspects || KEYS_FOR_THE_SAME.includes(event.key);
  }

  function switchOffBrowserShortcuts() {
    document.addEventListener("keydown", (event) => { if (wouldReloadOrInspect(event)) event.preventDefault(); }, { capture: true });
  }

  // --- Starting up ----------------------------------------------------------------------------

  // The browser's menu never opens. A part of the app that shows a menu of its own has already done so by now,
  // and is left to it. Anywhere else, `rowsFor` says what can be done to what was clicked.
  function start(menu, rowsFor) {
    document.addEventListener("contextmenu", (event) => {
      const alreadyAnswered = event.defaultPrevented;
      event.preventDefault();
      if (alreadyAnswered) return;
      const rows = isForTyping(event.target) ? rowsForTyping(event.target) : rowsFor(event.target);
      if (rows && rows.length) openMenu(menu, { x: event.clientX, y: event.clientY }, rows);
    });
    switchOffBrowserShortcuts();
  }

  window.appMenu = { start, row, DIVIDER };
})();
