import { describe, expect, it } from "vitest";
import { nextRailName, railNameProblem } from "../editor/railRules";
import type { StableId } from "../generated/StableId";
import { SchematicDocumentStore } from "./document";
import { blankSchematic } from "./projectWorkflow";

function store() {
  let next = 1;
  return new SchematicDocumentStore(
    blankSchematic,
    () => `00000000-0000-4000-8000-${(next++).toString(16).padStart(12, "0")}`,
  );
}

const rails = (document: SchematicDocumentStore) =>
  document.getState().schematic.rails;

describe("rail names", () => {
  it("accept the contract's grammar and nothing else", () => {
    for (const good of ["L+", "0V", "RAIL1", "a", "N", "PE", "L1", "x_y-z+1"]) {
      expect(railNameProblem(good, [])).toBeNull();
    }
    for (const bad of [
      " L+",
      "L+ ",
      "-L",
      "_L",
      "+L",
      "L 1",
      "é",
      "Lé",
      "L.1",
    ]) {
      expect(railNameProblem(bad, [])).toBe("grammar");
    }
    expect(railNameProblem("", [])).toBe("empty");
    expect(railNameProblem("A".repeat(32), [])).toBeNull();
    expect(railNameProblem("A".repeat(33), [])).toBe("grammar");
  });

  it("are unique, compared byte for byte", () => {
    expect(railNameProblem("L1", ["L1"])).toBe("duplicate");
    expect(railNameProblem("l1", ["L1"])).toBeNull();
  });

  it("default to the smallest unused RAIL<n>", () => {
    expect(nextRailName([])).toBe("RAIL1");
    expect(nextRailName(["RAIL1", "RAIL2"])).toBe("RAIL3");
    expect(nextRailName(["RAIL2", "L+"])).toBe("RAIL1");
    expect(nextRailName(["RAIL1", "RAIL3"])).toBe("RAIL2");
  });
});

describe("adding a rail", () => {
  it("records one history entry, with a default name and the ordered ends", () => {
    const document = store();

    const id = document.addRail({ x: 4, top: 0, bottom: 20 });

    expect(rails(document)).toEqual([
      {
        id,
        name: "RAIL1",
        start: { x: 4, y: 0 },
        end: { x: 4, y: 20 },
      },
    ]);
    expect(document.getState().canUndo).toBe(true);
    expect(document.getState().isDirty).toBe(true);
    // The next default moves on.
    document.addRail({ x: 8, top: 0, bottom: 20 });
    expect(rails(document).map((rail) => rail.name)).toEqual([
      "RAIL1",
      "RAIL2",
    ]);
  });

  it("refuses an invalid name, a duplicate, a bad extent and a 257th rail, and records nothing", () => {
    const document = store();
    document.addRail({ name: "L+", x: 0, top: 0, bottom: 10 });
    const before = document.getState();

    for (const rail of [
      { name: " L+", x: 3, top: 0, bottom: 10 },
      { name: "L+", x: 3, top: 0, bottom: 10 },
      { name: "A", x: 3, top: 0, bottom: 1 },
      { name: "A", x: 3, top: 5, bottom: 4 },
      { name: "A", x: 3, top: 0, bottom: 1_000_001 },
      { name: "A", x: 1.5, top: 0, bottom: 10 },
      { name: "A", x: 3, top: 0, bottom: 2 ** 31 },
    ]) {
      expect(() => document.addRail(rail)).toThrow();
    }
    expect(document.getState()).toBe(before);

    const full = store();
    for (let index = 0; index < 256; index += 1) {
      full.addRail({ name: `R${index}`, x: index, top: 0, bottom: 10 });
    }
    expect(() =>
      full.addRail({ name: "ONE_TOO_MANY", x: 999, top: 0, bottom: 10 }),
    ).toThrow(/at most 256/);
  });

  it("accepts the legal extremes", () => {
    const document = store();

    document.addRail({ name: "SHORT", x: 0, top: 0, bottom: 2 });
    document.addRail({ name: "LONG", x: 1, top: -500_000, bottom: 500_000 });
    document.addRail({
      name: "EDGE",
      x: 2,
      top: -2_147_483_648,
      bottom: -2_147_483_648 + 1_000_000,
    });

    expect(rails(document)).toHaveLength(3);
  });
});

describe("editing a rail", () => {
  it("renames, moves and resizes as one history entry each, keeping the ID", () => {
    const document = store();
    const id = document.addRail({ name: "L+", x: 4, top: 0, bottom: 10 });

    document.editRail(id, { name: "SUPPLY" });
    document.editRail(id, { x: 6, top: 2, bottom: 12 });
    document.editRail(id, { bottom: 15 });

    expect(rails(document)).toEqual([
      { id, name: "SUPPLY", start: { x: 6, y: 2 }, end: { x: 6, y: 15 } },
    ]);
    // Create + three edits: four entries.
    let steps = 0;
    while (document.undo()) steps += 1;
    expect(steps).toBe(4);
    expect(rails(document)).toEqual([]);
  });

  it("records nothing for an edit that changes nothing", () => {
    const document = store();
    const id = document.addRail({ name: "L+", x: 4, top: 0, bottom: 10 });
    document.markSaved();
    const before = document.getState();

    document.editRail(id, { name: "L+", x: 4, top: 0, bottom: 10 });

    expect(document.getState()).toBe(before);
    expect(document.getState().isDirty).toBe(false);
  });

  it("refuses what addRail refuses and leaves the rail as it was", () => {
    const document = store();
    const first = document.addRail({ name: "A", x: 0, top: 0, bottom: 10 });
    document.addRail({ name: "B", x: 2, top: 0, bottom: 10 });
    const before = document.getState();

    for (const patch of [
      { name: "B" },
      { name: "bad name" },
      { name: "" },
      { top: 9 },
      { bottom: -1 },
      { x: 0.5 },
    ]) {
      expect(() => document.editRail(first, patch)).toThrow();
    }
    expect(() =>
      document.editRail("00000000-0000-4000-8000-0000000000ff", { x: 1 }),
    ).toThrow();
    expect(document.getState()).toBe(before);
    // Its own name is not a duplicate of itself.
    expect(() => document.editRail(first, { name: "A", x: 1 })).not.toThrow();
  });

  it("moves no wire, junction or marker with the rail", () => {
    const document = store();
    const id = document.addRail({ name: "L+", x: 4, top: 0, bottom: 10 });
    document.addWire({
      points: [
        { x: 0, y: 5 },
        { x: 4, y: 5 },
      ],
    });
    document.addReferenceMarker({ x: 4, y: 8 });
    const others = (state: SchematicDocumentStore) => {
      const { wires, junctions, referenceMarkers, components } =
        state.getState().schematic;
      return JSON.stringify({ wires, junctions, referenceMarkers, components });
    };
    const before = others(document);

    document.editRail(id, { x: 9, top: 3, bottom: 13 });

    expect(others(document)).toBe(before);
  });
});

describe("deleting a rail", () => {
  it("removes only the rail, and undo brings it back with its ID and values", () => {
    const document = store();
    const id = document.addRail({ name: "L+", x: 4, top: 0, bottom: 10 });
    document.addWire({
      points: [
        { x: 0, y: 5 },
        { x: 4, y: 5 },
      ],
    });
    const before = rails(document);

    document.removeRail(id);

    expect(rails(document)).toEqual([]);
    expect(document.getState().schematic.wires).toHaveLength(1);
    expect(() => document.removeRail(id)).toThrow();

    document.undo();
    expect(rails(document)).toEqual(before);
    document.redo();
    expect(rails(document)).toEqual([]);
  });

  it("clears redo when a new edit follows an undo", () => {
    const document = store();
    const id = document.addRail({ name: "L+", x: 4, top: 0, bottom: 10 });
    document.removeRail(id);
    document.undo();
    expect(document.getState().canRedo).toBe(true);

    document.addRail({ name: "N", x: 9, top: 0, bottom: 10 });

    expect(document.getState().canRedo).toBe(false);
  });
});

describe("rail identity and persistence of the document", () => {
  it("never reuses an ID, even after a delete", () => {
    const document = store();
    const ids = new Set<StableId>();
    for (let index = 0; index < 5; index += 1) {
      const id = document.addRail({ x: index, top: 0, bottom: 10 });
      ids.add(id);
      document.removeRail(id);
    }

    expect(ids.size).toBe(5);
  });

  it("keeps the saved baseline: saving, then undoing, marks the document dirty again", () => {
    const document = store();
    document.addRail({ name: "L+", x: 4, top: 0, bottom: 10 });
    document.markSaved();
    expect(document.getState().isDirty).toBe(false);

    document.undo();

    expect(document.getState().isDirty).toBe(true);
  });
});
