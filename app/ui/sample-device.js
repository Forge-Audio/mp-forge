// The MPC Sample as pixel art that can be played: its pads, knobs, fader, encoder and buttons answer the mouse
// and the keyboard, and its screen shows what a Sample app draws, dot for dot.
//
// The art is painted on one canvas (see sample-art.js). Over it lies a part for each control, with nothing to
// see of its own: it is what the mouse, the keyboard and a screen reader find. A part tells the app what was
// done to it and tells the painting how it now looks.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { SCREEN, BODY, PADS, KNOBS, FADER, ENCODER, BUTTONS, told } = window.sampleSurface;

  // How many pixels of this window make one dot of the art. At 2 the app's screen shows at its true size, and
  // at 4 at twice that. Nothing in between is used, so a dot is always a whole number of pixels.
  const SIZES = [4, 2];
  // Zoomed in on, the device comes this many times closer, with its screen in the middle of what is seen. The
  // top of what is seen is then this many dots down the body, just over the buttons above the screen.
  const ZOOM = 3;
  const ZOOMED_IN = "zoomed-in";
  const OUT_OF_REACH = "out-of-reach";
  const TOP_OF_THE_ZOOMED_VIEW = 8;
  const A_CLICK_HIT = 100;
  const DRAG_FOR_A_FULL_TURN = 150;
  const DRAG_FOR_A_CLICK = 10;
  const WHERE_A_KNOB_STARTS = 0.5;
  const WHERE_THE_FADER_STARTS = 0.75;
  const ONE_STEP_OF_AN_ARROW = 0.05;
  // About how much of the window's height the app's own bars and the plugin's name take, above and below.
  const WINDOW_TAKEN_BY_THE_REST = 190;

  function placed(tag, className, place) {
    const part = element(tag, `sample-part ${className}`);
    ["x", "y", "w", "h"].forEach((side) => part.style.setProperty(`--${side}`, place[side]));
    return part;
  }

  function within(lowest, value, highest) {
    return Math.max(lowest, Math.min(highest, value));
  }

  // Follows the pointer from where it goes down on `part` until it is let go. `dragged` is told how far up it
  // has moved since it was last told, and `letGo` whether it moved at all.
  function followDrags(part, dragged, letGo) {
    part.addEventListener("pointerdown", (down) => {
      const untilLetGo = new AbortController();
      let lastHeight = down.clientY;
      let hasMoved = false;
      const move = (moved) => {
        hasMoved = hasMoved || moved.clientY !== down.clientY;
        dragged(lastHeight - moved.clientY);
        lastHeight = moved.clientY;
      };
      const finish = () => {
        untilLetGo.abort();
        if (letGo) letGo(hasMoved);
      };
      part.setPointerCapture(down.pointerId);
      part.addEventListener("pointermove", move, { signal: untilLetGo.signal });
      ["pointerup", "pointercancel"].forEach((ending) => part.addEventListener(ending, finish, { signal: untilLetGo.signal }));
    });
  }

  function stepWithArrows(pressed, step) {
    const steps = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1 }[pressed.key];
    if (!steps) return;
    pressed.preventDefault();
    step(steps);
  }

  // --- The screen ------------------------------------------------------------------------------

  const ZOOMING = {
    in: { isZoomedIn: false, title: "Click to zoom in on the screen" },
    out: { isZoomedIn: true, title: "Click to zoom back out" },
  };

  // The screen on the device. It is drawn to scale, so an app's picture is as small on it as on the device.
  function makeScreen() {
    const canvas = placed("canvas", "sample-screen", SCREEN);
    canvas.width = SCREEN.width;
    canvas.height = SCREEN.height;
    canvas.tabIndex = 0;
    canvas.setAttribute("role", "button");
    canvas.setAttribute("aria-label", "The MPC Sample's screen. Press to zoom in on it, and again to zoom back out.");
    const drawing = canvas.getContext("2d");
    const show = (pixels) => drawing.putImageData(new ImageData(pixels, SCREEN.width, SCREEN.height), 0, 0);
    return { canvas, show };
  }

  // A click on the screen zooms in on it: the whole device comes closer, until the screen is big enough to
  // read. Another click lets it move back.
  function zoomOnAClick(screen, holder) {
    const say = ({ isZoomedIn, title }) => {
      screen.title = title;
      screen.setAttribute("aria-pressed", String(isZoomedIn));
    };
    const zoom = () => say(holder.classList.toggle(ZOOMED_IN) ? ZOOMING.out : ZOOMING.in);
    holder.classList.remove(ZOOMED_IN);
    say(ZOOMING.in);
    screen.addEventListener("click", zoom);
    screen.addEventListener("keydown", (pressed) => {
      if (pressed.key !== "Enter" && pressed.key !== " ") return;
      pressed.preventDefault();
      zoom();
    });
  }

  // --- Pads and buttons: held down, then let go --------------------------------------------------

  // Makes `part` something that is held down and let go, by the mouse or by the keys that press a button.
  // `changed` is told each time it goes down or comes up, and never twice running the same way.
  function heldAndLetGo(part, changed) {
    let isDown = false;
    const press = (goesDown) => {
      if (isDown === goesDown) return;
      isDown = goesDown;
      changed(isDown);
    };
    part.addEventListener("pointerdown", () => press(true));
    ["pointerup", "pointerleave", "pointercancel", "blur"].forEach((ending) => part.addEventListener(ending, () => press(false)));
    part.addEventListener("keydown", (pressed) => { if (pressed.key === " " || pressed.key === "Enter") press(true); });
    part.addEventListener("keyup", () => press(false));
    return press;
  }

  function makePad(pad, { tell, art }) {
    const made = placed("button", "sample-pad", pad);
    made.type = "button";
    made.title = `Pad ${pad.number} (key ${pad.key.toUpperCase()})`;
    made.setAttribute("aria-label", `Pad ${pad.number}`);
    const press = heldAndLetGo(made, (isDown) => {
      art.padHeld(pad.number, isDown);
      tell(isDown ? told.padHit(pad.number, A_CLICK_HIT) : told.padLetGo(pad.number));
    });
    return { key: pad.key, made, press };
  }

  // The pads are played from the computer's keys while the device is in view and nothing is being typed.
  function playPadsFromKeys(pads, body) {
    const padFor = (pressed) => pads.find((pad) => pad.key === pressed.key.toLowerCase());
    const isTyping = (pressed) => pressed.target instanceof HTMLElement && (pressed.target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(pressed.target.tagName));
    const isHeld = (pressed) => pressed.repeat || pressed.metaKey || pressed.ctrlKey || pressed.altKey;
    const play = (pressed, isDown) => {
      const pad = padFor(pressed);
      if (!pad || body.offsetParent === null || body.classList.contains(OUT_OF_REACH) || isTyping(pressed) || isHeld(pressed)) return;
      pad.press(isDown);
    };
    document.addEventListener("keydown", (pressed) => play(pressed, true));
    document.addEventListener("keyup", (pressed) => play(pressed, false));
  }

  function makeButton(button, { tell, art }) {
    const made = placed("button", "sample-button", button);
    made.type = "button";
    made.title = button.name;
    made.setAttribute("aria-label", button.name);
    heldAndLetGo(made, (isDown) => {
      art.buttonHeld(button.name, isDown);
      tell(told.button(button.name, isDown));
    });
    return made;
  }

  // --- Knobs, the fader and the encoder ---------------------------------------------------------

  // Something that stays where it is left, between 0 and 1: a knob or the fader. It is moved by dragging up
  // and down or with the arrow keys, and can say again where it stands, for an app that has just started and
  // was not there to hear it moved.
  function staysWhereItIsLeft(made, startsAt, moved) {
    let stands = startsAt;
    const moveTo = (value) => {
      stands = within(0, value, 1);
      made.setAttribute("aria-valuenow", stands.toFixed(2));
      moved(stands);
    };
    made.tabIndex = 0;
    made.setAttribute("role", "slider");
    made.setAttribute("aria-valuemin", "0");
    made.setAttribute("aria-valuemax", "1");
    followDrags(made, (up) => moveTo(stands + up / DRAG_FOR_A_FULL_TURN));
    made.addEventListener("keydown", (pressed) => stepWithArrows(pressed, (steps) => moveTo(stands + steps * ONE_STEP_OF_AN_ARROW)));
    moveTo(stands);
    return { made, sayWhereItStands: () => moveTo(stands) };
  }

  function makeKnob(knob, { tell, art }) {
    const made = placed("div", "sample-knob", knob);
    made.title = `${knob.words}: drag up or down to turn it`;
    made.setAttribute("aria-label", `Knob ${knob.words}`);
    return staysWhereItIsLeft(made, WHERE_A_KNOB_STARTS, (stands) => {
      art.knobTurned(knob.number, stands);
      tell(told.knobTurned(knob.number, stands));
    });
  }

  function makeFader({ tell, art }) {
    const made = placed("div", "sample-fader", { ...FADER, x: FADER.x - 3, w: FADER.w + 6 });
    made.title = "Fader: drag up or down";
    made.setAttribute("aria-label", "Fader");
    made.setAttribute("aria-orientation", "vertical");
    return staysWhereItIsLeft(made, WHERE_THE_FADER_STARTS, (stands) => {
      art.faderMoved(stands);
      tell(told.faderMoved(stands));
    });
  }

  // The encoder has no end stops: it clicks round as it is dragged or wheeled, and a press without a drag
  // pushes it in.
  function makeEncoder({ tell, art }) {
    const made = placed("div", "sample-encoder", ENCODER);
    let clicksTurned = 0;
    let draggedSinceAClick = 0;
    const turn = (clicks) => {
      if (!clicks) return;
      clicksTurned += clicks;
      art.encoderTurned(clicksTurned);
      tell(told.encoderTurned(clicks));
    };
    const drag = (up) => {
      draggedSinceAClick += up;
      const clicks = Math.trunc(draggedSinceAClick / DRAG_FOR_A_CLICK);
      draggedSinceAClick -= clicks * DRAG_FOR_A_CLICK;
      turn(clicks);
    };
    const pushIn = () => {
      tell(told.button("ENCODER", true));
      tell(told.button("ENCODER", false));
    };
    made.tabIndex = 0;
    made.title = "Encoder: drag or scroll to turn it, click to press it";
    made.setAttribute("role", "button");
    made.setAttribute("aria-label", "Encoder");
    followDrags(made, drag, (hasMoved) => { if (!hasMoved) pushIn(); });
    made.addEventListener("wheel", (wheeled) => { wheeled.preventDefault(); turn(wheeled.deltaY < 0 ? 1 : -1); }, { passive: false });
    made.addEventListener("keydown", (pressed) => (pressed.key === "Enter" ? pushIn() : stepWithArrows(pressed, turn)));
    return made;
  }

  // --- The whole device ------------------------------------------------------------------------

  // The biggest size at which the whole body fits in `room`, across and down, or the smallest when none does.
  // A device that has to be scrolled to reach its pads is no good for playing, so height counts as much.
  function sizeThatFits(room) {
    const fits = (size) => BODY.width * size <= room.width && window.sampleArt.HEIGHT * size <= room.height;
    return SIZES.find(fits) || SIZES[SIZES.length - 1];
  }

  function keepFitted(body, holder) {
    const room = () => ({ width: holder.clientWidth, height: window.innerHeight - WINDOW_TAKEN_BY_THE_REST });
    const fit = () => body.style.setProperty("--dot", `${sizeThatFits(room())}px`);
    new ResizeObserver(fit).observe(holder);
    window.addEventListener("resize", fit);
    fit();
  }

  // Says where on the body the zoom is aimed, in dots of the art: at the middle of the screen.
  function aimTheZoomAtTheScreen(body) {
    body.style.setProperty("--zoom", ZOOM);
    body.style.setProperty("--zoomed-on-across", SCREEN.x + SCREEN.w / 2);
    body.style.setProperty("--zoomed-on-down", SCREEN.y + SCREEN.h / 2);
    body.style.setProperty("--top-of-the-zoomed-view", TOP_OF_THE_ZOOMED_VIEW);
  }

  function paintingIn(body) {
    const canvas = element("canvas", "sample-art");
    canvas.setAttribute("aria-hidden", "true");
    body.append(canvas);
    return window.sampleArt.make(canvas);
  }

  // Puts every control out of reach, or back within it: grayed, and answering neither the mouse nor the keys.
  // The screen stays as it is, to be looked at and zoomed in on.
  function putOutOfReach(body, controls, isOutOfReach) {
    body.classList.toggle(OUT_OF_REACH, isOutOfReach);
    controls.forEach((control) => { control.inert = isOutOfReach; });
  }

  // Draws the device into `holder`. `tell` is given everything the musician does on it, in the words a Sample
  // app is told. Answers with a way to show what the app sends back (its picture, the colour of each pad and
  // the buttons it lights), a way to have the knobs and the fader say where they stand, and a way to put the
  // controls out of reach while there is no app to play.
  function make(holder, tell) {
    const body = element("div", "sample-body");
    const touched = { tell, art: paintingIn(body) };
    const screen = makeScreen();
    const pads = PADS.map((pad) => makePad(pad, touched));
    const thatStayPut = [...KNOBS.map((knob) => makeKnob(knob, touched)), makeFader(touched)];
    const controls = [...BUTTONS.map((button) => makeButton(button, touched)), ...thatStayPut.map((part) => part.made), ...pads.map((pad) => pad.made), makeEncoder(touched)];
    const parts = [screen.canvas, ...controls];
    body.style.setProperty("--dots-across", BODY.width);
    body.style.setProperty("--dots-down", window.sampleArt.HEIGHT);
    aimTheZoomAtTheScreen(body);
    body.append(...parts);
    holder.replaceChildren(body);
    keepFitted(body, holder);
    zoomOnAClick(screen.canvas, holder);
    playPadsFromKeys(pads, body);
    return {
      show: (sent) => { screen.show(sent.pixels); touched.art.lit(sent); },
      sayWhereThingsStand: () => thatStayPut.forEach((part) => part.sayWhereItStands()),
      putOutOfReach: (isOutOfReach) => putOutOfReach(body, controls, isOutOfReach),
    };
  }

  window.sampleDevice = { make };
})();
