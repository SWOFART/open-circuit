import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { componentLabels } from "../store/fixtures";
import { symbols } from "../symbols";
import { hitTest, sceneContains } from "./hitTest";
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
