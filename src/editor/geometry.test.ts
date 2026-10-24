import { describe, expect, it } from "vitest";
import geometryFixture from "../../tests/golden/stage-1-divider.geometry.json";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { Schematic } from "../generated/Schematic";
import { symbols, type SymbolRegistry } from "../symbols";
import {
  buildGeometryEnvelope,
  resolvePinPositions,
  rotateLocal,
} from "./geometry";

const resistor: ComponentSpec = {
  typeId: "resistor",
  id: "00000000-0000-4000-8000-0000000000f1",
  position: { x: 10, y: 10 },
  rotationDeg: 0,
  parameters: { resistanceOhms: 1000 },
};

describe("pin geometry", () => {
  it("derives exactly the golden pin positions from the symbols", () => {
    const envelope = buildGeometryEnvelope(
      schematicFixture as Schematic,
      symbols,
    );

    expect(envelope).toEqual(geometryFixture);
  });

  it("rotates a local vector clockwise in screen coordinates", () => {
    const point = { x: 2, y: 1 };

    expect(rotateLocal(point, 0)).toEqual({ x: 2, y: 1 });
    expect(rotateLocal(point, 90)).toEqual({ x: -1, y: 2 });
    expect(rotateLocal(point, 180)).toEqual({ x: -2, y: -1 });
    expect(rotateLocal(point, 270)).toEqual({ x: 1, y: -2 });
  });

  it("never produces negative zero", () => {
    const rotated = rotateLocal({ x: 1, y: 0 }, 90);

    expect(Object.is(rotated.x, 0)).toBe(true);
    expect(Object.is(rotateLocal({ x: 0, y: 0 }, 180).y, 0)).toBe(true);
  });

  it("rotates and translates the pins of a placed component", () => {
    const pins = resolvePinPositions({ ...resistor, rotationDeg: 90 }, symbols);

    expect(pins).toEqual([
      { pinId: "pin1", position: { x: 10, y: 9 } },
      { pinId: "pin2", position: { x: 10, y: 11 } },
    ]);
  });

  it("returns one position for every catalog pin", () => {
    for (const rotationDeg of [0, 90, 180, 270] as const) {
      const pins = resolvePinPositions({ ...resistor, rotationDeg }, symbols);

      expect(pins.map((pin) => pin.pinId)).toEqual(["pin1", "pin2"]);
    }
  });

  it("rejects a pin that does not land on an integer grid point", () => {
    const offGrid: SymbolRegistry = {
      ...symbols,
      resistor: {
        ...symbols.resistor,
        pins: [{ id: "pin1", local: { x: 0.5, y: 0 } }],
      },
    };

    expect(() => resolvePinPositions(resistor, offGrid)).toThrow(
      /does not land on an integer grid point/,
    );
  });
});
