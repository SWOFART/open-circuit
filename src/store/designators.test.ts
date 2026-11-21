import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import {
  DESIGNATOR_PATTERN,
  designatorProblem,
  nextDesignator,
} from "../editor/designators";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { StableId } from "../generated/StableId";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";
import { ComponentLabels } from "./labels";
import { hasSameElectricalPart } from "./simulation";

const catalog = catalogFixture as ComponentCatalog;
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";

function newStore(): SchematicDocumentStore {
  let next = 0xd1;
  return new SchematicDocumentStore(
    dividerSchematic,
    () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`,
  );
}

function place(
  store: SchematicDocumentStore,
  typeId: "resistor" | "contactor" | "dc_voltage_source",
  designate = true,
): StableId {
  const parameters =
    typeId === "resistor"
      ? { resistanceOhms: 1000 }
      : typeId === "dc_voltage_source"
        ? { voltageVolts: 5, internalResistanceOhms: 0 }
        : {
            coilResistanceOhms: 100,
            coilInductanceHenries: 0.1,
            pickupCurrentAmps: 0.02,
            dropoutCurrentAmps: 0.01,
            closedContactResistanceOhms: 0.01,
          };
  return store.addComponent(
    { typeId, position: { x: 30, y: 10 }, rotationDeg: 0, parameters } as never,
    catalog,
    { designate },
  );
}

function designators(store: SchematicDocumentStore): Record<string, string> {
  return Object.fromEntries(
    store
      .getState()
      .schematic.documentation.designators.map((entry) => [
        entry.componentId,
        entry.designator,
      ]),
  );
}

describe("the designator rules", () => {
  it("accepts the grammar of the contract and nothing else", () => {
    for (const good of ["R1", "K12", "QF1", "FR9999", "XAB7", "S1"]) {
      expect(DESIGNATOR_PATTERN.test(good), good).toBe(true);
    }
    for (const bad of [
      "",
      "r1",
      "R",
      "R0",
      "R01",
      "R10000",
      "I1",
      "O2",
      "D1",
      "Z1",
      "RI1",
      "RRRR1",
      "R 1",
      "R1a",
      "Р" + "1",
    ]) {
      expect(DESIGNATOR_PATTERN.test(bad), bad).toBe(false);
    }
  });

  it("names the problem, and a duplicate by exact comparison", () => {
    expect(designatorProblem("", [])).toBe("empty");
    expect(designatorProblem("k1", [])).toBe("form");
    expect(designatorProblem("K1", ["K1"])).toBe("duplicate");
    expect(designatorProblem("K1", ["K2", "k1", "QK1"])).toBeNull();
  });

  it("numbers after the highest in use with the same letters, leaving gaps", () => {
    expect(nextDesignator("Q", [])).toBe("Q1");
    expect(nextDesignator("Q", ["Q1", "Q3", "QF9", "K7"])).toBe("Q4");
    expect(nextDesignator("QF", ["Q1", "QF2"])).toBe("QF3");
    expect(() => nextDesignator("Q", ["Q9999"])).toThrow(/no free/);
  });
});

describe("designators in the document", () => {
  it("stores the catalog letter and the next number with the part, in one history entry", () => {
    const store = newStore();
    const first = place(store, "resistor");
    const second = place(store, "resistor");
    const source = place(store, "dc_voltage_source");

    expect(designators(store)).toEqual({
      [first]: "R1",
      [second]: "R2",
      [source]: "G1",
    });

    store.undo();
    expect(designators(store)).toEqual({ [first]: "R1", [second]: "R2" });
    expect(
      store
        .getState()
        .schematic.components.some((component) => component.id === source),
    ).toBe(false);
    store.redo();
    expect(designators(store)[source]).toBe("G1");
  });

  it("takes a freed number again, and leaves an undesignated part out of the documentation", () => {
    const store = newStore();
    const first = place(store, "resistor");
    place(store, "resistor");
    store.removeComponent(first);
    expect(Object.values(designators(store))).toEqual(["R2"]);
    const again = place(store, "resistor");
    expect(designators(store)[again]).toBe("R3");

    const plain = place(store, "resistor", false);
    expect(designators(store)[plain]).toBeUndefined();
  });

  it("removes the name with the part in the same history entry", () => {
    const store = newStore();
    const named = place(store, "contactor");
    expect(designators(store)[named]).toBe("Q1");

    store.removeComponent(named);
    expect(designators(store)).toEqual({});
    store.undo();
    expect(designators(store)[named]).toBe("Q1");
    store.redo();
    expect(designators(store)).toEqual({});
  });

  it("renames with one history entry that undoes, redoes and marks the document modified", () => {
    const store = newStore();
    expect(store.getState().isDirty).toBe(false);

    store.setDesignator(R1, "R7");
    expect(designators(store)).toEqual({ [R1]: "R7" });
    expect(store.designatorOf(R1)).toBe("R7");
    expect(store.getState().isDirty).toBe(true);

    store.setDesignator(R1, "RV1");
    expect(designators(store)).toEqual({ [R1]: "RV1" });

    store.undo();
    expect(designators(store)).toEqual({ [R1]: "R7" });
    store.undo();
    expect(designators(store)).toEqual({});
    expect(store.getState().isDirty).toBe(false);
    store.redo();
    store.redo();
    expect(designators(store)).toEqual({ [R1]: "RV1" });
  });

  it("refuses a bad or duplicate name and leaves the document and history alone", () => {
    const store = newStore();
    store.setDesignator(R1, "R1");
    const before = store.getState().schematic;

    expect(() => store.setDesignator(R2, "r2")).toThrow(/capital letters/);
    expect(() => store.setDesignator(R2, "")).toThrow(/empty/);
    expect(() => store.setDesignator(R2, "R1")).toThrow(/already/);
    expect(() =>
      store.setDesignator("00000000-0000-4000-8000-0000000000ff", "R9"),
    ).toThrow(/does not exist/);
    expect(store.getState().schematic).toBe(before);

    // Keeping its own name is not a duplicate, and adds no history entry.
    expect(() => store.setDesignator(R1, "R1")).not.toThrow();
    store.undo();
    expect(designators(store)).toEqual({});
  });

  it("is a documentation edit: the circuit is the very same object", () => {
    const store = newStore();
    const before = store.getState().schematic;
    store.setDesignator(V1, "G1");
    const after = store.getState().schematic;

    expect(after).not.toBe(before);
    expect(hasSameElectricalPart(before, after)).toBe(true);
    place(store, "resistor", false);
    expect(hasSameElectricalPart(after, store.getState().schematic)).toBe(
      false,
    );
  });

  it("shows a renamed part under its new name, and the old one after undo", () => {
    const store = newStore();
    expect(store.getState().labels.get(R1)).toBe("R1");
    store.setDesignator(R1, "RV3");
    expect(store.getState().labels.get(R1)).toBe("RV3");
    store.undo();
    expect(store.getState().labels.get(R1)).toBe("R1");
  });
});

describe("display names with stored designators", () => {
  it("shows a stored designator and skips it for automatic names", () => {
    const store = newStore();
    // R2 is named R1, so the part that was shown as R1 must not be shown as R1 as well.
    store.setDesignator(R2, "R1");
    const labels = store.getState().labels;

    expect(labels.get(R2)).toBe("R1");
    expect(labels.get(R1)).not.toBe("R1");
    const shown = [...labels.values()];
    expect(new Set(shown).size).toBe(shown.length);
  });

  it("keeps the last name of a part that is gone", () => {
    const store = newStore();
    const named = place(store, "contactor");
    expect(store.getState().labels.get(named)).toBe("Q1");
    store.removeComponent(named);
    expect(store.getState().labels.get(named)).toBe("Q1");
  });

  it("hands out the same map until a name changes", () => {
    const names = new ComponentLabels();
    const first = names.labelsFor(dividerSchematic);
    expect(names.labelsFor(dividerSchematic)).toBe(first);
  });
});
