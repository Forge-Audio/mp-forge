// The versions of a plugin laid out as a timeline, oldest on the left. The musician steps along it and picks the
// version to go back to. Going back never throws anything away, so they can step forward again afterwards.
// Versions they no longer want can be thrown away from here, which frees the room they take: the one chosen,
// several marked one by one, or every one but how the plugin is now.

(function () {
  "use strict";

  const { element } = window.helpers;
  const { asked, button } = window.sheets;

  const NOW = "now";
  const ABOUT_GOING_BACK = "Going back puts the plugin's sound, screen and pictures back as that version had them, then builds and checks it again. How it is now is kept as a version too, so you can step forward again.";
  const ABOUT_DELETING = "Throws versions away for good, which frees the room they take. The plugin as it is now is not touched.";
  const ABOUT_MARKING = "Marks this version to be deleted with others. Shift-click a version does the same.";
  const DELETE_THIS = "DELETE THIS VERSION";
  const DELETE_ALL_BUT_NOW = "DELETE ALL BUT NOW";
  const REALLY = "REALLY DELETE? IT CANNOT BE UNDONE";
  const MARK = "MARK TO DELETE";
  const UNMARK = "UNMARK";
  const BYTES_IN_A_KB = 1024;

  function whenKept(version) {
    if (!version.keptAt) return "";
    return new Date(version.keptAt * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
  }

  // The room some files take, in the largest unit that keeps the number short: "340 KB", "1.3 MB".
  function roomTaken(bytes) {
    const kilobytes = (bytes || 0) / BYTES_IN_A_KB;
    return kilobytes < BYTES_IN_A_KB ? `${Math.max(1, Math.round(kilobytes))} KB` : `${(kilobytes / BYTES_IN_A_KB).toFixed(1)} MB`;
  }

  function wordsAbout(version, count) {
    const reason = version.keptBefore ? `Kept before ${version.keptBefore}.` : "Kept before a change.";
    const standing = version.isHowItIsNow ? " This is how the plugin is now." : "";
    const when = [whenKept(version), `takes ${roomTaken(version.bytes)}`].filter(Boolean).join(" · ");
    return { heading: `Version ${version.number} (${count} kept)`, when, reason: reason + standing };
  }

  const WORDS_ABOUT_NOW = { heading: "Now", when: "", reason: "How the plugin is now, after its last change." };

  // One stop for each kept version. When the plugin has changed since the newest of them, "now" is a stop of its own.
  function stopsOf(versions) {
    const stops = versions.map((version) => ({ id: version.number, label: String(version.number), isNow: version.isHowItIsNow, version }));
    const isAVersionAlready = versions.some((version) => version.isHowItIsNow);
    return isAVersionAlready ? stops : [...stops, { id: NOW, label: "NOW", isNow: true, version: null }];
  }

  function stopButton(stop) {
    const made = button("timeline-stop", stop.label);
    made.dataset.stop = String(stop.id);
    if (stop.isNow) made.classList.add("is-now");
    return made;
  }

  // The stops in a row, with dashes between each two.
  function joinedByLinks(buttons) {
    return buttons.flatMap((made, index) => (index ? [element("div", "timeline-link"), made] : [made]));
  }

  function roomTakenByAll(versions) {
    const bytes = versions.reduce((sum, version) => sum + (version.bytes || 0), 0);
    return versions.length ? `${versions.length} version${versions.length === 1 ? "" : "s"} kept, taking ${roomTaken(bytes)} in all.` : "No versions are kept.";
  }

  function wordsToDeleteMarked(count) {
    return count === 1 ? "DELETE THE MARKED VERSION" : `DELETE THE ${count} MARKED VERSIONS`;
  }

  // A button for something that cannot be undone: the first press only asks, and the second one does it.
  function askingTwice(made, doIt) {
    const isAsking = () => made.classList.contains("is-asking");
    made.addEventListener("click", () => {
      if (isAsking()) return doIt();
      made.textContent = REALLY;
      made.classList.add("is-asking");
    });
    return { showAs: (words) => { made.textContent = words; made.classList.remove("is-asking"); } };
  }

  // Shows the timeline. Answers with the number of the version to go back to, or with nothing.
  // `forget` throws one version away for good and answers with the versions that are left.
  function pick(sheet, pluginName, versionsKept, forget) {
    let versions = versionsKept;
    let stops = [];
    let buttons = [];
    let chosen;
    let confirm;
    const marked = new Set();
    const track = element("div", "timeline");
    const heading = element("div", "timeline-heading");
    const when = element("div", "soft small");
    const reason = element("div", "");
    const room = element("div", "soft small");
    const problem = element("div", "sheet-problem");
    const markButton = button("item-button", MARK);
    const deleteButton = button("item-button", DELETE_THIS);
    const deleteOthersButton = button("item-button", DELETE_ALL_BUT_NOW);
    const deleting = askingTwice(deleteButton, () => deleteAll(marked.size ? [...marked] : [chosen.version.number]));
    const deletingOthers = askingTwice(deleteOthersButton, () => deleteAll(numbersOfAllButNow()));
    const deleteButtons = element("div", "item-buttons");

    function numbersOfAllButNow() {
      return versions.filter((version) => !version.isHowItIsNow).map((version) => version.number);
    }

    function showDeleteButtons() {
      const isMarked = Boolean(chosen.version) && marked.has(chosen.version.number);
      markButton.textContent = isMarked ? UNMARK : MARK;
      markButton.disabled = !chosen.version;
      deleting.showAs(marked.size ? wordsToDeleteMarked(marked.size) : DELETE_THIS);
      deleteButton.disabled = !marked.size && !chosen.version;
      deletingOthers.showAs(DELETE_ALL_BUT_NOW);
      deleteOthersButton.disabled = numbersOfAllButNow().length === 0;
    }

    function showMarks() {
      buttons.forEach((made, index) => made.classList.toggle("is-marked", marked.has(stops[index].id)));
    }

    function choose(stop) {
      chosen = stop;
      const words = stop.version ? wordsAbout(stop.version, versions.length) : WORDS_ABOUT_NOW;
      heading.textContent = words.heading;
      when.textContent = words.when;
      reason.textContent = words.reason;
      buttons.forEach((made, index) => made.setAttribute("aria-pressed", String(stops[index] === stop)));
      buttons[stops.indexOf(stop)].scrollIntoView({ block: "nearest", inline: "nearest" });
      showDeleteButtons();
      if (confirm) confirm.disabled = stop.isNow;
    }

    function toggleMark(stop) {
      if (!stop.version) return;
      if (!marked.delete(stop.id)) marked.add(stop.id);
      showMarks();
    }

    function stopPressed(stop, press) {
      if (press.shiftKey || press.metaKey || press.ctrlKey) toggleMark(stop);
      choose(stop);
    }

    function drawStops(idToChoose) {
      stops = stopsOf(versions);
      buttons = stops.map(stopButton);
      buttons.forEach((made, index) => made.addEventListener("click", (press) => stopPressed(stops[index], press)));
      track.replaceChildren(...joinedByLinks(buttons));
      room.textContent = roomTakenByAll(versions);
      showMarks();
      choose(stops.find((stop) => stop.id === idToChoose) || stops.find((stop) => stop.isNow));
    }

    function stepBy(distance) {
      const next = stops[stops.indexOf(chosen) + distance];
      if (!next) return;
      choose(next);
      buttons[stops.indexOf(next)].focus();
    }

    // The nearest stop that is staying is chosen next, so more can be thrown away one after another.
    function stopToChooseAfterDeleting(numbers) {
      const staying = (stop) => !numbers.includes(stop.id);
      const place = stops.indexOf(chosen);
      const neighbour = stops.slice(place).find(staying) || stops.slice(0, place).reverse().find(staying);
      return neighbour && neighbour.id;
    }

    // Versions go one at a time. One that will not go stops the rest, and says why.
    async function deleteAll(numbers) {
      const idToChoose = stopToChooseAfterDeleting(numbers);
      problem.hidden = true;
      try {
        for (const number of numbers) {
          versions = await forget(number);
          marked.delete(number);
        }
      } catch (refusal) {
        problem.textContent = String(refusal);
        problem.hidden = false;
      }
      drawStops(idToChoose);
    }

    track.setAttribute("role", "group");
    track.setAttribute("aria-label", "Versions, oldest first");
    track.addEventListener("keydown", (event) => {
      if (event.key === "ArrowLeft") stepBy(-1);
      if (event.key === "ArrowRight") stepBy(1);
    });
    markButton.title = ABOUT_MARKING;
    markButton.addEventListener("click", () => { toggleMark(chosen); showDeleteButtons(); });
    deleteButton.title = ABOUT_DELETING;
    deleteOthersButton.title = "Throws away every kept version but how the plugin is now.";
    deleteButtons.append(markButton, deleteButton, deleteOthersButton);
    problem.hidden = true;
    drawStops();

    const details = {
      title: `VERSIONS OF ${pluginName.toUpperCase()}`,
      confirmWords: "GO BACK TO THIS VERSION",
      cancelWords: "CLOSE",
      wide: true,
      holds: [track, heading, when, reason, deleteButtons, problem, room, element("div", "soft small", ABOUT_GOING_BACK)],
      focusOn: buttons[stops.indexOf(chosen)],
      onMade: (made) => { confirm = made; choose(chosen); },
    };
    return asked(sheet, details, () => chosen.id).then((answer) => (typeof answer === "number" ? answer : null));
  }

  window.versionTimeline = { pick };
})();
