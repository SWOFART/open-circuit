import { describe, expect, it, vi } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { EngineAdapter } from "./engineAdapter";
import { dividerSchematic } from "./fixtures";
import { SchematicDocumentStore, type StableIdFactory } from "./document";
import { createDividerFixtureAdapter } from "./mockEngineAdapter";
import { SimulationController } from "./simulation";
import { symbols } from "../symbols";

const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";

function ids(): StableIdFactory {
  let next = 0xb1;
  return () => {
    const id = `00000000-0000-4000-8000-0000000000${next.toString(16)}`;
    next += 1;
    return id;
  };
}

function createStore(): SchematicDocumentStore {
  return new SchematicDocumentStore(dividerSchematic, ids());
}

describe("SchematicDocumentStore", () => {
  it("keeps an immutable schematic separate from simulation state", () => {
    const store = createStore();
    const original = store.getState().schematic;

    expect(Object.isFrozen(store.getState())).toBe(true);
    expect(Object.isFrozen(original.components)).toBe(true);
    expect(Object.isFrozen(original.components[0])).toBe(true);

    store.moveComponent(R1, { x: 6, y: 1 });
    expect(store.getState().schematic).not.toBe(dividerSchematic);

    expect(store.getState().schematic).not.toBe(original);
    expect(
      original.components.find((component) => component.id === R1)?.position,
    ).toEqual({ x: 4, y: 0 });
    expect(
      store
        .getState()
        .schematic.components.find((component) => component.id === R1),
    ).toMatchObject({ id: R1, position: { x: 6, y: 1 } });
  });

  it("adds objects with fresh IDs and preserves IDs across edits and history", () => {
    const store = createStore();
    const initial = store.getState().schematic;
    const componentId = store.addComponent(
      {
        typeId: "resistor",
        position: { x: 10, y: 4 },
        rotationDeg: 0,
        parameters: { resistanceOhms: 4700 },
      },
      catalog,
    );
    const wireId = store.addWire({
      points: [
        { x: 0, y: 0 },
        { x: 2, y: 0 },
        { x: 2, y: 3 },
      ],
    });
    const junctionId = store.addJunction({ x: 2, y: 0 });
    const referenceId = store.addReferenceMarker({ x: 2, y: 0 });

    expect(new Set([componentId, wireId, junctionId, referenceId]).size).toBe(
      4,
    );

    store.moveComponent(componentId, { x: 12, y: 5 });
    store.rotateComponent(componentId, 90);
    store.editWire(wireId, [
      { x: 1, y: 0 },
      { x: 3, y: 0 },
    ]);
    store.moveJunction(junctionId, { x: 3, y: 0 });
    store.moveReferenceMarker(referenceId, { x: 1, y: 0 });
    store.setParameter(componentId, "resistanceOhms", 5600, catalog);

    const schematic = store.getState().schematic;
    expect(
      schematic.components.find((item) => item.id === componentId),
    ).toMatchObject({
      id: componentId,
      position: { x: 12, y: 5 },
      rotationDeg: 90,
      parameters: { resistanceOhms: 5600 },
    });
    expect(schematic.wires.find((item) => item.id === wireId)?.points).toEqual([
      { x: 1, y: 0 },
      { x: 3, y: 0 },
    ]);
    expect(schematic.junctions[0]).toEqual({
      id: junctionId,
      position: { x: 3, y: 0 },
    });
    expect(
      schematic.referenceMarkers.find((item) => item.id === referenceId),
    ).toEqual({
      id: referenceId,
      position: { x: 1, y: 0 },
    });

    expect(store.undo()).toBe(true);
    expect(
      store
        .getState()
        .schematic.components.find((item) => item.id === componentId),
    ).toMatchObject({ id: componentId, parameters: { resistanceOhms: 4700 } });
    expect(store.redo()).toBe(true);
    expect(
      store
        .getState()
        .schematic.components.find((item) => item.id === componentId),
    ).toMatchObject({ id: componentId, parameters: { resistanceOhms: 5600 } });

    const final = store.getState().schematic;
    while (store.undo()) {
      // Every method above is one atomic history operation.
    }
    expect(store.getState().schematic).toEqual(initial);
    while (store.redo()) {
      // Replay each operation, including object creation with its original ID.
    }
    expect(store.getState().schematic).toEqual(final);
  });

  it("removes components, wires, junctions, and reference markers as separate operations", () => {
    const store = createStore();
    const wireId = store.addWire({
      points: [
        { x: 1, y: 1 },
        { x: 3, y: 1 },
      ],
    });
    const junctionId = store.addJunction({ x: 3, y: 1 });
    const referenceId = store.addReferenceMarker({ x: 1, y: 1 });
    const beforeRemovals = store.getState().schematic;

    store.removeComponent(R1);
    store.removeWire(wireId);
    store.removeJunction(junctionId);
    store.removeReferenceMarker(referenceId);
    const afterRemovals = store.getState().schematic;

    expect(
      store.getState().schematic.components.some((item) => item.id === R1),
    ).toBe(false);
    expect(store.getState().schematic.wires).toEqual(dividerSchematic.wires);
    expect(store.getState().schematic.junctions).toEqual([]);
    expect(store.getState().schematic.referenceMarkers).toEqual(
      dividerSchematic.referenceMarkers,
    );

    store.undo();
    store.undo();
    store.undo();
    store.undo();
    expect(store.getState().schematic).toEqual(beforeRemovals);
    store.redo();
    store.redo();
    store.redo();
    store.redo();
    expect(store.getState().schematic).toEqual(afterRemovals);
  });

  it("moves a component together with its following wires in one history entry", () => {
    const store = createStore();
    const before = store.getState().schematic;
    const wireId = "00000000-0000-4000-8000-000000000005";
    const newPath = [
      { x: 5, y: 2 },
      { x: 7, y: 2 },
      { x: 7, y: 0 },
    ];

    store.transformComponent(R1, { x: 4, y: 2 }, 90, [
      { id: wireId, points: newPath },
    ]);
    const after = store.getState().schematic;

    expect(after.components.find((item) => item.id === R1)).toMatchObject({
      id: R1,
      position: { x: 4, y: 2 },
      rotationDeg: 90,
    });
    expect(after.wires.find((wire) => wire.id === wireId)?.points).toEqual(
      newPath,
    );

    expect(store.undo()).toBe(true);
    expect(store.getState().schematic).toEqual(before);
    expect(store.getState().canUndo).toBe(false);
    expect(store.getState().isDirty).toBe(false);

    expect(store.redo()).toBe(true);
    expect(store.getState().schematic).toEqual(after);
  });

  it("rejects a transform with an invalid wire path or unknown object", () => {
    const store = createStore();
    const before = store.getState().schematic;
    const wireId = "00000000-0000-4000-8000-000000000005";

    expect(() =>
      store.transformComponent(R1, { x: 4, y: 2 }, 0, [
        {
          id: wireId,
          points: [
            { x: 5, y: 2 },
            { x: 7, y: 0 },
          ],
        },
      ]),
    ).toThrow();
    expect(() =>
      store.transformComponent(R1, { x: 4, y: 2 }, 0, [
        {
          id: "00000000-0000-4000-8000-0000000000ff",
          points: [
            { x: 5, y: 2 },
            { x: 7, y: 2 },
          ],
        },
      ]),
    ).toThrow();
    expect(() =>
      store.transformComponent(R1, { x: 4, y: 2 }, 45 as 0, []),
    ).toThrow();
    expect(() =>
      store.transformComponent(
        "00000000-0000-4000-8000-0000000000ff",
        { x: 0, y: 0 },
        0,
      ),
    ).toThrow();

    expect(store.getState().schematic).toBe(before);
    expect(store.getState().canUndo).toBe(false);
  });

  it("clears the redo branch after a new document edit", () => {
    const store = createStore();

    store.moveComponent(R1, { x: 5, y: 0 });
    store.undo();
    expect(store.getState().canRedo).toBe(true);

    store.rotateComponent(R1, 180);

    expect(store.getState().canRedo).toBe(false);
    expect(store.redo()).toBe(false);
  });

  it("tracks dirty state against the saved baseline and clears it when undo returns there", () => {
    const store = createStore();
    expect(store.getState().isDirty).toBe(false);

    store.moveComponent(R1, { x: 5, y: 0 });
    expect(store.getState().isDirty).toBe(true);
    store.undo();
    expect(store.getState().isDirty).toBe(false);

    store.moveComponent(R1, { x: 5, y: 0 });
    store.markSaved();
    expect(store.getState().isDirty).toBe(false);
    store.undo();
    expect(store.getState().isDirty).toBe(true);
    store.redo();
    expect(store.getState().isDirty).toBe(false);
  });

  it("replaces the active document with a clean history", () => {
    const store = createStore();
    store.moveComponent(R1, { x: 5, y: 0 });
    store.replaceDocument(dividerSchematic);

    expect(store.getState()).toMatchObject({
      schematic: dividerSchematic,
      canUndo: false,
      canRedo: false,
      isDirty: false,
    });
  });

  it("does not reuse an ID after an object is undone", () => {
    const reusedId = "00000000-0000-4000-8000-0000000000b1";
    const store = new SchematicDocumentStore(dividerSchematic, () => reusedId);
    store.addJunction({ x: 2, y: 0 });
    store.undo();

    expect(() => store.addReferenceMarker({ x: 2, y: 0 })).toThrow(
      /already been used/,
    );
    expect(store.getState().canRedo).toBe(true);
  });

  it("rejects invalid edits without changing the document or creating history", () => {
    const store = createStore();
    const before = store.getState();

    expect(() => store.moveComponent(R1, { x: 1.5, y: 0 })).toThrow(/integer/);
    expect(() => store.rotateComponent(R1, 45 as never)).toThrow(/rotation/);
    expect(() => store.setParameter(R1, "resistanceOhms", 0, catalog)).toThrow(
      /catalog domain/,
    );
    expect(() =>
      store.addWire({
        points: [
          { x: 0, y: 0 },
          { x: 1, y: 1 },
        ],
      }),
    ).toThrow(/orthogonal/);

    expect(store.getState()).toBe(before);
    expect(store.undo()).toBe(false);
  });

  it("rejects duplicate IDs already present in the input document", () => {
    const duplicate = structuredClone(dividerSchematic);
    duplicate.wires[0]!.id = duplicate.components[0]!.id;

    expect(() => new SchematicDocumentStore(duplicate)).toThrow(/duplicated/);
  });

  it("notifies subscribers after committed changes and history changes", () => {
    const store = createStore();
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);

    store.moveComponent(R1, { x: 5, y: 0 });
    store.undo();
    unsubscribe();
    store.moveComponent(R1, { x: 6, y: 0 });

    expect(listener).toHaveBeenCalledTimes(2);
  });
});

describe("document and Engine separation", () => {
  it("keeps a document edit when the Engine rejects its parameter update", async () => {
    const base = createDividerFixtureAdapter();
    const adapter: EngineAdapter = {
      getCatalog: () => base.getCatalog(),
      compile: (request) => base.compile(request),
      setParameter: async () => ({ status: "failure", diagnostics: [] }),
      startFrames: (runId, handlers) => base.startFrames(runId, handlers),
      readTraces: (request) => base.readTraces(request),
    };
    const controller = new SimulationController(
      adapter,
      symbols,
      () => "revision",
    );
    const store = createStore();
    await controller.start(dividerSchematic);
    const previousSnapshot = controller.getState().snapshot;

    store.setParameter(R1, "resistanceOhms", 4700, catalog);
    await controller.setParameter(R1, "resistanceOhms", 4700);

    expect(
      store.getState().schematic.components.find((item) => item.id === R1),
    ).toMatchObject({ parameters: { resistanceOhms: 4700 } });
    expect(store.getState().isDirty).toBe(true);
    expect(controller.getState().snapshot).toBe(previousSnapshot);
    expect(controller.getState()).not.toHaveProperty("schematic");
    controller.dispose();
  });
});
