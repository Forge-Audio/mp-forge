// The small buttons and fields the editor's strips and panels are built from.

(function () {
  "use strict";

  const { element } = window.helpers;

  function button(className, words, onPressed) {
    const made = element("button", className, words);
    made.type = "button";
    made.addEventListener("click", onPressed);
    return made;
  }

  // --- For the strip of options under the title ------------------------------------------------

  function stripLabel(words) {
    return element("span", "strip-label", words);
  }

  function stripGap() {
    return element("span", "strip-gap");
  }

  function stripButton(words, onPressed, isOff) {
    const made = button("strip-button", words, onPressed);
    made.disabled = Boolean(isOff);
    return made;
  }

  // One of a set of buttons of which exactly one is on.
  function stripChoice(words, isChosen, onChosen) {
    const made = stripButton(words, onChosen);
    made.setAttribute("aria-pressed", String(isChosen));
    return made;
  }

  // --- Inputs, for strips and panels alike -----------------------------------------------------

  function numberInput(value, limits, onChanged) {
    const input = element("input", "item-input number");
    input.type = "number";
    input.min = String(limits.least);
    input.max = String(limits.most);
    input.value = String(Math.round(value));
    const within = () => Math.min(limits.most, Math.max(limits.least, Math.round(Number(input.value) || 0)));
    input.addEventListener("change", () => onChanged(within()));
    return input;
  }

  function textInput(value, longest, onChanged) {
    const input = element("input", "item-input");
    input.type = "text";
    input.maxLength = longest;
    input.value = value;
    input.addEventListener("change", () => onChanged(input.value));
    return input;
  }

  function colourInput(value, onChanged) {
    const input = element("input", "item-input colour");
    input.type = "color";
    input.value = value;
    input.addEventListener("change", () => onChanged(input.value));
    return input;
  }

  // `choices` are [value, words] pairs.
  function selectInput(value, choices, onChanged) {
    const input = element("select", "item-input");
    choices.forEach(([choice, words]) => {
      const option = element("option", "", words);
      option.value = choice;
      input.append(option);
    });
    input.value = value;
    input.addEventListener("change", () => onChanged(input.value));
    return input;
  }

  function tickBox(isTicked, onChanged) {
    const input = element("input", "item-tick");
    input.type = "checkbox";
    input.checked = Boolean(isTicked);
    input.addEventListener("change", () => onChanged(input.checked));
    return input;
  }

  // --- For the panel about the selected item ---------------------------------------------------

  // An input with its name beside it.
  function field(words, input) {
    const made = element("label", "item-field");
    made.append(element("span", "item-field-name", words), input);
    return made;
  }

  // Two short fields side by side, such as X and Y.
  function fieldPair(one, other) {
    const made = element("div", "item-pair");
    made.append(one, other);
    return made;
  }

  function panelButton(words, onPressed, isOff) {
    const made = button("item-button", words, onPressed);
    made.disabled = Boolean(isOff);
    return made;
  }

  function panelHeading(words) {
    return element("div", "item-kind", words);
  }

  function panelNote(words) {
    return element("div", "soft small", words);
  }

  window.editorWidgets = {
    button, stripLabel, stripGap, stripButton, stripChoice, numberInput, textInput, colourInput, selectInput, tickBox,
    field, fieldPair, panelButton, panelHeading, panelNote,
  };
})();
