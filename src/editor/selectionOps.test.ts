import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { commandForIntent, dragPreviewSchematic } from "./commands";
import { resolvePinPositions } from "./geometry";
import { moveSelection } from "./selectionOps";

// Group move and delete (docs/contracts/stage-16-editor-improvements.md section 3) on the
// divider: V1 at (0, 0), R1 at (4, 0), R2 at (8, 0). Wire 4 runs from V1's pin at (-1, 0) round
// the bottom to R1's pin at (3, 0); wire 5 joins R1's pin at (5, 0) and R2's at (7, 0); wire 6
// runs from R2's pin at (9, 0) over the top to V1's pin at (1, 0).

const divider = schematicFixture as Schematic;
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const WIRE_4 = "00000000-0000-4000-8000-000000000004";
const WIRE_5 = "00000000-0000-4000-8000-000000000005";
const WIRE_6 = "00000000-0000-4000-8000-000000000006";
const MARKER = "00000000-0000-4000-8000-000000000007";

const component = (id: string) => ({ kind: "component", id }) as const;
const wire = (id: string) => ({ kind: "wire", id }) as const;

function wirePath(schematic: Schematic, id: string) {
  return schematic.wires.find((item) => item.id === id)!.points;
}

function position(schematic: Schematic, id: string) {
  return schematic.components.find((item) => item.id === id)!.position;
}

describe("moving a selection of components and wires", () => {
  it("moves the parts, translates a selected wire and follows with the others", () => {
    const move = moveSelection(
      divider,
      [component(R1), component(R2), wire(WIRE_5)],
      { x: 0, y: 2 },
      symbols,
    )!;

    expect(position(move.schematic, R1)).toEqual({ x: 4, y: 2 });
    expect(position(move.schematic, R2)).toEqual({ x: 8, y: 2 });
    expect(position(move.schematic, V1)).toEqual({ x: 0, y: 0 });
    // The selected wire between them is translated as it is.
    expect(wirePath(move.schematic, WIRE_5)).toEqual([
      { x: 5, y: 2 },
      { x: 7, y: 2 },
    ]);
    // An unselected wire keeps its end at V1 and follows R1's pin to its new place.
    const first = wirePath(move.schematic, WIRE_4);
    expect(first[0]).toEqual({ x: -1, y: 0 });
    expect(first[first.length - 1]).toEqual({ x: 3, y: 2 });
    const second = wirePath(move.schematic, WIRE_6);
    expect(second[0]).toEqual({ x: 9, y: 2 });
    expect(second[second.length - 1]).toEqual({ x: 1, y: 0 });
  });

  it("keeps every wire end on the pin it was on, so the connections survive", () => {
    const move = moveSelection(
      divider,
      [component(R1), component(R2)],
      { x: 3, y: -2 },
      symbols,
    )!;

    const pins = new Set(
      move.schematic.components.flatMap((part) =>
        resolvePinPositions(part, symbols).map(
          ({ position: p }) => `${p.x},${p.y}`,
        ),
      ),
    );
    for (const item of move.schematic.wires) {
      const ends = [item.points[0]!, item.points[item.points.length - 1]!];
      for (const end of ends) {
        expect(pins.has(`${end.x},${end.y}`), item.id).toBe(true);
      }
    }
  });

  it("re-routes a wire between two moved parts at both of its ends when it is not selected", () => {
    const move = moveSelection(
      divider,
      [component(R1), component(R2)],
      { x: 0, y: 3 },
      symbols,
    )!;

    // Each end follows its pin: the wire is not translated, it is re-routed.
    const path = wirePath(move.schematic, WIRE_5);
    expect(path[0]).toEqual({ x: 5, y: 3 });
    expect(path[path.length - 1]).toEqual({ x: 7, y: 3 });
  });

  it("matches wire ends against the original drawing when the move equals the pin pitch", () => {
    // R1's pin at 5 lands on R2's old pin at 7: the wire between them must still end on the
    // new pins (7 and 9), not be matched a second time by the pin it passed.
    const move = moveSelection(
      divider,
      [component(R1), component(R2)],
      { x: 2, y: 0 },
      symbols,
    )!;

    expect(wirePath(move.schematic, WIRE_5)).toEqual([
      { x: 7, y: 0 },
      { x: 9, y: 0 },
    ]);
    const first = wirePath(move.schematic, WIRE_4);
    expect(first[first.length - 1]).toEqual({ x: 5, y: 0 });
    const second = wirePath(move.schematic, WIRE_6);
    expect(second[0]).toEqual({ x: 11, y: 0 });
  });

  it("changes no wire that is not selected and not attached to a moved object", () => {
    const move = moveSelection(
      divider,
      [component(V1)],
      { x: 0, y: 5 },
      symbols,
    )!;

    expect(wirePath(move.schematic, WIRE_5)).toEqual(wirePath(divider, WIRE_5));
  });

  it("translates a selected wire whole and leaves the parts it joins where they are", () => {
    const move = moveSelection(
      divider,
      [wire(WIRE_5)],
      { x: 0, y: 1 },
      symbols,
    )!;

    expect(wirePath(move.schematic, WIRE_5)).toEqual([
      { x: 5, y: 1 },
      { x: 7, y: 1 },
    ]);
    expect(position(move.schematic, R1)).toEqual({ x: 4, y: 0 });
    expect(move.commands.map(({ kind }) => kind)).toEqual(["editWire"]);
  });

  it("moves the ground marker with the part it sits on", () => {
    const move = moveSelection(
      divider,
      [component(V1), { kind: "referenceMarker", id: MARKER }],
      { x: 2, y: 1 },
      symbols,
    )!;

    expect(move.schematic.referenceMarkers[0]!.position).toEqual({
      x: 3,
      y: 1,
    });
    expect(position(move.schematic, V1)).toEqual({ x: 2, y: 1 });
  });

  it("is nothing for an empty selection or a move of zero", () => {
    expect(moveSelection(divider, [], { x: 1, y: 0 }, symbols)).toBeNull();
    expect(
      moveSelection(divider, [component(R1)], { x: 0, y: 0 }, symbols),
    ).toBeNull();
  });

  it("is deterministic: the same move gives the same commands", () => {
    const targets = [component(R2), component(R1), wire(WIRE_5)];

    expect(moveSelection(divider, targets, { x: 1, y: 1 }, symbols)).toEqual(
      moveSelection(divider, targets, { x: 1, y: 1 }, symbols),
    );
  });
});

describe("moving junctions, notes and rails", () => {
  const withObjects: Schematic = {
    ...divider,
    junctions: [
      { id: "00000000-0000-4000-8000-0000000000a1", position: { x: 6, y: 0 } },
    ],
    rails: [
      {
        id: "00000000-0000-4000-8000-0000000000a2",
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

  it("moves a junction and a note by the offset", () => {
    const move = moveSelection(
      withObjects,
      [
        { kind: "junction", id: "00000000-0000-4000-8000-0000000000a1" },
        { kind: "note", id: "00000000-0000-4000-8000-0000000000a4" },
      ],
      { x: 1, y: 2 },
      symbols,
    )!;

    expect(move.schematic.junctions[0]!.position).toEqual({ x: 7, y: 2 });
    expect(move.schematic.documentation.notes[0]!.position).toEqual({
      x: 3,
      y: 10,
    });
  });

  it("moves a rail and drags the end of a wire that is attached to it", () => {
    const move = moveSelection(
      withObjects,
      [{ kind: "rail", id: "00000000-0000-4000-8000-0000000000a2" }],
      { x: 3, y: 1 },
      symbols,
    )!;

    expect(move.schematic.rails[0]).toMatchObject({
      start: { x: 23, y: 1 },
      end: { x: 23, y: 11 },
    });
    const path = wirePath(
      move.schematic,
      "00000000-0000-4000-8000-0000000000a3",
    );
    expect(path[0]).toEqual({ x: 23, y: 5 });
    expect(path[path.length - 1]).toEqual({ x: 12, y: 4 });
  });

  it("refuses a move that would take a rail off the grid", () => {
    const edge: Schematic = {
      ...withObjects,
      rails: [
        {
          id: "00000000-0000-4000-8000-0000000000a2",
          name: "RAIL1",
          start: { x: 2147483647, y: 0 },
          end: { x: 2147483647, y: 10 },
        },
      ],
    };

    expect(
      moveSelection(
        edge,
        [{ kind: "rail", id: "00000000-0000-4000-8000-0000000000a2" }],
        { x: 1, y: 0 },
        symbols,
      ),
    ).toBeNull();
  });
});

describe("the commands a group becomes", () => {
  it("is one batch for a group move, and the preview shows its result", () => {
    const intent = {
      kind: "moveSelection",
      targets: [component(R1), component(R2)],
      delta: { x: 1, y: 0 },
    } as const;
    const command = commandForIntent(intent, divider, symbols);

    expect(command?.kind).toBe("batch");
    expect(
      dragPreviewSchematic(
        divider,
        { kind: "group", targets: intent.targets, delta: intent.delta },
        symbols,
      ),
    ).toEqual(
      moveSelection(divider, intent.targets, intent.delta, symbols)!.schematic,
    );
  });

  it("is one batch for a group delete, skipping what is no longer in the document", () => {
    const command = commandForIntent(
      {
        kind: "deleteSelection",
        targets: [
          component(R1),
          wire(WIRE_5),
          component("00000000-0000-4000-8000-0000000000ff"),
        ],
      },
      divider,
      symbols,
    );

    expect(command).toEqual({
      kind: "batch",
      commands: [
        { kind: "deleteComponent", componentId: R1 },
        { kind: "deleteWire", wireId: WIRE_5 },
      ],
    });
  });

  it("is nothing when nothing in the group exists any more", () => {
    expect(
      commandForIntent(
        {
          kind: "deleteSelection",
          targets: [component("00000000-0000-4000-8000-0000000000ff")],
        },
        divider,
        symbols,
      ),
    ).toBeNull();
  });
});
