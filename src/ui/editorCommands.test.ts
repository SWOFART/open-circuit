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
  it.each(catalog.componentTypes)(
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
