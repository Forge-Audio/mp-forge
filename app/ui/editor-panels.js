// The editor's panels on the right: the list of everything on the page, the details of what is selected,
// and the shelf of controls that are not on this page yet.
//
// The panels only show things and pass on what the musician asks for. Everything is done through the `desk`
// they are given, which is the editor itself.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { PAGE } = window.mpcStage;
  const { button, stripButton, numberInput, textInput, colourInput, selectInput, tickBox, field, fieldPair, panelButton, panelHeading, panelNote } = window.editorWidgets;
  const { isChanged } = window.lookEdits;
  const { picturesHiddenOn } = window.pluginPictures;

  const MPC_FONTS = ["Titillium Web", "Roboto", "Nexa"];
  const WAYS_TO_ALIGN = [["left", "Left"], ["centre", "Centre"], ["right", "Right"]];
  const SHAPES = [["rectangle", "Rectangle"], ["ellipse", "Ellipse"]];
  const EDGES_TO_LINE_UP = [["left", "LEFT"], ["centre", "CENTRE"], ["right", "RIGHT"], ["top", "TOP"], ["middle", "MIDDLE"], ["bottom", "BOTTOM"]];
  const WAYS_TO_SPACE_OUT = [["across", "ACROSS"], ["down", "DOWN"]];
  const DEPTHS = [["front", "Over the controls"], ["behind", "Behind the controls"], ["back", "Behind everything"]];
  const LONGEST_SHOWN_NAME = 24;
  const LONGEST_WORDS = 120;
  const ON_THE_PAGE = { least: 0, most: PAGE.width };
  const ROUND_THE_PAGE = { least: -4096, most: 4096 };
  const SIDES = { least: 1, most: 2560 };
  const MPC_LETTERS = { least: 8, most: 200 };
  const OWN_LETTERS = { least: 6, most: 400 };
  const SHARE = { least: 0, most: 100 };
  const TILTS = { least: -180, most: 180 };
  const CORNERS = { least: 0, most: 400 };
  const EDGES = { least: 0, most: 64 };
  const NOTHING_SELECTED = "Click something on the screen to select it.";
  const ONLY_ARTWORK_TILTS = "Only artwork can be tilted. The MPC draws controls and their lettering upright itself.";
  const KIND_WORDS = { control: "Control", heading: "Heading", picture: "Picture", backdrop: "Backdrop", block: "Colour", words: "Words", art: "Art" };
  const PART_WORDS = { art: "Its image", name: "Its name", value: "Its value" };
  const DETAIL_WORDS = { block: "Colour block", words: "Words", picture: "Picture you added" };

  // --- What things are called ------------------------------------------------------------------

  function kindOf(item) {
    if (item.kind === "layer") return item.layer.kind === "picture" ? "art" : item.layer.kind;
    return item.pickedFromListOnly ? "backdrop" : item.kind;
  }

  function nameOfLayer(layer) {
    if (layer.kind === "words") return layer.text;
    if (layer.kind === "picture") return layer.file.replace(".png", "");
    return `${layer.shape === "ellipse" ? "Ellipse" : "Block"} ${layer.colour}`;
  }

  function nameOf(item, desk) {
    return item.kind === "layer" ? nameOfLayer(item.layer) : desk.shownNameOf(item);
  }

  // --- The list of everything on the page ------------------------------------------------------

  function eyeButton(item, desk) {
    const made = button("layer-eye", item.layer.hidden ? "OFF" : "ON", (event) => {
      event.stopPropagation();
      desk.changeLayer(item, { hidden: item.layer.hidden ? undefined : true });
    });
    made.title = item.layer.hidden ? "Hidden. Click to show it." : "Showing. Click to hide it.";
    return made;
  }

  // Artwork is deleted for good. Anything else only leaves this page, and can be put back from the shelf.
  function removeButton(item, desk) {
    const made = button("layer-remove", "X", (event) => {
      event.stopPropagation();
      desk.remove([item]);
    });
    made.title = item.kind === "layer" ? "Delete it." : "Take it off this page. It waits on the shelf below.";
    made.setAttribute("aria-label", made.title);
    return made;
  }

  function layerRow(item, desk) {
    const row = element("div", "layer-row");
    const pick = button("layer-pick", "", (event) => (event.shiftKey ? desk.selectAlso(item) : desk.select([item])));
    pick.append(element("span", "layer-kind", KIND_WORDS[kindOf(item)]), element("span", "layer-name", nameOf(item, desk)));
    row.setAttribute("aria-selected", String(desk.isSelected(item)));
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      desk.openMenuFor(item, { x: event.clientX, y: event.clientY });
    });
    row.append(pick, ...(item.kind === "layer" ? [eyeButton(item, desk)] : []), removeButton(item, desk));
    return row;
  }

  // The list runs from the front of the page to the back, as in a picture editor. What is selected is kept in view.
  function drawLayers(container, desk) {
    container.replaceChildren(...desk.items().slice().reverse().map((item) => layerRow(item, desk)));
    const selectedRow = container.querySelector('[aria-selected="true"]');
    if (selectedRow) selectedRow.scrollIntoView({ block: "nearest" });
  }

  // --- The shelf of controls that are not on this page -----------------------------------------

  function controlsNotOnThePage(desk) {
    const known = ((desk.plugin().parameters || {}).params || []).map((parameter) => parameter.name);
    const onThePage = desk.items().filter((item) => item.kind === "control").map((item) => item.name.toLowerCase());
    return known.filter((name) => !onThePage.includes(name.toLowerCase()));
  }

  function controlRow(name, desk) {
    const row = button("shelf-row", `+ ${name}`, () => desk.addControl(name));
    row.title = `Put ${name} on this page. It lands in a free slot, and you can drag it from there.`;
    return row;
  }

  function pictureRow(picture, desk) {
    const row = button("shelf-row", `+ ${picture.name} (picture)`, () => desk.showPicture(picture.name));
    row.title = `Show ${picture.name} on this page again, where it was.`;
    return row;
  }

  // What could be on this page and is not: the plugin's other controls, and pictures taken off the page by hand.
  function drawShelf(container, desk) {
    const controls = controlsNotOnThePage(desk).map((name) => controlRow(name, desk));
    const pictures = picturesHiddenOn(desk.plugin(), desk.pageName()).map((picture) => pictureRow(picture, desk));
    const rows = [...controls, ...pictures];
    container.replaceChildren(...(rows.length ? rows : [panelNote("Everything this plugin has for this page is already on it.")]));
  }

  // --- Fields that many kinds of item share ----------------------------------------------------

  function placeFields(item, desk) {
    const limits = item.mayLeavePage ? ROUND_THE_PAGE : ON_THE_PAGE;
    const at = (side) => numberInput(item.area[side], limits, (pixels) => desk.placeItem(item, { ...item.area, [side]: pixels }));
    return fieldPair(field("X", at("x")), field("Y", at("y")));
  }

  // Something that keeps its shape has one size. Anything else has a width and a height.
  function sizeFields(item, desk) {
    const sized = (side) => numberInput(item.area[side], SIDES, (pixels) => desk.placeItem(item, desk.resizedTo(item, side, pixels)));
    if (item.resize === "shape") return field("Size", sized("width"));
    return fieldPair(field("W", sized("width")), field("H", sized("height")));
  }

  // Artwork can be tilted about its middle. A positive tilt leans it to the right.
  function tiltField(items, desk) {
    const tilt = numberInput(items[0].layer.tilt || 0, TILTS, (degrees) => desk.tiltTo(items, degrees));
    tilt.title = "Degrees, clockwise. Dragging the round handle above the artwork does the same.";
    return field("Tilt °", tilt);
  }

  function opacityField(item, desk) {
    const shown = item.layer.opacity === undefined ? 100 : item.layer.opacity;
    return field("Solid %", numberInput(shown, SHARE, (share) => desk.changeLayer(item, { opacity: share === 100 ? undefined : share })));
  }

  function stackingButtons(item, desk) {
    const made = fieldPair(panelButton("FORWARD", () => desk.restack(item, 1)), panelButton("BACK", () => desk.restack(item, -1)));
    made.title = "Which layers this one is in front of. The keys ] and [ do the same.";
    return made;
  }

  function layerButtons(item, desk) {
    return [stackingButtons(item, desk), fieldPair(panelButton("COPY", () => desk.duplicate(item)), panelButton("DELETE", () => desk.remove([item])))];
  }

  // --- Artwork ---------------------------------------------------------------------------------

  function edgeFields(item, desk) {
    const edge = item.layer.edge;
    const colour = edge ? edge.colour : desk.colours().back;
    const width = numberInput(edge ? edge.width : 0, EDGES, (pixels) => desk.changeLayer(item, { edge: pixels ? { colour, width: pixels } : undefined }));
    const colourOfEdge = edge ? [field("Edge colour", colourInput(edge.colour, (picked) => desk.changeLayer(item, { edge: { ...edge, colour: picked } })))] : [];
    return [field("Edge", width), ...colourOfEdge];
  }

  function fadeFields(item, desk) {
    const fade = item.layer.fade;
    if (!fade) return [panelNote("The Fade tool (G) fades a block from one colour to another.")];
    return [
      field("Fades to", colourInput(fade.colour, (colour) => desk.changeLayer(item, { fade: { ...fade, colour } }))),
      panelButton("REMOVE THE FADE", () => desk.changeLayer(item, { fade: undefined })),
    ];
  }

  function blockDetails(item, desk) {
    const layer = item.layer;
    const corner = layer.shape === "ellipse" ? [] : [field("Corner", numberInput(layer.corner || 0, CORNERS, (pixels) => desk.changeLayer(item, { corner: pixels || undefined })))];
    return [
      field("Colour", colourInput(layer.colour, (colour) => desk.changeLayer(item, { colour }))),
      field("Shape", selectInput(layer.shape || "rectangle", SHAPES, (shape) => desk.changeLayer(item, { shape }))),
      ...corner,
      ...edgeFields(item, desk),
      ...fadeFields(item, desk),
    ];
  }

  // The fonts on the computer, with the layer's own font among them even if this computer lacks it.
  function fontChoices(layer, desk) {
    const known = desk.fonts();
    return (known.includes(layer.font) ? known : [layer.font, ...known]).map((name) => [name, name]);
  }

  function wordsDetails(item, desk) {
    const layer = item.layer;
    const text = textInput(layer.text, LONGEST_WORDS, (words) => desk.changeLayer(item, { text: words.trim() || layer.text }));
    text.dataset.field = "text";
    return [
      field("Words", text),
      field("Font", selectInput(layer.font, fontChoices(layer, desk), (font) => desk.changeLayer(item, { font }))),
      fieldPair(
        field("Size", numberInput(layer.size, OWN_LETTERS, (size) => desk.changeLayer(item, { size }))),
        field("Bold", tickBox(layer.bold, (bold) => desk.changeLayer(item, { bold: bold || undefined }))),
      ),
      field("Colour", colourInput(layer.colour, (colour) => desk.changeLayer(item, { colour }))),
    ];
  }

  function layerDetails(item, desk) {
    const ofItsKind = { block: blockDetails, words: wordsDetails, picture: () => [] }[item.layer.kind](item, desk);
    const size = item.layer.kind === "words" ? [] : [sizeFields(item, desk)];
    return [panelHeading(DETAIL_WORDS[item.layer.kind]), ...ofItsKind, opacityField(item, desk), placeFields(item, desk), ...size, tiltField([item], desk), ...layerButtons(item, desk)];
  }

  // --- Controls, headings and the AI's pictures ------------------------------------------------

  function nameField(item, desk, words) {
    const name = textInput(desk.shownNameOf(item), LONGEST_SHOWN_NAME, (typed) => desk.rename(item, typed));
    name.dataset.field = "name";
    return field(words, name);
  }

  function putBackButton(items, desk) {
    const made = panelButton("PUT BACK", () => desk.putBack(items), !items.some((item) => isChanged(desk.look(), desk.pageName(), item)));
    made.title = "Back to where the screen's own layout puts it, at its own size and under its own name.";
    return made;
  }

  function takeOffButton(items, desk) {
    const made = panelButton("TAKE OFF THIS PAGE", () => desk.remove(items));
    made.title = "Takes it off this page only. The plugin keeps it, and it waits on the shelf below.";
    return made;
  }

  // The lettering the MPC draws for a name, a value or a heading: only the fonts the MPC has can be used.
  function letteringFields(item, partName, desk) {
    const part = item.parts[partName];
    const change = (fields) => desk.keepPart(item, partName, { ...part, ...fields });
    return [
      field("Font", selectInput(part.font, MPC_FONTS.map((name) => [name, name]), (font) => change({ font }))),
      fieldPair(field("Size", numberInput(part.height, MPC_LETTERS, (height) => change({ height }))), field("Bold", tickBox(part.bold, (bold) => change({ bold })))),
      field("Colour", colourInput(part.colour, (colour) => change({ colour }))),
      field("Align", selectInput(part.align, WAYS_TO_ALIGN, (align) => change({ align }))),
    ];
  }

  function controlDetails(item, desk) {
    const size = item.resize ? [sizeFields(item, desk)] : [];
    const note = `${item.parts ? "The Parts tool (A) moves or restyles its knob, name and value one at a time." : "This kind of control moves as a whole."} It cannot be tilted: the MPC draws controls upright itself.`;
    return [panelHeading("Control"), nameField(item, desk, "Name"), placeFields(item, desk), ...size, panelNote(note), putBackButton([item], desk), takeOffButton([item], desk)];
  }

  function headingDetails(item, desk) {
    return [panelHeading("Heading"), nameField(item, desk, "Words"), placeFields(item, desk), ...letteringFields(item, "name", desk), putBackButton([item], desk), takeOffButton([item], desk)];
  }

  // How solid one of the AI's pictures is and how far back it sits. The same choices are in its right-click menu.
  function pictureStyleFields(item, desk) {
    return [
      field("Solid %", numberInput(item.style.opacity, SHARE, (opacity) => desk.stylePicture(item, { opacity }))),
      field("Sits", selectInput(item.style.depth, DEPTHS, (depth) => desk.stylePicture(item, { depth }))),
    ];
  }

  function pictureDetails(item, desk) {
    const note = panelNote("The AI drew this picture. It is drawn again, sharp, at whatever size you give it.");
    const place = [placeFields(item, desk), sizeFields(item, desk)];
    return [panelHeading("Picture"), element("div", "item-own-name", item.name), ...place, ...pictureStyleFields(item, desk), note, putBackButton([item], desk), takeOffButton([item], desk)];
  }

  // --- One part of a control -------------------------------------------------------------------

  // A part's place is measured inside its control, so these fields move it there, not on the page.
  function partPlaceFields(held, desk) {
    const within = { x: held.area.x - held.item.area.x, y: held.area.y - held.item.area.y, width: held.area.width, height: held.area.height };
    const moved = (side) => numberInput(within[side], ON_THE_PAGE, (pixels) => desk.keepPartArea(held, { ...within, [side]: pixels }));
    const sized = (side) => numberInput(within[side], SIDES, (pixels) => desk.keepPartArea(held, desk.partResizedTo(held, within, side, pixels)));
    const place = fieldPair(field("X", moved("x")), field("Y", moved("y")));
    if (!held.resize) return [place];
    return [place, held.resize === "shape" ? field("Size", sized("width")) : fieldPair(field("W", sized("width")), field("H", sized("height")))];
  }

  function partDetails(held, desk) {
    const { item, part } = held;
    const heading = panelHeading(`${desk.shownNameOf(item)}: ${PART_WORDS[part].toLowerCase()}`);
    if (part === "art") return [heading, ...partPlaceFields(held, desk), panelNote(held.resize ? "MP Forge draws it again, sharp, at any size." : "This image keeps the size it was made at.")];
    const label = item.parts[part];
    const shows = field("Shows", tickBox(!label.hidden, (isShown) => desk.keepPart(item, part, { ...label, hidden: !isShown })));
    return [heading, shows, ...letteringFields(item, part, desk), ...partPlaceFields(held, desk), panelNote("The MPC letters this itself, so only its own three fonts can be used.")];
  }

  // --- The panel about what is selected --------------------------------------------------------

  function detailsOfOne(item, desk) {
    if (item.kind === "layer") return layerDetails(item, desk);
    return { control: controlDetails, heading: headingDetails, picture: pictureDetails }[item.kind](item, desk);
  }

  function buttonRow(buttons) {
    const row = element("div", "item-buttons");
    row.append(...buttons);
    return row;
  }

  // Spacing out needs three things, so that one of them sits between the other two.
  function arrangeButtons(items, desk) {
    const lineUp = EDGES_TO_LINE_UP.map(([edge, words]) => stripButton(words, () => desk.lineUp(edge)));
    const spaceOut = WAYS_TO_SPACE_OUT.map(([direction, words]) => stripButton(words, () => desk.spaceOut(direction), items.length < 3));
    return [panelNote("Line them up"), buttonRow(lineUp), panelNote("Space them out evenly"), buttonRow(spaceOut)];
  }

  function detailsOfSeveral(items, desk) {
    const allArtwork = items.every((item) => item.kind === "layer");
    const ending = allArtwork ? panelButton("DELETE", () => desk.remove(items)) : putBackButton(items.filter((item) => item.kind !== "layer"), desk);
    const artwork = items.filter((item) => item.kind === "layer");
    const tilt = artwork.length ? [tiltField(artwork, desk), ...(allArtwork ? [] : [panelNote(ONLY_ARTWORK_TILTS)])] : [panelNote(ONLY_ARTWORK_TILTS)];
    return [panelHeading(`${items.length} things selected`), panelNote("Drag any of them to move them all."), ...arrangeButtons(items, desk), ...tilt, ending];
  }

  // Puts the cursor in the field a tool asked for, ready to type over. It waits a moment, because the press
  // that asked for it has not finished yet, and finishing it would take the cursor away again.
  function focusAskedField(container, desk) {
    const asked = desk.fieldToFocus() && container.querySelector(`[data-field="${desk.fieldToFocus()}"]`);
    if (!asked || document.activeElement === asked) return;
    setTimeout(() => {
      asked.focus();
      asked.select();
    });
  }

  function drawItem(container, desk) {
    const items = desk.selected();
    const held = desk.held();
    if (!items.length) return container.replaceChildren(panelNote(NOTHING_SELECTED));
    const details = held && held.part ? partDetails(held, desk) : items.length === 1 ? detailsOfOne(items[0], desk) : detailsOfSeveral(items, desk);
    container.replaceChildren(...details);
    focusAskedField(container, desk);
  }

  window.editorPanels = { drawLayers, drawShelf, drawItem, nameOf, kindOf };
})();
