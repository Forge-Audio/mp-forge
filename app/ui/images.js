// Images the app hands over as bytes, kept as addresses the page can show.
//
// Each image is fetched once. When its version changes, the newer image replaces the one that was kept.

(function () {
  "use strict";

  const { ask } = window.backend;

  const kept = new Map();
  // Drawings are asked for by what they show, so every change makes another. Only the newest few are held on to.
  const drawings = new Map();
  const MOST_DRAWINGS_KEPT = 120;

  function fetched(command, details) {
    return ask(command, details).then((bytes) => URL.createObjectURL(new Blob([bytes], { type: "image/png" })));
  }

  // `command` and `details` say which image to ask the app for. The answer is a promise of its address.
  function addressOf(command, details, version) {
    const key = `${command} ${JSON.stringify(details)}`;
    const held = kept.get(key);
    if (held && held.version === version) return held.address;
    if (held) held.address.then(URL.revokeObjectURL, () => {});
    const address = fetched(command, details);
    kept.set(key, { version, address });
    return address;
  }

  function forgetOldestDrawing() {
    const [oldestKey, oldest] = drawings.entries().next().value;
    drawings.delete(oldestKey);
    oldest.then(URL.revokeObjectURL, () => {});
  }

  // An image the app draws from the details alone, such as one layer of artwork. The same details always give
  // the same image, so it is fetched once and found again for as long as it is among the newest.
  function addressOfDrawing(command, details) {
    const key = `${command} ${JSON.stringify(details)}`;
    const held = drawings.get(key);
    drawings.delete(key);
    const address = held || fetched(command, details);
    drawings.set(key, address);
    if (drawings.size > MOST_DRAWINGS_KEPT) forgetOldestDrawing();
    return address;
  }

  // The image itself, once it has arrived and can be drawn onto a canvas.
  function loaded(address) {
    return address.then((url) => new Promise((resolve, reject) => {
      const image = new Image();
      image.onload = () => resolve(image);
      image.onerror = reject;
      image.src = url;
    }));
  }

  // Shows the image as the element's background once it arrives. An image that cannot be had marks the element instead.
  function showAsBackground(made, address) {
    const show = (url) => { made.style.backgroundImage = `url("${url}")`; };
    address.then(show, () => made.classList.add("missing"));
  }

  window.fetchedImages = { addressOf, addressOfDrawing, loaded, showAsBackground };
})();
