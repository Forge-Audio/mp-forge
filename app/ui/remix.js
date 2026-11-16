// Picking a plugin to remix from the Open MPC catalog, the community's list of open plugins for the MPC.
//
// Each plugin is a tile with a small picture of its screen, and the whole catalog can be scrolled through.
// For the one picked, what its makers wrote about it (its README) and the picture of its screen are fetched
// to go along with the request. None of its code is fetched: the remix is built anew from what is said and
// shown of the original and what the musician wants different.

(function () {
  "use strict";

  const { ask } = window.backend;
  const { element } = window.helpers;
  const { asked } = window.sheets;

  const TILES_AT_A_TIME = 40;
  const NEAR_THE_LIST = "160px";
  const FETCHING = "Fetching the Open MPC catalog…";
  const FETCHING_ITS_README = "Fetching its README…";
  const ABOUT_REMIXING = "MP Forge builds your own version from what the original's makers say of it and how its screen looks. None of its code is copied or installed.";
  const CREDIT_THE_ORIGINAL = "Credit the original if you share your remix.";
  const NO_PICTURE = "No picture";
  const NOTHING_FOUND = "Nothing in the catalog by those words.";
  const NOTHING_IN_PARTICULAR = "Nothing in particular. Make it your own in one clear way, and say which.";

  // The catalog is asked for once while the app is open. A try that failed is forgotten, so the next one asks again.
  let catalog;
  const materialAskedFor = new Map();

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

  function licenceLine(plugin) {
    return plugin.license ? `Shared under ${plugin.license}. ${CREDIT_THE_ORIGINAL}` : CREDIT_THE_ORIGINAL;
  }

  // What the makers wrote, shown once it has been fetched. A plugin with no README shows nothing here.
  function readmeBox(plugin) {
    const box = element("div", "remix-readme soft small", FETCHING_ITS_README);
    const show = (readme) => box.replaceChildren(element("div", "field-label", "From its README"), readme.shown);
    materialOf(plugin).then(({ readme }) => (readme ? show(readme) : box.remove()));
    return box;
  }

  function aboutPicked(plugin) {
    const words = element("div", "remix-words");
    words.append(element("div", "remix-summary", plugin.summary), element("div", "soft small", licenceLine(plugin)), readmeBox(plugin));
    return plugin.screenshot ? [largePictureOf(plugin), words] : [words];
  }

  // --- The sheet -------------------------------------------------------------------------------

  // Shows the catalog on a sheet. Answers with the plugin picked, or with nothing.
  function pick(sheet) {
    const search = element("input", "item-input");
    const count = element("div", "soft small");
    const list = element("div", "remix-list");
    const about = element("div", "remix-about");
    const moreToCome = element("div", "remix-note");
    const picking = { plugins: [], found: [], tiles: new Map(), picked: null, confirm: null };
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

    list.replaceChildren(element("div", "remix-note soft", FETCHING));
    search.addEventListener("input", draw);
    listedPlugins().then(
      (plugins) => { picking.plugins = plugins; draw(); },
      (problem) => list.replaceChildren(element("div", "remix-note sheet-problem", String(problem))),
    );
    const details = {
      title: "REMIX A PLUGIN FROM OPEN MPC", confirmWords: "REMIX THIS", wide: true, focusOn: search,
      holds: [element("div", "soft small", ABOUT_REMIXING), search, count, list, about],
      onMade: (confirm) => { picking.confirm = confirm; confirm.disabled = true; },
    };
    return asked(sheet, details, () => picking.picked);
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
