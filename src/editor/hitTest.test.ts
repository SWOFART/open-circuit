import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { componentLabels } from "../store/fixtures";
import { symbols } from "../symbols";
import { hitTest, objectsInBox, sceneContains } from "./hitTest";
import { buildScene } from "./scene";

const schematic = schematicFixture as Schematic;
const scene = buildScene(schematic, symbols, {
  labels: componentLabels(schematic),
});

const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const LOOP_WIRE = "00000000-0000-4000-8000-000000000004";
const MIDDLE_WIRE = "00000000-0000-4000-8000-000000000005";
const GROUND = "00000000-0000-4000-8000-000000000007";
const JUNCTION = "00000000-0000-4000-8000-0000000000f2";
const ROTATED = "00000000-0000-4000-8000-0000000000f3";

const TOLERANCE = 0.25;

describe("hit testing the divider", () => {
  it.each([
    { point: { x: 4, y: 0 }, expected: { kind: "component", id: R1 } },
    { point: { x: 4.5, y: 0.2 }, expected: { kind: "component", id: R1 } },
    { point: { x: 8, y: -0.3 }, expected: { kind: "component", id: R2 } },
    { point: { x: -0.5, y: 0.35 }, expected: { kind: "component", id: V1 } },
    { point: { x: -1, y: 2 }, expected: { kind: "wire", id: LOOP_WIRE } },
    { point: { x: 1.2, y: 4.1 }, expected: { kind: "wire", id: LOOP_WIRE } },
    { point: { x: 6, y: 0.05 }, expected: { kind: "wire", id: MIDDLE_WIRE } },
    {
      point: { x: 1, y: 0.8 },
      expected: { kind: "referenceMarker", id: GROUND },
    },
  ])(
    "selects $expected.kind at ($point.x, $point.y)",
    ({ point, expected }) => {
      expect(hitTest(scene, point, TOLERANCE)).toEqual(expected);
    },
  );

  it("finds nothing on empty canvas", () => {
    expect(hitTest(scene, { x: 20, y: 20 }, TOLERANCE)).toBeNull();
    expect(hitTest(scene, { x: 6, y: 2 }, TOLERANCE)).toBeNull();
  });

  it("respects the pick tolerance", () => {
    const nearWire = { x: -1.3, y: 2 };

    expect(hitTest(scene, nearWire, 0.25)).toBeNull();
    expect(hitTest(scene, nearWire, 0.35)).toEqual({
      kind: "wire",
      id: LOOP_WIRE,
    });
  });

  it("prefers a component over the wire ending at its pin", () => {
    expect(hitTest(scene, { x: 5, y: 0 }, TOLERANCE)).toEqual({
      kind: "component",
      id: R1,
    });
  });

  it("prefers a point marker over the components and wires under it", () => {
    // The ground marker sits on V1's pin2, where a wire also ends.
    expect(hitTest(scene, { x: 1, y: 0 }, TOLERANCE)).toEqual({
      kind: "referenceMarker",
      id: GROUND,
    });
  });

  it("selects a component through its label", () => {
    expect(hitTest(scene, { x: 4.6, y: -1.2 }, TOLERANCE)).toEqual({
      kind: "component",
      id: R1,
    });
  });

  it("selects a junction over the wire it sits on", () => {
    const withJunction = buildScene(
      {
        ...schematic,
        junctions: [{ id: JUNCTION, position: { x: 3, y: 4 } }],
      },
      symbols,
    );

    expect(hitTest(withJunction, { x: 3.05, y: 4 }, TOLERANCE)).toEqual({
      kind: "junction",
      id: JUNCTION,
    });
  });

  it("uses the rotated body of a rotated component", () => {
    const rotated = buildScene(
      {
        ...schematic,
        components: [
          {
            typeId: "resistor",
            id: ROTATED,
            position: { x: 20, y: 20 },
            rotationDeg: 90,
            parameters: { resistanceOhms: 1000 },
          },
        ],
      },
      symbols,
    );

    expect(hitTest(rotated, { x: 20, y: 20.9 }, 0.1)).toEqual({
      kind: "component",
      id: ROTATED,
    });
    expect(hitTest(rotated, { x: 20.9, y: 20 }, 0.1)).toBeNull();
  });
});

describe("scene membership", () => {
  it("reports whether a target is still drawn", () => {
    expect(sceneContains(scene, { kind: "component", id: R1 })).toBe(true);
    expect(sceneContains(scene, { kind: "wire", id: R1 })).toBe(false);
    expect(sceneContains(scene, { kind: "junction", id: JUNCTION })).toBe(
      false,
    );
  });
});

describe("selecting by a rectangle", () => {
  const ids = (box: Parameters<typeof objectsInBox>[1]) =>
    objectsInBox(scene, box)
      .map(({ id }) => id)
      .sort();

  it("takes what is entirely inside and nothing it only touches", () => {
    // R1, R2 and the wire between them; the long wires and V1 reach outside.
    expect(ids({ minX: 2.4, minY: -1.6, maxX: 9.6, maxY: 1.6 })).toEqual(
      [R1, R2, MIDDLE_WIRE].sort(),
    );
    expect(ids({ minX: 2.4, minY: -1.6, maxX: 8.2, maxY: 1.6 })).not.toContain(
      R2,
    );
  });

  it("takes a wire only when every point of it is inside", () => {
    // The loop wire runs from x = -1 to x = 3 and from y = 0 to y = 4.
    expect(ids({ minX: -1.5, minY: -0.5, maxX: 2, maxY: 4.5 })).not.toContain(
      LOOP_WIRE,
    );
    expect(ids({ minX: -1.5, minY: -0.5, maxX: 3.5, maxY: 4.5 })).toContain(
      LOOP_WIRE,
    );
  });

  it("is not kept out by a part's name label", () => {
    // The body of V1 and its pins fit in this box; its label sits above it.
    expect(ids({ minX: -1.6, minY: -1.2, maxX: 1.6, maxY: 1.2 })).toContain(V1);
  });

  it("takes everything with an unbounded rectangle", () => {
    expect(
      objectsInBox(scene, {
        minX: -Infinity,
        minY: -Infinity,
        maxX: Infinity,
        maxY: Infinity,
      }).length,
    ).toBeGreaterThanOrEqual(7);
  });
});
