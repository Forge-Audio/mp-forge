// Pictures the musician wants the AI to see: screenshots, sketches, photos of gear.
//
// They wait under the message box until the message is sent. They can be picked from a file, pasted, or
// dragged from anywhere and dropped on the conversation: a screenshot's thumbnail, a file, a picture from
// another app. Each is shrunk and turned into a PNG here, so the app only ever keeps PNGs.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { send } = window.backend;

  const MOST_PICTURES_PER_MESSAGE = 4;
  // Large enough for the AI to read small lettering, small enough to be cheap to show.
  const LONGEST_SIDE = 1568;
  const TOO_MANY = `A message can carry ${MOST_PICTURES_PER_MESSAGE} pictures at most.`;
  const NOT_A_PICTURE = "That file is not a picture MP Forge can read.";

  let parts;
  let listeners;
  let waiting = [];
  let busy = false;

  function isPicture(file) {
    return file && file.type.startsWith("image/");
  }

  function shrunkSize(picture) {
    const shrink = Math.min(1, LONGEST_SIDE / Math.max(picture.width, picture.height));
    return { width: Math.max(1, Math.round(picture.width * shrink)), height: Math.max(1, Math.round(picture.height * shrink)) };
  }

  // Any picture the page can read becomes a PNG no longer than LONGEST_SIDE on either side.
  async function asModestPng(file) {
    const picture = await createImageBitmap(file);
    const canvas = Object.assign(document.createElement("canvas"), shrunkSize(picture));
    canvas.getContext("2d").drawImage(picture, 0, 0, canvas.width, canvas.height);
    return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
  }

  // Puts a picture among those waiting to be sent and returns it as it waits, or nothing when it could not be.
  async function add(file) {
    if (waiting.length >= MOST_PICTURES_PER_MESSAGE) return listeners.onProblem(TOO_MANY);
    let picture;
    try {
      const png = await asModestPng(file);
      picture = { png, address: URL.createObjectURL(png) };
      waiting.push(picture);
    } catch (unreadable) {
      listeners.onProblem(NOT_A_PICTURE);
    }
    draw();
    return picture;
  }

  async function addEach(files) {
    for (const file of files.filter(isPicture)) await add(file);
  }

  function isWaiting(picture) {
    return waiting.includes(picture);
  }

  function stopWaiting(picture) {
    if (!isWaiting(picture)) return;
    waiting = waiting.filter((other) => other !== picture);
    URL.revokeObjectURL(picture.address);
    draw();
  }

  function thumbnail(picture, number) {
    const made = element("span", "attachment");
    const image = element("img", "attachment-image");
    image.src = picture.address;
    image.alt = `Picture ${number} to send`;
    const removeButton = element("button", "sample-remove", "x");
    removeButton.type = "button";
    removeButton.disabled = busy;
    removeButton.setAttribute("aria-label", `Remove picture ${number}`);
    removeButton.addEventListener("click", () => stopWaiting(picture));
    made.append(image, removeButton);
    return made;
  }

  function draw() {
    parts.button.disabled = busy;
    parts.list.replaceChildren(...waiting.map((picture, index) => thumbnail(picture, index + 1)));
  }

  function setBusy(isBusy) {
    busy = isBusy;
    draw();
  }

  function keep(forPlugin, picture) {
    return picture.png.arrayBuffer().then((contents) => send("add_attachment", new Uint8Array(contents), {
      "plugin-name": encodeURIComponent(forPlugin),
      "file-name": "picture.png",
    }));
  }

  // Hands the waiting pictures to the app for one plugin. Returns the names they were kept under, for the
  // AI, and their addresses, for showing them in the conversation. They stay in the tray until `letGo`, so a
  // request that is refused loses none of them.
  async function handOverFor(pluginName) {
    const handed = [...waiting];
    const names = [];
    for (const picture of handed) names.push(await keep(pluginName, picture));
    return { names, addresses: handed.map((picture) => picture.address), handed };
  }

  // The request that carried these pictures has gone out, so they leave the tray. Their addresses are still
  // shown in the conversation, so they are not given up.
  function letGo(pictures) {
    waiting = waiting.filter((picture) => !pictures.handed.includes(picture));
    draw();
  }

  function picturesIn(transfer) {
    return Array.from((transfer && transfer.files) || []).filter(isPicture);
  }

  // A pasted or dropped picture is taken. Anything else, such as pasted words, is left to the message box.
  function takePicturesFrom(event, transfer) {
    const pictures = picturesIn(transfer);
    if (!pictures.length) return;
    event.preventDefault();
    addEach(pictures);
  }

  function carriesFiles(event) {
    return Array.from((event.dataTransfer && event.dataTransfer.types) || []).includes("Files");
  }

  // Whether the pointer has really left the place, and not just crossed onto something inside it.
  function hasLeft(place, event) {
    return !place.contains(event.relatedTarget);
  }

  // The conversation shows it will take the picture for as long as one is held over it.
  function takeDropsOn(dropZone) {
    const showTaking = (isTaking) => dropZone.classList.toggle("taking-a-drop", isTaking);
    dropZone.addEventListener("dragenter", (event) => { if (carriesFiles(event)) showTaking(true); });
    dropZone.addEventListener("dragleave", (event) => { if (hasLeft(dropZone, event)) showTaking(false); });
    dropZone.addEventListener("drop", (event) => {
      showTaking(false);
      takePicturesFrom(event, event.dataTransfer);
    });
    leaveThePageAloneWhenAFileIsDropped();
  }

  // A file dropped on a web page makes the page show that file in place of itself. The app is never to do that.
  function leaveThePageAloneWhenAFileIsDropped() {
    ["dragover", "drop"].forEach((moment) => document.addEventListener(moment, (event) => { if (carriesFiles(event)) event.preventDefault(); }));
  }

  function start(pageParts, pageListeners) {
    parts = pageParts;
    listeners = pageListeners;
    parts.button.addEventListener("click", () => parts.picker.click());
    parts.picker.addEventListener("change", () => {
      addEach(Array.from(parts.picker.files));
      parts.picker.value = "";
    });
    parts.message.addEventListener("paste", (event) => takePicturesFrom(event, event.clipboardData));
    takeDropsOn(parts.dropZone);
    draw();
  }

  window.pictureTray = { start, setBusy, handOverFor, letGo, add, isWaiting, stopWaiting };
})();
