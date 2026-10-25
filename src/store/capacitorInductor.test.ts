import { describe, expect, it, vi } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import dcInductor from "../../tests/golden/stage-3-dc-inductor.schematic.json";
import losslessLc from "../../tests/golden/stage-3-lossless-lc.schematic.json";
import rcCharge from "../../tests/golden/stage-3-rc-charge.schematic.json";
import rcDischarge from "../../tests/golden/stage-3-rc-discharge.schematic.json";
import rlDecay from "../../tests/golden/stage-3-rl-decay.schematic.json";
import rlRise from "../../tests/golden/stage-3-rl-rise.schematic.json";
import type { CapacitorParameters } from "../generated/CapacitorParameters";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { InductorParameters } from "../generated/InductorParameters";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols } from "../symbols";
import { buildGeometryEnvelope } from "../editor/geometry";
import { buildScene } from "../editor/scene";
import { SchematicDocumentStore } from "./document";
import { blankSchematic, ProjectWorkflow } from "./projectWorkflow";

const catalog = catalogFixture as ComponentCatalog;
const stage3Fixtures: Record<string, Schematic> = {
  "RC charge": rcCharge as Schematic,
  "RC discharge": rcDischarge as Schematic,
  "RL rise": rlRise as Schematic,
  "RL decay": rlDecay as Schematic,
  "DC initialization, inductor": dcInductor as Schematic,
  "lossless LC": losslessLc as Schematic,
};

function ids(): () => StableId {
  let next = 0xe1;
  return () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`;
}

function blankStore(): SchematicDocumentStore {
  return new SchematicDocumentStore(blankSchematic, ids());
}

function addCapacitor(
  store: SchematicDocumentStore,
  parameters: Record<string, number> = { capacitanceFarads: 1e-5 },
): StableId {
  return store.addComponent(
    {
      typeId: "capacitor",
      position: { x: 4, y: 0 },
      rotationDeg: 0,
      parameters: parameters as unknown as CapacitorParameters,
    },
    catalog,
  );
}

function addInductor(
  store: SchematicDocumentStore,
  parameters: Record<string, number> = { inductanceHenries: 0.1 },
): StableId {
  return store.addComponent(
    {
      typeId: "inductor",
      position: { x: 8, y: 0 },
      rotationDeg: 0,
      parameters: parameters as unknown as InductorParameters,
    },
    catalog,
  );
}

function parametersOf(
  store: SchematicDocumentStore,
  id: StableId,
): Record<string, number> {
  const component = store
    .getState()
    .schematic.components.find((item) => item.id === id);
  if (!component) throw new Error(`Component ${id} is missing.`);

  return component.parameters as unknown as Record<string, number>;
}

describe("generated types for the new parameters", () => {
  it("make the initial conditions optional and the values required", () => {
    // These are compile-time checks; the test fails to build if the generated
    // types drift from the contract.
    const unset: CapacitorParameters = { capacitanceFarads: 1e-5 };
    const set: CapacitorParameters = {
      capacitanceFarads: 1e-5,
      initialVoltageVolts: -3,
    };
    const coil: InductorParameters = { inductanceHenries: 0.1 };
    const energized: InductorParameters = {
      inductanceHenries: 0.1,
      initialCurrentAmps: 2e-3,
    };

    expect(unset.initialVoltageVolts).toBeUndefined();
    expect(set.initialVoltageVolts).toBe(-3);
    expect(coil.initialCurrentAmps).toBeUndefined();
    expect(energized.initialCurrentAmps).toBe(2e-3);
  });

  it.each(Object.entries(stage3Fixtures))(
    "match the Engine's own %s fixture, parameter by parameter",
    (_, schematic) => {
      for (const component of schematic.components) {
        const entry = catalog.componentTypes.find(
          (item) => item.typeId === component.typeId,
        );
        const given = Object.keys(component.parameters);
        const known = (entry?.parameters ?? []).map((item) => item.id);

        // Nothing outside the catalog, and every required parameter present.
        expect(known).toEqual(expect.arrayContaining(given));
        for (const definition of entry?.parameters ?? []) {
          if (!definition.optional) {
            expect(given).toContain(definition.id);
          }
        }
      }
    },
  );
});

describe("the Engine's Stage 3 fixtures in the editor", () => {
  it.each(Object.entries(stage3Fixtures))(
    "opens %s as an editable document with a symbol for every part",
    (_, schematic) => {
      const store = new SchematicDocumentStore(schematic);

      expect(store.getState().schematic).toEqual(schematic);
      expect(store.getState().isDirty).toBe(false);
      expect(() => buildScene(schematic, symbols)).not.toThrow();
      expect(() => buildGeometryEnvelope(schematic, symbols)).not.toThrow();
    },
  );

  it("names the parts C1 and L1 on open", () => {
    const store = new SchematicDocumentStore(losslessLc as Schematic);
    const names = [...store.getState().labels.values()].sort();

    expect(names).toEqual(["C1", "L1"]);
  });

  it("saves exactly what was opened, with unset initial conditions absent", () => {
    const store = new SchematicDocumentStore(dcInductor as Schematic);
    const saved = JSON.parse(JSON.stringify(store.getState().schematic));

    expect(saved).toEqual(dcInductor);
    expect(JSON.stringify(saved)).not.toContain("initialCurrentAmps");
  });
});

describe("placing a capacitor or an inductor", () => {
  it("accepts the required parameter alone, leaving the initial condition unset", () => {
    const store = blankStore();
    const capacitor = addCapacitor(store);
    const inductor = addInductor(store);

    expect(parametersOf(store, capacitor)).toEqual({ capacitanceFarads: 1e-5 });
    expect(parametersOf(store, inductor)).toEqual({ inductanceHenries: 0.1 });
    expect(store.getState().labels.get(capacitor)).toBe("C1");
    expect(store.getState().labels.get(inductor)).toBe("L1");
  });

  it("accepts a signed initial condition, including zero", () => {
    const store = blankStore();
    const negative = addCapacitor(store, {
      capacitanceFarads: 2.2e-6,
      initialVoltageVolts: -4.5,
    });
    const zero = addInductor(store, {
      inductanceHenries: 1,
      initialCurrentAmps: 0,
    });

    expect(parametersOf(store, negative).initialVoltageVolts).toBe(-4.5);
    expect(parametersOf(store, zero).initialCurrentAmps).toBe(0);
  });

  it.each([
    ["zero capacitance", { capacitanceFarads: 0 }],
    ["negative capacitance", { capacitanceFarads: -1e-6 }],
    ["infinite capacitance", { capacitanceFarads: Infinity }],
    ["NaN capacitance", { capacitanceFarads: Number.NaN }],
    ["a missing capacitance", {}],
    [
      "a non-finite initial voltage",
      { capacitanceFarads: 1e-5, initialVoltageVolts: Infinity },
    ],
    [
      "a parameter that belongs to another type",
      { capacitanceFarads: 1e-5, inductanceHenries: 1 },
    ],
  ])("refuses %s and leaves the document unchanged", (_, parameters) => {
    const store = blankStore();
    const before = store.getState();

    expect(() => addCapacitor(store, parameters)).toThrow();
    expect(store.getState().schematic).toBe(before.schematic);
    expect(store.getState().canUndo).toBe(false);
  });

  it.each([
    ["zero inductance", { inductanceHenries: 0 }],
    ["negative inductance", { inductanceHenries: -0.1 }],
    [
      "a non-finite initial current",
      { inductanceHenries: 0.1, initialCurrentAmps: Number.NaN },
    ],
  ])("refuses an inductor with %s", (_, parameters) => {
    const store = blankStore();

    expect(() => addInductor(store, parameters)).toThrow();
    expect(store.getState().schematic.components).toEqual([]);
  });
});

describe("editing a capacitor or an inductor", () => {
  it("edits the value through the catalog domain, with undo and redo", () => {
    const store = blankStore();
    const capacitor = addCapacitor(store);

    store.setParameter(capacitor, "capacitanceFarads", 4.7e-6, catalog);
    expect(parametersOf(store, capacitor).capacitanceFarads).toBe(4.7e-6);

    store.undo();
    expect(parametersOf(store, capacitor).capacitanceFarads).toBe(1e-5);
    store.redo();
    expect(parametersOf(store, capacitor).capacitanceFarads).toBe(4.7e-6);
  });

  it("refuses a value outside the domain without a history entry", () => {
    const store = blankStore();
    const inductor = addInductor(store);
    store.undo();
    store.redo();
    const before = store.getState().schematic;

    for (const bad of [0, -1, Number.NaN, Infinity]) {
      expect(() =>
        store.setParameter(inductor, "inductanceHenries", bad, catalog),
      ).toThrow();
    }
    expect(store.getState().schematic).toBe(before);
  });

  it("sets an initial condition and takes it back, one history entry each", () => {
    const store = blankStore();
    const capacitor = addCapacitor(store);

    store.setParameter(capacitor, "initialVoltageVolts", 3, catalog);
    expect(parametersOf(store, capacitor)).toEqual({
      capacitanceFarads: 1e-5,
      initialVoltageVolts: 3,
    });

    store.clearParameter(capacitor, "initialVoltageVolts", catalog);
    expect(parametersOf(store, capacitor)).toEqual({ capacitanceFarads: 1e-5 });
    expect("initialVoltageVolts" in parametersOf(store, capacitor)).toBe(false);

    store.undo();
    expect(parametersOf(store, capacitor).initialVoltageVolts).toBe(3);
    store.undo();
    expect("initialVoltageVolts" in parametersOf(store, capacitor)).toBe(false);
  });

  it("accepts a negative initial condition and zero for both types", () => {
    const store = blankStore();
    const capacitor = addCapacitor(store);
    const inductor = addInductor(store);

    store.setParameter(capacitor, "initialVoltageVolts", -2, catalog);
    store.setParameter(inductor, "initialCurrentAmps", 0, catalog);

    expect(parametersOf(store, capacitor).initialVoltageVolts).toBe(-2);
    expect(parametersOf(store, inductor).initialCurrentAmps).toBe(0);
  });

  it("clears only optional parameters", () => {
    const store = blankStore();
    const capacitor = addCapacitor(store);

    expect(() =>
      store.clearParameter(capacitor, "capacitanceFarads", catalog),
    ).toThrow("cannot be cleared");
    expect(() =>
      store.clearParameter(capacitor, "inductanceHenries", catalog),
    ).toThrow();
    expect(() =>
      store.clearParameter(
        "00000000-0000-4000-8000-0000000000ff",
        "initialVoltageVolts",
        catalog,
      ),
    ).toThrow("does not exist");
    expect(parametersOf(store, capacitor)).toEqual({ capacitanceFarads: 1e-5 });
  });

  it("does not record a change when an unset condition is cleared again", () => {
    const store = blankStore();
    const capacitor = addCapacitor(store);
    const before = store.getState();

    store.clearParameter(capacitor, "initialVoltageVolts", catalog);

    expect(store.getState().schematic).toBe(before.schematic);
  });

  it("refuses a parameter that the component type does not have", () => {
    const store = blankStore();
    const capacitor = addCapacitor(store);

    expect(() =>
      store.setParameter(capacitor, "resistanceOhms", 5, catalog),
    ).toThrow();
    expect(() =>
      store.setParameter(capacitor, "initialCurrentAmps", 5, catalog),
    ).toThrow();
  });

  it("keeps the ID, the name, and the parameters through move, rotate, and delete with undo", () => {
    const store = blankStore();
    const inductor = addInductor(store, {
      inductanceHenries: 0.25,
      initialCurrentAmps: 1e-3,
    });

    store.moveComponent(inductor, { x: 12, y: 3 });
    store.rotateComponent(inductor, 90);
    store.removeComponent(inductor);
    store.undo();

    const restored = store
      .getState()
      .schematic.components.find((item) => item.id === inductor);
    expect(restored).toMatchObject({
      id: inductor,
      typeId: "inductor",
      position: { x: 12, y: 3 },
      rotationDeg: 90,
      parameters: { inductanceHenries: 0.25, initialCurrentAmps: 1e-3 },
    });
    expect(store.getState().labels.get(inductor)).toBe("L1");
  });
});

describe("opening and saving projects with capacitors and inductors", () => {
  function createFiles(opened: Schematic | null = null) {
    return {
      open: vi.fn<() => Promise<Schematic | null>>().mockResolvedValue(opened),
      save: vi.fn<(_: Schematic) => Promise<boolean>>().mockResolvedValue(true),
      saveAs: vi
        .fn<(_: Schematic) => Promise<boolean>>()
        .mockResolvedValue(true),
      clearPath: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    };
  }

  it("opens a file with initial conditions as a clean, named, editable document", async () => {
    const store = new SchematicDocumentStore(blankSchematic);
    const workflow = new ProjectWorkflow(
      store,
      createFiles(losslessLc as Schematic),
    );

    await expect(workflow.perform("open")).resolves.toBe(true);

    expect(store.getState().schematic).toEqual(losslessLc);
    expect(store.getState().isDirty).toBe(false);
    expect([...store.getState().labels.values()].sort()).toEqual(["C1", "L1"]);
  });

  it("saves an edited initial condition, and leaves a cleared one out of the file", async () => {
    const files = createFiles(rcCharge as Schematic);
    const store = new SchematicDocumentStore(blankSchematic);
    const workflow = new ProjectWorkflow(store, files);
    await workflow.perform("open");
    const capacitor = store
      .getState()
      .schematic.components.find((item) => item.typeId === "capacitor");
    if (!capacitor) throw new Error("The fixture has no capacitor.");

    store.setParameter(capacitor.id, "initialVoltageVolts", 7, catalog);
    await workflow.save();
    store.clearParameter(capacitor.id, "initialVoltageVolts", catalog);
    expect(store.getState().isDirty).toBe(true);
    await workflow.save();

    const [first, second] = files.save.mock.calls.map(([schematic]) =>
      JSON.stringify(schematic),
    );
    expect(first).toContain('"initialVoltageVolts":7');
    expect(second).not.toContain("initialVoltageVolts");
    expect(store.getState().isDirty).toBe(false);
  });

  it("starts a new document without the previous document's names", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(losslessLc as Schematic);
    const workflow = new ProjectWorkflow(store, files);

    await workflow.perform("new");
    const added = addCapacitor(store);

    expect(store.getState().labels.get(added)).toBe("C1");
  });
});
