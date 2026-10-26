import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import { resolvePinPositions } from "../editor/geometry";
import type { ComponentSpec } from "../generated/ComponentSpec";
import { symbols } from ".";

// The catalog fixture equals the Rust catalog; crates/app/tests/catalog_contract.rs
// enforces that. Together the two tests guarantee that every device type in Rust
// has a symbol with the same pins. The registry may lag behind the catalog while
// a device is being added (the palette then hides it), but not at release.

describe("symbol and catalog contract", () => {
  it("has symbols only for device types present in the Engine catalog", () => {
    const catalogTypes = new Set(
      catalogFixture.componentTypes.map((entry) => entry.typeId),
    );
    expect(
      (Object.keys(symbols) as ComponentTypeId[]).every((typeId) =>
        catalogTypes.has(typeId),
      ),
    ).toBe(true);
  });

  it("gives every registered symbol the same pin IDs and count as the catalog", () => {
    for (const [typeId, symbol] of Object.entries(symbols)) {
      if (!symbol) throw new Error(`Symbol ${typeId} is missing.`);
      const entry = catalogFixture.componentTypes.find(
        (candidate) => candidate.typeId === typeId,
      );

      if (!entry) throw new Error(`Catalog entry ${typeId} is missing.`);
      expect(symbol.typeId).toBe(entry.typeId);
      expect(symbol.pins).toHaveLength(entry.pins.length);
      expect(symbol.pins.map((pin) => pin.id).sort()).toEqual(
        entry.pins.map((pin) => pin.id).sort(),
      );
    }
  });

  it("places every pin on an integer grid point and never repeats a pin", () => {
    for (const symbol of Object.values(symbols)) {
      const ids = symbol.pins.map((pin) => pin.id);

      expect(new Set(ids).size).toBe(ids.length);

      for (const pin of symbol.pins) {
        expect(Number.isInteger(pin.local.x)).toBe(true);
        expect(Number.isInteger(pin.local.y)).toBe(true);
      }
    }
  });

  it.each([0, 90, 180, 270] as const)(
    "keeps every pin of every symbol on the grid at %i degrees",
    (rotationDeg) => {
      for (const symbol of Object.values(symbols)) {
        const placed = {
          typeId: symbol.typeId,
          id: "00000000-0000-4000-8000-0000000000aa",
          position: { x: 7, y: -3 },
          rotationDeg,
          parameters: {},
        } as unknown as ComponentSpec;
        const pins = resolvePinPositions(placed, symbols);

        expect(pins.map((pin) => pin.pinId)).toEqual(
          symbol.pins.map((pin) => pin.id),
        );
        for (const pin of pins) {
          expect(Number.isInteger(pin.position.x)).toBe(true);
          expect(Number.isInteger(pin.position.y)).toBe(true);
        }
        expect(
          new Set(pins.map((pin) => `${pin.position.x},${pin.position.y}`))
            .size,
        ).toBe(pins.length);
      }
    },
  );

  it.each(["capacitor", "inductor"] as const)(
    "puts the %s pins a symbol length apart on a row, like the resistor",
    (typeId) => {
      expect(symbols[typeId]?.pins).toEqual(symbols.resistor?.pins);
    },
  );

  it.each(["capacitor", "inductor"] as const)(
    "marks pin1 of the %s so that the sign of its initial value is visible",
    (typeId) => {
      // A small "+" sits above pin1: a horizontal and a vertical stroke whose
      // crossing point lies left of the centre.
      const strokes = (symbols[typeId]?.primitives ?? []).flatMap(
        (primitive) =>
          primitive.kind === "polyline" && primitive.points.length === 2
            ? [primitive.points]
            : [],
      );
      const mark = strokes.filter(([a, b]) =>
        [a, b].every((point) => point.x < -0.5 && point.y < 0),
      );

      expect(mark).toHaveLength(2);
    },
  );

  it("draws every symbol with at least one primitive", () => {
    for (const symbol of Object.values(symbols)) {
      expect(symbol.primitives.length).toBeGreaterThan(0);
    }
  });
});
