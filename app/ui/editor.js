// The edit mode: the musician styles a plugin's screen by hand, the way a picture editor works.
//
// The tools sit in a bar under the title, the MPC's page fills the middle, and the layers, the selected item's
// details and the shelf of spare controls sit on the right. Every finished change is kept by the app at
// once, in the plugin's look. Nothing here sends anything to an MPC: an edited plugin still has to pass
// its safety check first.
//
// The tools and the panels do their work through the `desk` at the heart of this file.

(function () {
  "use strict";

  const { ask, send } = window.backend;
  const { element } = window.helpers;
  const { PAGE, placeAt } = window.mpcStage;
  const { addressOfDrawing, loaded } = window.fetchedImages;
  const { drawForArranging, pageNamesOf, pageColourOf, layerToDraw } = window.pluginScreen;
  const edits = window.lookEdits;
  const { TILT_HANDLE, pagePoint, holds, touches, movedBy, keptOnPage, linedUp, spreadOut, handlesOf, uprightPoint, tiltHandleOf } = window.editorGeometry;
  const { ARRANGING_TOOLS, VIEW_TOOLS, MOVE, PARTS, HAND, ZOOM_STEP } = window.editorTools;
  const { ART_TOOLS } = window.editorArtTools;
  const { drawPalette, toolForKey } = window.editorPalette;
  const { drawLayers, drawShelf, drawItem } = window.editorPanels;
  const { colourShownAt } = window.shownColour;
  const { rowsFor, openMenu } = window.editorMenu;
  const { openCopySheet } = window.editorCopy;

  const TOOL_GROUPS = [ARRANGING_TOOLS, ART_TOOLS, VIEW_TOOLS];
  const TOOLS = TOOL_GROUPS.flat();
  const NUDGE = 1;
  const BIG_NUDGE = 10;
  // Arrow-key nudges are kept as one change, once the musician has stopped pressing for this long.
  const SETTLE_NUDGES_AFTER_MS = 400;
  const MOST_ZOOM = 6;
  const SMALLEST_PAGE_WIDTH = 240;
  const ROOM_ROUND_THE_PAGE = 24;
  // How near a handle the pointer has to be, in pixels of the window, to take hold of it.
  const HANDLE_REACH = 8;
  const COPY_SITS_THIS_FAR_OFF = 16;
  // While a hardware knob is turned, the MPC lays its own panel over this much of the page, as seen on a real one.
  const COVERED_BY_THE_KNOB_PANEL = { x: 1010, y: 0, width: 270, height: 628 };
  // A picture brought in is shrunk to this before it is kept: twice the page, which leaves room to crop.
  const LONGEST_PICTURE_SIDE = 2560;
  const SVG = "http://www.w3.org/2000/svg";
  const WHITE_ON_BLACK = { front: "#ffffff", back: "#000000" };
  const FONTS_TO_START_WITH = ["Helvetica Neue", "Helvetica", "Arial"];
  const PARTS_FROM_THE_FRONT = ["value", "name", "art"];
  const ARROW_KEYS = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  const NOT_A_PICTURE = "That file is not a picture MP Forge can read.";

  let parts;
  let listeners;
  let comment;

  // What is being edited. `look` is the newest look the musician asked for; `plugin.look` is the last one the app kept.
  // Each item on the page has `area`, where it is drawn now, and `settledArea`, where it was when last kept.
  const editing = {
    plugin: null,
    look: { pages: [] },
    pageIndex: 0,
    tool: MOVE,
    heldTool: null,
    selectedKeys: new Set(),
    // The one part of a control picked up with the Parts tool, and where it is being dragged to before that is kept.
    heldPart: null,
    heldPartPreview: null,
    items: [],
    stage: null,
    overlays: null,
    zoom: 1,
    snap: true,
    showsKnobPanel: false,
    history: edits.lookHistory(),
    sessionBegun: false,
    madeChanges: false,
    saving: Promise.resolve(),
    savesWaiting: 0,
    nudgeTimer: undefined,
    colours: { ...WHITE_ON_BLACK },
    options: { shape: "rectangle", corner: 16, font: FONTS_TO_START_WITH[0], wordSize: 48, bold: false, brushSize: 12 },
    fonts: FONTS_TO_START_WITH,
    fieldToFocus: null,
    artSizes: new Map(),
    // Who to tell once the file chooser has a picture.
    whenPictureChosen: null,
  };

  // --- Small helpers ---------------------------------------------------------------------------

  function isOpen() {
    return Boolean(parts) && !parts.window.hidden;
  }

  function keyOf(item) {
    return `${item.kind}:${item.name}`;
  }

  function pageName() {
    return pageNamesOf(editing.plugin)[editing.pageIndex];
  }

  function pluginName() {
    return editing.plugin.summary.name;
  }

  function isSelected(item) {
    return editing.selectedKeys.has(keyOf(item));
  }

  function selectedItems() {
    return editing.items.filter(isSelected);
  }

  function toolInHand() {
    return editing.heldTool || editing.tool;
  }

  function shownNameOf(item) {
    const change = edits.changeOf(editing.look, pageName(), item);
    return (change && change.shownName) || item.name || "(no words)";
  }

  function say(words, isAProblem) {
    parts.hint.textContent = words;
    parts.hint.classList.toggle("problem", Boolean(isAProblem));
  }

  function describe(problem) {
    return typeof problem === "string" ? problem : (problem && problem.message) || "That could not be kept.";
  }

  function wholePixels(area) {
    return { x: Math.round(area.x), y: Math.round(area.y), width: Math.max(1, Math.round(area.width)), height: Math.max(1, Math.round(area.height)) };
  }

  function withoutEmptyFields(fields) {
    return Object.fromEntries(Object.entries(fields).filter(([, value]) => value !== undefined && value !== null));
  }

  // --- Keeping a change ------------------------------------------------------------------------

  // The first change of a visit keeps a version of the plugin, so GO BACK undoes everything done here.
  async function beginSessionOnce() {
    if (editing.sessionBegun) return;
    await ask("begin_editing", { name: pluginName() });
    editing.sessionBegun = true;
  }

  async function keepWithTheApp(look) {
    try {
      await beginSessionOnce();
      const kept = await ask("keep_look", { name: pluginName(), look });
      editing.plugin = { ...editing.plugin, look, arrangement: kept.arrangement, screen: kept.screen };
      editing.madeChanges = true;
      say(toolInHand().hint);
    } catch (problem) {
      editing.look = editing.plugin.look;
      say(describe(problem), true);
    }
    editing.savesWaiting -= 1;
    drawOnceEverythingIsKept();
  }

  // Looks are kept one after another, so a slow answer can never put an older look over a newer one.
  function keep(look) {
    editing.look = look;
    editing.savesWaiting += 1;
    editing.saving = editing.saving.then(() => keepWithTheApp(look));
  }

  function change(look) {
    editing.history.remember(editing.look);
    keep(look);
  }

  function undo() {
    if (editing.history.canUndo()) keep(editing.history.undo(editing.look));
  }

  function redo() {
    if (editing.history.canRedo()) keep(editing.history.redo(editing.look));
  }

  // --- Changing where things are and how big ---------------------------------------------------

  function whereItMayGo(item, area) {
    return item.mayLeavePage ? area : keptOnPage(area);
  }

  // Puts each item over the area given for it, as one change.
  function placeAll(items, areas) {
    const placedOne = (look, item, position) => edits.placed(look, pageName(), item, item.settledArea, whereItMayGo(item, areas[position]));
    change(items.reduce(placedOne, editing.look));
  }

  function placeItem(item, area) {
    placeAll([item], [area]);
  }

  // An area with one side set, for something that may have to keep its shape.
  function sideSetTo(area, keepsShape, side, pixels) {
    if (!keepsShape) return { ...area, [side]: pixels };
    const by = pixels / area[side];
    return { ...area, width: area.width * by, height: area.height * by };
  }

  function lineUp(edge) {
    const items = selectedItems();
    placeAll(items, linedUp(items.map((item) => item.area), edge));
  }

  function spaceOut(direction) {
    const items = selectedItems();
    placeAll(items, spreadOut(items.map((item) => item.area), direction));
  }

  function rename(item, words) {
    const asked = words.trim();
    change(edits.renamed(editing.look, pageName(), item, asked === item.name ? "" : asked));
  }

  function putBack(items) {
    change(items.reduce((look, item) => edits.putBack(look, pageName(), item), editing.look));
  }

  // --- Moving things before the move is kept ---------------------------------------------------

  function isSameSize(one, other) {
    return one.width === other.width && one.height === other.height;
  }

  // Shows an item somewhere else. Something that cannot simply be stretched keeps its drawing until the change is kept.
  function showAt(item, area) {
    item.area = area;
    if (item.stretchesLive || isSameSize(area, item.settledArea)) placeAt(item.made, area);
  }

  function hasUnsettledChange(item) {
    return ["x", "y", "width", "height"].some((side) => item.area[side] !== item.settledArea[side]);
  }

  // Shows the selection moved from where it last settled. Nothing is kept until `settle`.
  function slideSelection(dx, dy) {
    selectedItems().forEach((item) => showAt(item, movedBy(item.settledArea, dx, dy)));
    drawSelection();
    drawItemPanel();
  }

  // Keeps every move and resize made since the last one was kept, as one change.
  function settle() {
    clearTimeout(editing.nudgeTimer);
    const changed = editing.items.filter(hasUnsettledChange);
    if (!changed.length) return;
    placeAll(changed, changed.map((item) => item.area));
    changed.forEach((item) => { item.settledArea = item.area; });
  }

  function nudgeSelection(arrow, step) {
    const [across, down] = arrow;
    selectedItems().forEach((item) => showAt(item, whereItMayGo(item, movedBy(item.area, across * step, down * step))));
    drawSelection();
    drawItemPanel();
    clearTimeout(editing.nudgeTimer);
    editing.nudgeTimer = setTimeout(settle, SETTLE_NUDGES_AFTER_MS);
  }

  // --- Selecting -------------------------------------------------------------------------------

  function showSelection() {
    drawSelection();
    drawOptions();
    drawPanels();
  }

  function select(items) {
    editing.selectedKeys = new Set(items.map(keyOf));
    editing.heldPart = null;
    showSelection();
  }

  function selectAlso(item) {
    const others = selectedItems().filter((other) => other !== item);
    select(isSelected(item) ? others : [...others, item]);
  }

  // The item nearest the front that the point is on. A backdrop is only ever picked from the list.
  function itemAt(point) {
    const isOn = (item) => holds(item.area, uprightPoint(point, item.area, tiltOf(item)));
    return editing.items.slice().reverse().find((item) => !item.pickedFromListOnly && !isHiddenLayer(item) && isOn(item));
  }

  // How far something is tilted, in degrees clockwise. Only artwork can be: the MPC draws everything else upright.
  function tiltOf(item) {
    return item.tilt || 0;
  }

  function isHiddenLayer(item) {
    return item.kind === "layer" && Boolean(item.layer.hidden);
  }

  // --- The parts inside a control --------------------------------------------------------------

  function areaOfPart(item, part) {
    const within = part === "art" ? item.parts.art : item.parts[part] && !item.parts[part].hidden && item.parts[part].area;
    return within ? { ...within, x: item.area.x + within.x, y: item.area.y + within.y } : null;
  }

  function withinItem(item, area) {
    return wholePixels({ ...area, x: area.x - item.area.x, y: area.y - item.area.y });
  }

  // The part nearest the front that the point is on, as { item, part }.
  function partAt(point) {
    const withParts = editing.items.slice().reverse().filter((item) => item.parts);
    for (const item of withParts) {
      const part = PARTS_FROM_THE_FRONT.find((name) => { const area = areaOfPart(item, name); return area && holds(area, point); });
      if (part) return { item, part };
    }
    return null;
  }

  function selectPart(hit) {
    editing.selectedKeys = new Set(hit ? [keyOf(hit.item)] : []);
    editing.heldPart = hit ? { key: keyOf(hit.item), part: hit.part } : null;
    showSelection();
  }

  // Keeps one part of a control as given. The control's parts are measured from its own corner, so when a part
  // goes above or left of that corner, the corner moves there and every part is measured afresh.
  function keepPart(item, partName, part) {
    const { shift, parts: fromCorner } = edits.partsFromTheirCorner({ ...item.parts, [partName]: part });
    const corner = { x: item.area.x + shift.x, y: item.area.y + shift.y };
    change(edits.withParts(edits.moved(editing.look, pageName(), item, corner), pageName(), item, fromCorner));
  }

  // Keeps where a part sits and how big it is, measured inside its control.
  function keepPartArea(held, within) {
    const area = wholePixels(within);
    keepPart(held.item, held.part, held.part === "art" ? area : { ...held.item.parts[held.part], area });
  }

  // --- What is held by its handles: one part of a control, or the one thing selected -----------

  function heldPart() {
    const item = editing.heldPart && editing.items.find((known) => keyOf(known) === editing.heldPart.key);
    const area = item && item.parts && (editing.heldPartPreview || areaOfPart(item, editing.heldPart.part));
    if (!area) return null;
    const part = editing.heldPart.part;
    return { item, part, area, resize: part === "art" ? item.artResize : "free", tilt: 0, tilts: false };
  }

  function held() {
    if (editing.heldPart) return heldPart();
    const items = selectedItems();
    const canBeResized = toolInHand() === MOVE && items.length === 1 && items[0].resize;
    if (!canBeResized) return null;
    return { item: items[0], part: null, area: items[0].area, resize: items[0].resize, tilt: tiltOf(items[0]), tilts: Boolean(items[0].canTilt) };
  }

  // Every handle of what is held: the ones that resize it and, for artwork, the one above it that tilts it.
  function handlesOfHeld(holding) {
    if (!holding || !holding.resize) return [];
    const resizers = handlesOf(holding.area, holding.resize === "shape", holding.tilt);
    return holding.tilts ? [...resizers, tiltHandleOf(holding.area, holding.tilt)] : resizers;
  }

  function clientPointOf(point) {
    const shown = editing.stage.getBoundingClientRect();
    const scale = shown.width / PAGE.width;
    return { x: shown.left + point.x * scale, y: shown.top + point.y * scale };
  }

  // The handle of whatever is held that the pointer is on, if it is on one.
  function handleAt(client) {
    const holding = held();
    const isNear = (handle) => { const at = clientPointOf(handle); return Math.hypot(at.x - client.x, at.y - client.y) <= HANDLE_REACH; };
    const handle = handlesOfHeld(holding).find(isNear);
    return handle ? { ...holding, handle: handle.name } : null;
  }

  function showHeldAt(area) {
    const holding = held();
    if (!holding) return;
    if (holding.part) editing.heldPartPreview = area;
    else showAt(holding.item, whereItMayGo(holding.item, area));
    drawSelection();
    if (!holding.part) drawItemPanel();
  }

  function settleHeld() {
    const holding = held();
    const draggedTo = editing.heldPartPreview;
    editing.heldPartPreview = null;
    if (holding && holding.part && draggedTo) return keepPartArea(holding, withinItem(holding.item, draggedTo));
    settle();
  }

  // --- Tilting artwork --------------------------------------------------------------------------

  function showTilted(item, degrees) {
    item.tilt = degrees;
    item.made.style.transform = degrees ? `rotate(${degrees}deg)` : "";
  }

  // Shows what is held tilted this far. Nothing is kept until `settleHeldTilt`.
  function showHeldTilted(degrees) {
    const holding = held();
    if (!holding || !holding.tilts) return;
    showTilted(holding.item, degrees);
    drawSelection();
    drawItemPanel();
  }

  function settleHeldTilt() {
    const holding = held();
    if (holding && holding.tilts) tiltTo([holding.item], tiltOf(holding.item));
  }

  // Tilts every piece of artwork given to the same angle, each about its own middle, as one change.
  function tiltTo(items, degrees) {
    const tilted = (look, item) => edits.withLayer(look, pageName(), withoutEmptyFields({ ...item.layer, tilt: degrees || undefined }));
    const artwork = items.filter((item) => item.canTilt && (item.layer.tilt || 0) !== degrees);
    if (artwork.length) change(artwork.reduce(tilted, editing.look));
  }

  // --- Artwork layers --------------------------------------------------------------------------

  // Lays a new layer on the page, in front of the other artwork unless asked to go behind it, and selects it.
  function addLayer(layer, where) {
    const id = edits.nextLayerId(editing.look, pageName());
    const lay = where && where.atTheBack ? edits.withLayerAtTheBack : edits.withLayer;
    editing.selectedKeys = new Set([`layer:${id}`]);
    editing.heldPart = null;
    change(lay(editing.look, pageName(), { ...withoutEmptyFields(layer), id }));
  }

  function changeLayer(item, fields) {
    change(edits.withLayer(editing.look, pageName(), withoutEmptyFields({ ...item.layer, ...fields })));
  }

  function changeIfDifferent(look) {
    if (look !== editing.look) change(look);
  }

  function restack(item, step) {
    changeIfDifferent(edits.restacked(editing.look, pageName(), item.layer.id, step));
  }

  function restackToThe(end, item) {
    changeIfDifferent(edits.restackedToThe(end, editing.look, pageName(), item.layer.id));
  }

  function duplicate(item) {
    const { id, ...copy } = item.layer;
    const nudged = (corner) => ({ ...corner, x: corner.x + COPY_SITS_THIS_FAR_OFF, y: corner.y + COPY_SITS_THIS_FAR_OFF });
    addLayer(copy.kind === "words" ? { ...copy, place: nudged(copy.place) } : { ...copy, area: nudged(copy.area) });
  }

  // --- Pictures brought in, and painting -------------------------------------------------------

  function shrunkSize(picture) {
    const shrink = Math.min(1, LONGEST_PICTURE_SIDE / Math.max(picture.width, picture.height));
    return { width: Math.max(1, Math.round(picture.width * shrink)), height: Math.max(1, Math.round(picture.height * shrink)) };
  }

  // Any picture the page can read becomes a PNG no longer than LONGEST_PICTURE_SIDE on either side.
  async function asPng(file) {
    const picture = await createImageBitmap(file);
    const canvas = Object.assign(document.createElement("canvas"), shrunkSize(picture));
    canvas.getContext("2d").drawImage(picture, 0, 0, canvas.width, canvas.height);
    return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
  }

  // Hands a picture to the app to keep with the plugin. The answer says the name it was kept under and its size.
  async function keepArt(png) {
    await beginSessionOnce();
    const bytes = new Uint8Array(await png.arrayBuffer());
    const kept = await send("add_artwork", bytes, { "plugin-name": encodeURIComponent(pluginName()), "file-name": "art.png" });
    editing.artSizes.set(kept.file, { width: kept.width, height: kept.height });
    return kept;
  }

  async function keepChosenPicture(file) {
    try {
      return await keepArt(await asPng(file));
    } catch (problem) {
      say(problem instanceof Error ? NOT_A_PICTURE : describe(problem), true);
      return null;
    }
  }

  // Opens the computer's file chooser. The answer is the picture once it is kept, or nothing if it could not be.
  function choosePicture() {
    return new Promise((resolve) => {
      editing.whenPictureChosen = resolve;
      parts.picker.click();
    });
  }

  // Hears that the file chooser has a file, and answers whoever opened it.
  function pictureChosen() {
    const file = parts.picker.files[0];
    const answer = editing.whenPictureChosen;
    parts.picker.value = "";
    editing.whenPictureChosen = null;
    if (answer) answer(file ? keepChosenPicture(file) : null);
  }

  async function sizeOfArt(file) {
    if (!editing.artSizes.has(file)) editing.artSizes.set(file, await ask("art_size", { name: pluginName(), file }));
    return editing.artSizes.get(file);
  }

  function imageOfLayer(layer) {
    return loaded(addressOfDrawing("layer_image", { name: pluginName(), layer: layerToDraw(layer) }));
  }

  // A canvas the size of the page to paint on, laid over the page. When a layer is being painted on, the
  // canvas takes over showing what that layer already holds, fully solid, as soon as it has arrived.
  function easelOver(target) {
    const canvas = Object.assign(element("canvas", "edit-easel"), { width: PAGE.width, height: PAGE.height });
    const easel = { canvas, painted: null, ready: Promise.resolve() };
    editing.stage.insertBefore(canvas, editing.overlays.knobPanel);
    if (target) {
      easel.ready = imageOfLayer({ ...target.layer, opacity: undefined, hidden: undefined }).then((image) => {
        easel.painted = image;
        target.made.style.visibility = "hidden";
      }, () => {});
    }
    return easel;
  }

  // Keeps what was painted as a picture and answers with the name it was kept under.
  // If it cannot be kept, the page is drawn again as it was and the answer is nothing.
  async function keepPainting(canvas) {
    try {
      const png = await new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
      return (await keepArt(png)).file;
    } catch (problem) {
      draw();
      say(describe(problem), true);
      return null;
    }
  }

  // --- Picking up a colour ---------------------------------------------------------------------

  // Everything the MPC's page itself is made of, from the back to the front. What the editor draws over it is left out.
  function drawnOnThePage() {
    const onTheStage = Array.from(editing.stage.querySelectorAll("*")).filter((made) => !made.closest(".edit-layer, .edit-easel"));
    return [editing.stage, ...onTheStage];
  }

  // The colour showing at a place in the window: of artwork, a picture or a control's image. Bare page gives the page's own colour.
  async function colourAt(client) {
    return (await colourShownAt(drawnOnThePage(), client)) || pageColourOf(editing.plugin);
  }

  function pickUp(which, colour) {
    editing.colours = { ...editing.colours, [which]: colour };
    drawTools();
  }

  // Picks up the colour at a place in the window as the front colour, unless the back one is asked for.
  async function pickUpAt(client, which) {
    pickUp(which || "front", await colourAt(client));
  }

  function swapColours() {
    editing.colours = { front: editing.colours.back, back: editing.colours.front };
    drawTools();
  }

  function resetColours() {
    editing.colours = { ...WHITE_ON_BLACK };
    drawTools();
  }

  // --- Controls put on the page or taken off it ------------------------------------------------

  // A control with a few fixed settings is shown as a switch, and any other as a knob.
  function widgetFor(controlName) {
    const known = ((editing.plugin.parameters || {}).params || []).find((parameter) => parameter.name === controlName) || {};
    const choices = (known.choices || []).length;
    return known.kind === "bool" || (choices >= 2 && choices <= 4) ? "switch" : "knob";
  }

  function addControl(controlName) {
    editing.selectedKeys = new Set([`control:${controlName}`]);
    editing.heldPart = null;
    change(edits.withControlAdded(editing.look, pageName(), controlName, widgetFor(controlName)));
  }

  // Artwork is deleted. A control or a heading is taken off the page, and one of the AI's pictures is hidden from it:
  // those three can all be put back from the shelf.
  function lookWithout(look, item) {
    if (item.kind === "layer") return edits.withoutLayers(look, pageName(), [item.layer.id]);
    if (item.kind === "picture") return edits.withPictureHidden(look, pageName(), item);
    return edits.withItemTakenOff(look, pageName(), item, item.slot, item.entry);
  }

  function remove(items) {
    if (items.length) change(items.reduce(lookWithout, editing.look));
  }

  function stylePicture(item, style) {
    changeIfDifferent(edits.withPictureStyled(editing.look, pageName(), item, style));
  }

  function showPicture(name) {
    editing.selectedKeys = new Set([`picture:${name}`]);
    editing.heldPart = null;
    change(edits.putBack(editing.look, pageName(), { kind: "picture", name }));
  }

  // --- One page's design copied to another -----------------------------------------------------

  function hasASlot(item) {
    return item.slot !== undefined;
  }

  // What sits in the slots of another page, without drawing that page where the musician can see it.
  function slotItemsOnPage(pageIndex) {
    return drawForArranging(document.createElement("section"), editing.plugin, pageIndex).items.filter(hasASlot);
  }

  // Each thing in a slot of this page beside whatever sits in the same slot of the other page.
  function pairsWithPage(pageIndex) {
    const there = slotItemsOnPage(pageIndex);
    return editing.items.filter(hasASlot).flatMap((from) => {
      const to = there.find((item) => item.slot === from.slot);
      return to ? [{ from, to }] : [];
    });
  }

  // Copies what was ticked to another page, and turns to that page so the result can be seen.
  function copyDesignTo(pageIndex, what) {
    const pages = { from: pageName(), to: pageNamesOf(editing.plugin)[pageIndex] };
    const pairs = what.withControls ? pairsWithPage(pageIndex) : [];
    changeIfDifferent(edits.withDesignCopied(editing.look, pages, { layers: what.layers, replacesArtwork: what.replacesArtwork, pairs }));
    showPage(pageIndex);
  }

  function copyDesign() {
    settle();
    openCopySheet(parts.sheet, desk);
  }

  // --- The menu a right-click opens ------------------------------------------------------------

  function selectEverything() {
    select(editing.items.filter((item) => !item.pickedFromListOnly));
  }

  // Picks the Parts tool with the control selected, ready for one of its parts to be clicked.
  function layOutParts(item) {
    editing.selectedKeys = new Set([keyOf(item)]);
    chooseTool(PARTS);
  }

  // Opens the menu for what was right-clicked. Something that was not selected becomes the one thing selected.
  function openMenuAbout(press) {
    if (press.item && !isSelected(press.item)) select([press.item]);
    openMenu(parts.menu, press.client || press.at, rowsFor(desk, press));
  }

  function menuAskedOnThePage(event) {
    event.preventDefault();
    if (!editing.stage) return;
    settle();
    openMenuAbout(pressOf(event));
  }

  // --- The view: zooming and sliding -----------------------------------------------------------

  // The widest the page can be drawn with all of it in view.
  function widthThatFits() {
    const wide = parts.view.clientWidth - ROOM_ROUND_THE_PAGE * 2;
    const tall = (parts.view.clientHeight - ROOM_ROUND_THE_PAGE * 2) * (PAGE.width / PAGE.height);
    return Math.max(SMALLEST_PAGE_WIDTH, Math.min(wide, tall));
  }

  function showAtZoom() {
    parts.canvas.style.width = `${Math.round(widthThatFits() * editing.zoom)}px`;
    parts.zoom.textContent = `${Math.round(editing.zoom * 100)}%`;
  }

  // Slides the view so the given point of the page is under the given place in the window.
  function bringUnder(client, point) {
    const now = clientPointOf(point);
    parts.view.scrollLeft += now.x - client.x;
    parts.view.scrollTop += now.y - client.y;
  }

  // Zooms so that whatever is under `client` stays under it. Without `client`, the view just grows or shrinks.
  function zoomTo(zoom, client) {
    const stayingStill = client ? pagePoint(editing.stage, client) : null;
    editing.zoom = Math.min(MOST_ZOOM, Math.max(1, zoom));
    showAtZoom();
    drawOptions();
    if (stayingStill) bringUnder(client, stayingStill);
  }

  function zoomBy(factor, client) {
    zoomTo(editing.zoom * factor, client);
  }

  function panBy(dx, dy) {
    parts.view.scrollLeft += dx;
    parts.view.scrollTop += dy;
  }

  // --- What the tools and the panels work through ----------------------------------------------

  function setOption(name, value) {
    editing.options = { ...editing.options, [name]: value };
    drawOptions();
  }

  function toggleSnap() {
    editing.snap = !editing.snap;
    drawOptions();
  }

  function toggleKnobPanel() {
    editing.showsKnobPanel = !editing.showsKnobPanel;
    drawKnobPanel();
    drawOptions();
  }

  function focusField(name) {
    editing.fieldToFocus = name;
    drawItemPanel();
  }

  const desk = {
    // What is on the page and what is selected.
    plugin: () => editing.plugin,
    look: () => editing.look,
    pageName,
    items: () => editing.items,
    selected: selectedItems,
    isSelected,
    select,
    selectAlso,
    selectEverything,
    shownNameOf,
    pageNames: () => pageNamesOf(editing.plugin),
    itemsTouching: (box) => editing.items.filter((item) => !item.pickedFromListOnly && !isHiddenLayer(item) && touches(item.area, box)),
    areasOutsideSelection: () => editing.items.filter((item) => !isSelected(item) && !item.pickedFromListOnly && !isHiddenLayer(item)).map((item) => item.area),
    // Moving and resizing.
    snapIsOn: () => editing.snap,
    toggleSnap,
    knobPanelShows: () => editing.showsKnobPanel,
    toggleKnobPanel,
    slideSelection,
    settle,
    lineUp,
    spaceOut,
    placeItem,
    resizedTo: (item, side, pixels) => sideSetTo(item.area, item.resize === "shape", side, pixels),
    rename,
    putBack,
    held,
    handleAt,
    showHeldAt,
    settleHeld,
    showHeldTilted,
    settleHeldTilt,
    tiltTo,
    // The parts inside a control.
    partAt,
    selectPart,
    keepPart,
    keepPartArea,
    partResizedTo: (holding, within, side, pixels) => sideSetTo(within, holding.resize === "shape", side, pixels),
    // Artwork.
    addLayer,
    changeLayer,
    restack,
    restackToThe,
    duplicate,
    choosePicture,
    sizeOfArt,
    easelOver,
    keepPainting,
    colours: () => editing.colours,
    pickUp,
    pickUpAt,
    fonts: () => editing.fonts,
    options: () => editing.options,
    setOption,
    // Things put on the page and taken off it.
    addControl,
    showPicture,
    stylePicture,
    remove,
    layOutParts,
    // One page's design copied to another.
    copyDesign,
    copyDesignTo,
    // The menu for one thing, opened at a place in the window by a right-click in a list.
    openMenuFor: (item, at) => openMenuAbout({ item, at }),
    // What is drawn over the page, the view, and words for the musician.
    showGuides,
    showBox,
    showLine,
    panBy,
    zoomBy,
    zoomToFit: () => zoomTo(1),
    zoom: () => ({ isSmallest: editing.zoom <= 1, isLargest: editing.zoom >= MOST_ZOOM }),
    say,
    focusField,
    fieldToFocus: () => editing.fieldToFocus,
  };

  function pressOf(event) {
    const client = { x: event.clientX, y: event.clientY };
    const point = pagePoint(editing.stage, client);
    return { client, point, item: itemAt(point), shift: event.shiftKey, alt: event.altKey };
  }

  // Follows the pointer until it is let go, wherever it wanders.
  function follow(gesture, pointerId) {
    const dragged = (move) => gesture.dragged(pressOf(move));
    const letGo = (up) => {
      parts.canvas.removeEventListener("pointermove", dragged);
      parts.canvas.removeEventListener("pointerup", letGo);
      parts.canvas.removeEventListener("pointercancel", letGo);
      gesture.released(pressOf(up));
    };
    parts.canvas.setPointerCapture(pointerId);
    parts.canvas.addEventListener("pointermove", dragged);
    parts.canvas.addEventListener("pointerup", letGo);
    parts.canvas.addEventListener("pointercancel", letGo);
  }

  function pointerPressed(down) {
    if (down.button !== 0 || !editing.stage) return;
    settle();
    editing.fieldToFocus = null;
    const gesture = toolInHand().pressed(desk, pressOf(down));
    if (gesture) follow(gesture, down.pointerId);
  }

  // --- Drawing: the page and what is drawn over it ---------------------------------------------

  function overlay(stage, name) {
    const made = element("div", `edit-layer ${name}`);
    stage.append(made);
    return made;
  }

  // Lines that are not level or upright, such as the one a fade is dragged along, are drawn here.
  function lineOverlay(stage) {
    const drawing = document.createElementNS(SVG, "svg");
    drawing.setAttribute("class", "edit-layer lines");
    drawing.setAttribute("viewBox", `0 0 ${PAGE.width} ${PAGE.height}`);
    stage.append(drawing);
    return drawing;
  }

  function drawStage() {
    const drawn = drawForArranging(parts.canvas, editing.plugin, editing.pageIndex);
    editing.stage = drawn.stage;
    editing.items = drawn.items.map((item) => ({ ...item, settledArea: item.area }));
    editing.overlays = {
      knobPanel: overlay(drawn.stage, "knob-panel"),
      outlines: overlay(drawn.stage, "outlines"),
      guides: overlay(drawn.stage, "guides"),
      band: overlay(drawn.stage, "band"),
      lines: lineOverlay(drawn.stage),
    };
    drawKnobPanel();
    drawSelection();
  }

  // Marks the part of the page the MPC's own panel covers while a hardware knob is turned, when asked to.
  function drawKnobPanel() {
    const covered = placeAt(element("div", "edit-covered", "THE MPC COVERS THIS WHILE A HARDWARE KNOB IS TURNED"), COVERED_BY_THE_KNOB_PANEL);
    editing.overlays.knobPanel.replaceChildren(...(editing.showsKnobPanel ? [covered] : []));
  }

  function handleMark(handle) {
    const className = handle.name === TILT_HANDLE ? "edit-handle tilts" : "edit-handle";
    return placeAt(element("div", className), { x: handle.x, y: handle.y, width: 0, height: 0 });
  }

  // An outline tilts with what it is drawn round.
  function outlineOf(area, className, tilt) {
    const outline = placeAt(element("div", className), area);
    if (tilt) outline.style.transform = `rotate(${tilt}deg)`;
    return outline;
  }

  // An outline round everything selected, a brighter one round a part that is held, and the handles to resize by.
  function drawSelection() {
    const holding = held();
    const outlines = selectedItems().map((item) => outlineOf(item.area, holding && holding.part ? "edit-outline faint" : "edit-outline", tiltOf(item)));
    const partOutline = holding && holding.part ? [outlineOf(holding.area, "edit-outline part", 0)] : [];
    editing.overlays.outlines.replaceChildren(...outlines, ...partOutline, ...handlesOfHeld(holding).map(handleMark));
  }

  // A guide is a line right across the page, showing what the thing being moved has lined up with.
  function guideLine(guide) {
    const across = guide.axis === "x";
    const where = across ? { x: guide.at, y: 0, width: 0, height: PAGE.height } : { x: 0, y: guide.at, width: PAGE.width, height: 0 };
    return placeAt(element("div", `edit-guide ${across ? "upright" : "level"}`), where);
  }

  function showGuides(guides) {
    editing.overlays.guides.replaceChildren(...guides.map(guideLine));
  }

  function showBox(box) {
    editing.overlays.band.replaceChildren(...(box ? [placeAt(element("div", "edit-band"), box)] : []));
  }

  // Shows a line from one point of the page to another, or takes it away when given nothing.
  function showLine(from, to) {
    if (!from) return editing.overlays.lines.replaceChildren();
    const line = document.createElementNS(SVG, "line");
    [["x1", from.x], ["y1", from.y], ["x2", to.x], ["y2", to.y], ["class", "edit-line"]].forEach(([name, value]) => line.setAttribute(name, String(value)));
    editing.overlays.lines.replaceChildren(line);
  }

  // --- Drawing: the rows of buttons ------------------------------------------------------------

  function showPage(index) {
    settle();
    editing.pageIndex = index;
    editing.selectedKeys = new Set();
    editing.heldPart = null;
    draw();
  }

  function pageTab(name, index) {
    const tab = element("button", "editor-page", name);
    tab.type = "button";
    tab.addEventListener("click", () => showPage(index));
    if (index === editing.pageIndex) tab.setAttribute("aria-current", "page");
    return tab;
  }

  function drawPages() {
    parts.pages.replaceChildren(...pageNamesOf(editing.plugin).map(pageTab));
  }

  function drawUndoButtons() {
    parts.undo.disabled = !editing.history.canUndo();
    parts.redo.disabled = !editing.history.canRedo();
  }

  // Copying a design needs another page to copy it to.
  function drawCopyButton() {
    parts.copyDesign.hidden = pageNamesOf(editing.plugin).length < 2;
  }

  function drawOptions() {
    parts.options.replaceChildren(...toolInHand().strip(desk));
  }

  function chooseTool(tool) {
    editing.tool = tool;
    editing.heldPart = null;
    drawTools();
    drawSelection();
    drawItemPanel();
    say(tool.hint);
  }

  function drawTools() {
    const colours = { ...editing.colours, onPicked: pickUp, onSwapped: swapColours, onReset: resetColours };
    drawPalette(parts.palette, TOOL_GROUPS, toolInHand(), { onChosen: chooseTool, colours });
    parts.canvas.dataset.tool = toolInHand().id;
    drawOptions();
  }

  // --- Drawing: the panels on the right --------------------------------------------------------

  function drawItemPanel() {
    drawItem(parts.item, desk);
  }

  function drawPanels() {
    drawLayers(parts.layers, desk);
    drawItemPanel();
    comment.draw();
    drawShelf(parts.shelf, desk);
  }

  function draw() {
    drawPages();
    drawCopyButton();
    drawStage();
    drawTools();
    drawPanels();
    drawUndoButtons();
  }

  // The page is drawn afresh from what the app kept, unless something newer is still on its way there.
  function drawOnceEverythingIsKept() {
    const stillChanging = editing.savesWaiting > 0 || editing.items.some(hasUnsettledChange);
    if (stillChanging) return drawUndoButtons();
    draw();
  }

  // --- The keyboard ----------------------------------------------------------------------------

  function isTyping(event) {
    return event.target instanceof HTMLElement && event.target.matches("input, textarea, select");
  }

  function duplicateSelection() {
    const artwork = selectedItems().filter((item) => item.kind === "layer");
    if (artwork.length === 1) duplicate(artwork[0]);
  }

  function commandFor(event) {
    const commands = {
      z: event.shiftKey ? redo : undo,
      a: selectEverything,
      d: duplicateSelection,
      "=": () => zoomBy(ZOOM_STEP),
      "+": () => zoomBy(ZOOM_STEP),
      "-": () => zoomBy(1 / ZOOM_STEP),
      0: () => zoomTo(1),
    };
    return commands[event.key.toLowerCase()];
  }

  // Holding Space borrows the Hand tool, whichever tool is chosen.
  function holdHand(isHeld) {
    const borrowed = isHeld ? HAND : null;
    if (editing.heldTool === borrowed) return;
    editing.heldTool = borrowed;
    drawTools();
  }

  function restackSelection(step) {
    const artwork = selectedItems().filter((item) => item.kind === "layer");
    if (artwork.length === 1) restack(artwork[0], step);
  }

  function plainKeyPressed(event) {
    const pressed = {
      " ": () => holdHand(true),
      Escape: () => select([]),
      Delete: () => remove(selectedItems()),
      Backspace: () => remove(selectedItems()),
      x: swapColours,
      d: resetColours,
      "]": () => restackSelection(1),
      "[": () => restackSelection(-1),
    }[event.key.length === 1 ? event.key.toLowerCase() : event.key];
    if (pressed) return pressed();
    if (ARROW_KEYS[event.key]) return nudgeSelection(ARROW_KEYS[event.key], event.shiftKey ? BIG_NUDGE : NUDGE);
    const tool = toolForKey(TOOLS, event.key);
    if (tool) chooseTool(tool);
  }

  // While the copy sheet is up, the keys belong to it.
  function keyPressed(event) {
    if (event.key === "Escape" && isOpen() && !parts.sheet.hidden) return closeSheet();
    if (!isOpen() || !parts.sheet.hidden || isTyping(event) || event.altKey) return;
    const command = (event.metaKey || event.ctrlKey) && commandFor(event);
    const handled = command || [" ", "Backspace", "Delete"].includes(event.key) || ARROW_KEYS[event.key];
    if (handled) event.preventDefault();
    if (event.metaKey || event.ctrlKey) return command && command();
    plainKeyPressed(event);
  }

  function closeSheet() {
    parts.sheet.hidden = true;
    parts.sheet.replaceChildren();
  }

  function keyLetGo(event) {
    if (!isOpen() || event.key !== " ") return;
    if (!isTyping(event)) event.preventDefault();
    holdHand(false);
  }

  // --- Opening and closing ---------------------------------------------------------------------

  // The fonts on this computer are asked for once. Words start in a plain one that the computer has.
  async function learnFonts() {
    const found = await ask("font_names").catch(() => []);
    if (!found.length) return;
    editing.fonts = found;
    const plain = FONTS_TO_START_WITH.find((name) => found.includes(name)) || found[0];
    if (!found.includes(editing.options.font)) editing.options = { ...editing.options, font: plain };
    if (isOpen()) drawOptions();
  }

  // Opens the editor on one page of a plugin's screen.
  function open(plugin, pageIndex) {
    Object.assign(editing, {
      plugin,
      look: plugin.look || { pages: [] },
      pageIndex,
      selectedKeys: new Set(),
      heldPart: null,
      heldPartPreview: null,
      heldTool: null,
      zoom: 1,
      history: edits.lookHistory(),
      sessionBegun: false,
      madeChanges: false,
      artSizes: new Map(),
    });
    parts.window.hidden = false;
    document.body.classList.add("editing");
    parts.title.textContent = plugin.summary.name;
    comment.clear();
    showAtZoom();
    draw();
    say(editing.tool.hint);
  }

  // Closes the editor once every change is kept, and says which page was showing and whether anything changed.
  // `request` is what the musician asked the AI to do about something on the page, when they asked.
  async function close(request) {
    settle();
    await editing.saving;
    parts.window.hidden = true;
    document.body.classList.remove("editing");
    listeners.onDone({ pageIndex: editing.pageIndex, madeChanges: editing.madeChanges, request });
  }

  // `givenParts` are the editor's elements on the page. `givenListeners.onDone` hears when the musician is finished.
  function start(givenParts, givenListeners) {
    parts = givenParts;
    listeners = givenListeners;
    comment = window.editorComment.start(parts.comment, desk, close);
    parts.canvas.addEventListener("pointerdown", pointerPressed);
    parts.canvas.addEventListener("contextmenu", menuAskedOnThePage);
    parts.copyDesign.addEventListener("click", copyDesign);
    parts.undo.addEventListener("click", undo);
    parts.redo.addEventListener("click", redo);
    parts.done.addEventListener("click", () => close());
    parts.picker.addEventListener("change", pictureChosen);
    document.addEventListener("keydown", keyPressed);
    document.addEventListener("keyup", keyLetGo);
    new ResizeObserver(() => { if (isOpen()) showAtZoom(); }).observe(parts.view);
    learnFonts();
  }

  window.lookEditor = { start, open, isOpen };
})();
