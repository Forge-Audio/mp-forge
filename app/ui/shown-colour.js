// The colour the page shows at one place in the window, looking from the front of the page to the back.
//
// Everything on the MPC's page is drawn as an image laid over an element or as a plain colour behind it,
// so the colour is read from whichever of those the place falls on first.

(function () {
  "use strict";

  // An image is taken to be clear where it is this faint or fainter, out of 255.
  const CLEAR_UP_TO = 8;
  const ADDRESS_OF_IMAGE = /url\("?(.+?)"?\)/;

  function asHex([red, green, blue]) {
    return `#${[red, green, blue].map((channel) => Math.round(channel).toString(16).padStart(2, "0")).join("")}`;
  }

  function holds(box, client) {
    return client.x >= box.left && client.x < box.right && client.y >= box.top && client.y < box.bottom;
  }

  function isShowing(style) {
    return style.display !== "none" && style.visibility !== "hidden" && Number(style.opacity) > 0;
  }

  function loaded(address) {
    return new Promise((resolve, reject) => {
      const image = new Image();
      image.onload = () => resolve(image);
      image.onerror = reject;
      image.src = address;
    });
  }

  // A length the page's styling gives as "50%" or "12px", measured against the room it has.
  function lengthOf(written, room) {
    return written.endsWith("%") ? (parseFloat(written) / 100) * room : parseFloat(written);
  }

  // How big an element draws its image: stretched to a share of the element, fitted to it, or at its own size.
  function drawnSize(style, box, image) {
    const [wide, tall = "auto"] = style.backgroundSize.split(" ");
    if (wide === "cover" || wide === "contain") {
      const fit = (wide === "cover" ? Math.max : Math.min)(box.width / image.naturalWidth, box.height / image.naturalHeight);
      return { width: image.naturalWidth * fit, height: image.naturalHeight * fit };
    }
    const shape = image.naturalWidth / image.naturalHeight;
    const width = wide === "auto" ? null : lengthOf(wide, box.width);
    const height = tall === "auto" ? null : lengthOf(tall, box.height);
    if (width === null && height === null) return { width: image.naturalWidth, height: image.naturalHeight };
    return { width: width === null ? height * shape : width, height: height === null ? width / shape : height };
  }

  // Where in its image, from 0 to 1 each way, an element shows the given place. A film strip shows one frame of many,
  // so the image is usually taller than the element and slid up behind it.
  function placeInImage(style, box, image, client) {
    const drawn = drawnSize(style, box, image);
    const left = box.left + lengthOf(style.backgroundPositionX, box.width - drawn.width);
    const top = box.top + lengthOf(style.backgroundPositionY, box.height - drawn.height);
    return { across: (client.x - left) / drawn.width, down: (client.y - top) / drawn.height };
  }

  function pixelOf(image, place) {
    const probe = Object.assign(document.createElement("canvas"), { width: 1, height: 1 }).getContext("2d");
    probe.drawImage(image, Math.floor(place.across * image.naturalWidth), Math.floor(place.down * image.naturalHeight), 1, 1, 0, 0, 1, 1);
    const pixel = probe.getImageData(0, 0, 1, 1).data;
    return pixel[3] > CLEAR_UP_TO ? asHex(pixel) : null;
  }

  async function colourOfImageAt(style, box, client) {
    const address = ADDRESS_OF_IMAGE.exec(style.backgroundImage);
    if (!address) return null;
    const image = await loaded(address[1]);
    const place = placeInImage(style, box, image, client);
    const isOnTheImage = place.across >= 0 && place.across < 1 && place.down >= 0 && place.down < 1;
    return isOnTheImage ? pixelOf(image, place) : null;
  }

  // The plain colour behind an element, which styling gives as "rgb(1, 2, 3)" or "rgba(1, 2, 3, 0.5)".
  function plainColourOf(style) {
    const numbers = (style.backgroundColor.match(/[\d.]+/g) || []).map(Number);
    const [red, green, blue, solid = 1] = numbers;
    return numbers.length >= 3 && solid > 0 ? asHex([red, green, blue]) : null;
  }

  async function colourOf(made, client) {
    const style = getComputedStyle(made);
    const box = made.getBoundingClientRect();
    if (!isShowing(style) || !holds(box, client)) return null;
    const fromImage = await colourOfImageAt(style, box, client).catch(() => null);
    return fromImage || plainColourOf(style);
  }

  // `drawn` are the elements to look through, from the back of the page to the front.
  // The answer is nothing when none of them shows a colour at that place.
  async function colourShownAt(drawn, client) {
    for (const made of drawn.slice().reverse()) {
      const colour = await colourOf(made, client);
      if (colour) return colour;
    }
    return null;
  }

  window.shownColour = { colourShownAt };
})();
