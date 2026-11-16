// The editor's tool palette: one button for each tool, in a row under the title,
// with the front and back colours after them.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { button } = window.editorWidgets;

  const SVG = "http://www.w3.org/2000/svg";
  const ICON_GRID = "0 0 16 16";

  function iconOf(tool) {
    const drawing = document.createElementNS(SVG, "svg");
    drawing.setAttribute("class", "tool-icon");
    drawing.setAttribute("viewBox", ICON_GRID);
    drawing.setAttribute("shape-rendering", "crispEdges");
    drawing.setAttribute("aria-hidden", "true");
    const shape = document.createElementNS(SVG, "path");
    shape.setAttribute("d", tool.icon);
    shape.setAttribute("fill-rule", "evenodd");
    drawing.append(shape);
    return drawing;
  }

  function nameWithKey(tool) {
    return `${tool.name} (${tool.key.toUpperCase()})`;
  }

  function toolButton(tool, chosen, onChosen) {
    const made = button("tool-button", "", () => onChosen(tool));
    made.title = nameWithKey(tool);
    made.setAttribute("aria-label", nameWithKey(tool));
    made.setAttribute("aria-pressed", String(tool === chosen));
    made.append(iconOf(tool));
    return made;
  }

  // One of the two colours the artwork tools use. Clicking it opens the computer's own colour chooser.
  function swatch(which, words, colours) {
    const made = element("label", `swatch ${which}`);
    made.title = words;
    made.style.background = colours[which];
    const chooser = element("input", "swatch-chooser");
    chooser.type = "color";
    chooser.value = colours[which];
    chooser.setAttribute("aria-label", words);
    chooser.addEventListener("change", () => colours.onPicked(which, chooser.value));
    made.append(chooser);
    return made;
  }

  function smallButton(words, title, onPressed) {
    const made = button("swatch-button", words, onPressed);
    made.title = title;
    return made;
  }

  // `colours` gives the `front` and `back` colours and hears when one is picked, when they are swapped and when they are reset.
  function swatches(colours) {
    const pair = element("div", "swatches");
    pair.append(swatch("back", "Back colour: fades end in it", colours), swatch("front", "Front colour: blocks, words and paint use it", colours));
    const buttons = element("div", "swatch-buttons");
    buttons.append(smallButton("SWAP", "Swap the front and back colours (X)", colours.onSwapped), smallButton("B/W", "Back to white on black (D)", colours.onReset));
    return [pair, buttons];
  }

  // Draws the palette, one group of tools after another, with the chosen tool marked.
  // `listeners.onChosen` hears which tool was picked.
  function drawPalette(container, toolGroups, chosen, listeners) {
    const groups = toolGroups.flatMap((tools) => [...tools.map((tool) => toolButton(tool, chosen, listeners.onChosen)), element("div", "tool-gap")]);
    container.replaceChildren(...groups, ...swatches(listeners.colours));
  }

  // The tool a plain key press asks for, if any.
  function toolForKey(tools, key) {
    return tools.find((tool) => tool.key === key.toLowerCase());
  }

  window.editorPalette = { drawPalette, toolForKey };
})();
