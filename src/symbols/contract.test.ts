import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import { symbols } from ".";

// The catalog fixture equals the Rust catalog; crates/app/tests/catalog_contract.rs
// enforces that. Together the two tests guarantee that every device type in Rust
// has a symbol with the same pins.

describe("symbol and catalog contract", () => {
  it("has a symbol for every catalog device type and no others", () => {
    const catalogTypes = catalogFixture.componentTypes
      .map((entry) => entry.typeId)
      .sort();

    expect(Object.keys(symbols).sort()).toEqual(catalogTypes);
  });

  it.each(catalogFixture.componentTypes)(
    "gives $typeId the same pin IDs and pin count as the catalog",
    (entry) => {
      const symbol = symbols[entry.typeId as ComponentTypeId];

      expect(symbol.typeId).toBe(entry.typeId);
      expect(symbol.pins).toHaveLength(entry.pins.length);
      expect(symbol.pins.map((pin) => pin.id).sort()).toEqual(
        entry.pins.map((pin) => pin.id).sort(),
      );
    },
  );

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

  it("draws every symbol with at least one primitive", () => {
    for (const symbol of Object.values(symbols)) {
      expect(symbol.primitives.length).toBeGreaterThan(0);
    }
  });
});
