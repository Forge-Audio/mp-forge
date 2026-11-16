// The strip of plugin tabs in the top bar. However many plugins there are it stays on one line: it scrolls, by
// the wheel or by the two arrows that show when there is more to either side. A tab can be dragged to another
// place, or moved with Alt and an arrow key, and the order is remembered.

(function () {
  "use strict";

  const { remember, recall } = window.helpers;

  const ORDER_KEPT_AS = "tabOrder";
  const A_TAB = ".tab[data-plugin]";
  const DRAG_BEGINS_AFTER_PX = 6;
  // While a tab is held this near an end of the strip, the strip scrolls that way.
  const EDGE_THAT_SCROLLS_PX = 36;
  const SCROLLS_PER_MOVE_PX = 14;
  const SHARE_SCROLLED_BY_AN_ARROW = 0.6;
  const MOVES_A_TAB = { ArrowLeft: -1, ArrowRight: 1 };

  let parts;
  let tellOfNewOrder;
  let held = null;
  let shownAsOpen;

  // --- The order ------------------------------------------------------------------------------------------

  function orderKept() {
    try { return JSON.parse(recall(ORDER_KEPT_AS)) || []; } catch (unreadable) { return []; }
  }

  // A plugin that has no place yet, such as one just made, comes after those that have.
  function placeIn(order, plugin) {
    const place = order.indexOf(plugin.name);
    return place === -1 ? order.length : place;
  }

  // The plugins in the order the musician put them in. Those with no place yet keep the order they came in.
  function inOrder(plugins) {
    const order = orderKept();
    return plugins.map((plugin, cameAt) => ({ plugin, cameAt })).sort((one, other) => placeIn(order, one.plugin) - placeIn(order, other.plugin) || one.cameAt - other.cameAt).map((placed) => placed.plugin);
  }

  function tabsNow() {
    return Array.from(parts.strip.querySelectorAll(A_TAB));
  }

  function keepTheOrderShown() {
    remember(ORDER_KEPT_AS, JSON.stringify(tabsNow().map((tab) => tab.dataset.plugin)));
  }

  // A plugin keeps its place under its new name.
  function renamed(oldName, newName) {
    remember(ORDER_KEPT_AS, JSON.stringify(orderKept().map((name) => (name === oldName ? newName : name))));
  }

  // --- Scrolling ------------------------------------------------------------------------------------------

  function furthestScroll() {
    return parts.strip.scrollWidth - parts.strip.clientWidth;
  }

  // Each arrow shows only while there are tabs out of sight on its side.
  function drawArrows() {
    parts.earlier.hidden = parts.strip.scrollLeft <= 0;
    parts.later.hidden = parts.strip.scrollLeft >= furthestScroll() - 1;
  }

  function scrollBy(distance) {
    parts.strip.scrollLeft += distance;
    drawArrows();
  }

  function scrollByAnArrow(direction) {
    scrollBy(direction * parts.strip.clientWidth * SHARE_SCROLLED_BY_AN_ARROW);
  }

  // A mouse wheel turns up and down, and the strip runs left and right, so the one is taken for the other.
  function scrollWithTheWheel(event) {
    const turnedUpOrDown = Math.abs(event.deltaY) > Math.abs(event.deltaX);
    if (!turnedUpOrDown || furthestScroll() <= 0) return;
    event.preventDefault();
    scrollBy(event.deltaY);
  }

  function bringIntoView(tab) {
    const strip = parts.strip.getBoundingClientRect();
    const box = tab.getBoundingClientRect();
    if (box.left < strip.left) scrollBy(box.left - strip.left);
    else if (box.right > strip.right) scrollBy(box.right - strip.right);
  }

  // Called once the tabs are drawn. A plugin that has just been opened is brought into view. One that was open
  // already is left where the musician scrolled it to.
  function settle(openName) {
    const justOpened = openName !== shownAsOpen;
    shownAsOpen = openName;
    const open = tabsNow().find((tab) => tab.dataset.plugin === openName);
    if (justOpened && open) bringIntoView(open);
    drawArrows();
  }

  // --- Dragging a tab to another place ----------------------------------------------------------------------

  function isBeingRearranged() {
    return Boolean(held && held.isDragged);
  }

  function middleOf(tab) {
    const box = tab.getBoundingClientRect();
    return box.left + box.width / 2;
  }

  // The held tab changes places with a neighbour as soon as the pointer is past that neighbour's middle.
  function changePlacesAt(pointerX) {
    const before = held.tab.previousElementSibling;
    const after = held.tab.nextElementSibling;
    if (before && pointerX < middleOf(before)) parts.strip.insertBefore(held.tab, before);
    else if (after && pointerX > middleOf(after)) parts.strip.insertBefore(after, held.tab);
  }

  function scrollNearAnEdge(pointerX) {
    const strip = parts.strip.getBoundingClientRect();
    if (pointerX < strip.left + EDGE_THAT_SCROLLS_PX) scrollBy(-SCROLLS_PER_MOVE_PX);
    else if (pointerX > strip.right - EDGE_THAT_SCROLLS_PX) scrollBy(SCROLLS_PER_MOVE_PX);
  }

  function beginDragging(event) {
    held.isDragged = true;
    held.tab.classList.add("is-being-moved");
    parts.strip.setPointerCapture(event.pointerId);
  }

  function hold(event) {
    const tab = event.target.closest(A_TAB);
    if (tab && event.button === 0) held = { tab, pressedAt: event.clientX, isDragged: false };
  }

  function follow(event) {
    if (!held) return;
    const hasGoneFarEnough = Math.abs(event.clientX - held.pressedAt) >= DRAG_BEGINS_AFTER_PX;
    if (!held.isDragged && hasGoneFarEnough) beginDragging(event);
    if (!held.isDragged) return;
    scrollNearAnEdge(event.clientX);
    changePlacesAt(event.clientX);
  }

  // Letting go of a dragged tab is not a press on it, so the click that follows opens nothing.
  function swallowTheClickThatFollows() {
    const swallow = (click) => { click.stopPropagation(); click.preventDefault(); };
    parts.strip.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(() => parts.strip.removeEventListener("click", swallow, { capture: true }), 0);
  }

  function letGo() {
    const wasDragged = isBeingRearranged();
    held = null;
    if (!wasDragged) return;
    swallowTheClickThatFollows();
    keepTheOrderShown();
    tellOfNewOrder();
  }

  // --- Moving a tab from the keyboard -----------------------------------------------------------------------

  function moveWithTheKeyboard(event) {
    const tab = event.target.closest(A_TAB);
    const direction = MOVES_A_TAB[event.key];
    if (!tab || !direction || !event.altKey) return;
    event.preventDefault();
    const neighbour = direction < 0 ? tab.previousElementSibling : tab.nextElementSibling;
    if (!neighbour) return;
    if (direction < 0) parts.strip.insertBefore(tab, neighbour); else parts.strip.insertBefore(neighbour, tab);
    tab.focus();
    bringIntoView(tab);
    keepTheOrderShown();
  }

  // `onNewOrder` is told when a tab has been dropped in another place, so the tabs can be drawn afresh.
  function start(pageParts, onNewOrder) {
    parts = pageParts;
    tellOfNewOrder = onNewOrder;
    parts.strip.addEventListener("pointerdown", hold);
    parts.strip.addEventListener("pointermove", follow);
    parts.strip.addEventListener("pointerup", letGo);
    parts.strip.addEventListener("pointercancel", letGo);
    parts.strip.addEventListener("keydown", moveWithTheKeyboard);
    parts.strip.addEventListener("wheel", scrollWithTheWheel, { passive: false });
    parts.strip.addEventListener("scroll", drawArrows);
    parts.earlier.addEventListener("click", () => scrollByAnArrow(-1));
    parts.later.addEventListener("click", () => scrollByAnArrow(1));
    new ResizeObserver(drawArrows).observe(parts.strip);
  }

  window.tabStrip = { start, inOrder, settle, renamed, isBeingRearranged };
})();
