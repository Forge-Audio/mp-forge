// The settings are sorted into groups, and the page shows one group at a time, picked from a list beside it.
//
// A group is named by its own heading, so the list is drawn from the page and the two cannot disagree.

(function () {
  "use strict";

  const { button } = window.sheets;

  let groups = [];
  let buttons = [];

  function nameOf(group) {
    return group.querySelector(".group-heading").textContent;
  }

  function show(chosen) {
    groups.forEach((group) => { group.hidden = group !== chosen; });
    buttons.forEach((made, place) => made.setAttribute("aria-pressed", String(groups[place] === chosen)));
    chosen.parentElement.scrollTop = 0;
  }

  // Shows the group a setting is in, for whatever leads the musician straight to that setting.
  function showTheGroupOf(setting) {
    show(setting.closest(".settings-group"));
  }

  // Draws the list of groups into `list` and shows the first of them.
  function start(list, shown) {
    groups = [...shown.querySelectorAll(".settings-group")];
    buttons = groups.map((group) => button("group-button", nameOf(group)));
    buttons.forEach((made, place) => made.addEventListener("click", () => show(groups[place])));
    list.replaceChildren(...buttons);
    show(groups[0]);
  }

  window.settingsGroups = { start, showTheGroupOf };
})();
