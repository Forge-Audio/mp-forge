// The sheet that copies one page's design to another. The musician picks the page it goes to and ticks
// what goes across: each piece of artwork by itself, and how the controls are laid out.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { button, selectInput, tickBox, panelNote } = window.editorWidgets;
  const { nameOf } = window.editorPanels;

  const NO_ARTWORK = "This page has no artwork of its own to copy.";
  const HOW_CONTROLS_MATCH = "Place for place: whatever sits in each slot of that page ends up where the one in the same slot sits here, laid out the same way. Every control keeps its own name.";

  function ticked(words, isTicked) {
    const box = tickBox(isTicked, () => {});
    const made = element("label", "sheet-tick");
    made.append(box, element("span", "", words));
    return { made, isTicked: () => box.checked };
  }

  function heading(words) {
    return element("div", "field-label", words);
  }

  function otherPages(desk) {
    const here = desk.pageName();
    return desk.pageNames().map((name, index) => [String(index), name]).filter(([, name]) => name !== here);
  }

  // The rows that choose which artwork goes across: one tick for each piece, from the front of the page to the back.
  function artworkChoices(desk) {
    const artwork = desk.items().filter((item) => item.kind === "layer").reverse();
    return artwork.map((item) => ({ layer: item.layer, tick: ticked(nameOf(item, desk), true) }));
  }

  function buttonRow(buttons) {
    const made = element("div", "sheet-buttons");
    made.append(...buttons);
    return made;
  }

  // Opens the sheet for the page being edited. `desk.copyDesignTo` hears which page it goes to and what goes across.
  function openCopySheet(sheet, desk) {
    const close = () => { sheet.hidden = true; sheet.replaceChildren(); };
    const pages = otherPages(desk);
    const toPage = selectInput(pages[0][0], pages, () => {});
    const artwork = artworkChoices(desk);
    const replaces = ticked("Take away the artwork already on that page first", false);
    const controls = ticked("How the controls and headings are laid out", true);
    const copy = () => {
      // The page's artwork is listed from the back to the front, which is the order it has to arrive in.
      const layers = artwork.filter((choice) => choice.tick.isTicked()).map((choice) => choice.layer).reverse();
      close();
      desk.copyDesignTo(Number(toPage.value), { layers, replacesArtwork: replaces.isTicked(), withControls: controls.isTicked() });
    };
    const body = element("div", "window-body");
    const destination = element("label", "item-field");
    destination.append(element("span", "item-field-name", `From ${desk.pageName()} to`), toPage);
    body.append(
      destination,
      heading("Artwork"),
      ...(artwork.length ? [...artwork.map((choice) => choice.tick.made), replaces.made] : [panelNote(NO_ARTWORK)]),
      heading("Controls"),
      controls.made,
      panelNote(HOW_CONTROLS_MATCH),
      buttonRow([button("item-button", "CANCEL", close), button("item-button done", "COPY", copy)]),
    );
    const made = element("div", "window sheet");
    made.setAttribute("role", "dialog");
    made.setAttribute("aria-label", "Copy this page's design to another page");
    made.append(element("div", "titlebar", "COPY THIS PAGE'S DESIGN"), body);
    sheet.replaceChildren(made);
    sheet.hidden = false;
    toPage.focus();
  }

  window.editorCopy = { openCopySheet };
})();
