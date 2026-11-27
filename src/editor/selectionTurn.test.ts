import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import { SchematicDocumentStore } from "../store/document";
import { commandVerdict } from "../store/studentAssignment";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { commandForIntent, type EditorCommand } from "./commands";
import { resolvePinPositions } from "./geometry";
import {
  selectionPivot,
  turnSelection,
  type SelectionTurn,
} from "./selectionOps";

// Turns and mirrors of a selection (docs/contracts/stage-16-editor-improvements.md section 5)
// on the divider: V1 at (0, 0), R1 at (4, 0), R2 at (8, 0); wire 4 joins V1's pin at -1 to R1's
// at 3 round the bottom, wire 5 joins R1's pin at 5 to R2's at 7, wire 6 joins R2's pin at 9 to
// V1's at 1 round the top.

const divider = schematicFixture as Schematic;
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const WIRE_4 = "00000000-0000-4000-8000-000000000004";
const WIRE_5 = "00000000-0000-4000-8000-000000000005";
const JUNCTION = "00000000-0000-4000-8000-0000000000a1";
const RAIL = "00000000-0000-4000-8000-0000000000a2";

const component = (id: string) => ({ kind: "component", id }) as const;
const wire = (id: string) => ({ kind: "wire", id }) as const;

function turned(
  schematic: Schematic,
  targets: Parameters<typeof turnSelection>[1],
  turn: SelectionTurn,
) {
  const result = turnSelection(schematic, targets, turn, symbols);
  if (!result || "refusal" in result) {
    throw new Error(`no turn: ${JSON.stringify(result)}`);
  }
  return result;
}

const part = (schematic: Schematic, id: string) =>
  schematic.components.find((item) => item.id === id)!;
const pathOf = (schematic: Schematic, id: string) =>
  schematic.wires.find((item) => item.id === id)!.points;

/** The map of the plane each turn is, about `pivot`. */
function mapOf(turn: SelectionTurn, pivot: GridPoint) {
  return ({ x, y }: GridPoint): GridPoint => {
    switch (turn) {
      case "half":
        return { x: 2 * pivot.x - x, y: 2 * pivot.y - y };
      case "quarter":
        return { x: pivot.x - (y - pivot.y), y: pivot.y + (x - pivot.x) };
      case "mirror":
        return { x: 2 * pivot.x - x, y };
    }
  };
}

describe("the pivot", () => {
  it("is the origin of one selected part", () => {
    expect(selectionPivot(divider, [component(R1)], symbols)).toEqual({
      x: 4,
      y: 0,
    });
  });

  it("is the center of the bounds of several objects, rounded toward positive infinity", () => {
    const odd: Schematic = {
      ...divider,
      junctions: [
        { id: JUNCTION, position: { x: 0, y: 0 } },
        {
          id: "00000000-0000-4000-8000-0000000000a3",
          position: { x: 3, y: 0 },
        },
      ],
    };

    // The bounds run from 0 to 3: the center 1.5 goes up to 2; the y center is 0.
    expect(
      selectionPivot(
        odd,
        [
          { kind: "junction", id: JUNCTION },
          { kind: "junction", id: "00000000-0000-4000-8000-0000000000a3" },
        ],
        symbols,
      ),
    ).toEqual({ x: 2, y: 0 });
  });

  it("is nothing for an empty selection", () => {
    expect(selectionPivot(divider, [], symbols)).toBeNull();
    expect(turnSelection(divider, [], "half", symbols)).toBeNull();
  });
});

describe("every member is carried by the same map", () => {
  const group = [component(R1), component(R2), wire(WIRE_5)];

  it.each(["half", "quarter", "mirror"] as const)(
    "%s: every pin of a carried part goes where the map takes it",
    (turn) => {
      const pivot = selectionPivot(divider, group, symbols)!;
      const map = mapOf(turn, pivot);
      const result = turned(divider, group, turn);

      for (const id of [R1, R2]) {
        const before = resolvePinPositions(part(divider, id), symbols);
        const after = resolvePinPositions(part(result.schematic, id), symbols);
        before.forEach((pin, index) => {
          expect(after[index]!.position).toEqual(map(pin.position));
        });
      }
      // The selected wire is carried point by point.
      expect(pathOf(result.schematic, WIRE_5)).toEqual(
        pathOf(divider, WIRE_5).map(map),
      );
    },
  );

  it.each(["half", "quarter", "mirror"] as const)(
    "%s: keeps every wire end on a pin, so the connections survive",
    (turn) => {
      const result = turned(divider, group, turn);
      const pins = new Set(
        result.schematic.components.flatMap((item) =>
          resolvePinPositions(item, symbols).map(
            ({ position: p }) => `${p.x},${p.y}`,
          ),
        ),
      );

      for (const item of result.schematic.wires) {
        for (const end of [
          item.points[0]!,
          item.points[item.points.length - 1]!,
        ]) {
          expect(pins.has(`${end.x},${end.y}`), `${turn} ${item.id}`).toBe(
            true,
          );
        }
      }
    },
  );

  it("adds the angle to the rotation of each part", () => {
    expect(part(turned(divider, group, "half").schematic, R1).rotationDeg).toBe(
      180,
    );
    expect(
      part(turned(divider, group, "quarter").schematic, R1).rotationDeg,
    ).toBe(90);
  });

  it("re-routes a wire that is not selected and ends on a carried part, and leaves its other end", () => {
    const result = turned(divider, group, "half");
    const path = pathOf(result.schematic, WIRE_4);

    // Wire 4 starts at V1's pin, which stays; it ends on R1's pin, which moved.
    expect(path[0]).toEqual({ x: -1, y: 0 });
    const r1Pins = resolvePinPositions(part(result.schematic, R1), symbols);
    expect(r1Pins.map(({ position }) => position)).toContainEqual(
      path[path.length - 1],
    );
  });

  it("changes nothing for a wire that is not attached to a carried object", () => {
    const result = turned(divider, [component(R2)], "half");

    expect(pathOf(result.schematic, WIRE_4)).toEqual(pathOf(divider, WIRE_4));
  });
});

describe("mirror", () => {
  it("toggles the flag of one part about its own symbol and keeps its place and angle", () => {
    const result = turned(divider, [component(R1)], "mirror");

    const mirrored = part(result.schematic, R1);
    expect(mirrored.mirrored).toBe(true);
    expect(mirrored.position).toEqual({ x: 4, y: 0 });
    expect(mirrored.rotationDeg).toBe(0);
  });

  it("negates the angle of a part that is carried with others, so the group is a true mirror image", () => {
    const rotated: Schematic = {
      ...divider,
      components: divider.components.map((item) =>
        item.id === R1 ? { ...item, rotationDeg: 90 } : item,
      ),
    };
    const pair = [component(R1), component(R2)];
    const pivot = selectionPivot(rotated, pair, symbols)!;
    const result = turned(rotated, pair, "mirror");

    expect(part(result.schematic, R1).rotationDeg).toBe(270);
    expect(part(result.schematic, R1).mirrored).toBe(true);
    // The pins of the vertical part land exactly on the mirror images of the old pins.
    const before = resolvePinPositions(part(rotated, R1), symbols);
    const after = resolvePinPositions(part(result.schematic, R1), symbols);
    before.forEach((pin, index) => {
      expect(after[index]!.position).toEqual({
        x: 2 * pivot.x - pin.position.x,
        y: pin.position.y,
      });
    });
  });

  it("is the identity when done twice, and leaves no flag behind", () => {
    const store = new SchematicDocumentStore(divider);
    const mirrorOnce = () => {
      const result = turned(
        store.getState().schematic,
        [component(R1)],
        "mirror",
      );
      applyEditorCommand(
        store,
        { kind: "batch", commands: result.commands },
        null,
      );
    };

    mirrorOnce();
    expect(part(store.getState().schematic, R1).mirrored).toBe(true);
    mirrorOnce();

    // The part is as it was and carries no flag. (The wires on its pins are re-routed by each
    // mirror and need not come back to the same bends.)
    expect(store.getState().schematic.components).toEqual(divider.components);
    expect(JSON.stringify(store.getState().schematic)).not.toContain(
      "mirrored",
    );
  });

  it("mirrors a group twice back to the original when its center is on the grid", () => {
    // R1 is turned half a turn, which leaves the bounds of the pair symmetric about x = 6, a
    // grid point: the pivot is the same for both mirrors. (A half-integer center is rounded up,
    // so a second mirror would use a pivot one unit further.)
    const store = new SchematicDocumentStore({
      ...divider,
      components: divider.components.map((item) =>
        item.id === R1 ? { ...item, rotationDeg: 180 } : item,
      ),
    });
    const before = store.getState().schematic;
    const pair = [component(R1), component(R2), wire(WIRE_5)];

    for (let count = 0; count < 2; count += 1) {
      const result = turned(store.getState().schematic, pair, "mirror");
      applyEditorCommand(
        store,
        { kind: "batch", commands: result.commands },
        null,
      );
    }

    expect(
      store
        .getState()
        .schematic.components.map((item) => [
          item.id,
          item.position,
          item.rotationDeg,
          item.mirrored,
        ]),
    ).toEqual(
      before.components.map((item) => [
        item.id,
        item.position,
        item.rotationDeg,
        item.mirrored,
      ]),
    );
  });
});

describe("notes, markers, junctions and rails", () => {
  const withObjects: Schematic = {
    ...divider,
    junctions: [{ id: JUNCTION, position: { x: 6, y: 0 } }],
    rails: [
      {
        id: RAIL,
        name: "RAIL1",
        start: { x: 20, y: 0 },
        end: { x: 20, y: 10 },
      },
    ],
    wires: [
      ...divider.wires,
      {
        id: "00000000-0000-4000-8000-0000000000a3",
        points: [
          { x: 20, y: 4 },
          { x: 12, y: 4 },
        ],
      },
    ],
    documentation: {
      ...divider.documentation,
      notes: [
        {
          id: "00000000-0000-4000-8000-0000000000a4",
          position: { x: 2, y: 8 },
          text: "Hello",
        },
      ],
    },
  };

  it("carries a junction, a note and a marker by the map and keeps the note upright", () => {
    const targets = [
      { kind: "junction", id: JUNCTION },
      { kind: "note", id: "00000000-0000-4000-8000-0000000000a4" },
      { kind: "referenceMarker", id: "00000000-0000-4000-8000-000000000007" },
    ] as const;
    const pivot = selectionPivot(withObjects, targets, symbols)!;
    const map = mapOf("half", pivot);
    const result = turned(withObjects, targets, "half");

    expect(result.schematic.junctions[0]!.position).toEqual(
      map({ x: 6, y: 0 }),
    );
    expect(result.schematic.documentation.notes[0]!.position).toEqual(
      map({ x: 2, y: 8 }),
    );
    expect(result.schematic.documentation.notes[0]!.text).toBe("Hello");
    expect(result.schematic.referenceMarkers[0]!.position).toEqual(
      map({ x: 1, y: 0 }),
    );
  });

  it("refuses a quarter turn of a selection that holds a rail, whole", () => {
    const result = turnSelection(
      withObjects,
      [{ kind: "rail", id: RAIL }, component(R1)],
      "quarter",
      symbols,
    );

    expect(result).toEqual({ refusal: "A rail cannot be turned a quarter" });
  });

  it("allows a half turn of a rail, which stays vertical, and drags the wire on it", () => {
    const result = turned(withObjects, [{ kind: "rail", id: RAIL }], "half");

    const rail = result.schematic.rails[0]!;
    expect(rail.start.x).toBe(rail.end.x);
    expect(rail.end.y - rail.start.y).toBe(10);
    // The wire that ended on the rail at (20, 4) ends on it still, at the mapped place.
    const path = pathOf(
      result.schematic,
      "00000000-0000-4000-8000-0000000000a3",
    );
    expect(path[0]!.x).toBe(rail.start.x);
    expect(path[0]!.y).toBe(rail.start.y + 6);
    expect(path[path.length - 1]).toEqual({ x: 12, y: 4 });
  });

  it("mirrors a rail about the vertical axis of the pivot, keeping its extent", () => {
    const result = turned(withObjects, [{ kind: "rail", id: RAIL }], "mirror");

    const rail = result.schematic.rails[0]!;
    expect(rail.start.y).toBe(0);
    expect(rail.end.y).toBe(10);
    expect(rail.start.x).toBe(rail.end.x);
  });
});

describe("the commands a turn becomes", () => {
  it("is one batch, and an assignment lets a locked part be turned", () => {
    const command = commandForIntent(
      {
        kind: "turnSelection",
        targets: [component(R1), component(R2)],
        turn: "half",
      },
      divider,
      symbols,
    );

    expect(command?.kind).toBe("batch");
    expect(
      commandVerdict(command as EditorCommand, {
        allowedTypes: null,
        locked: new Set([R1]),
        findFault: false,
      }),
    ).toEqual({ allowed: true });
  });

  it("is one history entry: one undo restores the document exactly", () => {
    const store = new SchematicDocumentStore(divider);
    const command = commandForIntent(
      {
        kind: "turnSelection",
        targets: [component(R1), component(R2), wire(WIRE_5)],
        turn: "mirror",
      },
      divider,
      symbols,
    )!;

    applyEditorCommand(store, command, null);
    expect(store.getState().schematic).not.toEqual(divider);
    store.undo();

    expect(store.getState().schematic).toEqual(divider);
    expect(store.getState().canUndo).toBe(false);
  });

  it("reports a refusal by throwing it, and changes nothing", () => {
    const store = new SchematicDocumentStore({
      ...divider,
      rails: [
        {
          id: RAIL,
          name: "RAIL1",
          start: { x: 20, y: 0 },
          end: { x: 20, y: 10 },
        },
      ],
    });
    const command = commandForIntent(
      {
        kind: "turnSelection",
        targets: [{ kind: "rail", id: RAIL }],
        turn: "quarter",
      },
      store.getState().schematic,
      symbols,
    )!;

    expect(command.kind).toBe("refuse");
    expect(() => applyEditorCommand(store, command, null)).toThrow(
      "A rail cannot be turned a quarter",
    );
    expect(store.getState().canUndo).toBe(false);
  });

  it("is nothing for an empty selection", () => {
    expect(
      commandForIntent(
        { kind: "turnSelection", targets: [], turn: "half" },
        divider,
        symbols,
      ),
    ).toBeNull();
  });
});

it("keeps V1 as it was when only the resistors turn", () => {
  const result = turned(divider, [component(R1), component(R2)], "half");

  expect(part(result.schematic, V1)).toEqual(part(divider, V1));
});
