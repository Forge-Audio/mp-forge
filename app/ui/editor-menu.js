// The menu that opens where the musician right-clicks in the editor. What it offers depends on what was clicked:
// a piece of artwork, a control, a heading, one of the AI's pictures, several things at once, or the bare page.
//
// A row is { words, does, isOff }. A gap between groups of rows is written as DIVIDER.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { PAGE } = window.mpcStage;
  const { button } = window.editorWidgets;
  const { isChanged } = window.lookEdits;
  const { blockOver, wordsAt } = window.editorArtTools;

  const DIVIDER = null;
  const SIZE_OF_A_NEW_BLOCK = { width: 240, height: 140 };
  const ROOM_AT_THE_EDGE = 6;

  function row(words, does, isOff) {
    return { words, does, isOff: Boolean(isOff) };
  }

  // --- What can be done to each kind of thing --------------------------------------------------

  // A click in a list of layers has no place on the page, so there is no colour to pick up there.
  function colourRows(desk, press) {
    return press.client ? [row("Pick up the colour here", () => desk.pickUpAt(press.client))] : [];
  }

  function putBackRow(items, desk, words) {
    const changed = items.filter((item) => isChanged(desk.look(), desk.pageName(), item));
    return row(words, () => desk.putBack(changed), !changed.length);
  }

  function stackingRows(item, desk) {
    return [
      row("Bring to the front", () => desk.restackToThe("front", item)),
      row("Forward one", () => desk.restack(item, 1)),
      row("Back one", () => desk.restack(item, -1)),
      row("Send to the back", () => desk.restackToThe("back", item)),
    ];
  }

  function rowsForArtwork(item, desk, press) {
    const isHidden = Boolean(item.layer.hidden);
    const showOrHide = row(isHidden ? "Show" : "Hide", () => desk.changeLayer(item, { hidden: isHidden ? undefined : true }));
    return [row("Duplicate", () => desk.duplicate(item)), DIVIDER, ...stackingRows(item, desk), DIVIDER, showOrHide, ...colourRows(desk, press), DIVIDER, row("Delete", () => desk.remove([item]))];
  }

  function rowsForAControl(item, desk, press) {
    const itsParts = item.parts ? [row("Lay out its knob, name and value", () => desk.layOutParts(item))] : [];
    return [row("Rename", () => desk.focusField("name")), ...itsParts, putBackRow([item], desk, "Put back as it was"), ...colourRows(desk, press), DIVIDER, row("Take off this page", () => desk.remove([item]))];
  }

  function rowsForAHeading(item, desk, press) {
    return [row("Change its words", () => desk.focusField("name")), putBackRow([item], desk, "Put back as it was"), ...colourRows(desk, press), DIVIDER, row("Take off this page", () => desk.remove([item]))];
  }

  // One row for each place a picture can sit, with the one it is in now switched off.
  function depthRows(item, desk) {
    const sendTo = (depth, words) => row(words, () => desk.stylePicture(item, { depth }), item.style.depth === depth);
    return [sendTo("front", "Bring over the controls"), sendTo("behind", "Send behind the controls"), sendTo("back", "Send behind everything")];
  }

  function rowsForAPicture(item, desk, press) {
    return [...depthRows(item, desk), DIVIDER, putBackRow([item], desk, "Put back as it was"), ...colourRows(desk, press), DIVIDER, row("Take off this page", () => desk.remove([item]))];
  }

  function rowsForSeveral(items, desk) {
    const notArtwork = items.filter((item) => item.kind !== "layer");
    return [putBackRow(notArtwork, desk, "Put them back as they were"), DIVIDER, row(`Delete all ${items.length}`, () => desk.remove(items))];
  }

  function addWordsAt(desk, point) {
    desk.addLayer(wordsAt(desk, point));
    desk.focusField("text");
  }

  function rowsForThePage(desk, press) {
    const newBlock = { x: press.point.x, y: press.point.y, ...SIZE_OF_A_NEW_BLOCK };
    return [
      row("Add a colour block here", () => desk.addLayer(blockOver(desk, newBlock))),
      row("Add words here", () => addWordsAt(desk, press.point)),
      row("Fill the page with the front colour", () => desk.addLayer(blockOver(desk, PAGE), { atTheBack: true })),
      ...colourRows(desk, press),
      DIVIDER,
      row("Select everything", desk.selectEverything),
      row("Copy this page's design to another page…", desk.copyDesign, desk.pageNames().length < 2),
    ];
  }

  // What can be done to what was right-clicked. `press.item` is what was clicked, if anything was.
  // A click on one of several selected things is about all of them.
  function rowsFor(desk, press) {
    const item = press.item;
    if (!item) return rowsForThePage(desk, press);
    if (desk.selected().length > 1 && desk.isSelected(item)) return rowsForSeveral(desk.selected(), desk);
    const rowsForItsKind = { layer: rowsForArtwork, control: rowsForAControl, heading: rowsForAHeading, picture: rowsForAPicture }[item.kind];
    return rowsForItsKind(item, desk, press);
  }

  // --- The menu itself -------------------------------------------------------------------------

  function menuRow(given, close) {
    if (given === DIVIDER) return element("div", "menu-divider");
    const made = button("menu-row", given.words, () => { close(); given.does(); });
    made.setAttribute("role", "menuitem");
    made.disabled = given.isOff;
    return made;
  }

  function keptInWindow(start, length, room) {
    return Math.max(ROOM_AT_THE_EDGE, Math.min(start, room - length - ROOM_AT_THE_EDGE));
  }

  // Opens the menu at a place in the window. It closes when a row is picked, when anything else is pressed,
  // on Escape, and when the window is scrolled or left.
  function openMenu(menu, client, rows) {
    const listening = new AbortController();
    const close = () => { menu.hidden = true; listening.abort(); };
    const closeOnEscape = (event) => { if (event.key === "Escape") { event.stopPropagation(); close(); } };
    const closeIfOutside = (event) => { if (!menu.contains(event.target)) close(); };
    menu.replaceChildren(...rows.map((given) => menuRow(given, close)));
    menu.hidden = false;
    menu.style.left = `${keptInWindow(client.x, menu.offsetWidth, window.innerWidth)}px`;
    menu.style.top = `${keptInWindow(client.y, menu.offsetHeight, window.innerHeight)}px`;
    document.addEventListener("pointerdown", closeIfOutside, { capture: true, signal: listening.signal });
    document.addEventListener("keydown", closeOnEscape, { capture: true, signal: listening.signal });
    document.addEventListener("scroll", close, { capture: true, signal: listening.signal });
    window.addEventListener("blur", close, { signal: listening.signal });
  }

  window.editorMenu = { rowsFor, openMenu };
})();
