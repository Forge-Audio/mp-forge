// A menu that opens from a button. It closes on a second press, a click anywhere else, or Escape.

(function () {
  "use strict";

  const NOTHING_TO_DO = () => {};

  function popupMenu(opener, menu, whenItChanges = NOTHING_TO_DO) {
    const isOpen = () => !menu.hidden;

    function show(open) {
      menu.hidden = !open;
      opener.setAttribute("aria-expanded", String(open));
      whenItChanges(open);
    }

    function closeWhenClickedAway(event) {
      const clickedInside = event.composedPath().includes(opener.parentElement);
      if (isOpen() && !clickedInside) show(false);
    }

    function closeOnEscape(event) {
      if (event.key !== "Escape" || !isOpen()) return;
      show(false);
      opener.focus();
    }

    opener.addEventListener("click", () => show(!isOpen()));
    document.addEventListener("click", closeWhenClickedAway);
    document.addEventListener("keydown", closeOnEscape);
    return { close: () => show(false) };
  }

  window.popupMenu = popupMenu;
})();
