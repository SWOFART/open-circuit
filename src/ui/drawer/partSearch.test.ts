import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import { message } from "../messages";
import {
  categoryKey,
  normalizeSearchText,
  searchParts,
  searchTokens,
} from "./partSearch";

const catalog = catalogFixture as ComponentCatalog;
const entries = catalog.componentTypes;
const find = (query: string) =>
  searchParts(entries, query).map((entry) => entry.typeId);

describe("search text normalization", () => {
  it("lower-cases, drops diacritics and reads ё as е", () => {
    expect(normalizeSearchText("RÉSISTOR")).toBe("resistor");
    expect(normalizeSearchText("Ёлка")).toBe("елка");
    expect(normalizeSearchText("ｒｅｓｉｓｔｏｒ")).toBe("resistor");
  });

  it("splits a query into tokens on white space", () => {
    expect(searchTokens("  DC   motor ")).toEqual(["dc", "motor"]);
    expect(searchTokens("   ")).toEqual([]);
  });
});

describe("part search", () => {
  it("returns the whole catalog in catalog order for an empty query", () => {
    const all = entries.map((entry) => entry.typeId);

    expect(find("")).toEqual(all);
    expect(find("   ")).toEqual(all);
  });

  it("matches the display name case-insensitively and ignoring diacritics", () => {
    expect(find("RESISTOR")).toEqual(["resistor"]);
    expect(find("résistor")).toEqual(["resistor"]);
  });

  it("requires every token to match somewhere", () => {
    expect(find("dc motor")).toEqual(["pmdc_motor"]);
    expect(find("motor dc")).toEqual(["pmdc_motor"]);
    expect(find("dc banana")).toEqual([]);
  });

  it("finds parts by keyword, by type ID and by category", () => {
    expect(find("choke")).toEqual(["inductor"]);
    expect(find("bjt")).toEqual(["bjt_npn", "bjt_pnp"]);
    expect(find("protection")).toEqual([
      "fuse",
      "thermal_overload_three_pole",
      "rcd_two_pole",
      "rcd_four_pole",
      "breaker_three_pole",
      "earth_electrode",
    ]);
  });

  it("ranks an exact name first", () => {
    expect(find("fuse")[0]).toBe("fuse");
  });

  it("ranks a name prefix before a later word of a name", () => {
    // "DC voltage source" starts with "dc"; "AC voltage source" only has "voltage".
    expect(find("dc")[0]).toBe("dc_voltage_source");
    const volt = find("volt");
    expect(volt.indexOf("voltmeter")).toBeGreaterThanOrEqual(0);
  });

  it("ranks a name word before a keyword before any other substring", () => {
    // "relay": a word of "SPDT relay" (3) ranks before the contactor, where it is a keyword (4).
    expect(find("relay")).toEqual([
      "relay_spdt",
      "thermal_overload_three_pole",
      "contactor",
    ]);
    // "coil" is a keyword (4) of three parts, which keep the catalog order.
    expect(find("coil")).toEqual(["inductor", "relay_spdt", "contactor"]);
    // "ode" is only a substring (5) of "diode".
    expect(find("ode")).toContain("diode");
  });

  it("breaks ties by catalog order, so the result is deterministic", () => {
    const order = entries.map((entry) => entry.typeId);
    const first = find("coil");

    expect(find("coil")).toEqual(first);
    const positions = first.map((id) => order.indexOf(id));
    expect(positions).toEqual([...positions].sort((a, b) => a - b));
  });

  it("does no fuzzy matching", () => {
    expect(find("resistr")).toEqual([]);
  });
});

describe("the interface messages the catalog metadata points at", () => {
  it("has a text for every keyword key and every category", () => {
    for (const entry of entries) {
      expect(entry.searchKeywordKeys.length).toBeGreaterThan(0);
      for (const key of entry.searchKeywordKeys) {
        expect(message(key), key).not.toBe(key);
      }
    }
    for (const category of catalog.categories) {
      expect(message(categoryKey(category)), category).not.toBe(
        categoryKey(category),
      );
    }
    for (const group of ["favorites", "recent"]) {
      expect(message(`category.${group}`)).not.toBe(`category.${group}`);
    }
  });
});
