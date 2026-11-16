// The editor's tools for arranging what is on the MPC's page: moving, selecting, laying out a control's parts,
// and getting around the view. The tools that make artwork are in editor-art-tools.js.
//
// A tool is told of a press and may hand back a gesture: what to do as the pointer is dragged and when it
// is let go. It works through the `workspace` it is given, so no tool draws anything or keeps anything itself.
// `strip` gives what the tool shows after the tools, in the bar under the title.
// A new tool is one more entry in a list of tools; nothing else has to change.
//
// A press says where the pointer is on the MPC's page (`point`) and in the window (`client`), which item
// is under it (`item`), and whether Shift or Alt is held.

(function () {
  "use strict";

  const { NO_SNAP, TILT_HANDLE, boxBetween, around, movedBy, keptOnPage, snapped, resizedBy, tiltToward } = window.editorGeometry;
  const { stripLabel, stripButton, stripChoice } = window.editorWidgets;

  // A box smaller than this, in the MPC's pixels, was a click and not a drag.
  const SMALLEST_DRAGGED_BOX = 4;
  // A press only becomes a drag once the pointer has travelled this far across the window, so a click never shifts anything.
  const DRAG_STARTS_AFTER = 3;
  const ZOOM_STEP = 1.5;

  // Icons are drawn on a 16 by 16 grid of whole pixels, to match the workbench's pixel look.
  const ICONS = {
    move: "M3 1v12h1v-1h1v-1h1v2h1v2h2v-2H8v-2h4v-1h-1V9h-1V8H9V7H8V6H7V5H6V4H5V3H4V2z",
    selectBox: "M2 2h3v1H3v2H2zM7 2h3v1H7zM12 2h3v3h-1V3h-2zM14 7h1v3h-1zM14 12h1v3h-3v-1h2zM7 14h3v1H7zM2 12h1v2h2v1H2zM2 7h1v3H2z",
    parts: "M1 2h7v7H1zM3 4v3h3V4zM10 3h5v1h-5zM10 6h5v1h-5zM1 11h14v1H1zM1 13h9v1H1z",
    hand: "M7 1h2v6h1V2h2v5h1V4h2v7h-1v2h-1v2H7v-2H6v-1H5v-2H4V9H3V7h2v1h1V3h1z",
    zoom: "M5 1h5v1h2v2h1v5h-1v2h-1v1h1v1h1v1h1v1h-1v1h-1v-1h-1v-1h-1v-1h-1v-1H5v-1H3V9H2V4h1V2h2zM5 3v1H4v5h1v1h5V9h1V4h-1V3z",
  };

  function hasTravelled(press, now) {
    return Math.hypot(now.client.x - press.client.x, now.client.y - press.client.y) >= DRAG_STARTS_AFTER;
  }

  function dragBetween(press, now) {
    return { dx: now.point.x - press.point.x, dy: now.point.y - press.point.y };
  }

  function toggled(selected, item) {
    return selected.includes(item) ? selected.filter((other) => other !== item) : [...selected, item];
  }

  function isAClick(box) {
    return box.width < SMALLEST_DRAGGED_BOX && box.height < SMALLEST_DRAGGED_BOX;
  }

  // --- Resizing whatever is held by its handles --------------------------------------------------

  // What is held grows or shrinks with the handle that was pressed. Something that only resizes in its own
  // shape keeps it, and anything else keeps its shape while Shift is held.
  function resizing(workspace, press, held) {
    return {
      dragged(now) {
        const keepsShape = held.resize === "shape" || now.shift;
        workspace.showHeldAt(resizedBy(held.area, { handle: held.handle, drag: dragBetween(press, now), keepsShape, tilt: held.tilt }));
      },
      released: () => workspace.settleHeld(),
    };
  }

  // What is held turns to point its top at the pointer. With Shift held it catches on every fifteen degrees.
  function tilting(workspace, held) {
    return {
      dragged: (now) => workspace.showHeldTilted(tiltToward(held.area, now.point, now.shift)),
      released: () => workspace.settleHeldTilt(),
    };
  }

  function draggingByHandle(workspace, press, held) {
    return held.handle === TILT_HANDLE ? tilting(workspace, held) : resizing(workspace, press, held);
  }

  // --- Move ------------------------------------------------------------------------------------

  // Artwork may hang off the page, as in any picture editor. Everything the MPC draws itself has to stay on it.
  function whereItMayGo(moving, area) {
    return moving.every((item) => item.mayLeavePage) ? area : keptOnPage(area);
  }

  // The selection follows the pointer as one block, snapping to what is around it.
  // Holding Alt moves freely, without snapping.
  function draggingSelection(workspace, press) {
    const moving = workspace.selected();
    const from = around(moving.map((item) => item.area));
    const stillAreas = workspace.areasOutsideSelection();
    let isDragging = false;
    return {
      dragged(now) {
        isDragging = isDragging || hasTravelled(press, now);
        if (!isDragging) return;
        const drag = dragBetween(press, now);
        const wanted = whereItMayGo(moving, movedBy(from, drag.dx, drag.dy));
        const snap = workspace.snapIsOn() && !now.alt ? snapped(wanted, stillAreas) : NO_SNAP;
        const settled = whereItMayGo(moving, movedBy(wanted, snap.dx, snap.dy));
        workspace.slideSelection(settled.x - from.x, settled.y - from.y);
        workspace.showGuides(snap.guides);
      },
      released() {
        workspace.showGuides([]);
        workspace.settle();
      },
    };
  }

  function snapButton(workspace) {
    const made = stripChoice(workspace.snapIsOn() ? "SNAP: ON" : "SNAP: OFF", workspace.snapIsOn(), workspace.toggleSnap);
    made.title = "When on, things jump into line with what is near them. Hold Alt while dragging to move freely.";
    return made;
  }

  function knobPanelButton(workspace) {
    const made = stripChoice("KNOB PANEL", workspace.knobPanelShows(), workspace.toggleKnobPanel);
    made.title = "Shows the part of the page the MPC covers with its own panel while a hardware knob is turned.";
    return made;
  }

  function arrangeStrip(workspace) {
    return [knobPanelButton(workspace), snapButton(workspace)];
  }

  const move = {
    id: "move",
    name: "Move",
    key: "v",
    icon: ICONS.move,
    hint: "Click to select, drag to move, drag a handle to resize. The handle above a piece of artwork tilts it. Shift-click adds to the selection. Arrow keys nudge.",
    strip: arrangeStrip,
    pressed(workspace, press) {
      const held = workspace.handleAt(press.client);
      if (held) return draggingByHandle(workspace, press, held);
      if (!press.item) return workspace.select([]);
      if (press.shift) return workspace.select(toggled(workspace.selected(), press.item));
      if (!workspace.isSelected(press.item)) workspace.select([press.item]);
      return draggingSelection(workspace, press);
    },
  };

  // --- Select box ------------------------------------------------------------------------------

  const selectBox = {
    id: "select-box",
    name: "Select box",
    key: "m",
    icon: ICONS.selectBox,
    hint: "Drag a box round several things to select them all. Shift adds to the selection.",
    strip: arrangeStrip,
    pressed(workspace, press) {
      return {
        dragged: (now) => workspace.showBox(boxBetween(press.point, now.point)),
        released(now) {
          const box = boxBetween(press.point, now.point);
          const caught = isAClick(box) ? [press.item].filter(Boolean) : workspace.itemsTouching(box);
          const alreadySelected = press.shift ? workspace.selected() : [];
          workspace.showBox(null);
          workspace.select([...new Set([...alreadySelected, ...caught])]);
        },
      };
    },
  };

  // --- Parts -----------------------------------------------------------------------------------

  // One part of a control follows the pointer inside it. Nothing is kept until the pointer is let go.
  function draggingPart(workspace, press, held) {
    let isDragging = false;
    return {
      dragged(now) {
        isDragging = isDragging || hasTravelled(press, now);
        if (!isDragging) return;
        const drag = dragBetween(press, now);
        workspace.showHeldAt(keptOnPage(movedBy(held.area, drag.dx, drag.dy)));
      },
      released: () => workspace.settleHeld(),
    };
  }

  function partsStrip(workspace) {
    const held = workspace.held();
    const words = held && held.part ? `Its ${held.part === "art" ? "image" : held.part} is selected. Its details are on the right.` : "Click a knob, a name or a value to pick up just that part.";
    return [stripLabel(words)];
  }

  const parts = {
    id: "parts",
    name: "Parts",
    key: "a",
    icon: ICONS.parts,
    hint: "Click the knob, the name or the value inside a control to move, resize or restyle just that part.",
    strip: partsStrip,
    pressed(workspace, press) {
      const handle = workspace.handleAt(press.client);
      if (handle) return draggingByHandle(workspace, press, handle);
      const hit = workspace.partAt(press.point);
      workspace.selectPart(hit);
      return hit ? draggingPart(workspace, press, workspace.held()) : undefined;
    },
  };

  // --- Hand ------------------------------------------------------------------------------------

  function viewStrip(workspace) {
    const zoom = workspace.zoom();
    return [
      stripLabel("ZOOM"),
      stripButton("OUT", () => workspace.zoomBy(1 / ZOOM_STEP), zoom.isSmallest),
      stripButton("IN", () => workspace.zoomBy(ZOOM_STEP), zoom.isLargest),
      stripButton("FIT", () => workspace.zoomToFit(), zoom.isSmallest),
    ];
  }

  const hand = {
    id: "hand",
    name: "Hand",
    key: "h",
    icon: ICONS.hand,
    hint: "Drag to slide the view around. Holding Space does the same with any tool.",
    strip: viewStrip,
    pressed(workspace, press) {
      let last = press.client;
      return {
        dragged(now) {
          workspace.panBy(last.x - now.client.x, last.y - now.client.y);
          last = now.client;
        },
        released() {},
      };
    },
  };

  // --- Zoom ------------------------------------------------------------------------------------

  const zoom = {
    id: "zoom",
    name: "Zoom",
    key: "z",
    icon: ICONS.zoom,
    hint: "Click to zoom in on a spot. Alt-click zooms out.",
    strip: viewStrip,
    pressed(workspace, press) {
      workspace.zoomBy(press.alt ? 1 / ZOOM_STEP : ZOOM_STEP, press.client);
    },
  };

  window.editorTools = { ARRANGING_TOOLS: [move, selectBox, parts], VIEW_TOOLS: [hand, zoom], MOVE: move, PARTS: parts, HAND: hand, ZOOM_STEP, isAClick };
})();
