import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { symbols } from ".";

// The catalog fixture equals the Rust catalog; crates/app/tests/catalog_contract.rs
// enforces that. This test checks registered UI symbols against the shared pin
// contract while allowing Engine devices that are not yet placeable in the UI.

describe("symbol and catalog contract", () => {
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

  it("draws every symbol with at least one primitive", () => {
    for (const symbol of Object.values(symbols)) {
      expect(symbol.primitives.length).toBeGreaterThan(0);
    }
  });
});
