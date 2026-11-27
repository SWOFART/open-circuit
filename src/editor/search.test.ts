import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { MAX_SEARCH_RESULTS, searchDocument, searchKeyAction } from "./search";

// Canvas search (docs/contracts/stage-16-editor-improvements.md section 7) on the divider: V1 at
// (0, 0), R1 at (4, 0), R2 at (8, 0).

const divider = schematicFixture as Schematic;
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const NOTE = "00000000-0000-4000-8000-0000000000a4";
const RAIL = "00000000-0000-4000-8000-0000000000a2";

const labels = new Map([
  [V1, "V1"],
  [R1, "R1"],
  [R2, "R2"],
]);
const typeNames: Record<string, string> = {
  dc_voltage_source: "DC voltage source",
  resistor: "Resistor",
};
const typeName = (typeId: string) => typeNames[typeId] ?? typeId;
const find = (document: Schematic, query: string) =>
  searchDocument(document, labels, typeName, query);

const withExtras: Schematic = {
  ...divider,
  rails: [
    {
      id: RAIL,
      name: "L1",
      start: { x: 20, y: 0 },
      end: { x: 20, y: 10 },
    },
  ],
  documentation: {
    ...divider.documentation,
    designators: [{ componentId: R2, designator: "Load" }],
    notes: [
      {
        id: NOTE,
        position: { x: 2, y: 8 },
        text: "Check the fuse rating\nbefore start",
      },
    ],
  },
};

describe("searching the drawing", () => {
  it("finds a part by its display name, case-insensitively and by substring", () => {
    expect(find(divider, "r1")).toEqual([
      { kind: "component", id: R1, name: "R1", typeName: "Resistor" },
    ]);
    expect(find(divider, "  V ")).toHaveLength(1);
  });

  it("finds a part by its stored designator, and lists it under that name", () => {
    expect(find(withExtras, "load")).toEqual([
      { kind: "component", id: R2, name: "Load", typeName: "Resistor" },
    ]);
  });

  it("finds the parts of a type by the catalog name of the type", () => {
    expect(find(divider, "resistor").map(({ id }) => id)).toEqual([R1, R2]);
    expect(find(divider, "voltage").map(({ id }) => id)).toEqual([V1]);
  });

  it("finds a rail by its name and a note by its text", () => {
    expect(find(withExtras, "L1")).toEqual([
      { kind: "rail", id: RAIL, name: "L1" },
    ]);
    expect(find(withExtras, "FUSE")).toEqual([
      { kind: "note", id: NOTE, name: "Check the fuse rating" },
    ]);
    // A note is found by any of its lines.
    expect(find(withExtras, "before start")).toHaveLength(1);
  });

  it("lists parts, then rails, then notes", () => {
    const kinds = find(withExtras, "r").map(({ kind }) => kind);

    expect(kinds).toEqual(
      [...kinds].sort(
        (a, b) =>
          ["component", "rail", "note"].indexOf(a) -
          ["component", "rail", "note"].indexOf(b),
      ),
    );
    expect(kinds).toContain("note");
  });

  it("finds nothing for an empty query or one that matches nothing", () => {
    expect(find(withExtras, "")).toEqual([]);
    expect(find(withExtras, "   ")).toEqual([]);
    expect(find(withExtras, "zzz")).toEqual([]);
  });

  it("shortens the name of a long note", () => {
    const long: Schematic = {
      ...withExtras,
      documentation: {
        ...withExtras.documentation,
        notes: [{ id: NOTE, position: { x: 0, y: 0 }, text: "x".repeat(200) }],
      },
    };

    const [result] = find(long, "xx");
    expect(result!.name.length).toBeLessThanOrEqual(40);
    expect(result!.name.endsWith("…")).toBe(true);
  });

  it("caps a long list", () => {
    const many: Schematic = {
      ...divider,
      documentation: {
        ...divider.documentation,
        notes: Array.from({ length: MAX_SEARCH_RESULTS + 20 }, (_, index) => ({
          id: `00000000-0000-4000-8000-${(0xe000 + index).toString(16).padStart(12, "0")}`,
          position: { x: 0, y: 0 },
          text: `note ${index}`,
        })),
      },
    };

    expect(find(many, "note")).toHaveLength(MAX_SEARCH_RESULTS);
  });

  it("changes nothing", () => {
    const before = JSON.stringify(withExtras);

    find(withExtras, "r");

    expect(JSON.stringify(withExtras)).toBe(before);
  });
});

describe("the keys of the search field", () => {
  it("moves through the results and wraps round", () => {
    expect(searchKeyAction({ key: "ArrowDown" }, 0, 3)).toEqual({
      action: "move",
      current: 1,
    });
    expect(searchKeyAction({ key: "ArrowDown" }, 2, 3)).toEqual({
      action: "move",
      current: 0,
    });
    expect(searchKeyAction({ key: "ArrowUp" }, 0, 3)).toEqual({
      action: "move",
      current: 2,
    });
  });

  it("chooses with Enter only when there is a result, and closes with Escape always", () => {
    expect(searchKeyAction({ key: "Enter" }, 1, 3)).toEqual({
      action: "choose",
    });
    expect(searchKeyAction({ key: "Enter" }, 0, 0)).toBeNull();
    expect(searchKeyAction({ key: "Escape" }, 0, 0)).toEqual({
      action: "close",
    });
  });

  it("ignores other keys and a move with nothing to move through", () => {
    expect(searchKeyAction({ key: "a" }, 0, 3)).toBeNull();
    expect(searchKeyAction({ key: "ArrowDown" }, 0, 0)).toBeNull();
  });
});
