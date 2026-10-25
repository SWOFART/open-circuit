import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { SchematicDocumentStore } from "./document";
import { componentLabels, dividerSchematic } from "./fixtures";
import { ComponentLabels } from "./labels";

const catalog = catalogFixture as ComponentCatalog;
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";

function idFactory(): () => StableId {
  let next = 0xd1;
  return () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`;
}

function newStore(): SchematicDocumentStore {
  return new SchematicDocumentStore(dividerSchematic, idFactory());
}

function addResistor(store: SchematicDocumentStore, x: number): StableId {
  return store.addComponent(
    {
      typeId: "resistor",
      position: { x, y: 10 },
      rotationDeg: 0,
      parameters: { resistanceOhms: 1000 },
    },
    catalog,
  );
}

function names(store: SchematicDocumentStore): Record<string, string> {
  const { labels, schematic } = store.getState();

  return Object.fromEntries(
    schematic.components.map((component) => [
      component.id,
      labels.get(component.id) ?? "?",
    ]),
  );
}

describe("component display names", () => {
  it("numbers a new document per type in file order", () => {
    expect(newStore().getState().labels).toEqual(
      new Map([
        [V1, "V1"],
        [R1, "R1"],
        [R2, "R2"],
      ]),
    );
  });

  it("matches the one-shot helper used for fixed schematics", () => {
    expect(componentLabels(dividerSchematic)).toEqual(
      newStore().getState().labels,
    );
  });

  it("keeps R2 as R2 when R1 is deleted", () => {
    const store = newStore();

    store.removeComponent(R1);

    expect(names(store)).toEqual({ [V1]: "V1", [R2]: "R2" });
  });

  it("gives a new component the next number and never reuses a name", () => {
    const store = newStore();
    store.removeComponent(R2);

    const added = addResistor(store, 20);

    // R2 is gone, but its name is not handed out again.
    expect(store.getState().labels.get(added)).toBe("R3");
    expect(store.getState().labels.get(R1)).toBe("R1");
  });

  it("numbers each type on its own", () => {
    const store = newStore();

    const source = store.addComponent(
      {
        typeId: "dc_voltage_source",
        position: { x: 0, y: 12 },
        rotationDeg: 0,
        parameters: { voltageVolts: 9, internalResistanceOhms: 0.1 },
      },
      catalog,
    );

    expect(store.getState().labels.get(source)).toBe("V2");
  });

  it("restores a deleted component under its own name on undo", () => {
    const store = newStore();
    store.removeComponent(R1);
    const added = addResistor(store, 20);
    expect(store.getState().labels.get(added)).toBe("R3");

    store.undo();
    store.undo();

    expect(names(store)).toEqual({ [V1]: "V1", [R1]: "R1", [R2]: "R2" });
  });

  it("keeps names stable across the whole undo and redo history", () => {
    const store = newStore();
    store.removeComponent(R1);
    const added = addResistor(store, 20);
    const afterEdits = names(store);

    store.undo();
    store.undo();
    store.redo();
    store.redo();

    expect(names(store)).toEqual(afterEdits);
    expect(store.getState().labels.get(added)).toBe("R3");
  });

  it("does not hand a redone placement's name to a different component", () => {
    const store = newStore();
    const first = addResistor(store, 20);
    expect(store.getState().labels.get(first)).toBe("R3");

    store.undo();
    const second = addResistor(store, 30);

    expect(store.getState().labels.get(second)).toBe("R4");
  });

  it("leaves every name alone when a component moves, rotates, or is edited", () => {
    const store = newStore();
    const before = names(store);

    store.moveComponent(R1, { x: 6, y: 2 });
    store.rotateComponent(R2, 90);
    store.setParameter(R1, "resistanceOhms", 4700, catalog);

    expect(names(store)).toEqual(before);
  });

  it("keeps the same map until a name is added, so the canvas is not redrawn", () => {
    const store = newStore();
    const first = store.getState().labels;

    store.moveComponent(R1, { x: 6, y: 2 });
    store.markSaved();
    expect(store.getState().labels).toBe(first);

    addResistor(store, 20);
    expect(store.getState().labels).not.toBe(first);
  });

  it("still names a component that is gone, for results that mention it", () => {
    const store = newStore();

    store.removeComponent(R1);

    expect(store.getState().labels.get(R1)).toBe("R1");
  });

  it("numbers a replacement document from scratch, in file order", () => {
    const store = newStore();
    addResistor(store, 20);
    store.removeComponent(R1);

    const reopened: Schematic = {
      ...dividerSchematic,
      components: [...dividerSchematic.components].reverse(),
    };
    store.replaceDocument(reopened);

    expect(names(store)).toEqual({
      [R2]: "R1",
      [R1]: "R2",
      [V1]: "V1",
    });
    expect(
      store.getState().labels.has("00000000-0000-4000-8000-0000000000d1"),
    ).toBe(false);
  });

  it("starts at one again for a blank document", () => {
    const store = newStore();
    store.replaceDocument({ ...dividerSchematic, components: [] });

    const added = addResistor(store, 4);

    expect(store.getState().labels.get(added)).toBe("R1");
  });
});

describe("ComponentLabels", () => {
  it("returns the same instance when nothing new was seen", () => {
    const labels = new ComponentLabels();
    const first = labels.labelsFor(dividerSchematic);

    expect(labels.labelsFor(dividerSchematic)).toBe(first);
  });

  it("forgets everything on reset", () => {
    const labels = new ComponentLabels();
    labels.labelsFor(dividerSchematic);

    labels.reset();

    expect(labels.labelsFor({ ...dividerSchematic, components: [] }).size).toBe(
      0,
    );
    expect(
      labels
        .labelsFor({
          ...dividerSchematic,
          components: [...dividerSchematic.components].reverse(),
        })
        .get(R2),
    ).toBe("R1");
  });
});
