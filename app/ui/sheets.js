// A question put on a sheet laid over the app, and answered by a promise.

(function () {
  "use strict";

  const { element } = window.helpers;

  function button(className, words) {
    const made = element("button", className, words);
    made.type = "button";
    return made;
  }

  // Shows a sheet with a title, what it holds, and two buttons. `answerOf` gives the answer when the second one is
  // pressed, or when Enter is pressed in a box for typing. Escape and the first button both answer with nothing.
  // `details.onMade` is handed the second button, for a sheet that has to switch it off and on.
  function asked(sheet, details, answerOf) {
    return new Promise((resolve) => {
      const cancel = button("item-button", details.cancelWords || "CANCEL");
      const confirm = button("item-button done", details.confirmWords);
      const buttons = element("div", "sheet-buttons");
      const body = element("div", "window-body");
      const made = element("div", `window sheet ${details.wide ? "wide" : ""}`);
      const answer = (given) => {
        sheet.hidden = true;
        sheet.replaceChildren();
        document.removeEventListener("keydown", answerFromKeys);
        resolve(given);
      };
      function answerFromKeys(event) {
        if (event.key === "Escape") answer(null);
        if (event.key === "Enter" && event.target instanceof HTMLInputElement && !confirm.disabled) confirm.click();
      }
      cancel.addEventListener("click", () => answer(null));
      confirm.addEventListener("click", () => answer(answerOf()));
      document.addEventListener("keydown", answerFromKeys);
      buttons.append(cancel, confirm);
      body.append(...details.holds, buttons);
      made.setAttribute("role", "dialog");
      made.setAttribute("aria-label", details.title);
      made.append(element("div", "titlebar", details.title), body);
      sheet.replaceChildren(made);
      sheet.hidden = false;
      if (details.onMade) details.onMade(confirm);
      (details.focusOn || cancel).focus();
    });
  }

  // Asks for a key or a password. What is typed shows as dots. Answers with it, or with nothing.
  // `details.problem` says why the last one typed was not kept.
  function askForSecret(sheet, details) {
    const secret = element("input", "item-input");
    secret.type = "password";
    secret.autocomplete = "off";
    secret.spellcheck = false;
    secret.placeholder = details.placeholder;
    secret.setAttribute("aria-label", details.placeholder);
    const holds = [secret, element("div", "soft small", details.words)];
    if (details.problem) holds.push(element("div", "sheet-problem", details.problem));
    return asked(sheet, { title: details.title, confirmWords: details.confirmWords, holds, focusOn: secret }, () => secret.value.trim());
  }

  window.sheets = { asked, askForSecret, button };
})();
