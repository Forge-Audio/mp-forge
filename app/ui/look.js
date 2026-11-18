// What the musician did by hand to a plugin's screen: where they put things, what they renamed and resized,
// how they laid out the inside of a control, the artwork behind each page, what they added or took off, and
// the hardware knobs they chose for a page's controls.
//
// A look holds only what was changed, page by page, in the shape the app keeps in the plugin's look.json.
// Nothing here draws or talks to the app. Every function hands back a new look and leaves the old one alone.

(function () {
  "use strict";

  const SMALLEST_LETTERS = 8;
  const SMALLEST_WORDS = 6;
  const SMALLEST_SIDE = 8;
  const FULLY_SOLID = 100;

  // --- Pages -----------------------------------------------------------------------------------

  function pageOf(look, pageName) {
    const known = look.pages.find((page) => page.page === pageName) || {};
    return { page: pageName, items: known.items || [], artwork: known.artwork || [], added: known.added || [], removed: known.removed || [], knobs: known.knobs || [] };
  }

  function saysNothing(page) {
    return !page.items.length && !page.artwork.length && !page.added.length && !page.removed.length && !page.knobs.length;
  }

  // The look with one page replaced. A page with nothing left to say is dropped, and empty lists are not written.
  function withPage(look, page) {
    const others = look.pages.filter((known) => known.page !== page.page);
    if (saysNothing(page)) return { pages: others };
    const written = { page: page.page, items: page.items };
    ["artwork", "added", "removed", "knobs"].filter((list) => page[list].length).forEach((list) => { written[list] = page[list]; });
    return { pages: [...others, written] };
  }

  // --- The hardware knobs ----------------------------------------------------------------------

  // The knob each control has once `controlName` is turned by `knob`. Whatever that knob turned before takes the
  // knob the control gives up, so no control is ever left without one.
  function knobsAfterGiving(controls, controlName, knob) {
    const taking = controls.find((control) => control.name === controlName);
    const givingWay = controls.find((control) => control.knob === knob && control !== taking);
    return controls.map((control) => {
      if (control === taking) return { ...control, knob };
      return control === givingWay ? { ...control, knob: taking.knob } : control;
    });
  }

  // The look once a control of the page is turned by another hardware knob. `controls` are the page's controls,
  // each with its `name`, the `knob` that turns it now and its `ownKnob`, the one beside its slot.
  // Only the controls that are not on their own knob are written down.
  function withKnobGiven(look, pageName, controls, controlName, knob) {
    const offTheirOwn = knobsAfterGiving(controls, controlName, knob).filter((control) => control.knob !== control.ownKnob);
    const chosen = offTheirOwn.map((control) => ({ control: control.name, knob: control.knob }));
    return withPage(look, { ...pageOf(look, pageName), knobs: chosen });
  }

  // --- Controls, headings and the AI's pictures ------------------------------------------------

  function isSameItem(change, item) {
    return change.kind === item.kind && change.name === item.name;
  }

  // What was changed about one item, or undefined when it is as the screen's own layout has it.
  function changeOf(look, pageName, item) {
    return pageOf(look, pageName).items.find((change) => isSameItem(change, item));
  }

  function withoutEmptyFields(change) {
    return Object.fromEntries(Object.entries(change).filter(([, value]) => value !== null && value !== undefined));
  }

  function saysSomething(change) {
    const saysOfAPicture = change.hidden || change.depth || change.opacity !== undefined;
    return Boolean(change.place || change.shownName || change.size || change.parts || saysOfAPicture);
  }

  // The look with `changes` laid over what was already changed about the item. A field set to null is taken away,
  // and an item with nothing left to say about it is dropped from the look.
  function withChange(look, pageName, item, changes) {
    const page = pageOf(look, pageName);
    const before = changeOf(look, pageName, item) || { kind: item.kind, name: item.name };
    const after = withoutEmptyFields({ ...before, ...changes });
    const others = page.items.filter((change) => !isSameItem(change, item));
    return withPage(look, { ...page, items: saysSomething(after) ? [...others, after] : others });
  }

  function cornerOf(area) {
    return { x: Math.round(area.x), y: Math.round(area.y) };
  }

  function moved(look, pageName, item, corner) {
    return withChange(look, pageName, item, { place: cornerOf(corner) });
  }

  // An empty name means the item goes back to showing its own.
  function renamed(look, pageName, item, shownName) {
    return withChange(look, pageName, item, { shownName: shownName || null });
  }

  function withParts(look, pageName, item, parts) {
    return withChange(look, pageName, item, { parts });
  }

  function putBack(look, pageName, item) {
    return withChange(look, pageName, item, { place: null, shownName: null, size: null, parts: null, hidden: null, opacity: null, depth: null });
  }

  // How solid one of the AI's pictures is, out of 100, and how far back it sits: "front" is over the controls,
  // "behind" is behind them but over the artwork, and "back" is behind everything. Either can be given alone.
  function withPictureStyled(look, pageName, picture, style) {
    const fullySolid = style.opacity === FULLY_SOLID;
    return withChange(look, pageName, picture, { ...style, ...(fullySolid ? { opacity: null } : {}) });
  }

  // A picture's style as the look has it. Where the look says nothing, it is solid and sits where the AI put it.
  function styleOfPicture(look, pageName, picture, ownDepth) {
    const change = changeOf(look, pageName, picture) || {};
    return { opacity: change.opacity === undefined ? FULLY_SOLID : change.opacity, depth: change.depth || ownDepth };
  }

  // One of the AI's pictures is never thrown away by hand. It is hidden from its page, and can be shown again.
  function withPictureHidden(look, pageName, picture) {
    return withChange(look, pageName, picture, { hidden: true });
  }

  function isHidden(look, pageName, picture) {
    return Boolean((changeOf(look, pageName, picture) || {}).hidden);
  }

  function isChanged(look, pageName, item) {
    return Boolean(changeOf(look, pageName, item));
  }

  // --- The parts inside a control --------------------------------------------------------------

  // `by` says how much wider and how much taller: { across, down }.
  function scaledArea(area, by) {
    const scaled = (pixels, share) => Math.round(pixels * share);
    return { x: scaled(area.x, by.across), y: scaled(area.y, by.down), width: Math.max(1, scaled(area.width, by.across)), height: Math.max(1, scaled(area.height, by.down)) };
  }

  // Letters only grow with a label's height, so a control stretched sideways keeps its lettering the size it was.
  function scaledLabel(label, by) {
    return label && { ...label, area: scaledArea(label.area, by), height: Math.max(SMALLEST_LETTERS, Math.round(label.height * by.down)) };
  }

  // Every part of a control made wider and taller together. Stretched the same both ways, the control keeps its shape.
  function scaledParts(parts, by) {
    return withoutEmptyFields({ art: parts.art && scaledArea(parts.art, by), name: scaledLabel(parts.name, by), value: scaledLabel(parts.value, by) });
  }

  // The parts shifted so the one furthest up and left starts at the control's own corner, with how far they were shifted.
  // Parts are measured from that corner and cannot sit before it, so the control itself moves instead.
  function partsFromTheirCorner(parts) {
    const areas = [parts.art, parts.name && parts.name.area, parts.value && parts.value.area].filter(Boolean);
    const shift = { x: Math.min(...areas.map((area) => area.x)), y: Math.min(...areas.map((area) => area.y)) };
    const shifted = (area) => ({ ...area, x: area.x - shift.x, y: area.y - shift.y });
    const label = (part) => part && { ...part, area: shifted(part.area) };
    return { shift, parts: withoutEmptyFields({ art: parts.art && shifted(parts.art), name: label(parts.name), value: label(parts.value) }) };
  }

  // --- Artwork layers --------------------------------------------------------------------------

  function layersOn(look, pageName) {
    return pageOf(look, pageName).artwork;
  }

  function withLayers(look, pageName, layers) {
    return withPage(look, { ...pageOf(look, pageName), artwork: layers });
  }

  function nextLayerId(look, pageName) {
    return Math.max(0, ...layersOn(look, pageName).map((layer) => layer.id)) + 1;
  }

  // The look with a layer put in place of the one with its id, or laid on top when it is new.
  function withLayer(look, pageName, layer) {
    const layers = layersOn(look, pageName);
    const isKnown = layers.some((known) => known.id === layer.id);
    return withLayers(look, pageName, isKnown ? layers.map((known) => (known.id === layer.id ? layer : known)) : [...layers, layer]);
  }

  // The look with a new layer laid behind all the others on the page.
  function withLayerAtTheBack(look, pageName, layer) {
    return withLayers(look, pageName, [layer, ...layersOn(look, pageName)]);
  }

  function withoutLayers(look, pageName, ids) {
    return withLayers(look, pageName, layersOn(look, pageName).filter((layer) => !ids.includes(layer.id)));
  }

  // Moves a layer one place towards the front (`step` 1) or the back (`step` -1), as far as it can go.
  function restacked(look, pageName, id, step) {
    const layers = layersOn(look, pageName).slice();
    const from = layers.findIndex((layer) => layer.id === id);
    const to = from + step;
    if (from < 0 || to < 0 || to >= layers.length) return look;
    [layers[from], layers[to]] = [layers[to], layers[from]];
    return withLayers(look, pageName, layers);
  }

  // Moves a layer right to the `front` of the page's artwork or right to the `back` of it.
  function restackedToThe(end, look, pageName, id) {
    const layers = layersOn(look, pageName);
    const layer = layers.find((known) => known.id === id);
    const others = layers.filter((known) => known !== layer);
    if (!layer) return look;
    return withLayers(look, pageName, end === "front" ? [...others, layer] : [layer, ...others]);
  }

  function wholePixels(area) {
    return { x: Math.round(area.x), y: Math.round(area.y), width: Math.max(1, Math.round(area.width)), height: Math.max(1, Math.round(area.height)) };
  }

  // A layer put over another area. Words have no box of their own, so stretching them changes the size of their letters.
  function layerOver(layer, from, to) {
    if (layer.kind !== "words") return { ...layer, area: wholePixels(to) };
    const size = Math.max(SMALLEST_WORDS, Math.round(layer.size * (to.height / from.height)));
    return { ...layer, place: cornerOf(to), size };
  }

  // --- Putting anything somewhere else, at any size --------------------------------------------

  function hasNewSize(from, to) {
    return Math.round(from.width) !== Math.round(to.width) || Math.round(from.height) !== Math.round(to.height);
  }

  function sizeOf(area) {
    return { width: Math.max(SMALLEST_SIDE, Math.round(area.width)), height: Math.max(SMALLEST_SIDE, Math.round(area.height)) };
  }

  // The look with one thing on the page moved from one area to another. What a new size means depends on what it is:
  // artwork and pictures are redrawn at it, and a control's parts all grow or shrink together.
  function placed(look, pageName, item, from, to) {
    if (item.kind === "layer") return withLayer(look, pageName, layerOver(item.layer, from, to));
    const resized = hasNewSize(from, to);
    if (item.kind === "picture") return withChange(look, pageName, item, { place: cornerOf(to), ...(resized ? { size: sizeOf(to) } : {}) });
    const rescaled = resized && item.parts ? { parts: scaledParts(item.parts, { across: to.width / from.width, down: to.height / from.height }) } : {};
    return withChange(look, pageName, item, { place: cornerOf(to), ...rescaled });
  }

  // --- Controls put on a page or taken off it --------------------------------------------------

  function withControlAdded(look, pageName, control, widget) {
    const page = pageOf(look, pageName);
    return withPage(look, { ...page, added: [...page.added.filter((added) => added.control !== control), { control, widget }] });
  }

  // Takes something off the page. `entry` is how the layout wrote it and `slot` where, so it can be put back.
  // Something the musician put on themselves is simply no longer added. Whatever was changed about it goes too.
  function withItemTakenOff(look, pageName, item, slot, entry) {
    const page = pageOf(look, pageName);
    const wasAdded = item.kind === "control" && page.added.some((added) => added.control === item.name);
    const added = page.added.filter((known) => !(item.kind === "control" && known.control === item.name));
    const removed = wasAdded ? page.removed : [...page.removed.filter((known) => known.slot !== slot), { slot, entry }];
    return withPage(look, { ...page, items: page.items.filter((change) => !isSameItem(change, item)), added, removed });
  }

  // --- One page's design copied to another -----------------------------------------------------

  function withLayersCopied(look, toPage, layers, replacesArtwork) {
    const kept = replacesArtwork ? [] : layersOn(look, toPage);
    const firstFreeId = Math.max(0, ...kept.map((layer) => layer.id)) + 1;
    const copies = layers.map((layer, position) => ({ ...layer, id: firstFreeId + position }));
    return withLayers(look, toPage, [...kept, ...copies]);
  }

  // What is changed about one item so that it matches another: it sits where the other sits and, when both
  // are made the same way, its parts are laid out the same. It keeps its own name.
  function changesToMatch(look, fromPage, pair) {
    const source = changeOf(look, fromPage, pair.from) || {};
    const madeAlike = pair.from.madeLike === pair.to.madeLike;
    return { place: source.place || null, ...(madeAlike ? { parts: source.parts || null } : {}) };
  }

  // The look with some of one page's design copied to another. `pages` names the two as { from, to }.
  // `what` lists the artwork `layers` to copy, whether that `replacesArtwork` already on the other page,
  // and the `pairs` of items, as { from, to }, that should end up laid out alike.
  function withDesignCopied(look, pages, what) {
    const withArtwork = withLayersCopied(look, pages.to, what.layers, what.replacesArtwork);
    return what.pairs.reduce((copied, pair) => withChange(copied, pages.to, pair.to, changesToMatch(look, pages.from, pair)), withArtwork);
  }

  // --- Undo and redo ---------------------------------------------------------------------------

  // The looks before and after the one on screen, so a change can be undone and done again.
  function lookHistory() {
    const earlier = [];
    const later = [];
    return {
      remember: (look) => { earlier.push(look); later.length = 0; },
      canUndo: () => earlier.length > 0,
      canRedo: () => later.length > 0,
      undo: (current) => { later.push(current); return earlier.pop(); },
      redo: (current) => { earlier.push(current); return later.pop(); },
    };
  }

  window.lookEdits = {
    changeOf, moved, renamed, putBack, isChanged, placed, withParts, scaledParts, partsFromTheirCorner, withPictureHidden, isHidden,
    withPictureStyled, styleOfPicture,
    layersOn, withLayer, withLayerAtTheBack, withoutLayers, restacked, restackedToThe, nextLayerId, withControlAdded, withItemTakenOff,
    withDesignCopied, withKnobGiven, lookHistory,
  };
})();
