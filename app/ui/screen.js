// Draws a plugin's own screen the way the MPC lays it out, and the hardware knobs that go with it.
//
// Where things sit inside a slot, and how tall their lettering is, are the measurements the SDK's gen_ui.py
// writes into the screen file. Where each slot sits on the page comes from the app, which knows what the
// musician arranged by hand. The knob, fader and switch artwork is the plugin's own, taken from its last build.
// Everything is built as page elements, never as markup text, so nothing in a layout file can run as code.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { picturesOf, filmStrip } = window.pluginPictures;
  const { addressOf, addressOfDrawing, showAsBackground } = window.fetchedImages;
  const { PAGE, SLOT, SLOTS_PER_ROW, placeAt, areaOfSlot } = window.mpcStage;
  const { layersOn } = window.lookEdits;

  // The six colour themes of Akai's control kit, from the SDK's make_ui_kit.py.
  const KIT_COLOURS = { "mpc-red": "#d31145", amber: "#ffa21a", cyan: "#19c6e0", lime: "#8fd21f", violet: "#9b63ff", mono: "#e4e4e6" };
  const DEFAULT_KIT = "mpc-red";
  const DEFAULT_STYLE = { background: "ff1e1d1e", text: "ffffffff", muted: "ff6a6a6c" };
  const SLOTS_PER_PAGE = 16;
  // Every moving image in the control kit has this many frames.
  const KIT_FRAMES = 127;
  const DRAG_PIXELS_FOR_A_FULL_TURN = 160;
  const KEY_STEP = 0.02;
  const SVG = "http://www.w3.org/2000/svg";
  const KNOB_SWEEP = 0.75;

  const area = (x, y, width, height) => ({ x, y, width, height });
  // Where the parts of each kind of control sit inside its 278 by 107 slot.
  const KNOB_FROM_KIT = { art: area(8, 8, 90, 90), name: area(110, 12, 156, 36), value: area(110, 52, 156, 42) };
  const KNOB_PLAIN = { art: area(15, 15, 77, 77), name: area(104, 14, 160, 38), value: area(104, 54, 160, 40) };
  const FADER = { name: area(15, 8, 135, 34), value: area(150, 8, 113, 34), art: area(15, 54, 248, 36) };
  const TOGGLE = { name: area(15, 12, 170, 36), value: area(15, 52, 170, 42), art: area(186, 20, 84, 68) };
  const CHOICE = { name: area(15, 8, 248, 34), top: 54, height: 40 };
  const CHOICE_BUTTONS = { 2: { width: 122, lefts: [15, 141] }, 3: { width: 80, lefts: [15, 99, 183] }, 4: { width: 58, lefts: [15, 77, 139, 201] } };
  const SLIDER = { name: area(15, 15, 150, 34), value: area(165, 15, 98, 34), bar: area(15, 58, 248, 22) };
  const HEADING_HEIGHT = 30;
  const MPC_FONT = "Titillium Web";
  const WAYS_TO_ALIGN = { left: "flex-start", centre: "center", right: "flex-end" };
  // A line of words is this much taller than its letters, and about half as wide as it is long, until the app measures it.
  const ROUGH_LINE_HEIGHT = 1.25;
  const ROUGH_LETTER_WIDTH = 0.5;
  // A centre-zero control reads as zero where it rests, such as "0", "0.0 dB" or "+0 st".
  const READS_AS_ZERO = /^[+-]?0*(\.0+)?\s*[^0-9.]*$/;

  // Pixel outlines for a round hardware knob, and where its pointer sits from fully down to fully up.
  const KNOB_OUTLINE = "M4 0h4v1h2v1h1v2h1v4h-1v2h-1v1h-2v1h-4v-1h-2v-1h-1v-2h-1v-4h1v-2h1v-1h2z";
  const KNOB_FACE = "M3 0h4v1h2v2h1v4h-1v2h-2v1h-4v-1h-2v-2h-1v-4h1v-2h2z";
  const KNOB_SHINE = "M4 2h3v1h-3z";
  const POINTER_POSITIONS = [[2, 8], [2, 5], [3, 3], [5, 2], [7, 3], [8, 5], [8, 8]];

  // --- What the layout file says ---------------------------------------------------------------

  // A layout may name a control as bare text or as an object. This gives every slot the same shape.
  function asControl(entry) {
    if (entry === "empty" || entry === undefined) return { widget: "empty" };
    if (typeof entry === "string") return { widget: "slider", param: entry };
    return { widget: entry.widget || "slider", param: entry.param, text: entry.text, polarity: entry.polarity };
  }

  function controlsOf(page) {
    return (page.controls || []).map(asControl).slice(0, SLOTS_PER_PAGE);
  }

  function pagesOf(plugin) {
    return (plugin && plugin.screen && plugin.screen.pages) || [];
  }

  function pageName(pages, index) {
    return pages[index].name || String(index + 1);
  }

  function arrangedItems(plugin, pageIndex) {
    const pages = (plugin.arrangement && plugin.arrangement.pages) || [];
    return (pages[pageIndex] && pages[pageIndex].items) || [];
  }

  // Every slot of a page with where it sits and the name it shows, once the musician's own arrangement is applied.
  // A slot the arrangement does not mention sits in its place on the grid under its own name.
  // `entry` is the slot as the layout file wrote it, and `partsByHand` how the musician laid out its inside, if they did.
  function placedControls(plugin, pageIndex) {
    const arranged = arrangedItems(plugin, pageIndex);
    const written = pagesOf(plugin)[pageIndex].controls || [];
    return controlsOf(pagesOf(plugin)[pageIndex]).map((control, position) => {
      const item = arranged.find((known) => known.slot === position);
      const byHand = item ? { area: item.area, shownName: item.shownName, partsByHand: item.parts } : { area: areaOfSlot(position) };
      return { ...control, ...byHand, slot: position, entry: written[position] };
    });
  }

  // Colours in a layout file are written alpha first: AARRGGBB.
  function cssColour(argb) {
    return typeof argb === "string" && argb.length === 8 ? "#" + argb.slice(2) : null;
  }

  function styleColour(layout, name) {
    return cssColour(layout.style && layout.style[name]) || cssColour(DEFAULT_STYLE[name]);
  }

  function kitColourOf(layout) {
    const kit = layout.kit;
    const named = typeof kit === "string" ? kit : kit && kit.theme;
    const custom = cssColour((kit && kit.accent) || (layout.style && layout.style.accent));
    return custom || KIT_COLOURS[named] || KIT_COLOURS[DEFAULT_KIT];
  }

  // --- What the built plugin says about its controls -------------------------------------------

  function keyOf(name) {
    return String(name || "").toLowerCase();
  }

  function findParameter(parameters, name) {
    const known = (parameters && parameters.params) || [];
    return known.find((parameter) => keyOf(parameter.name) === keyOf(name));
  }

  function within(amount) {
    return Math.min(1, Math.max(0, Number.isFinite(amount) ? amount : 0.5));
  }

  function restingAmount(parameter) {
    return within(parameter ? Number(parameter.default) : 0.5);
  }

  function choicesOf(parameter) {
    if (!parameter) return [];
    if (parameter.choices && parameter.choices.length) return parameter.choices;
    return parameter.kind === "bool" ? ["Off", "On"] : [];
  }

  function chosenStep(amount, choices) {
    return Math.round(amount * (choices.length - 1));
  }

  function isCentreZero(control, parameter) {
    if (control.polarity === "bipolar") return true;
    if (control.polarity === "unipolar" || !parameter || choicesOf(parameter).length) return false;
    return Math.abs(restingAmount(parameter) - 0.5) <= 0.01 && READS_AS_ZERO.test(String(parameter.display || "").trim());
  }

  // --- Where the musician has turned each control ----------------------------------------------

  // Turning a control in the preview is kept until the plugin is built again or another plugin is opened.
  const turned = { pluginName: null, builtAt: null, amounts: new Map() };
  // What the live player says each turned control reads, with the amount it was turned to when it said so.
  const told = new Map();
  let pageOnScreen = null;
  // Where the plugin itself last moved a control, as the live player passed it on: lamps it lights for the
  // notes held, say. The pictures that follow such a control go where the plugin put it.
  const movedByThePlugin = new Map();
  let picturesOnScreen = null;

  function forgetTurnsOfAnotherBuild(plugin) {
    if (turned.pluginName === plugin.summary.name && turned.builtAt === plugin.builtAt) return;
    turned.pluginName = plugin.summary.name;
    turned.builtAt = plugin.builtAt;
    turned.amounts.clear();
    told.clear();
    movedByThePlugin.clear();
  }

  function toldValue(parameter, amount) {
    const said = told.get(keyOf(parameter.name));
    return said && Math.abs(said.amount - amount) < 0.0001 ? said.text : null;
  }

  function amountNow(name, parameter) {
    const turnedTo = turned.amounts.get(keyOf(name));
    return turnedTo === undefined ? restingAmount(parameter) : turnedTo;
  }

  // A picture follows the plugin's own last move of its control. Failing one, it follows the musician's hand.
  function amountForPictures(plugin, name) {
    const movedTo = movedByThePlugin.get(keyOf(name));
    return movedTo === undefined ? amountNow(name, findParameter(plugin.parameters, name)) : movedTo;
  }

  // The live player tells the real value. Without it, the real value is only known where the control rests
  // or chooses from a list, and anywhere else the preview shows how far the control is turned.
  function valueWords(parameter, amount) {
    if (!parameter) return "";
    const fromThePlayer = toldValue(parameter, amount);
    if (fromThePlayer !== null) return fromThePlayer;
    const choices = choicesOf(parameter);
    if (choices.length) return choices[chosenStep(amount, choices)];
    const atRest = Math.abs(amount - restingAmount(parameter)) < 0.0005;
    return atRest ? [parameter.display, parameter.label].filter(Boolean).join(" ") : `${Math.round(amount * 100)}%`;
  }

  // One control's setting: what it rests at, where it is now, and how to turn it.
  function settingOf(page, control) {
    const parameter = findParameter(page.plugin.parameters, control.param);
    const steps = choicesOf(parameter).length;
    const snapped = (amount) => (steps > 1 ? Math.round(within(amount) * (steps - 1)) / (steps - 1) : within(amount));
    return {
      name: control.param || "",
      parameter,
      resting: restingAmount(parameter),
      amount: () => amountNow(control.param, parameter),
      turnTo: (amount) => page.turn(control.param, snapped(amount)),
    };
  }

  // --- Which kind of control a slot shows, and how the generator lays out its parts ------------

  // The five looks a control can have: a row of choice buttons, an on-off toggle, a knob, a fader, or the MPC's own slider.
  function kindOfLook(control, parameter) {
    const choices = choicesOf(parameter).length;
    if (control.widget === "switch") return CHOICE_BUTTONS[choices] && parameter.kind !== "bool" ? "choice" : "toggle";
    return control.widget === "knob" || control.widget === "fader" ? control.widget : "slider";
  }

  // One line of the MPC's own lettering: where it sits in its control and how it is lettered.
  function lettering(area, height, colour, align) {
    return { area, font: MPC_FONT, bold: false, height, colour, align: align || "left", hidden: false };
  }

  // The parts of a control as Akai's generator lays them out: the image that draws it, its name and its value.
  // A row of choice buttons and the MPC's own slider have no such parts, and move only as a whole.
  function generatedParts(kind, layout) {
    const name = (area, height) => lettering(area, height, styleColour(layout, "muted"));
    const value = (area, height, align) => lettering(area, height, styleColour(layout, "text"), align);
    const knob = layout.kit ? KNOB_FROM_KIT : KNOB_PLAIN;
    if (kind === "knob") return { art: knob.art, name: name(knob.name, 26), value: value(knob.value, 30) };
    if (kind === "fader") return { art: FADER.art, name: name(FADER.name, 24), value: value(FADER.value, 26, "right") };
    if (kind === "toggle") return { art: TOGGLE.art, name: name(TOGGLE.name, 26), value: value(TOGGLE.value, 30) };
    return null;
  }

  function generatedHeading(layout) {
    return { name: lettering({ x: 0, y: 0, ...SLOT }, HEADING_HEIGHT, styleColour(layout, "text")) };
  }

  // How the image that draws a control can be resized: a knob keeps its shape, a fader stretches freely,
  // and anything the workbench cannot draw again at another size keeps the size it has.
  function waysToResize(kind, layout) {
    if (kind === "knob" && !layout.knobImage) return { whole: "shape", art: "shape" };
    if (kind === "fader") return { whole: "free", art: "free" };
    return { whole: null, art: null };
  }

  // --- The parts a control is made of ----------------------------------------------------------

  function label(words, where, height, className) {
    const made = placeAt(element("div", `mpc-label ${className}`, words), where);
    made.style.setProperty("--size", String(height));
    return made;
  }

  // A name or a value, lettered the way its part says.
  function partLabel(words, part, className) {
    const made = label(words, part.area, part.height, className);
    made.style.color = part.colour;
    made.style.fontFamily = `"${part.font}", "${MPC_FONT}", sans-serif`;
    made.style.fontWeight = part.bold ? "700" : "400";
    made.style.justifyContent = WAYS_TO_ALIGN[part.align];
    if (part.hidden) made.style.display = "none";
    return made;
  }

  function nameLabel(control, where, height) {
    return label(control.shownName || control.param || "", where, height, "name");
  }

  function namePart(control) {
    return partLabel(control.shownName || control.param || "", control.parts.name, "name");
  }

  function kitImage(plugin, file) {
    return addressOf("skin_image", { name: plugin.summary.name, file }, plugin.builtAt);
  }

  // A moving image from the control kit: one frame for each place the control can be.
  function kitStrip(plugin, file, where, className) {
    const strip = filmStrip(`mpc-art ${className}`, KIT_FRAMES);
    placeAt(strip.made, where);
    showAsBackground(strip.made, kitImage(plugin, file));
    return strip;
  }

  // A button from the control kit: one image when it is on and another when it is off.
  function kitButton(plugin, stem, where) {
    const made = placeAt(element("div", "mpc-art button"), where);
    const imageWhen = (on) => {
      const image = element("div", on ? "mpc-button-image on" : "mpc-button-image off");
      showAsBackground(image, kitImage(plugin, `${stem}_${on ? "on" : "off"}.png`));
      return image;
    };
    made.append(imageWhen(false), imageWhen(true));
    return { made, showOn: (on) => made.classList.toggle("is-on", on) };
  }

  // The MPC's own knob, used by a plugin that has no control kit: a ring that fills as it is turned.
  function plainKnob(where) {
    const radius = 18;
    const around = 2 * Math.PI * radius;
    const drawing = document.createElementNS(SVG, "svg");
    drawing.setAttribute("class", "mpc-plain-knob");
    drawing.setAttribute("viewBox", "0 0 46 46");
    const arc = (className) => {
      const circle = document.createElementNS(SVG, "circle");
      [["class", className], ["cx", "23"], ["cy", "23"], ["r", String(radius)], ["fill", "none"], ["stroke-width", "5"], ["transform", "rotate(135 23 23)"]]
        .forEach(([name, value]) => circle.setAttribute(name, value));
      return circle;
    };
    const fill = (circle, fraction) => circle.setAttribute("stroke-dasharray", `${around * fraction} ${around}`);
    const track = arc("track");
    const amountArc = arc("amount");
    fill(track, KNOB_SWEEP);
    drawing.append(track, amountArc);
    const made = placeAt(element("div", "mpc-art"), where);
    made.append(drawing);
    return { made, showAmount: (amount) => fill(amountArc, KNOB_SWEEP * amount) };
  }

  // --- The five looks a control can have -------------------------------------------------------
  //
  // Each gives the parts to put in the slot, the label that shows the value, how to show an amount,
  // and, for one that is dragged, the part to take hold of (`handle`) and which way to drag it ("up" or "across").

  function knobLook(page, control, setting) {
    const file = isCentreZero(control, setting.parameter) ? "knob_bi.png" : "knob_uni.png";
    const art = page.plugin.screen.kit ? kitStrip(page.plugin, file, control.parts.art, "knob") : plainKnob(control.parts.art);
    const value = partLabel("", control.parts.value, "value");
    return { parts: [art.made, namePart(control), value], value, showAmount: art.showAmount, handle: art.made, drag: "up" };
  }

  function faderLook(page, control, setting) {
    const file = isCentreZero(control, setting.parameter) ? "fader_h_bi.png" : "fader_h_uni.png";
    const art = kitStrip(page.plugin, file, control.parts.art, "fader");
    const value = partLabel("", control.parts.value, "value");
    return { parts: [namePart(control), value, art.made], value, showAmount: art.showAmount, handle: art.made, drag: "across" };
  }

  // The toggle's image is never drawn again at another size, so it keeps its own wherever it is put.
  function toggleLook(page, control, setting) {
    const button = kitButton(page.plugin, "toggle", { ...control.parts.art, width: TOGGLE.art.width, height: TOGGLE.art.height });
    const value = partLabel("", control.parts.value, "value");
    button.made.addEventListener("click", () => setting.turnTo(setting.amount() >= 0.5 ? 0 : 1));
    return { parts: [namePart(control), value, button.made], value, showAmount: (amount) => button.showOn(amount >= 0.5) };
  }

  function choiceLook(page, control, setting) {
    const choices = choicesOf(setting.parameter);
    const { width, lefts } = CHOICE_BUTTONS[choices.length];
    const buttons = choices.map((choice, step) => {
      const where = area(lefts[step], CHOICE.top, width, CHOICE.height);
      const button = kitButton(page.plugin, `seg${choices.length}`, where);
      button.made.addEventListener("click", () => setting.turnTo(step / (choices.length - 1)));
      return { ...button, caption: label(choice, where, 22, "caption") };
    });
    const showAmount = (amount) => buttons.forEach((button, step) => button.showOn(step === chosenStep(amount, choices)));
    return { parts: [nameLabel(control, CHOICE.name, 24), ...buttons.flatMap((button) => [button.made, button.caption])], showAmount };
  }

  // The MPC's standard slider: name and value above a bar. Its exact look is the MPC's own, so this is close, not exact.
  function sliderLook(page, control) {
    const bar = placeAt(element("div", "mpc-bar"), SLIDER.bar);
    const fill = element("div", "mpc-bar-fill");
    bar.append(fill);
    const value = label("", SLIDER.value, 26, "value right");
    const showAmount = (amount) => { fill.style.width = `${amount * 100}%`; };
    return { parts: [nameLabel(control, SLIDER.name, 24), value, bar], value, showAmount, handle: bar, drag: "across" };
  }

  const LOOKS = { choice: choiceLook, toggle: toggleLook, knob: knobLook, fader: faderLook, slider: sliderLook };

  // --- Turning a control with the mouse or the keyboard ----------------------------------------

  // Only the part that turns takes hold of a press: the knob, the fader or the bar itself. A control's slot is much
  // bigger than what it shows, so when controls sit close together a press on the whole slot would turn the wrong one.
  function dragToTurn(made, setting, direction) {
    made.classList.add("turns");
    made.addEventListener("pointerdown", (down) => {
      const from = setting.amount();
      const fullTravel = direction === "across" ? made.getBoundingClientRect().width * 0.9 : DRAG_PIXELS_FOR_A_FULL_TURN;
      const travelled = (move) => (direction === "across" ? move.clientX - down.clientX : down.clientY - move.clientY);
      const follow = (move) => setting.turnTo(from + travelled(move) / fullTravel);
      const letGo = () => made.removeEventListener("pointermove", follow);
      made.setPointerCapture(down.pointerId);
      made.addEventListener("pointermove", follow);
      made.addEventListener("pointerup", letGo, { once: true });
      made.addEventListener("pointercancel", letGo, { once: true });
    });
    made.addEventListener("dblclick", () => setting.turnTo(setting.resting));
  }

  function keysToTurn(made, setting) {
    const moves = { ArrowUp: KEY_STEP, ArrowRight: KEY_STEP, ArrowDown: -KEY_STEP, ArrowLeft: -KEY_STEP };
    made.addEventListener("keydown", (event) => {
      const steps = choicesOf(setting.parameter).length;
      const stride = steps > 1 ? Math.sign(moves[event.key] || 0) / (steps - 1) : moves[event.key];
      if (!stride) return;
      event.preventDefault();
      setting.turnTo(setting.amount() + stride);
    });
  }

  function describeForScreenReaders(made, setting) {
    made.tabIndex = 0;
    made.setAttribute("role", "slider");
    made.setAttribute("aria-label", setting.name);
    made.setAttribute("aria-valuemin", "0");
    made.setAttribute("aria-valuemax", "100");
  }

  // --- Slots -----------------------------------------------------------------------------------

  // A heading is one line of lettering, which sits in its slot wherever its part says.
  function headingSlot(control) {
    const part = control.parts.name;
    const where = { ...part.area, x: control.area.x + part.area.x, y: control.area.y + part.area.y };
    return partLabel(control.shownName || control.text || "", { ...part, area: where }, "heading");
  }

  // While the musician is arranging the screen, a control is something to pick up, so it cannot be turned.
  function letItBeTurned(made, setting, look) {
    describeForScreenReaders(made, setting);
    if (look.handle) dragToTurn(look.handle, setting, look.drag);
    keysToTurn(made, setting);
  }

  function controlSlot(page, control, setting) {
    const look = LOOKS[control.look](page, control, setting);
    const made = placeAt(element("div", "mpc-slot"), control.area);
    made.append(...look.parts);
    if (!page.isBeingArranged) letItBeTurned(made, setting, look);
    const show = (amount) => {
      const words = valueWords(setting.parameter, amount);
      look.showAmount(amount);
      if (look.value) look.value.textContent = words;
      made.classList.toggle("turned", amount !== setting.resting);
      made.setAttribute("aria-valuenow", String(Math.round(amount * 100)));
      made.setAttribute("aria-valuetext", words);
    };
    show(setting.amount());
    page.watch(control.param, show);
    return made;
  }

  // The parts the musician laid out by hand, laid over the generator's own, so that every part is always there.
  function partsOf(control, generated) {
    return generated && { ...generated, ...(control.partsByHand || {}) };
  }

  function headingItem(page, control) {
    const laidOut = { ...control, parts: partsOf(control, generatedHeading(page.plugin.screen)) };
    return { kind: "heading", name: control.text || "", area: control.area, made: headingSlot(laidOut), parts: laidOut.parts, madeLike: "heading", slot: control.slot, entry: control.entry };
  }

  function controlItem(page, control) {
    const setting = settingOf(page, control);
    const look = kindOfLook(control, setting.parameter);
    const resize = waysToResize(look, page.plugin.screen);
    const laidOut = { ...control, look, parts: partsOf(control, generatedParts(look, page.plugin.screen)) };
    const made = controlSlot(page, laidOut, setting);
    return { kind: "control", name: control.param || "", area: control.area, made, parts: laidOut.parts, madeLike: look, resize: resize.whole, artResize: resize.art, slot: control.slot, entry: control.entry };
  }

  // What is drawn for one slot, described the way the musician's arrangement names it. An empty slot draws nothing.
  // Besides where it is, it says how its parts are laid out, what it is made like, how it can be resized,
  // and how the layout file wrote it.
  function slotFor(page, control) {
    if (control.widget === "empty") return null;
    return control.widget === "text" ? headingItem(page, control) : controlItem(page, control);
  }

  // --- The artwork behind a page's controls ----------------------------------------------------

  function lookOf(plugin) {
    return plugin.look || { pages: [] };
  }

  // A layer as the app is asked to draw it: by itself at the corner, so the same drawing serves wherever it is moved.
  function layerToDraw(layer) {
    const atCorner = layer.kind === "words" ? { place: { x: 0, y: 0 } } : { area: { ...layer.area, x: 0, y: 0 } };
    const { hidden, tilt, ...shown } = layer;
    return { ...shown, ...atCorner, id: 0 };
  }

  // Where a layer is until the app has said: words are only measured once they are kept.
  function roughAreaOf(layer) {
    if (layer.kind !== "words") return layer.area;
    return { ...layer.place, width: Math.max(1, layer.size * layer.text.length * ROUGH_LETTER_WIDTH), height: Math.ceil(layer.size * ROUGH_LINE_HEIGHT) };
  }

  function layerItem(plugin, layer, arranged) {
    const known = arranged.find((placed) => placed.id === layer.id);
    const area = known ? known.area : roughAreaOf(layer);
    const made = placeAt(element("div", layer.hidden ? "mpc-layer hidden-layer" : "mpc-layer"), area);
    showAsBackground(made, addressOfDrawing("layer_image", { name: plugin.summary.name, layer: layerToDraw(layer) }));
    const resize = layer.kind === "words" ? "shape" : "free";
    const tilt = layer.tilt || 0;
    if (tilt) made.style.transform = `rotate(${tilt}deg)`;
    return { kind: "layer", name: String(layer.id), layer, area, made, resize, tilt, canTilt: true, stretchesLive: true, mayLeavePage: true };
  }

  // Every layer of a page's artwork as something to pick up, from the back to the front.
  function layerItems(plugin, pageIndex, pageName) {
    const pages = (plugin.arrangement && plugin.arrangement.pages) || [];
    const arranged = (pages[pageIndex] && pages[pageIndex].artwork) || [];
    return layersOn(lookOf(plugin), pageName).map((layer) => layerItem(plugin, layer, arranged));
  }

  // All of a page's artwork as the one picture the MPC will show. A page with none showing draws nothing.
  function pageArt(plugin, pageName) {
    const layers = layersOn(lookOf(plugin), pageName);
    if (!layers.some((layer) => !layer.hidden)) return [];
    const made = placeAt(element("div", "mpc-picture page-art"), PAGE);
    showAsBackground(made, addressOf("page_art_image", { name: plugin.summary.name, page: pageName }, JSON.stringify(layers)));
    return [made];
  }

  // --- The whole page --------------------------------------------------------------------------

  // What one drawn page shares: the plugin, and who is told when a control is turned.
  // `listeners.onTurned` hears of every turn. A page with no such listener is being arranged, not played.
  function drawnPage(plugin, onTurned) {
    const watchers = [];
    const tell = (name, amount) => watchers.filter((watcher) => watcher.key === keyOf(name)).forEach((watcher) => watcher.show(amount));
    return {
      plugin,
      isBeingArranged: !onTurned,
      watch: (name, show) => watchers.push({ key: keyOf(name), show }),
      showAgain: (name) => tell(name, turned.amounts.get(keyOf(name))),
      turn: (name, amount) => {
        if (!onTurned) return;
        turned.amounts.set(keyOf(name), amount);
        tell(name, amount);
        onTurned(name, amount);
      },
    };
  }

  function pageButtons(pages, currentPage, onPageChosen) {
    const row = element("div", "mpc-pages");
    pages.forEach((page, index) => {
      const button = element("button", "mpc-page", pageName(pages, index));
      button.type = "button";
      if (index === currentPage) button.setAttribute("aria-current", "page");
      button.addEventListener("click", () => onPageChosen(index));
      row.append(button);
    });
    return row;
  }

  function showMessage(container, words) {
    container.style.background = "";
    container.replaceChildren(element("div", "mpc-empty", words));
  }

  function useColoursOf(container, layout) {
    container.style.background = styleColour(layout, "background");
    container.style.setProperty("--kit", kitColourOf(layout));
    container.style.setProperty("--text", styleColour(layout, "text"));
    container.style.setProperty("--muted", styleColour(layout, "muted"));
  }

  // One page of the plugin's screen: the stage it is drawn on, the page's shared state, and every item on it
  // that can be picked up, from the back to the front. With no `onTurned`, the controls cannot be turned and
  // each layer of artwork is drawn by itself, so it can be picked up too.
  function stageOf(plugin, currentPage, onTurned) {
    const name = pageName(pagesOf(plugin), currentPage);
    const pictures = picturesOf(plugin, name, (control) => amountForPictures(plugin, control));
    const page = drawnPage(plugin, onTurned && ((control, amount) => {
      movedByThePlugin.delete(keyOf(control));
      pictures.controlTurned(control, amount);
      onTurned(control, amount);
    }));
    const layers = onTurned ? [] : layerItems(plugin, currentPage, name);
    const artwork = onTurned ? pageArt(plugin, name) : layers.map((layer) => layer.made);
    const slots = placedControls(plugin, currentPage).map((control) => slotFor(page, control)).filter(Boolean);
    const stage = element("div", "mpc-stage");
    stage.append(...pictures.back.made, ...artwork, ...pictures.behind.made, ...slots.map((slot) => slot.made), ...pictures.front.made);
    return { stage, page, pictures, items: [...pictures.back.items, ...layers, ...pictures.behind.items, ...slots, ...pictures.front.items] };
  }

  // Draws one page of the plugin's screen, with its page buttons along the bottom as on the MPC.
  // `listeners.onPageChosen` hears which page button was pressed, and `listeners.onTurned` that a control was turned.
  function drawScreen(container, plugin, currentPage, listeners) {
    const pages = pagesOf(plugin);
    if (!pages.length) return showMessage(container, plugin ? "This plugin has no screen layout yet." : "Describe a plugin and its screen will appear here.");
    forgetTurnsOfAnotherBuild(plugin);
    useColoursOf(container, plugin.screen);
    const drawn = stageOf(plugin, currentPage, listeners.onTurned);
    pageOnScreen = drawn.page;
    picturesOnScreen = drawn.pictures;
    container.replaceChildren(drawn.stage, pageButtons(pages, currentPage, listeners.onPageChosen));
  }

  // The colour of the page itself, behind everything on it.
  function pageColourOf(plugin) {
    return styleColour(plugin.screen, "background");
  }

  // Draws one page for the musician to arrange: nothing on it turns, and every item that can be moved is
  // handed back as { kind, name, area, made, ... }, from the back of the page to the front.
  function drawForArranging(container, plugin, currentPage) {
    useColoursOf(container, plugin.screen);
    const drawn = stageOf(plugin, currentPage, null);
    container.replaceChildren(drawn.stage);
    return { stage: drawn.stage, items: drawn.items };
  }

  function pageNamesOf(plugin) {
    const pages = pagesOf(plugin);
    return pages.map((_, index) => pageName(pages, index));
  }

  // The live player's word on what a control reads at an amount. The control on screen takes it up at once.
  function valueArrived(name, amount, text) {
    told.set(keyOf(name), { amount, text });
    if (pageOnScreen && turned.amounts.has(keyOf(name))) pageOnScreen.showAgain(name);
  }

  // The live player's word that the plugin moved a control by itself. Its pictures move as they would on an MPC.
  function pluginMoved(name, amount) {
    movedByThePlugin.set(keyOf(name), amount);
    if (picturesOnScreen) picturesOnScreen.controlTurned(name, amount);
  }

  // Where a preset put the controls, each with what it then reads. Every one is taken as turned there, so the
  // screen and the hardware knobs show the preset, and a player started later is put the same way.
  function controlsStandAt(settings) {
    settings.forEach(({ name, amount, text }) => {
      turned.amounts.set(keyOf(name), amount);
      told.set(keyOf(name), { amount, text });
      movedByThePlugin.delete(keyOf(name));
      if (pageOnScreen) pageOnScreen.showAgain(name);
    });
  }

  // Every control the musician has turned away from where it rests, by name in small letters.
  function turnedSettings() {
    return Array.from(turned.amounts, ([name, amount]) => ({ name, amount }));
  }

  // --- The hardware knobs ----------------------------------------------------------------------

  // On the MPC, the slot in row r and column c is turned by hardware knob 13 + c - 4r.
  function hardwareKnobNumber(row, column) {
    return 13 + column - SLOTS_PER_ROW * row;
  }

  function pixelShape(className, path, offset) {
    const shape = document.createElementNS(SVG, "path");
    shape.setAttribute("class", className);
    shape.setAttribute("d", path);
    if (offset) shape.setAttribute("transform", offset);
    return shape;
  }

  function pixelPointer(amount) {
    const [x, y] = POINTER_POSITIONS[Math.round(amount * (POINTER_POSITIONS.length - 1))];
    const pointer = document.createElementNS(SVG, "rect");
    [["class", "knob-pointer"], ["x", String(x)], ["y", String(y)], ["width", "2"], ["height", "2"]].forEach(([name, value]) => pointer.setAttribute(name, value));
    return pointer;
  }

  // A hardware knob drawn on a 12 by 12 pixel grid. A free knob has no pointer.
  function pixelKnob(amount) {
    const drawing = document.createElementNS(SVG, "svg");
    drawing.setAttribute("class", "hardware-knob");
    drawing.setAttribute("viewBox", "0 0 12 12");
    drawing.setAttribute("shape-rendering", "crispEdges");
    drawing.setAttribute("aria-hidden", "true");
    drawing.append(pixelShape("knob-edge", KNOB_OUTLINE), pixelShape("knob-face", KNOB_FACE, "translate(1 1)"), pixelShape("knob-shine", KNOB_SHINE));
    if (amount !== null) drawing.append(pixelPointer(amount));
    return drawing;
  }

  function hardwareCell(position, control, parameters) {
    const name = control && control.param;
    const number = hardwareKnobNumber(Math.floor(position / SLOTS_PER_ROW), position % SLOTS_PER_ROW);
    const amount = name ? amountNow(name, findParameter(parameters, name)) : null;
    const cell = element("div", name ? "hardware-cell in-use" : "hardware-cell");
    cell.append(element("span", "hardware-number", String(number)), pixelKnob(amount), element("span", "hardware-name", name || "Free"));
    return cell;
  }

  // What Akai's guide says of the knobs: sixteen assignments to a page, which an MPC with fewer knobs banks through.
  // It is kept for whoever rests the pointer on the note, so the note itself stays one short line.
  const ABOUT_THE_KNOBS = "Every page has its own 16. An MPC with four knobs reaches them as four banks of four.";

  // The 16 hardware knobs, each showing the control it turns on this page and where that control sits.
  function drawHardware(grid, note, plugin, currentPage) {
    const pages = pagesOf(plugin);
    const controls = pages.length ? controlsOf(pages[currentPage]) : [];
    const cells = Array.from({ length: SLOTS_PER_PAGE }, (_, position) => hardwareCell(position, controls[position], plugin && plugin.parameters));
    grid.replaceChildren(...cells);
    const inUse = controls.filter((control) => control.param).length;
    note.textContent = pages.length ? `${inUse} of ${SLOTS_PER_PAGE} knobs in use on this page.` : "";
    note.title = pages.length ? `${ABOUT_THE_KNOBS} ${knobsInAll(pages.length)}`.trim() : "";
  }

  function knobsInAll(pages) {
    return pages === 1 ? "" : `This plugin's ${pages} pages give ${pages * SLOTS_PER_PAGE} in all.`;
  }

  window.pluginScreen = { drawScreen, drawForArranging, pageNamesOf, pageColourOf, layerToDraw, drawHardware, valueArrived, pluginMoved, controlsStandAt, turnedSettings, pixelKnob, letItBeTurned };
})();
