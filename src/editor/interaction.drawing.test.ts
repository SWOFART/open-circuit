import { describe, expect, it } from "vitest";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import { dividerSchematic } from "../store/fixtures";
import { symbols } from "../symbols";
import { analyzeGeometry } from "./connectivity";
import {
  CanvasInteraction,
  type InteractionChange,
  type InteractionIntent,
} from "./interaction";
import { buildScene } from "./scene";
import { toScreen } from "./view";

const W1 = "00000000-0000-4000-8000-000000000101";
const W2 = "00000000-0000-4000-8000-000000000102";
const J1 = "00000000-0000-4000-8000-0000000001a1";
const M1 = "00000000-0000-4000-8000-0000000001b1";

type Wires = Schematic["wires"];

function drawing(
  wires: Wires,
  extra: Partial<Pick<Schematic, "junctions" | "referenceMarkers">> = {},
): Schematic {
  return {
    ...dividerSchematic,
    wires,
    junctions: [],
    referenceMarkers: [],
    ...extra,
  };
}

function wire(id: string, ...points: [number, number][]): Wires[number] {
  return { id, points: points.map(([x, y]) => ({ x, y })) };
}

function harness(schematic: Schematic) {
  const intents: InteractionIntent[] = [];
  const changes: InteractionChange[] = [];
  const interaction = new CanvasInteraction(
    (change) => changes.push(change),
    (intent) => intents.push(intent),
  );
  interaction.setViewport({ width: 800, height: 600 });
  const geometry = analyzeGeometry(schematic, symbols);
  interaction.setScene(buildScene(schematic, symbols, { geometry }), geometry);

  const pointer = (x: number, y: number, extra = {}) => ({
    ...toScreen({ x, y }, interaction.view),
    button: 0,
    pointerId: 1,
    ...extra,
  });

  return {
    interaction,
    intents,
    changes,
    click(x: number, y: number, extra = {}) {
      interaction.pointerDown(pointer(x, y, extra));
      interaction.pointerUp(pointer(x, y, extra));
    },
    move(x: number, y: number, extra = {}) {
      interaction.pointerMove(pointer(x, y, extra));
    },
    press(x: number, y: number) {
      return interaction.pointerDown(pointer(x, y));
    },
    release(x: number, y: number) {
      interaction.pointerUp(pointer(x, y));
    },
  };
}

function path(...points: [number, number][]): GridPoint[] {
  return points.map(([x, y]) => ({ x, y }));
}

describe("the Wire tool", () => {
  // No wires, so every pin and free point is open ground.
  const open = () => harness(drawing([]));

  it("starts a route on the first click and draws nothing yet", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });

    click(10, 5);

    expect(interaction.status.routing).toBe(true);
    expect(intents).toEqual([]);
    expect(interaction.routeAnchor).toEqual({ x: 10, y: 5 });
  });

  it("shows the route with the leg to the pointer, horizontal-first", () => {
    const { interaction, click, move } = open();
    interaction.setTool({ kind: "wire" });
    click(10, 5);

    move(14, 8);

    expect(interaction.routePreview).toEqual(path([10, 5], [14, 5], [14, 8]));
  });

  it("bends vertical-first while Shift is held", () => {
    const { interaction, click, move } = open();
    interaction.setTool({ kind: "wire" });
    click(10, 5);

    move(14, 8, { shiftKey: true });
    expect(interaction.routePreview).toEqual(path([10, 5], [10, 8], [14, 8]));

    interaction.setVerticalFirst(false);
    expect(interaction.routePreview).toEqual(path([10, 5], [14, 5], [14, 8]));
  });

  it("fixes each bend as it is clicked", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });

    click(10, 5);
    click(14, 8, { shiftKey: true });
    click(18, 12, { shiftKey: false });
    click(18, 12);

    // The first leg went vertical-first (Shift), the second horizontal-first;
    // the straight run along row 8 is one segment.
    expect(intents).toEqual([
      {
        kind: "addWire",
        points: path([10, 5], [10, 8], [18, 8], [18, 12]),
      },
    ]);
  });

  it("ends the route at a pin and keeps the tool for the next wire", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });

    click(5, 0);
    click(7, 0);

    expect(intents).toEqual([
      { kind: "addWire", points: path([5, 0], [7, 0]) },
    ]);
    expect(interaction.status.routing).toBe(false);
    expect(interaction.tool).toEqual({ kind: "wire" });
  });

  it("ends the route on an existing wire at that exact point", () => {
    const { interaction, intents, click } = harness(
      drawing([wire(W1, [5, 0], [7, 0])]),
    );
    interaction.setTool({ kind: "wire" });

    click(6, 3);
    click(6, 0);

    expect(intents).toEqual([
      { kind: "addWire", points: path([6, 3], [6, 0]) },
    ]);
  });

  it("finishes at a free point when the last waypoint is clicked again", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });

    click(10, 5);
    click(12, 5);
    expect(intents).toEqual([]);
    click(12, 5);

    expect(intents).toEqual([
      { kind: "addWire", points: path([10, 5], [12, 5]) },
    ]);
  });

  it("finishes at the snapped pointer on Enter", () => {
    const { interaction, intents, click, move } = open();
    interaction.setTool({ kind: "wire" });
    click(10, 5);

    move(13, 7);
    interaction.finishRoute();

    expect(intents).toEqual([
      { kind: "addWire", points: path([10, 5], [13, 5], [13, 7]) },
    ]);
    expect(interaction.status.routing).toBe(false);
  });

  it("draws nothing when the route never leaves its start", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });

    click(10, 5);
    click(10, 5);

    expect(intents).toEqual([]);
    expect(interaction.status.routing).toBe(false);
  });

  it("does nothing on Enter without a route", () => {
    const { interaction, intents } = open();
    interaction.setTool({ kind: "wire" });

    interaction.finishRoute();

    expect(intents).toEqual([]);
  });

  it("cancels the route on Escape, then leaves the tool on the next Escape", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });
    click(10, 5);
    click(14, 5);

    interaction.escape();
    expect(interaction.status.routing).toBe(false);
    expect(interaction.tool).toEqual({ kind: "wire" });

    interaction.escape();
    expect(interaction.tool).toEqual({ kind: "select" });
    expect(intents).toEqual([]);
  });

  it("cancels the route on Undo instead of undoing the document", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });
    click(10, 5);

    interaction.undo();

    expect(interaction.status.routing).toBe(false);
    expect(intents).toEqual([]);

    interaction.undo();
    expect(intents).toEqual([{ kind: "undo" }]);
  });

  it("drops an unfinished route when the tool changes", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "wire" });
    click(10, 5);

    interaction.setTool({ kind: "junction" });

    expect(interaction.status.routing).toBe(false);
    expect(intents).toEqual([]);
  });

  it("ignores Delete and R while a route is being drawn", () => {
    const { interaction, intents, click } = harness(
      drawing([wire(W1, [5, 0], [7, 0])]),
    );
    interaction.select({ kind: "wire", id: W1 });
    interaction.setTool({ kind: "wire" });
    click(10, 5);

    interaction.deleteSelection();
    interaction.rotate();

    expect(intents).toEqual([]);
  });

  it("does not select anything while drawing", () => {
    const { interaction, click } = harness(drawing([wire(W1, [5, 0], [7, 0])]));
    interaction.select({ kind: "wire", id: W1 });

    interaction.setTool({ kind: "wire" });
    click(6, 0);

    expect(interaction.selection).toBeNull();
  });

  it("still pans with the middle button in the middle of a route", () => {
    const { interaction, click } = open();
    interaction.setTool({ kind: "wire" });
    click(10, 5);
    const before = interaction.view;

    interaction.pointerDown({ x: 10, y: 10, button: 1, pointerId: 2 });
    interaction.pointerMove({ x: 40, y: 10, button: 1, pointerId: 2 });
    interaction.pointerUp({ x: 40, y: 10, button: 1, pointerId: 2 });

    expect(interaction.view.offsetX).toBe(before.offsetX + 30);
    expect(interaction.status.routing).toBe(true);
    expect(interaction.routeAnchor).toEqual({ x: 10, y: 5 });
  });

  it("tells the host when a route starts and ends", () => {
    const { interaction, changes, click } = open();
    interaction.setTool({ kind: "wire" });
    changes.length = 0;

    click(10, 5);
    expect(changes).toContain("tool");

    changes.length = 0;
    interaction.escape();
    expect(changes).toContain("tool");
  });
});

describe("the Junction tool", () => {
  const crossing = () =>
    harness(drawing([wire(W1, [3, 5], [9, 5]), wire(W2, [6, 2], [6, 8])]));

  it("adds a marker where two wires cross", () => {
    const { interaction, intents, click } = crossing();
    interaction.setTool({ kind: "junction" });

    click(6, 5);

    expect(intents).toEqual([
      { kind: "addJunction", position: { x: 6, y: 5 } },
    ]);
    expect(interaction.status.rejected).toBeNull();
    expect(interaction.tool).toEqual({ kind: "junction" });
  });

  it("changes nothing elsewhere and says that a crossing is required", () => {
    const { interaction, intents, click } = crossing();
    interaction.setTool({ kind: "junction" });

    click(4, 5);

    expect(intents).toEqual([]);
    expect(interaction.status.rejected).toBe("crossing");
  });

  it("clears the refusal when the pointer moves to another point", () => {
    const { interaction, click, move } = crossing();
    interaction.setTool({ kind: "junction" });
    click(4, 5);

    move(4, 5);
    expect(interaction.status.rejected).toBe("crossing");
    move(6, 5);

    expect(interaction.status.rejected).toBeNull();
  });

  it("clears the refusal when the tool changes", () => {
    const { interaction, click } = crossing();
    interaction.setTool({ kind: "junction" });
    click(4, 5);

    interaction.setTool({ kind: "junction" });

    expect(interaction.status.rejected).toBeNull();
  });

  it("refuses a crossing that already has a marker", () => {
    const { interaction, intents, click } = harness(
      drawing([wire(W1, [3, 5], [9, 5]), wire(W2, [6, 2], [6, 8])], {
        junctions: [{ id: J1, position: { x: 6, y: 5 } }],
      }),
    );
    interaction.setTool({ kind: "junction" });

    click(6, 5);

    expect(intents).toEqual([]);
    expect(interaction.status.rejected).toBe("crossing");
  });
});

describe("the Reference tool", () => {
  const wired = () => harness(drawing([wire(W1, [5, 0], [7, 0])]));

  it.each([
    [5, 0, "a pin"],
    [6, 0, "the inside of a wire"],
    [3, 0, "a pin with no wire"],
  ])("adds a marker on %i,%i: %s", (x, y) => {
    const { interaction, intents, click } = wired();
    interaction.setTool({ kind: "reference" });

    click(x, y);

    expect(intents).toEqual([
      { kind: "addReferenceMarker", position: { x, y } },
    ]);
  });

  it("refuses a point with neither a pin nor a wire", () => {
    const { interaction, intents, click } = wired();
    interaction.setTool({ kind: "reference" });

    click(6, 3);

    expect(intents).toEqual([]);
    expect(interaction.status.rejected).toBe("attachment");
  });

  it("refuses a second marker at the same point", () => {
    const { interaction, intents, click } = harness(
      drawing([wire(W1, [5, 0], [7, 0])], {
        referenceMarkers: [{ id: M1, position: { x: 6, y: 0 } }],
      }),
    );
    interaction.setTool({ kind: "reference" });

    click(6, 0);

    expect(intents).toEqual([]);
    expect(interaction.status.rejected).toBe("attachment");
  });
});

describe("editing a wire's vertices", () => {
  const lShaped = () => harness(drawing([wire(W1, [0, 5], [4, 5], [4, 8])]));

  it("offers handles only once the wire is selected", () => {
    const { interaction, press, release } = lShaped();

    // Pressing the vertex of an unselected wire selects it and drags nothing.
    expect(press(4, 5)).toBe(false);
    release(4, 5);

    expect(interaction.selection).toEqual({ kind: "wire", id: W1 });
    expect(interaction.dragPreview).toBeNull();
  });

  it("commits one edit that keeps both neighbours orthogonal", () => {
    const { interaction, intents, click, press, move, release } = lShaped();
    click(2, 5);
    expect(interaction.selection).toEqual({ kind: "wire", id: W1 });

    expect(press(4, 5)).toBe(true);
    move(6, 6);

    expect(interaction.dragPreview).toEqual({
      kind: "vertex",
      wireId: W1,
      index: 1,
      to: { x: 6, y: 6 },
    });
    expect(intents).toEqual([]);

    release(6, 6);

    expect(intents).toEqual([
      {
        kind: "editWire",
        wireId: W1,
        points: path([0, 5], [6, 5], [6, 8], [4, 8]),
      },
    ]);
    expect(interaction.dragPreview).toBeNull();
  });

  it("can drag an end of the wire", () => {
    const { intents, click, press, move, release } = lShaped();
    click(2, 5);

    press(4, 8);
    move(4, 10);
    release(4, 10);

    expect(intents).toEqual([
      {
        kind: "editWire",
        wireId: W1,
        points: path([0, 5], [4, 5], [4, 10]),
      },
    ]);
  });

  it("cancels on Escape without committing", () => {
    const { interaction, intents, click, press, move, release } = lShaped();
    click(2, 5);
    press(4, 5);
    move(6, 6);

    interaction.escape();
    release(6, 6);

    expect(intents).toEqual([]);
    expect(interaction.dragPreview).toBeNull();
  });

  it("commits nothing when released back where it started", () => {
    const { intents, click, press, move, release } = lShaped();
    click(2, 5);

    press(4, 5);
    move(6, 6);
    move(4, 5);
    release(4, 5);

    expect(intents).toEqual([]);
  });

  it("refuses a move that would collapse the wire", () => {
    const { intents, click, press, move, release } = harness(
      drawing([wire(W1, [0, 5], [4, 5])]),
    );
    click(2, 5);

    press(4, 5);
    move(0, 5);
    release(0, 5);

    expect(intents).toEqual([]);
  });
});

describe("dragging markers", () => {
  const withJunction = () =>
    harness(
      drawing(
        [
          wire(W1, [3, 5], [9, 5]),
          wire(W2, [6, 2], [6, 8]),
          wire("00000000-0000-4000-8000-000000000103", [3, 8], [9, 8]),
          wire("00000000-0000-4000-8000-000000000104", [8, 2], [8, 11]),
        ],
        { junctions: [{ id: J1, position: { x: 6, y: 5 } }] },
      ),
    );

  it("selects a junction and moves it to another crossing in one edit", () => {
    const { interaction, intents, press, move, release } = withJunction();

    expect(press(6, 5)).toBe(true);
    expect(interaction.selection).toEqual({ kind: "junction", id: J1 });
    move(8, 5);

    expect(interaction.dragPreview).toEqual({
      kind: "marker",
      marker: "junction",
      id: J1,
      to: { x: 8, y: 5 },
      valid: true,
    });

    release(8, 5);

    expect(intents).toEqual([
      { kind: "moveJunction", id: J1, position: { x: 8, y: 5 } },
    ]);
  });

  it("cancels a junction released away from a crossing", () => {
    const { interaction, intents, press, move, release } = withJunction();

    press(6, 5);
    move(7, 6);
    expect(interaction.dragPreview).toMatchObject({ valid: false });
    release(7, 6);

    expect(intents).toEqual([]);
    expect(interaction.selection).toEqual({ kind: "junction", id: J1 });
  });

  it("does not move a marker that was only clicked", () => {
    const { intents, press, release } = withJunction();

    press(6, 5);
    release(6, 5);

    expect(intents).toEqual([]);
  });

  it("moves a reference marker to a pin or a wire", () => {
    const { interaction, intents, press, move, release } = harness(
      drawing([wire(W1, [5, 0], [7, 0])], {
        referenceMarkers: [{ id: M1, position: { x: 6, y: 0 } }],
      }),
    );

    expect(press(6, 0)).toBe(true);
    expect(interaction.selection).toEqual({ kind: "referenceMarker", id: M1 });
    move(7, 0);
    release(7, 0);

    expect(intents).toEqual([
      { kind: "moveReferenceMarker", id: M1, position: { x: 7, y: 0 } },
    ]);
  });

  it("cancels a reference marker released away from a pin or wire", () => {
    const { interaction, intents, press, move, release } = harness(
      drawing([wire(W1, [5, 0], [7, 0])], {
        referenceMarkers: [{ id: M1, position: { x: 6, y: 0 } }],
      }),
    );

    press(6, 0);
    move(6, 4);
    expect(interaction.dragPreview).toMatchObject({
      kind: "marker",
      marker: "referenceMarker",
      valid: false,
    });
    release(6, 4);

    expect(intents).toEqual([]);
  });
});

describe("deleting drawn objects", () => {
  it("removes exactly the selected wire, junction, or marker", () => {
    const { interaction, intents } = harness(
      drawing([wire(W1, [5, 0], [7, 0])], {
        junctions: [{ id: J1, position: { x: 6, y: 0 } }],
        referenceMarkers: [{ id: M1, position: { x: 5, y: 0 } }],
      }),
    );

    for (const target of [
      { kind: "wire", id: W1 },
      { kind: "junction", id: J1 },
      { kind: "referenceMarker", id: M1 },
    ] as const) {
      interaction.select(target);
      interaction.deleteSelection();
    }

    expect(intents).toEqual([
      { kind: "delete", target: { kind: "wire", id: W1 } },
      { kind: "delete", target: { kind: "junction", id: J1 } },
      { kind: "delete", target: { kind: "referenceMarker", id: M1 } },
    ]);
  });
});
