import { describe, expect, it } from "vitest";
import fixture from "../../tests/golden/stage-2-connectivity.json";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import { dividerSchematic } from "../store/fixtures";
import { symbols } from "../symbols";
import { analyzeGeometry, EMPTY_GEOMETRY } from "./connectivity";

type Case = (typeof fixture.cases)[number];

function schematicFor(drawing: {
  wires: unknown;
  junctions: unknown;
  referenceMarkers: unknown;
}): Schematic {
  return {
    ...dividerSchematic,
    wires: drawing.wires,
    junctions: drawing.junctions,
    referenceMarkers: drawing.referenceMarkers,
  } as Schematic;
}

function byPosition(points: readonly GridPoint[]): GridPoint[] {
  return [...points].sort((a, b) => a.x - b.x || a.y - b.y);
}

function wire(id: string, ...points: [number, number][]) {
  return { id, points: points.map(([x, y]) => ({ x, y })) };
}

const W1 = "00000000-0000-4000-8000-000000000101";
const W2 = "00000000-0000-4000-8000-000000000102";
const W3 = "00000000-0000-4000-8000-000000000103";
const J1 = "00000000-0000-4000-8000-0000000001a1";
const M1 = "00000000-0000-4000-8000-0000000001b1";

describe.each(fixture.cases as Case[])(
  "editor geometry for the drawing $name",
  (drawing) => {
    const geometry = analyzeGeometry(schematicFor(drawing), symbols);
    const expected = drawing.expected.editor;

    it("finds the junction spots the fixture lists", () => {
      expect(byPosition(geometry.junctionSpots)).toEqual(
        byPosition(expected.junctionSpots),
      );
    });

    it("draws the connection dots the fixture lists", () => {
      expect(byPosition(geometry.connectionDots)).toEqual(
        byPosition(expected.connectionDots),
      );
    });

    it("flags the detached markers the fixture lists", () => {
      expect([...geometry.detachedMarkers].sort()).toEqual(
        [...expected.detachedMarkers].sort(),
      );
    });
  },
);

describe("attachment points", () => {
  const geometry = analyzeGeometry(
    schematicFor({
      wires: [wire(W1, [5, 0], [7, 0], [7, 3])],
      junctions: [],
      referenceMarkers: [],
    }),
    symbols,
  );

  it.each([
    [{ x: 5, y: 0 }, "a wire end that is also a pin"],
    [{ x: 6, y: 0 }, "the inside of a segment"],
    [{ x: 7, y: 2 }, "the inside of a later segment"],
    [{ x: 7, y: 3 }, "the last point of a wire"],
    [{ x: 3, y: 0 }, "a pin with no wire"],
  ])("accepts %o: %s", (point) => {
    expect(geometry.isAttachment(point)).toBe(true);
  });

  it.each([
    { x: 6, y: 1 },
    { x: 8, y: 0 },
    { x: 7, y: 4 },
    { x: 20, y: 20 },
  ])("rejects %o, where nothing is", (point) => {
    expect(geometry.isAttachment(point)).toBe(false);
  });

  it("knows nothing before a schematic is analyzed", () => {
    expect(EMPTY_GEOMETRY.isAttachment({ x: 0, y: 0 })).toBe(false);
    expect(EMPTY_GEOMETRY.isJunctionSpot({ x: 0, y: 0 })).toBe(false);
    expect(EMPTY_GEOMETRY.junctionSpots).toEqual([]);
  });
});

describe("junction spots", () => {
  const cross = [wire(W1, [0, 5], [10, 5]), wire(W2, [5, 1], [5, 9])];

  function spots(extra: Partial<Schematic> = {}): GridPoint[] {
    return [
      ...analyzeGeometry(
        schematicFor({
          wires: cross,
          junctions: [],
          referenceMarkers: [],
          ...extra,
        }),
        symbols,
      ).junctionSpots,
    ];
  }

  it("is a crossing of two different wires", () => {
    expect(spots()).toEqual([{ x: 5, y: 5 }]);
  });

  it("is not offered where a marker already is", () => {
    expect(
      spots({ junctions: [{ id: J1, position: { x: 5, y: 5 } }] }),
    ).toEqual([]);
    expect(
      spots({ referenceMarkers: [{ id: M1, position: { x: 5, y: 5 } }] }),
    ).toEqual([]);
  });

  it("is not offered where a third wire ends, which connects them already", () => {
    expect(
      spots({
        wires: [...cross, wire(W3, [5, 5], [8, 5], [8, 7])],
      }),
    ).toEqual([]);
  });

  it("is not offered where a wire bends or ends on the crossing", () => {
    // A T: the vertical wire ends on the horizontal one.
    expect(
      spots({ wires: [wire(W1, [0, 5], [10, 5]), wire(W2, [5, 5], [5, 9])] }),
    ).toEqual([]);
  });

  it("is not offered for one wire that crosses itself", () => {
    expect(
      spots({
        wires: [wire(W1, [0, 5], [10, 5], [10, 9], [5, 9], [5, 1])],
      }),
    ).toEqual([]);
  });

  it("is not offered for parallel or merely touching wires", () => {
    expect(
      spots({
        wires: [wire(W1, [0, 5], [10, 5]), wire(W2, [3, 5], [12, 5])],
      }),
    ).toEqual([]);
  });

  it("finds every crossing of a grid of wires", () => {
    expect(
      byPosition(
        spots({
          wires: [
            wire(W1, [0, 2], [10, 2]),
            wire(W2, [0, 6], [10, 6]),
            wire(W3, [4, 0], [4, 8]),
          ],
        }),
      ),
    ).toEqual([
      { x: 4, y: 2 },
      { x: 4, y: 6 },
    ]);
  });
});

describe("markers", () => {
  it("finds a reference marker at its exact point", () => {
    const geometry = analyzeGeometry(
      schematicFor({
        wires: [wire(W1, [5, 0], [7, 0])],
        junctions: [],
        referenceMarkers: [{ id: M1, position: { x: 6, y: 0 } }],
      }),
      symbols,
    );

    expect(geometry.hasReferenceMarkerAt({ x: 6, y: 0 })).toBe(true);
    expect(geometry.hasReferenceMarkerAt({ x: 5, y: 0 })).toBe(false);
    expect(geometry.referenceMarkers.get(M1)).toEqual({ x: 6, y: 0 });
  });

  it("lists wires and junctions by ID for editing", () => {
    const geometry = analyzeGeometry(
      schematicFor({
        wires: [wire(W1, [5, 0], [7, 0])],
        junctions: [{ id: J1, position: { x: 6, y: 0 } }],
        referenceMarkers: [],
      }),
      symbols,
    );

    expect(geometry.wires.get(W1)).toEqual([
      { x: 5, y: 0 },
      { x: 7, y: 0 },
    ]);
    expect(geometry.junctions.get(J1)).toEqual({ x: 6, y: 0 });
  });
});
