// A list to pick one thing from, drawn in the app's own look in place of the computer's dropdown.
// A button shows what is picked. Pressing it opens the list below, with the picked row marked.

(function () {
  "use strict";

  const { element } = window.helpers;

  const STEP_OF_KEY = { ArrowDown: 1, ArrowUp: -1 };

  function pickList(opener, menu, whenPicked) {
    let names = [];
    let picked = 0;
    const popup = window.popupMenu(opener, menu, (isOpen) => { if (isOpen) focusRow(picked); });

    function rows() {
      return Array.from(menu.children);
    }

    function focusRow(index) {
      const row = rows()[index];
      if (row) row.focus();
    }

    function pick(index) {
      popup.close();
      opener.focus();
      whenPicked(index);
    }

    function rowFor(name, index) {
      const row = element("button", "menu-row");
      row.type = "button";
      row.setAttribute("role", "menuitemradio");
      row.setAttribute("aria-checked", String(index === picked));
      row.append(element("span", "", name), element("span", index === picked ? "menu-mark" : ""));
      row.addEventListener("click", () => pick(index));
      return row;
    }

    function draw() {
      opener.textContent = names[picked] || "";
      menu.replaceChildren(...names.map(rowFor));
    }

    function show(namesToShow, indexPicked) {
      names = namesToShow;
      picked = indexPicked;
      draw();
    }

    function choose(index) {
      show(names, index);
    }

    function stepThroughRows(event) {
      const step = STEP_OF_KEY[event.key];
      if (!step) return;
      event.preventDefault();
      const count = names.length;
      focusRow((rows().indexOf(document.activeElement) + step + count) % count);
    }

    menu.addEventListener("keydown", stepThroughRows);
    return { show, choose, count: () => names.length, picked: () => picked };
  }

  window.pickList = pickList;
})();
