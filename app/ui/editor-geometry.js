// The sums behind arranging a screen: where a pointer is on the MPC's page, what a box touches,
// what snaps to what, and where things go when they are lined up.
//
// Everything is measured in the MPC's own pixels. Nothing here draws or remembers anything.

(function () {
  "use strict";

  const { PAGE } = window.mpcStage;

  // How close an edge has to come to another before it jumps onto it.
  const SNAP_WITHIN = 6;
  const NO_SNAP = { dx: 0, dy: 0, guides: [] };
  const ACROSS = { start: "x", size: "width" };
  const DOWN = { start: "y", size: "height" };

  // Where on the MPC's page a pointer is, whatever size the page is drawn at.
  function pagePoint(stage, client) {
    const shown = stage.getBoundingClientRect();
    const scale = PAGE.width / shown.width;
    return { x: (client.x - shown.left) * scale, y: (client.y - shown.top) * scale };
  }

  function holds(area, point) {
    return point.x >= area.x && point.x < area.x + area.width && point.y >= area.y && point.y < area.y + area.height;
  }

  function touches(area, box) {
    const apartAcross = area.x + area.width <= box.x || box.x + box.width <= area.x;
    const apartDown = area.y + area.height <= box.y || box.y + box.height <= area.y;
    return !apartAcross && !apartDown;
  }

  function boxBetween(one, other) {
    const x = Math.min(one.x, other.x);
    const y = Math.min(one.y, other.y);
    return { x, y, width: Math.abs(one.x - other.x), height: Math.abs(one.y - other.y) };
  }

  // The smallest area that holds all of them.
  function around(areas) {
    const x = Math.min(...areas.map((area) => area.x));
    const y = Math.min(...areas.map((area) => area.y));
    const right = Math.max(...areas.map((area) => area.x + area.width));
    const bottom = Math.max(...areas.map((area) => area.y + area.height));
    return { x, y, width: right - x, height: bottom - y };
  }

  function movedBy(area, dx, dy) {
    return { ...area, x: area.x + dx, y: area.y + dy };
  }

  // The same area, pushed back onto the page if any of it hangs off, on whole pixels.
  function keptOnPage(area) {
    const within = (start, size, most) => Math.round(Math.min(Math.max(0, start), Math.max(0, most - size)));
    return { ...area, x: within(area.x, area.width, PAGE.width), y: within(area.y, area.height, PAGE.height) };
  }

  // --- Snapping --------------------------------------------------------------------------------

  // The three lines of an area that others can line up with: its near edge, its middle and its far edge.
  function linesOf(area, along) {
    const start = area[along.start];
    return [start, start + area[along.size] / 2, start + area[along.size]];
  }

  // The smallest nudge that puts one of the moving area's lines on one of the still lines, if any is close enough.
  function pullAlong(moving, stillAreas, along) {
    const stillLines = stillAreas.flatMap((area) => linesOf(area, along));
    const pulls = linesOf(moving, along).flatMap((line) => stillLines.map((still) => ({ by: still - line, at: still })));
    const close = pulls.filter((pull) => Math.abs(pull.by) <= SNAP_WITHIN);
    return close.sort((one, other) => Math.abs(one.by) - Math.abs(other.by))[0];
  }

  // How far to nudge a moving area so it lines up with something nearby, and the lines to draw where it does.
  // The page itself counts as something to line up with.
  function snapped(moving, stillAreas) {
    const across = pullAlong(moving, [...stillAreas, PAGE], ACROSS);
    const down = pullAlong(moving, [...stillAreas, PAGE], DOWN);
    const guides = [across && { axis: "x", at: across.at }, down && { axis: "y", at: down.at }].filter(Boolean);
    return { dx: across ? across.by : 0, dy: down ? down.by : 0, guides };
  }

  // --- Lining up -------------------------------------------------------------------------------

  const EDGES = {
    left: { along: ACROSS, share: 0 },
    centre: { along: ACROSS, share: 0.5 },
    right: { along: ACROSS, share: 1 },
    top: { along: DOWN, share: 0 },
    middle: { along: DOWN, share: 0.5 },
    bottom: { along: DOWN, share: 1 },
  };

  // Each area moved so that the named edge of all of them is on one line: the same edge of the space they take up together.
  function linedUp(areas, edgeName) {
    const { along, share } = EDGES[edgeName];
    const together = around(areas);
    const line = together[along.start] + together[along.size] * share;
    return areas.map((area) => keptOnPage({ ...area, [along.start]: line - area[along.size] * share }));
  }

  // Each area moved so their middles are evenly spaced between the first and the last, which stay where they are.
  function spreadOut(areas, direction) {
    const along = direction === "across" ? ACROSS : DOWN;
    const middleOf = (area) => area[along.start] + area[along.size] / 2;
    const inOrder = areas.map((area, position) => ({ area, position })).sort((one, other) => middleOf(one.area) - middleOf(other.area));
    const first = middleOf(inOrder[0].area);
    const step = (middleOf(inOrder[inOrder.length - 1].area) - first) / Math.max(1, inOrder.length - 1);
    const spread = areas.slice();
    inOrder.forEach(({ area, position }, place) => {
      spread[position] = keptOnPage({ ...area, [along.start]: first + step * place - area[along.size] / 2 });
    });
    return spread;
  }

  // --- Resizing --------------------------------------------------------------------------------

  // The eight places an area can be held to resize it, by which way each one pulls: across, then down.
  const PULLS = { nw: [-1, -1], n: [0, -1], ne: [1, -1], e: [1, 0], se: [1, 1], s: [0, 1], sw: [-1, 1], w: [-1, 0] };
  const CORNERS = ["nw", "ne", "se", "sw"];
  const SMALLEST_SIDE = 8;

  // --- Tilting ---------------------------------------------------------------------------------

  // How far above its top edge the handle that tilts an area stands, in the MPC's pixels.
  const TILT_HANDLE_STANDS_OFF = 34;
  const TILT_HANDLE = "tilt";
  const HALF_TURN = 180;
  const TILT_CATCHES_EVERY = 15;

  function middleOf(area) {
    return { x: area.x + area.width / 2, y: area.y + area.height / 2 };
  }

  // A point turned about another by so many degrees, clockwise as the page is seen.
  function turnedAbout(point, centre, degrees) {
    const angle = (degrees * Math.PI) / HALF_TURN;
    const [dx, dy] = [point.x - centre.x, point.y - centre.y];
    return { ...point, x: centre.x + dx * Math.cos(angle) - dy * Math.sin(angle), y: centre.y + dx * Math.sin(angle) + dy * Math.cos(angle) };
  }

  // Where a point on a tilted area would be were the area upright. Tilting is always about the area's middle.
  function uprightPoint(point, area, tilt) {
    return tilt ? turnedAbout(point, middleOf(area), -tilt) : point;
  }

  // The tilt that points an area's top at a point, in whole degrees from -179 to 180. Asked to, it catches on every 15.
  function tiltToward(area, point, catches) {
    const middle = middleOf(area);
    const degrees = (Math.atan2(point.x - middle.x, middle.y - point.y) * HALF_TURN) / Math.PI;
    const step = catches ? TILT_CATCHES_EVERY : 1;
    const caught = Math.round(degrees / step) * step;
    return caught <= -HALF_TURN ? HALF_TURN : caught;
  }

  // Where each handle of an area sits. Something that keeps its shape is held only by its corners.
  // A tilted area's handles are tilted with it.
  function handlesOf(area, keepsShape, tilt = 0) {
    const names = keepsShape ? CORNERS : Object.keys(PULLS);
    return names.map((name) => {
      const [across, down] = PULLS[name];
      const upright = { name, x: area.x + (area.width * (across + 1)) / 2, y: area.y + (area.height * (down + 1)) / 2 };
      return turnedAbout(upright, middleOf(area), tilt);
    });
  }

  // The handle that tilts an area stands off the middle of its top edge.
  function tiltHandleOf(area, tilt = 0) {
    const upright = { name: TILT_HANDLE, x: area.x + area.width / 2, y: area.y - TILT_HANDLE_STANDS_OFF };
    return turnedAbout(upright, middleOf(area), tilt);
  }

  // An area of this size with the edges its handle does not pull left where they were.
  function heldOpposite(area, [across, down], width, height) {
    return { ...area, x: across < 0 ? area.x + area.width - width : area.x, y: down < 0 ? area.y + area.height - height : area.y, width, height };
  }

  function resizedUpright(area, pull, drag, keepsShape) {
    const wider = area.width + pull[0] * drag.dx;
    const taller = area.height + pull[1] * drag.dy;
    if (!keepsShape) return heldOpposite(area, pull, Math.max(SMALLEST_SIDE, wider), Math.max(SMALLEST_SIDE, taller));
    const smallest = SMALLEST_SIDE / Math.min(area.width, area.height);
    const by = Math.max(smallest, wider / area.width, taller / area.height);
    return heldOpposite(area, pull, area.width * by, area.height * by);
  }

  // The spot on an area that a handle leaves where it is: the corner or edge opposite the one it pulls.
  function spotHeldStill(area, [across, down], tilt) {
    const upright = { x: area.x + (area.width * (1 - across)) / 2, y: area.y + (area.height * (1 - down)) / 2 };
    return turnedAbout(upright, middleOf(area), tilt);
  }

  // The area after one of its handles is dragged. `hold` says which handle, how far, whether the area keeps its
  // shape (it then grows by whichever way was dragged further) and how far it is tilted. A tilted area is
  // resized along its own sides, and the spot opposite the handle stays where it is on the page.
  function resizedBy(area, hold) {
    const pull = PULLS[hold.handle];
    const tilt = hold.tilt || 0;
    const alongItsSides = turnedAbout({ x: hold.drag.dx, y: hold.drag.dy }, { x: 0, y: 0 }, -tilt);
    const resized = resizedUpright(area, pull, { dx: alongItsSides.x, dy: alongItsSides.y }, hold.keepsShape);
    const [was, is] = [spotHeldStill(area, pull, tilt), spotHeldStill(resized, pull, tilt)];
    return { ...resized, x: resized.x + was.x - is.x, y: resized.y + was.y - is.y };
  }

  // The part two areas share, or null when they do not overlap.
  function sharedBy(one, other) {
    const x = Math.max(one.x, other.x);
    const y = Math.max(one.y, other.y);
    const right = Math.min(one.x + one.width, other.x + other.width);
    const bottom = Math.min(one.y + one.height, other.y + other.height);
    return right > x && bottom > y ? { x, y, width: right - x, height: bottom - y } : null;
  }

  window.editorGeometry = {
    NO_SNAP, TILT_HANDLE, pagePoint, holds, touches, boxBetween, around, movedBy, keptOnPage, snapped, linedUp, spreadOut, handlesOf, resizedBy, sharedBy,
    uprightPoint, tiltHandleOf, tiltToward,
  };
})();
