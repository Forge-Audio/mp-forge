// The pictures on a plugin's screen: where each one sits, and how its frames are played.
//
// A picture's image holds every frame, stacked top to bottom. One frame shows at a time, by sliding the image.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { addressOf, showAsBackground } = window.fetchedImages;
  const { PAGE, placeAt, areaOfBlock } = window.mpcStage;
  const { isHidden, styleOfPicture } = window.lookEdits;

  // Only for a picture whose own speed is not known, which the app always sends.
  const FRAMES_PER_SECOND = 15;

  // Makes an element that shows one frame of a film strip, and says how to show another.
  function filmStrip(className, frames) {
    const made = element("div", className);
    made.style.backgroundSize = `100% ${frames * 100}%`;
    const lastFrame = Math.max(1, frames - 1);
    const showFrame = (frame) => { made.style.backgroundPositionY = `${(frame / lastFrame) * 100}%`; };
    const showAmount = (amount) => showFrame(Math.round(amount * (frames - 1)));
    return { made, showFrame, showAmount };
  }

  // One play takes as long as the plugin's own clock takes to go round, so the preview keeps the MPC's pace.
  function playRoundAndRound(made, frames, seconds) {
    made.classList.add("loops");
    made.style.animationDuration = `${seconds || frames / FRAMES_PER_SECOND}s`;
    made.style.animationTimingFunction = `steps(${frames}, jump-none)`;
  }

  // Where a picture sits once the musician's own arrangement is applied. Without one, it sits where pictures.json says.
  function areaOf(plugin, picture) {
    const pages = (plugin.arrangement && plugin.arrangement.pages) || [];
    const itsPage = pages.find((page) => page.name === picture.page);
    const arranged = itsPage && itsPage.pictures.find((known) => known.name === picture.name);
    if (arranged) return arranged.area;
    return picture.slots ? areaOfBlock(picture.slots) : PAGE;
  }

  // The AI's own choice of where a picture sits: one given slots goes over the controls, and a backdrop behind everything.
  function ownDepthOf(picture) {
    return picture.slots ? "front" : "back";
  }

  function pictureOnPage(plugin, picture) {
    const strip = filmStrip(picture.slots ? "mpc-picture" : "mpc-picture backdrop", picture.frames);
    const area = areaOf(plugin, picture);
    const style = styleOfPicture(plugin.look || { pages: [] }, picture.page, { kind: "picture", name: picture.name }, ownDepthOf(picture));
    strip.made.style.opacity = String(style.opacity / 100);
    strip.made.setAttribute("role", "img");
    strip.made.setAttribute("aria-label", picture.name);
    placeAt(strip.made, area);
    showAsBackground(strip.made, addressOf("picture_image", { name: plugin.summary.name, picture: picture.name }, picture.version));
    if (picture.motion === "loops") playRoundAndRound(strip.made, picture.frames, picture.loopSeconds);
    return { ...strip, picture, area, style };
  }

  // A picture as something the musician can pick up. A backdrop sits behind everything, so it is only
  // picked from the list of layers, never by clicking the page.
  function asItem(drawn) {
    const isBackdrop = !drawn.picture.slots;
    return { kind: "picture", name: drawn.picture.name, area: drawn.area, made: drawn.made, style: drawn.style, resize: "free", stretchesLive: true, pickedFromListOnly: isBackdrop && drawn.style.depth === "back" };
  }

  function picturesMadeFor(plugin, pageName) {
    return (plugin.pictures || []).filter((picture) => picture.page === pageName);
  }

  function isHiddenByHand(plugin, pageName, picture) {
    return isHidden(plugin.look || { pages: [] }, pageName, { kind: "picture", name: picture.name });
  }

  // The musician can hide a picture from its page, and show it again.
  function picturesShowingOn(plugin, pageName) {
    return picturesMadeFor(plugin, pageName).filter((picture) => !isHiddenByHand(plugin, pageName, picture));
  }

  function picturesHiddenOn(plugin, pageName) {
    return picturesMadeFor(plugin, pageName).filter((picture) => isHiddenByHand(plugin, pageName, picture));
  }

  // The pictures of one page, by how far back they sit: `back` behind everything, `behind` behind the controls
  // but over the artwork, and `front` over the controls. Each is { made, items }: what to draw, and the same
  // pictures as things the musician can move and resize. `controlTurned` moves the pictures that follow a control.
  function picturesOf(plugin, pageName, amountOfControl) {
    const onPage = picturesShowingOn(plugin, pageName).map((picture) => pictureOnPage(plugin, picture));
    const atDepth = (depth) => {
      const there = onPage.filter((drawn) => drawn.style.depth === depth);
      return { made: there.map((drawn) => drawn.made), items: there.map(asItem) };
    };
    const followers = onPage.filter((drawn) => drawn.picture.motion === "follows");
    followers.forEach((drawn) => drawn.showAmount(amountOfControl(drawn.picture.follows)));
    const sameControl = (drawn, name) => String(drawn.picture.follows).toLowerCase() === String(name).toLowerCase();
    return {
      back: atDepth("back"),
      behind: atDepth("behind"),
      front: atDepth("front"),
      controlTurned: (name, amount) => followers.filter((drawn) => sameControl(drawn, name)).forEach((drawn) => drawn.showAmount(amount)),
    };
  }

  window.pluginPictures = { picturesOf, picturesHiddenOn, filmStrip };
})();
