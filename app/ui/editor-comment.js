// A comment on part of the screen, for the AI to act on. The musician selects something in the editor, says
// what should change about it, and the words go to MP Forge with what was selected and where it sits.

(function () {
  "use strict";

  const { kindOf, nameOf } = window.editorPanels;

  const KIND_WORDS = { control: "control", heading: "heading", picture: "picture", backdrop: "backdrop picture", block: "colour block", words: "words", art: "artwork" };
  const SEND_KEY = "Enter";

  // "a", "a and b", "a, b and c".
  function listed(things) {
    if (things.length < 2) return things.join("");
    return `${things.slice(0, -1).join(", ")} and ${things[things.length - 1]}`;
  }

  // Where something sits, in the MPC's own pixels: how far from the left and the top, and how big.
  function placeOf(area) {
    const [left, top, width, height] = [area.x, area.y, area.width, area.height].map(Math.round);
    return `at ${left}, ${top}, ${width} by ${height}`;
  }

  function described(item, desk) {
    return `the ${KIND_WORDS[kindOf(item)]} "${nameOf(item, desk)}" (${placeOf(item.area)})`;
  }

  // What the comment is about, in words the AI can find the thing by. With nothing selected it is the whole page.
  function subjectOf(desk) {
    const page = `the ${desk.pageName()} page`;
    const selected = desk.selected();
    return selected.length ? `${listed(selected.map((item) => described(item, desk)))} on ${page}` : page;
  }

  function shortSubjectOf(desk) {
    const selected = desk.selected();
    if (!selected.length) return `About the whole ${desk.pageName()} page. Select something to ask about just that.`;
    return selected.length === 1 ? `About "${nameOf(selected[0], desk)}"` : `About the ${selected.length} things selected`;
  }

  // `parts` are the box's elements. `send` is given the whole request once the musician sends it.
  function start(parts, desk, send) {
    function draw() {
      parts.about.textContent = shortSubjectOf(desk);
      parts.send.disabled = !parts.words.value.trim();
    }

    function clear() {
      parts.words.value = "";
      draw();
    }

    function sendComment() {
      const comment = parts.words.value.trim();
      if (!comment) return;
      const request = `About ${subjectOf(desk)}: ${comment}`;
      clear();
      send(request);
    }

    function sendOnCommandEnter(event) {
      if (event.key === SEND_KEY && (event.metaKey || event.ctrlKey)) sendComment();
    }

    parts.words.addEventListener("input", draw);
    parts.words.addEventListener("keydown", sendOnCommandEnter);
    parts.send.addEventListener("click", sendComment);
    return { draw, clear };
  }

  window.editorComment = { start };
})();
