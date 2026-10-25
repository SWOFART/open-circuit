import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { commandForIntent } from "../editor/commands";
import { resolvePinPositions } from "../editor/geometry";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import { SchematicDocumentStore } from "../store/document";
import { dividerSchematic } from "../store/fixtures";
import { symbols } from "../symbols";
import { applyEditorCommand, defaultParameters } from "./editorCommands";

const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";
const MIDDLE_WIRE = "00000000-0000-4000-8000-000000000005";
const NEW_ID = "00000000-0000-4000-8000-0000000000c1";

function createStore(): SchematicDocumentStore {
  return new SchematicDocumentStore(dividerSchematic, () => NEW_ID);
}

describe("catalog defaults", () => {
  it.each(catalog.componentTypes)(
    "gives a new $typeId every catalog parameter at its default",
    (entry) => {
      expect(defaultParameters(catalog, entry.typeId)).toEqual(
        Object.fromEntries(
          entry.parameters.map((parameter) => [
            parameter.id,
            parameter.defaultValue,
          ]),
        ),
      );
    },
  );

  it("refuses a type that is not in the catalog", () => {
    expect(() =>
      defaultParameters(catalog, "lamp" as ComponentTypeId),
    ).toThrow();
  });
});

describe("applying canvas commands to the document", () => {
  it.each(catalog.componentTypes.filter((entry) => symbols[entry.typeId]))(
    "places a $typeId the store accepts, with pins on the grid",
    (entry) => {
      const store = createStore();

      applyEditorCommand(
        store,
        {
          kind: "placeComponent",
          typeId: entry.typeId,
          position: { x: 20, y: -6 },
          rotationDeg: 90,
        },
        catalog,
      );

      const placed = store
        .getState()
        .schematic.components.find((item) => item.id === NEW_ID);
      expect(placed).toMatchObject({
        id: NEW_ID,
        typeId: entry.typeId,
        position: { x: 20, y: -6 },
        rotationDeg: 90,
        parameters: defaultParameters(catalog, entry.typeId),
      });
      if (!placed) throw new Error("not placed");
      // Catalog-to-symbol compatibility: the placed type has a symbol whose
      // pins resolve to integer grid points.
      expect(
        resolvePinPositions(placed, symbols).map((pin) => pin.pinId),
      ).toEqual(entry.pins.map((pin) => pin.id));
      expect(store.getState().canUndo).toBe(true);
    },
  );

  it("cannot place before the catalog has loaded", () => {
    const store = createStore();
    const before = store.getState().schematic;

    expect(() =>
      applyEditorCommand(
        store,
        {
          kind: "placeComponent",
          typeId: "resistor",
          position: { x: 0, y: 0 },
          rotationDeg: 0,
        },
        null,
      ),
    ).toThrow();
    expect(store.getState().schematic).toBe(before);
  });

  it("moves R1 with its wires, keeping its ID, and undoes it in one step", () => {
    const store = createStore();
    const before = store.getState().schematic;
    const command = commandForIntent(
      { kind: "move", componentId: R1, delta: { x: 0, y: 2 } },
      before,
      symbols,
    );
    if (!command) throw new Error("no command");

    applyEditorCommand(store, command, catalog);
    const after = store.getState().schematic;

    expect(after.components.find((item) => item.id === R1)?.position).toEqual({
      x: 4,
      y: 2,
    });
    expect(
      after.wires.find((wire) => wire.id === MIDDLE_WIRE)?.points[0],
    ).toEqual({ x: 5, y: 2 });
    expect(after.components.map((item) => item.id)).toEqual(
      before.components.map((item) => item.id),
    );

    applyEditorCommand(store, { kind: "undo" }, catalog);
    expect(store.getState().schematic).toEqual(before);
    applyEditorCommand(store, { kind: "redo" }, catalog);
    expect(store.getState().schematic).toEqual(after);
  });

  it("rotates R1 a quarter turn per command", () => {
    const store = createStore();

    for (const expected of [90, 180, 270, 0]) {
      const command = commandForIntent(
        { kind: "rotate", componentId: R1 },
        store.getState().schematic,
        symbols,
      );
      if (!command) throw new Error("no command");
      applyEditorCommand(store, command, catalog);

      expect(
        store.getState().schematic.components.find((item) => item.id === R1)
          ?.rotationDeg,
      ).toBe(expected);
    }
  });

  it("deletes only the component and leaves its wires as drawn", () => {
    const store = createStore();
    const wires = store.getState().schematic.wires;

    applyEditorCommand(
      store,
      { kind: "deleteComponent", componentId: R1 },
      catalog,
    );

    const after = store.getState().schematic;
    expect(after.components.map((item) => item.id)).not.toContain(R1);
    expect(after.wires).toEqual(wires);
    expect(after.referenceMarkers).toEqual(dividerSchematic.referenceMarkers);
  });
});

describe("applying drawing commands to the document", () => {
  const W1 = "00000000-0000-4000-8000-0000000001c1";
  const path = [
    { x: 5, y: 3 },
    { x: 9, y: 3 },
  ];

  function storeWithIds(...ids: string[]): SchematicDocumentStore {
    const queue = [...ids];
    return new SchematicDocumentStore(dividerSchematic, () => {
      const next = queue.shift();
      if (!next) throw new Error("out of IDs");
      return next;
    });
  }

  it("adds a wire with a fresh ID, and undoes it as one step", () => {
    const store = storeWithIds(W1);

    applyEditorCommand(store, { kind: "addWire", points: path }, catalog);

    expect(store.getState().schematic.wires.at(-1)).toEqual({
      id: W1,
      points: path,
    });
    applyEditorCommand(store, { kind: "undo" }, catalog);
    expect(store.getState().schematic.wires).toEqual(dividerSchematic.wires);
  });

  it("rejects a wire the document model refuses and changes nothing", () => {
    const store = storeWithIds(W1);
    const before = store.getState().schematic;

    expect(() =>
      applyEditorCommand(
        store,
        {
          kind: "addWire",
          points: [
            { x: 0, y: 0 },
            { x: 3, y: 4 },
          ],
        },
        catalog,
      ),
    ).toThrow();
    expect(store.getState().schematic).toBe(before);
    expect(store.getState().canUndo).toBe(false);
  });

  it("edits a wire path while keeping its ID", () => {
    const store = storeWithIds();
    const edited = [
      { x: 5, y: 0 },
      { x: 7, y: 0 },
      { x: 7, y: 2 },
    ];

    applyEditorCommand(
      store,
      { kind: "editWire", wireId: MIDDLE_WIRE, points: edited },
      catalog,
    );

    expect(
      store.getState().schematic.wires.find((wire) => wire.id === MIDDLE_WIRE),
    ).toEqual({ id: MIDDLE_WIRE, points: edited });
  });

  it("adds, moves, and removes a junction, keeping one ID throughout", () => {
    const store = storeWithIds("00000000-0000-4000-8000-0000000001a1");

    applyEditorCommand(
      store,
      { kind: "addJunction", position: { x: 6, y: 0 } },
      catalog,
    );
    const [junction] = store.getState().schematic.junctions;
    expect(junction).toEqual({
      id: "00000000-0000-4000-8000-0000000001a1",
      position: { x: 6, y: 0 },
    });

    applyEditorCommand(
      store,
      {
        kind: "moveJunction",
        id: junction?.id ?? "",
        position: { x: 8, y: 1 },
      },
      catalog,
    );
    expect(store.getState().schematic.junctions).toEqual([
      { id: junction?.id, position: { x: 8, y: 1 } },
    ]);

    applyEditorCommand(
      store,
      { kind: "deleteJunction", id: junction?.id ?? "" },
      catalog,
    );
    expect(store.getState().schematic.junctions).toEqual([]);
  });

  it("adds, moves, and removes a reference marker beside the existing one", () => {
    const store = storeWithIds("00000000-0000-4000-8000-0000000001b9");

    applyEditorCommand(
      store,
      { kind: "addReferenceMarker", position: { x: 5, y: 0 } },
      catalog,
    );
    expect(store.getState().schematic.referenceMarkers).toHaveLength(2);

    const added = "00000000-0000-4000-8000-0000000001b9";
    applyEditorCommand(
      store,
      { kind: "moveReferenceMarker", id: added, position: { x: 7, y: 0 } },
      catalog,
    );
    expect(
      store
        .getState()
        .schematic.referenceMarkers.find((marker) => marker.id === added)
        ?.position,
    ).toEqual({ x: 7, y: 0 });

    applyEditorCommand(
      store,
      { kind: "deleteReferenceMarker", id: added },
      catalog,
    );
    expect(store.getState().schematic.referenceMarkers).toEqual(
      dividerSchematic.referenceMarkers,
    );
  });

  it("deletes a wire and leaves everything connected to it as drawn", () => {
    const store = storeWithIds();

    applyEditorCommand(
      store,
      { kind: "deleteWire", wireId: MIDDLE_WIRE },
      catalog,
    );

    const after = store.getState().schematic;
    expect(after.wires.map((wire) => wire.id)).not.toContain(MIDDLE_WIRE);
    expect(after.components).toEqual(dividerSchematic.components);
    expect(after.referenceMarkers).toEqual(dividerSchematic.referenceMarkers);
  });

  it("reports an operation on an object that is gone, leaving the document", () => {
    const store = storeWithIds();
    const before = store.getState().schematic;

    expect(() =>
      applyEditorCommand(
        store,
        { kind: "deleteWire", wireId: "00000000-0000-4000-8000-0000000001ff" },
        catalog,
      ),
    ).toThrow();
    expect(store.getState().schematic).toBe(before);
  });
});
