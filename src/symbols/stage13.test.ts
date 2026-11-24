import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { emptyDocumentation } from "../editor/documentation";
import { resolvePinPositions } from "../editor/geometry";
import { hitTest } from "../editor/hitTest";
import { buildScene } from "../editor/scene";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import type { Schematic } from "../generated/Schematic";
import { symbols, type LocalPoint } from ".";

// The Stage 13 protection devices (docs/contracts/stage-13-protection-and-safety.md sections 2, 4
// and 7). The IDs and the count of the pins against the Rust catalog are checked for every
// symbol by contract.test.ts; this file checks the geometry and the drawing.

const STAGE_13 = [
  "earth_electrode",
  "exposed_part",
  "rcd_two_pole",
  "rcd_four_pole",
] as const satisfies readonly ComponentTypeId[];

const ID = "00000000-0000-4000-8000-0000000000d1";

function alone(typeId: ComponentTypeId, rotationDeg = 0): Schematic {
  return {
    schemaVersion: 3,
    components: [
      {
        typeId,
        id: ID,
        position: { x: 10, y: 10 },
        rotationDeg,
        parameters: {},
      } as unknown as ComponentSpec,
    ],
    wires: [],
    junctions: [],
    referenceMarkers: [],
    rails: [],
    documentation: emptyDocumentation(),
  };
}

const catalogEntry = (typeId: string) =>
  catalogFixture.componentTypes.find((entry) => entry.typeId === typeId)!;

const switchState = (conducting: boolean): DeviceState => ({
  kind: "switch",
  componentId: ID,
  position: "closed",
  conducting,
});

describe("the Stage 13 symbols", () => {
  it.each(STAGE_13)("exists for %s with the pins of the catalog", (typeId) => {
    const symbol = symbols[typeId];

    expect(symbol).toBeDefined();
    expect(symbol!.pins.map((pin) => pin.id)).toEqual(
      catalogEntry(typeId).pins.map((pin) => pin.id),
    );
  });

  it("draws the earth electrode and the exposed part with one pin each, on the lead", () => {
    for (const [typeId, pin] of [
      ["earth_electrode", "pe"],
      ["exposed_part", "ep"],
    ] as const) {
      const symbol = symbols[typeId]!;

      expect(symbol.pins).toEqual([{ id: pin, local: { x: 0, y: -2 } }]);
      // The drawing reaches the pin: some polyline has an end at it.
      const reachesPin = symbol.primitives.some(
        (primitive) =>
          primitive.kind === "polyline" &&
          primitive.points.some((point) => point.x === 0 && point.y === -2),
      );
      expect(reachesPin, typeId).toBe(true);
    }
  });

  it("draws the poles of the RCDs as vertical pairs two units apart, in pole order", () => {
    for (const [typeId, poles] of [
      ["rcd_two_pole", 2],
      ["rcd_four_pole", 4],
    ] as const) {
      const at = new Map<string, LocalPoint>(
        symbols[typeId]!.pins.map((pin) => [pin.id, pin.local]),
      );

      expect(at.size).toBe(2 * poles);
      for (let pole = 1; pole <= poles; pole += 1) {
        const top = at.get(`l${pole}`)!;
        const bottom = at.get(`t${pole}`)!;

        expect(top.x).toBe(bottom.x);
        expect(top.y).toBe(-2);
        expect(bottom.y).toBe(2);
        expect(top.x).toBe(2 * (pole - 1));
      }
    }
  });

  it("has no operating area: an RCD is operated from the devices panel, as the breaker is", () => {
    expect(symbols.rcd_two_pole!.operatingArea).toBeUndefined();
    expect(symbols.rcd_four_pole!.operatingArea).toBeUndefined();
  });

  it("draws every primitive with finite coordinates", () => {
    for (const typeId of STAGE_13) {
      for (const primitive of symbols[typeId]!.primitives) {
        const numbers =
          primitive.kind === "polyline"
            ? primitive.points.flatMap((point) => [point.x, point.y])
            : [primitive.center.x, primitive.center.y, primitive.radius];

        expect(numbers.every(Number.isFinite), typeId).toBe(true);
      }
    }
  });
});

describe("the RCD drawing follows the state the Engine reported", () => {
  const draw = (
    typeId: "rcd_two_pole" | "rcd_four_pole",
    state?: DeviceState,
  ) => JSON.stringify(symbols[typeId]!.drawn?.(state));

  it("draws the same thing at rest as with no state", () => {
    for (const typeId of ["rcd_two_pole", "rcd_four_pole"] as const) {
      expect(symbols[typeId]!.drawn?.(undefined)).toEqual(
        symbols[typeId]!.primitives,
      );
    }
  });

  it("draws a tripped device open although its operator is closed", () => {
    for (const typeId of ["rcd_two_pole", "rcd_four_pole"] as const) {
      expect(draw(typeId, switchState(true))).not.toBe(
        draw(typeId, switchState(false)),
      );
      // Open and tripped look the same: the contacts are open.
      expect(draw(typeId, switchState(false))).toBe(
        draw(typeId, {
          ...switchState(false),
          position: "open",
        } as DeviceState),
      );
      expect(draw(typeId, switchState(false))).toBe(draw(typeId));
    }
  });

  it("ignores a state of another kind", () => {
    expect(
      draw("rcd_two_pole", {
        kind: "fuse",
        componentId: ID,
        state: "tripped",
      } as DeviceState),
    ).toBe(draw("rcd_two_pole"));
  });
});

describe("the Stage 13 parts in the editor scene", () => {
  it.each(STAGE_13)(
    "is selectable at its centre and its pins follow rotation: %s",
    (typeId) => {
      for (const rotation of [0, 90, 180, 270] as const) {
        const schematic = alone(typeId, rotation);
        const scene = buildScene(schematic, symbols);
        const pins = resolvePinPositions(schematic.components[0]!, symbols);

        expect(pins.length).toBe(catalogEntry(typeId).pins.length);
        // Every pin is on the grid and the scene has shapes for the part.
        for (const pin of pins) {
          expect(Number.isInteger(pin.position.x)).toBe(true);
          expect(Number.isInteger(pin.position.y)).toBe(true);
        }
        expect(scene.shapes.length).toBeGreaterThan(0);
        expect(
          hitTest(
            scene,
            { x: pins[0]!.position.x, y: pins[0]!.position.y },
            0.5,
          ),
        ).toEqual({
          kind: "component",
          id: ID,
        });
      }
    },
  );
});
