// Picking a plugin from the Open MPC catalog, the community's list of open plugins for the MPC, to remix it
// or to take it in as it is.
//
// Each plugin is a tile with a small picture of its screen, and the whole catalog can be scrolled through.
// For the one picked, what its makers wrote about it (its README) and the picture of its screen are fetched
// to go along with the request. For a remix none of its code is fetched: it is built anew from what is said
// and shown of the original and what the musician wants different. Taking a plugin in as it is fetches its
// makers' own code, with no AI, when its licence gives everybody leave to.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element } = window.helpers;
  const { asked } = window.sheets;

  const TILES_AT_A_TIME = 40;
  const NEAR_THE_LIST = "160px";
  const FETCHING = "Fetching the Open MPC catalog…";
  const FETCHING_ITS_README = "Fetching its README…";
  const ABOUT_THE_TWO_WAYS = "Pick a plugin, then REMIX THIS to have the AI build your own version of it, or IMPORT AS IT IS to bring in its makers' own code with no AI.";
  const NOTHING_PICKED_YET = "Pick a plugin from the list to see what it is.";
  const COMES_IN_UNRUN = "Its makers' code comes in as a new plugin. Nothing of it runs until you run its safety check.";
  const CHECKING_ITS_SOURCE = "Checking its source…";
  const NOTHING_TO_BUILD = "Its makers do not keep it as a project MP Forge can build, so it can only be remixed.";
  const ONLY_TO_REMIX = "Its licence, or where its source is kept, does not let MP Forge take it in, so it can only be remixed.";
  const ABOUT_A_REMIX = "The AI builds your own version from what its makers say of it and how its screen looks. None of its code is copied.";
  const CREDIT_THE_ORIGINAL = "Credit the original if you share your remix.";
  const NO_PICTURE = "No picture";
  const NOTHING_FOUND = "Nothing in the catalog by those words.";
  const NOTHING_IN_PARTICULAR = "Nothing in particular. Make it your own in one clear way, and say which.";

  // The catalog is asked for once while the app is open. A try that failed is forgotten, so the next one asks again.
  let catalog;
  const materialAskedFor = new Map();
  const projectAskedFor = new Map();

  function listedPlugins() {
    if (!catalog) {
      catalog = ask("open_mpc_catalog");
      catalog.catch(() => { catalog = undefined; });
    }
    return catalog;
  }

  // --- What goes along with a remix ------------------------------------------------------------

  function readmeOf(plugin) {
    return plugin.source ? ask("remix_readme", { source: plugin.source }).catch(() => null) : Promise.resolve(null);
  }

  function screenshotOf(plugin) {
    if (!plugin.screenshot) return Promise.resolve(null);
    return ask("remix_screenshot", { address: plugin.screenshot }).then((bytes) => new Blob([bytes]), () => null);
  }

  // What its makers wrote about a plugin and the picture of its screen, asked for once for each plugin. Either
  // is nothing when there is none or it could not be fetched, and the remix then goes ahead without it.
  function materialOf(plugin) {
    if (!materialAskedFor.has(plugin.id)) {
      const both = Promise.all([readmeOf(plugin), screenshotOf(plugin)]);
      materialAskedFor.set(plugin.id, both.then(([readme, screenshot]) => ({ readme, screenshot })));
    }
    return materialAskedFor.get(plugin.id);
  }

  // Whether the plugin's source holds a project MP Forge can build, asked once for each plugin. When that
  // cannot be found out, the import is still offered, and says for itself why it did not go through.
  function holdsAProject(plugin) {
    if (!projectAskedFor.has(plugin.id)) projectAskedFor.set(plugin.id, ask("open_mpc_source_holds_a_project", { source: plugin.source }).catch(() => true));
    return projectAskedFor.get(plugin.id);
  }

  // Whether the plugin can be taken in as it is, and the line that says so or says why not. Its licence has to
  // give everybody leave, and its source has to hold a project.
  function takingItIn(plugin) {
    if (!plugin.termsForTakingItIn) return Promise.resolve({ can: false, says: ONLY_TO_REMIX });
    const asItIs = { can: true, says: `${COMES_IN_UNRUN} ${plugin.termsForTakingItIn}` };
    return holdsAProject(plugin).then((holdsOne) => (holdsOne ? asItIs : { can: false, says: NOTHING_TO_BUILD }));
  }

  // --- The tiles -------------------------------------------------------------------------------

  function matches(sought, plugin) {
    const words = `${plugin.name} ${plugin.author} ${plugin.kind} ${plugin.style || ""} ${plugin.summary}`.toLowerCase();
    return sought.split(/\s+/).every((word) => words.includes(word));
  }

  function inAWord(plugin) {
    return [plugin.kind, plugin.style, plugin.author].filter(Boolean).join(" · ");
  }

  function counted(found, all) {
    return found === all ? `${all} plugins` : `${found} of ${all} plugins`;
  }

  // The catalog's screenshots are full size, so a tile's is fetched only when the tile comes near the part of
  // the list that shows, and not for every plugin the moment the list opens.
  function fetchingWhenNear(list) {
    const fetchThoseNear = (seen, watching) => seen.filter((tile) => tile.isIntersecting).forEach((tile) => {
      tile.target.src = tile.target.dataset.screenshot;
      watching.unobserve(tile.target);
    });
    return new IntersectionObserver(fetchThoseNear, { root: list, rootMargin: NEAR_THE_LIST });
  }

  function pictureFetchedWhenNear(plugin, thumbnails, onNotFetched) {
    const picture = element("img", "remix-thumbnail-picture");
    picture.alt = "";
    picture.decoding = "async";
    picture.dataset.screenshot = plugin.screenshot;
    picture.addEventListener("error", onNotFetched);
    thumbnails.observe(picture);
    return picture;
  }

  // A small picture of the plugin's screen, or words in its place when there is none to show.
  function thumbnailOf(plugin, thumbnails) {
    const frame = element("span", "remix-thumbnail");
    const showNoPicture = () => frame.replaceChildren(NO_PICTURE);
    if (plugin.screenshot) frame.append(pictureFetchedWhenNear(plugin, thumbnails, showNoPicture));
    else showNoPicture();
    return frame;
  }

  function pluginTile(plugin, thumbnails, onPick) {
    const tile = element("button", "remix-tile");
    tile.type = "button";
    tile.setAttribute("role", "option");
    tile.append(thumbnailOf(plugin, thumbnails), element("span", "remix-name", plugin.name), element("span", "soft small", inAWord(plugin)));
    tile.addEventListener("click", () => onPick(plugin));
    return tile;
  }

  // --- The plugin that is picked ---------------------------------------------------------------

  // A picture that cannot be fetched is taken away.
  function largePictureOf(plugin) {
    const picture = element("img", "remix-screenshot");
    picture.src = plugin.screenshot;
    picture.alt = `The screen of ${plugin.name}`;
    picture.addEventListener("error", () => picture.remove());
    return picture;
  }

  // One thing worth knowing about the plugin: what it is about in a word, then what is to be known.
  function fact(about, known) {
    const row = element("div", "remix-fact");
    row.append(element("span", "remix-fact-about", about), known);
    return row;
  }

  // Whether the plugin can be taken in as it is: a mark that says yes or no at a glance, and why.
  function onTakingItIn(plugin) {
    const mark = element("span", "remix-mark is-unknown", "…");
    const why = element("span", "", CHECKING_ITS_SOURCE);
    const known = element("span", "remix-fact-known");
    known.append(mark, why);
    takingItIn(plugin).then(({ can, says }) => {
      mark.textContent = can ? "YES" : "NO";
      mark.className = `remix-mark ${can ? "is-yes" : "is-no"}`;
      why.textContent = says;
    });
    return known;
  }

  // What the makers wrote, folded away until it is asked for. A plugin with no README shows nothing here.
  function readmeBox(plugin) {
    const box = element("details", "remix-readme soft small");
    box.append(element("summary", "", FETCHING_ITS_README));
    const show = (readme) => box.replaceChildren(element("summary", "", "What its makers wrote about it"), element("div", "remix-readme-words", readme.shown));
    materialOf(plugin).then(({ readme }) => (readme ? show(readme) : box.remove()));
    return box;
  }

  function aboutPicked(plugin) {
    const heading = element("div", "remix-heading");
    heading.append(element("div", "remix-picked-name", plugin.name), element("div", "soft small", inAWord(plugin)));
    const facts = [
      fact("Licence", element("span", "remix-fact-known", plugin.license || "Not said")),
      fact("Remix", element("span", "remix-fact-known", `${ABOUT_A_REMIX} ${CREDIT_THE_ORIGINAL}`)),
      fact("Import as it is", onTakingItIn(plugin)),
    ];
    const picture = plugin.screenshot ? [largePictureOf(plugin)] : [];
    return [heading, ...picture, element("div", "remix-summary", plugin.summary), ...facts, readmeBox(plugin)];
  }

  // --- The sheet -------------------------------------------------------------------------------

  // Shows the catalog on a sheet. Answers with the plugin picked as `original` and whether it is to be taken in
  // `asItIs` and not remixed, or with nothing.
  function pick(sheet) {
    const search = element("input", "item-input");
    const count = element("div", "soft small");
    const list = element("div", "remix-list");
    const about = element("div", "remix-about");
    const finding = element("div", "remix-finding");
    const columns = element("div", "remix-columns");
    const moreToCome = element("div", "remix-note");
    const picking = { plugins: [], found: [], tiles: new Map(), picked: null, confirm: null, takeIn: null };
    const thumbnails = fetchingWhenNear(list);
    const reachingTheEnd = new IntersectionObserver((seen) => seen.some((end) => end.isIntersecting) && showMore(), { root: list, rootMargin: NEAR_THE_LIST });
    search.type = "search";
    search.placeholder = "Search the catalog: reverb, drum machine, a maker's name…";
    search.setAttribute("aria-label", "Search the Open MPC catalog");
    search.autocomplete = "off";
    search.spellcheck = false;
    list.setAttribute("role", "listbox");
    list.setAttribute("aria-label", "Plugins in the Open MPC catalog");

    // Only the mark moves when a plugin is picked. The tiles stay as they are, so no picture is fetched again.
    function markThePicked() {
      picking.tiles.forEach((tile, plugin) => tile.setAttribute("aria-selected", String(plugin === picking.picked)));
    }

    function choose(plugin) {
      picking.picked = plugin;
      picking.confirm.disabled = false;
      picking.takeIn.disabled = true;
      takingItIn(plugin).then(({ can }) => { if (picking.picked === plugin) picking.takeIn.disabled = !can; });
      about.replaceChildren(...aboutPicked(plugin));
      markThePicked();
    }

    // Adds the next tiles at the end of the list. The end is watched afresh each time, so that a list still
    // short of its end goes on filling without being scrolled.
    function showMore() {
      const next = picking.found.slice(picking.tiles.size, picking.tiles.size + TILES_AT_A_TIME);
      next.forEach((plugin) => picking.tiles.set(plugin, pluginTile(plugin, thumbnails, choose)));
      moreToCome.before(...next.map((plugin) => picking.tiles.get(plugin)));
      moreToCome.hidden = picking.tiles.size >= picking.found.length;
      reachingTheEnd.disconnect();
      if (!moreToCome.hidden) reachingTheEnd.observe(moreToCome);
      markThePicked();
    }

    function draw() {
      const sought = search.value.trim().toLowerCase();
      picking.found = picking.plugins.filter((plugin) => !sought || matches(sought, plugin));
      picking.tiles = new Map();
      thumbnails.disconnect();
      count.textContent = counted(picking.found.length, picking.plugins.length);
      list.replaceChildren(...(picking.found.length ? [] : [element("div", "remix-note soft", NOTHING_FOUND)]), moreToCome);
      list.scrollTop = 0;
      showMore();
    }

    about.replaceChildren(element("div", "remix-note soft", NOTHING_PICKED_YET));
    finding.append(search, count, list);
    columns.append(finding, about);
    list.replaceChildren(element("div", "remix-note soft", FETCHING));
    search.addEventListener("input", draw);
    listedPlugins().then(
      (plugins) => { picking.plugins = plugins; draw(); },
      (problem) => list.replaceChildren(element("div", "remix-note sheet-problem", String(problem))),
    );
    const takenAsItIs = {
      words: "IMPORT AS IT IS", answerOf: () => ({ original: picking.picked, asItIs: true }),
      onMade: (takeIn) => { picking.takeIn = takeIn; takeIn.disabled = true; },
    };
    const details = {
      title: "A PLUGIN FROM OPEN MPC", confirmWords: "REMIX THIS", roomy: true, focusOn: search,
      holds: [element("div", "soft small", ABOUT_THE_TWO_WAYS), columns],
      alsoOffers: [takenAsItIs],
      onMade: (confirm) => { picking.confirm = confirm; confirm.disabled = true; },
    };
    return asked(sheet, details, () => ({ original: picking.picked, asItIs: false }));
  }

  // --- What the builder is asked for -----------------------------------------------------------

  function whatIsWantedDifferent(typed) {
    return `What the musician wants different: ${typed || NOTHING_IN_PARTICULAR}`;
  }

  // The catalog's brief for the original, what its makers wrote about it, a word on the picture of its screen
  // when that goes along, then what the musician wants different. `sent` says which of those went along.
  function describedWith(plugin, wantedDifferent, sent) {
    const parts = [plugin.brief, sent.readme && sent.readme.toldToTheBuilder, sent.itsPicture && plugin.aboutItsPicture, whatIsWantedDifferent(wantedDifferent)];
    return parts.filter(Boolean).join("\n\n");
  }

  // The same as it shows in the conversation: a README is long, so it is only said to have gone along.
  function shownWith(plugin, wantedDifferent, sent) {
    const alongWent = [sent.readme && "its README", sent.itsPicture && "the picture of its screen"].filter(Boolean).join(" and ");
    const parts = [plugin.brief, alongWent && `MP Forge sent ${alongWent} along.`, whatIsWantedDifferent(wantedDifferent)];
    return parts.filter(Boolean).join("\n\n");
  }

  window.remixPicker = { pick, materialOf, describedWith, shownWith };
})();
