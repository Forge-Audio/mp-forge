// The editor's tools for making artwork behind a page's controls: blocks of colour, words, pictures,
// fades, picking up a colour, cropping, and painting by hand.
//
// Each has the shape described in editor-tools.js. Artwork is kept as layers in the plugin's look, and the
// app draws all of a page's layers into the one background picture the MPC shows.

(function () {
  "use strict";

  const { PAGE } = window.mpcStage;
  const { boxBetween, keptOnPage, sharedBy } = window.editorGeometry;
  const { stripLabel, stripButton, stripChoice, numberInput, selectInput } = window.editorWidgets;
  const { isAClick } = window.editorTools;

  const SHAPES = [["rectangle", "RECTANGLE"], ["rounded", "ROUNDED"], ["ellipse", "ELLIPSE"]];
  const WORDS_TO_START_WITH = "WORDS";
  const LETTER_SIZES = { least: 6, most: 400 };
  const BRUSH_SIZES = { least: 1, most: 200 };
  const CORNER_SIZES = { least: 1, most: 400 };
  // A fade's ends are kept in hundredths of the block's size, and may lie this far outside it.
  const FURTHEST_FADE_END = 400;
  const SHORTEST_FADE = 4;

  const ICONS = {
    block: "M2 4h12v9H2z",
    words: "M3 2h10v3h-1V4H9v8h1v1H6v-1h1V4H4v1H3z",
    picture: "M1 3h14v10H1zM2 4v8h12V4zM3 11h10v-1h-1V9h-1V8h-1v1H9v1H8V9H7V8H6V7H5v1H4v1H3zM10 5h2v2h-2z",
    fade: "M1 3h4v10H1zM6 3h3v10H6zM10 3h2v10h-2zM13 3h1v10h-1z",
    picker: "M11 1h2v1h1v2h-1v1h-1v1h-1V5h-1V4h1V3h1zM9 5h1v1h1v1h-1v1H9v1H8v1H7v1H6v1H5v1H3v1H2v-1h1v-2h1v-1h1v-1h1V9h1V8h1V7h1z",
    crop: "M4 1h1v3h7v7h3v1h-3v3h-1v-3H4zM5 5v6h6V5zM1 4h2v1H1z",
    brush: "M12 1h2v1h1v2h-1v1h-1v1h-1v1h-1v1h-1v1H8V8H7V7h1V6h1V5h1V4h1V3h1zM5 9h2v1h1v2H7v1H6v1H2v-1h1v-1h1v-2h1z",
    eraser: "M9 2h1v1h1v1h1v1h1v1h1v2h-1v1h-1v1h-1v1h-1v1H9v1H5v-1H4v-1H3v-1H2V9h1V8h1V7h1V6h1V5h1V4h1V3h1zM2 14h12v1H2z",
  };

  function wholePixels(area) {
    return { x: Math.round(area.x), y: Math.round(area.y), width: Math.max(1, Math.round(area.width)), height: Math.max(1, Math.round(area.height)) };
  }

  function isLayerOf(item, kind) {
    return Boolean(item) && item.kind === "layer" && item.layer.kind === kind;
  }

  // The layer of one kind a tool should work on: the one under the pointer, or failing that the only one selected.
  function layerToWorkOn(workspace, press, kind) {
    if (isLayerOf(press.item, kind)) return press.item;
    const selected = workspace.selected();
    return selected.length === 1 && isLayerOf(selected[0], kind) ? selected[0] : null;
  }

  function onlySelected(workspace, kind) {
    return layerToWorkOn(workspace, {}, kind);
  }

  // A gesture that drags out a box and, once it is let go, hands the box over. A click hands over nothing.
  function draggingABox(workspace, press, onDrawn) {
    return {
      dragged: (now) => workspace.showBox(boxBetween(press.point, now.point)),
      released(now) {
        const box = boxBetween(press.point, now.point);
        workspace.showBox(null);
        onDrawn(isAClick(box) ? null : box);
      },
    };
  }

  // --- Colour block ----------------------------------------------------------------------------

  function blockOver(workspace, area) {
    const { shape, corner } = workspace.options();
    const rounding = shape === "rounded" ? { corner } : {};
    return { kind: "block", shape: shape === "ellipse" ? "ellipse" : "rectangle", area: wholePixels(area), colour: workspace.colours().front, ...rounding };
  }

  function blockStrip(workspace) {
    const { shape, corner } = workspace.options();
    const shapes = SHAPES.map(([name, words]) => stripChoice(words, shape === name, () => workspace.setOption("shape", name)));
    const rounding = shape === "rounded" ? [stripLabel("CORNER"), numberInput(corner, CORNER_SIZES, (pixels) => workspace.setOption("corner", pixels))] : [];
    const fillThePage = stripButton("FILL THE PAGE", () => workspace.addLayer(blockOver(workspace, PAGE), { atTheBack: true }));
    fillThePage.title = "Lays a block of the front colour behind everything on the page.";
    return [stripLabel("SHAPE"), ...shapes, ...rounding, stripLabel("OR"), fillThePage];
  }

  const block = {
    id: "block",
    name: "Colour block",
    key: "u",
    icon: ICONS.block,
    hint: "Drag out a block of the front colour. It goes behind the controls.",
    strip: blockStrip,
    pressed(workspace, press) {
      return draggingABox(workspace, press, (box) => {
        if (!box) return workspace.say("Drag across the page to draw a block.");
        workspace.addLayer(blockOver(workspace, box));
      });
    },
  };

  // --- Words -----------------------------------------------------------------------------------

  // What the strip's settings say about lettering. Changing one also changes the words that are selected.
  function setLettering(workspace, name, value) {
    workspace.setOption(name, value);
    const selected = onlySelected(workspace, "words");
    const fields = { font: { font: value }, wordSize: { size: value }, bold: { bold: value || undefined } }[name];
    if (selected) workspace.changeLayer(selected, fields);
  }

  function wordsStrip(workspace) {
    const { font, wordSize, bold } = workspace.options();
    const fonts = workspace.fonts().map((name) => [name, name]);
    return [
      stripLabel("FONT"),
      selectInput(font, fonts, (name) => setLettering(workspace, "font", name)),
      stripLabel("SIZE"),
      numberInput(wordSize, LETTER_SIZES, (size) => setLettering(workspace, "wordSize", size)),
      stripChoice("BOLD", bold, () => setLettering(workspace, "bold", !bold)),
    ];
  }

  function wordsAt(workspace, point) {
    const { font, wordSize, bold } = workspace.options();
    const lettering = bold ? { bold: true } : {};
    return { kind: "words", text: WORDS_TO_START_WITH, font, size: wordSize, ...lettering, colour: workspace.colours().front, place: { x: Math.round(point.x), y: Math.round(point.y) } };
  }

  const words = {
    id: "words",
    name: "Words",
    key: "t",
    icon: ICONS.words,
    hint: "Click to add words in any font on this computer, then type them on the right. Click words to change them.",
    strip: wordsStrip,
    pressed(workspace, press) {
      if (isLayerOf(press.item, "words")) workspace.select([press.item]);
      else workspace.addLayer(wordsAt(workspace, press.point));
      workspace.focusField("text");
    },
  };

  // --- Picture ---------------------------------------------------------------------------------

  function scaledToFit(art, room, grows) {
    const by = Math.min(room.width / art.width, room.height / art.height);
    return grows ? by : Math.min(1, by);
  }

  // The picture as large as fits inside the box, in its own shape, in the middle of it.
  function fittedInside(art, box) {
    const by = scaledToFit(art, box, true);
    const size = { width: art.width * by, height: art.height * by };
    return wholePixels({ x: box.x + (box.width - size.width) / 2, y: box.y + (box.height - size.height) / 2, ...size });
  }

  // The picture at its own size, shrunk only if it is bigger than the page, with its middle where asked.
  function atItsOwnSize(art, middle) {
    const by = scaledToFit(art, PAGE, false);
    const size = { width: art.width * by, height: art.height * by };
    return wholePixels(keptOnPage({ x: middle.x - size.width / 2, y: middle.y - size.height / 2, ...size }));
  }

  // The picture covering the whole page in its own shape: whatever hangs over the edges is cropped away.
  function coveringThePage(art) {
    const by = Math.max(PAGE.width / art.width, PAGE.height / art.height);
    const shown = { width: PAGE.width / by, height: PAGE.height / by };
    const crop = wholePixels({ x: (art.width - shown.width) / 2, y: (art.height - shown.height) / 2, ...shown });
    return { kind: "picture", file: art.file, area: { ...PAGE }, crop };
  }

  async function bringInPicture(workspace, layerFor, where) {
    const art = await workspace.choosePicture();
    if (art) workspace.addLayer(layerFor(art), where);
  }

  function pictureStrip(workspace) {
    const middleOfThePage = { x: PAGE.width / 2, y: PAGE.height / 2 };
    const ownSize = (art) => ({ kind: "picture", file: art.file, area: atItsOwnSize(art, middleOfThePage) });
    const asBackground = stripButton("AS THE BACKGROUND", () => bringInPicture(workspace, coveringThePage, { atTheBack: true }));
    asBackground.title = "Fills the page with a picture, behind everything else.";
    return [stripButton("CHOOSE A PICTURE", () => bringInPicture(workspace, ownSize)), stripLabel("OR"), asBackground];
  }

  const picture = {
    id: "picture",
    name: "Picture",
    key: "p",
    icon: ICONS.picture,
    hint: "Drag out a frame, or just click, then choose a picture file. It goes behind the controls.",
    strip: pictureStrip,
    pressed(workspace, press) {
      return draggingABox(workspace, press, (box) => {
        const area = (art) => (box ? fittedInside(art, box) : atItsOwnSize(art, press.point));
        bringInPicture(workspace, (art) => ({ kind: "picture", file: art.file, area: area(art) }));
      });
    },
  };

  // --- Fade ------------------------------------------------------------------------------------

  // Where a point is across a block, in hundredths of the block's size.
  function spotIn(area, point) {
    const hundredths = (distance, size) => Math.max(-FURTHEST_FADE_END, Math.min(FURTHEST_FADE_END, Math.round((distance / size) * 100)));
    return { x: hundredths(point.x - area.x, area.width), y: hundredths(point.y - area.y, area.height) };
  }

  function fadeAcross(workspace, area, from, to) {
    const { front, back } = workspace.colours();
    return { colour: front, fade: { colour: back, from: spotIn(area, from), to: spotIn(area, to) } };
  }

  function fadeStrip(workspace) {
    const selected = onlySelected(workspace, "block");
    const hasFade = Boolean(selected && selected.layer.fade);
    return [stripLabel("FROM THE FRONT COLOUR TO THE BACK COLOUR"), stripButton("REMOVE THE FADE", () => workspace.changeLayer(selected, { fade: undefined }), !hasFade)];
  }

  const fade = {
    id: "fade",
    name: "Fade",
    key: "g",
    icon: ICONS.fade,
    hint: "Drag across a colour block to fade it from the front colour to the back colour. Drag across bare page to fade the whole page.",
    strip: fadeStrip,
    pressed(workspace, press) {
      const target = layerToWorkOn(workspace, press, "block");
      return {
        dragged: (now) => workspace.showLine(press.point, now.point),
        released(now) {
          workspace.showLine(null);
          const length = Math.hypot(now.point.x - press.point.x, now.point.y - press.point.y);
          if (length < SHORTEST_FADE) return workspace.say("Drag from where the front colour starts to where the back colour ends.");
          if (target) return workspace.changeLayer(target, fadeAcross(workspace, target.area, press.point, now.point));
          workspace.addLayer({ kind: "block", shape: "rectangle", area: { ...PAGE }, ...fadeAcross(workspace, PAGE, press.point, now.point) }, { atTheBack: true });
        },
      };
    },
  };

  // --- Colour picker ---------------------------------------------------------------------------

  const picker = {
    id: "picker",
    name: "Colour picker",
    key: "i",
    icon: ICONS.picker,
    hint: "Click anything on the page to pick up its colour as the front colour. Alt-click picks it up as the back colour.",
    strip: () => [stripLabel("CLICK ANYTHING ON THE PAGE TO PICK UP ITS COLOUR")],
    pressed(workspace, press) {
      workspace.pickUpAt(press.client, press.alt ? "back" : "front");
    },
  };

  // --- Crop ------------------------------------------------------------------------------------

  // The part of the picture a layer shows now, in the picture's own pixels, and how many of those go to one pixel of the page.
  function showingOf(item, whole) {
    const crop = item.layer.crop || { x: 0, y: 0, width: whole.width, height: whole.height };
    return { crop, across: crop.width / item.area.width, down: crop.height / item.area.height };
  }

  async function cropTo(workspace, item, box) {
    const kept = sharedBy(box, item.area);
    if (!kept) return workspace.say("Drag the box over the picture, round the part to keep.");
    const { crop, across, down } = showingOf(item, await workspace.sizeOfArt(item.layer.file));
    const newCrop = { x: crop.x + (kept.x - item.area.x) * across, y: crop.y + (kept.y - item.area.y) * down, width: kept.width * across, height: kept.height * down };
    workspace.changeLayer(item, { area: wholePixels(kept), crop: wholePixels(newCrop) });
  }

  // Shows the whole picture again, at the size its cropped part is shown at now.
  async function uncrop(workspace, item) {
    const whole = await workspace.sizeOfArt(item.layer.file);
    const { crop, across, down } = showingOf(item, whole);
    const area = { x: item.area.x - crop.x / across, y: item.area.y - crop.y / down, width: whole.width / across, height: whole.height / down };
    workspace.changeLayer(item, { area: wholePixels(area), crop: undefined });
  }

  function cropStrip(workspace) {
    const selected = onlySelected(workspace, "picture");
    const isCropped = Boolean(selected && selected.layer.crop);
    return [stripLabel("DRAG ROUND THE PART OF A PICTURE TO KEEP"), stripButton("SHOW ALL OF IT AGAIN", () => uncrop(workspace, selected), !isCropped)];
  }

  const crop = {
    id: "crop",
    name: "Crop",
    key: "c",
    icon: ICONS.crop,
    hint: "Drag a box over a picture you added, round the part to keep.",
    strip: cropStrip,
    pressed(workspace, press) {
      const target = layerToWorkOn(workspace, press, "picture");
      if (!target) return workspace.say("Crop works on a picture you added. Click one first.");
      workspace.select([target]);
      return draggingABox(workspace, press, (box) => {
        if (box) cropTo(workspace, target, box);
      });
    },
  };

  // --- Brush and eraser ------------------------------------------------------------------------

  // Paint goes on a picture that covers the whole page. The selected one is painted on, if it is such a picture.
  function paintedLayer(workspace) {
    const selected = onlySelected(workspace, "picture");
    const coversThePage = selected && !selected.layer.crop && ["x", "y", "width", "height"].every((side) => selected.area[side] === PAGE[side]);
    return coversThePage ? selected : null;
  }

  function strokeThrough(pen, points) {
    pen.beginPath();
    pen.moveTo(points[0].x, points[0].y);
    points.slice(1).forEach((point) => pen.lineTo(point.x, point.y));
    // A single click leaves a dot.
    if (points.length === 1) pen.lineTo(points[0].x + 0.01, points[0].y);
    pen.stroke();
  }

  // Paints the stroke over what the layer already holds. Rubbing out takes paint away instead of adding it.
  function paintOnto(easel, points, how) {
    const pen = easel.canvas.getContext("2d");
    pen.clearRect(0, 0, PAGE.width, PAGE.height);
    if (easel.painted) pen.drawImage(easel.painted, 0, 0, PAGE.width, PAGE.height);
    pen.globalCompositeOperation = how.rubsOut ? "destination-out" : "source-over";
    pen.strokeStyle = how.colour;
    pen.lineWidth = how.size;
    pen.lineCap = "round";
    pen.lineJoin = "round";
    strokeThrough(pen, points);
    pen.globalCompositeOperation = "source-over";
  }

  async function keepStroke(workspace, easel, target, points, how) {
    await easel.ready;
    paintOnto(easel, points, how);
    const file = await workspace.keepPainting(easel.canvas);
    if (!file) return;
    if (target) workspace.changeLayer(target, { file });
    else workspace.addLayer({ kind: "picture", file, area: { ...PAGE } });
  }

  function painting(workspace, press, how) {
    const target = paintedLayer(workspace);
    if (how.rubsOut && !target) return workspace.say("Select a painted layer in the list first, then rub it out.");
    const easel = workspace.easelOver(target);
    const points = [press.point];
    const repaint = () => paintOnto(easel, points, how);
    easel.ready.then(repaint);
    repaint();
    return {
      dragged(now) {
        points.push(now.point);
        repaint();
      },
      released: () => keepStroke(workspace, easel, target, points, how),
    };
  }

  function sizeStrip(workspace, words) {
    const { brushSize } = workspace.options();
    const note = paintedLayer(workspace) ? "ON THE SELECTED PAINT LAYER" : words;
    return [stripLabel("SIZE"), numberInput(brushSize, BRUSH_SIZES, (size) => workspace.setOption("brushSize", size)), stripLabel(note)];
  }

  const brush = {
    id: "brush",
    name: "Brush",
    key: "b",
    icon: ICONS.brush,
    hint: "Paint by hand in the front colour, behind the controls. Select a painted layer to add to it.",
    strip: (workspace) => sizeStrip(workspace, "ON A NEW PAINT LAYER"),
    pressed: (workspace, press) => painting(workspace, press, { colour: workspace.colours().front, size: workspace.options().brushSize, rubsOut: false }),
  };

  const eraser = {
    id: "eraser",
    name: "Eraser",
    key: "e",
    icon: ICONS.eraser,
    hint: "Rub out part of a painted layer. Select the layer in the list first.",
    strip: (workspace) => sizeStrip(workspace, "SELECT A PAINT LAYER FIRST"),
    pressed: (workspace, press) => painting(workspace, press, { colour: "#000000", size: workspace.options().brushSize, rubsOut: true }),
  };

  window.editorArtTools = { ART_TOOLS: [crop, picker, brush, eraser, fade, words, block, picture], blockOver, wordsAt };
})();
