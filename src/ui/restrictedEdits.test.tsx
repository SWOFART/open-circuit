import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import rcCharge from "../../tests/golden/stage-3-rc-charge.schematic.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import { SchematicDocumentStore } from "../store/document";
import { componentLabels, dividerSchematic } from "../store/fixtures";
import {
  initialSimulationState,
  type SimulationState,
} from "../store/simulation";
import type { EditorRestrictions } from "../store/studentAssignment";
import {
  applyRestrictedCommand,
  clearRestrictedParameter,
  setRestrictedParameter,
} from "./editorCommands";
import { Inspector } from "./Inspector";

const catalog = catalogFixture as ComponentCatalog;
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const CAPACITOR = "00000000-0000-4000-8000-000000000003";

const restrictions: EditorRestrictions = {
  allowedTypes: ["resistor"],
  locked: new Set([R1]),
};

describe("commands under an assignment's restrictions", () => {
  it("refuses to delete a locked part and leaves the document exactly as it was", () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    const before = store.getState();

    const verdict = applyRestrictedCommand(
      store,
      { kind: "deleteComponent", componentId: R1 },
      catalog,
      restrictions,
    );

    expect(verdict).toEqual({
      allowed: false,
      reason: "That part is locked by the assignment and cannot be deleted.",
    });
    expect(store.getState()).toBe(before);
  });

  it("deletes a part that is not locked, as the store would", () => {
    const store = new SchematicDocumentStore(dividerSchematic);

    const verdict = applyRestrictedCommand(
      store,
      { kind: "deleteComponent", componentId: R2 },
      catalog,
      restrictions,
    );

    expect(verdict.allowed).toBe(true);
    expect(
      store.getState().schematic.components.map((component) => component.id),
    ).toEqual([V1, R1]);
  });

  it("refuses to place a part type the assignment does not allow, and places one it does", () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    const before = store.getState();
    const place = (typeId: "fuse" | "resistor") =>
      applyRestrictedCommand(
        store,
        {
          kind: "placeComponent",
          typeId,
          position: { x: 20, y: 20 },
          rotationDeg: 0,
        },
        catalog,
        restrictions,
      );

    expect(place("fuse").allowed).toBe(false);
    expect(store.getState()).toBe(before);
    expect(place("resistor").allowed).toBe(true);
    expect(store.getState().schematic.components).toHaveLength(4);
  });

  it("lets a locked part be moved, with the document's own rules deciding the rest", () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    const part = dividerSchematic.components.find((c) => c.id === R1)!;

    const verdict = applyRestrictedCommand(
      store,
      {
        kind: "transformComponent",
        componentId: R1,
        position: { x: part.position.x + 2, y: part.position.y },
        rotationDeg: part.rotationDeg,
        wires: [],
      },
      catalog,
      restrictions,
    );

    expect(verdict.allowed).toBe(true);
    expect(
      store.getState().schematic.components.find((c) => c.id === R1)?.position,
    ).toEqual({ x: part.position.x + 2, y: part.position.y });
  });

  it("still lets the store refuse a command the assignment allows", () => {
    const store = new SchematicDocumentStore(dividerSchematic);

    expect(() =>
      applyRestrictedCommand(
        store,
        {
          kind: "deleteComponent",
          componentId: "00000000-0000-4000-8000-0000000000ee",
        },
        catalog,
        restrictions,
      ),
    ).toThrow();
  });

  it("changes nothing for a document without an assignment", () => {
    const store = new SchematicDocumentStore(dividerSchematic);

    const verdict = applyRestrictedCommand(
      store,
      { kind: "deleteComponent", componentId: R1 },
      catalog,
      null,
    );

    expect(verdict.allowed).toBe(true);
    expect(store.getState().schematic.components).toHaveLength(2);
  });
});

describe("parameter edits under an assignment's restrictions", () => {
  it("refuses to edit a locked part's parameter and leaves the document as it was", () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    const before = store.getState();

    const verdict = setRestrictedParameter(
      store,
      R1,
      "resistanceOhms",
      4700,
      catalog,
      restrictions,
    );

    expect(verdict).toEqual({
      allowed: false,
      reason: "That part is locked by the assignment and cannot be edited.",
    });
    expect(store.getState()).toBe(before);
  });

  it("edits the parameter of a part that is not locked", () => {
    const store = new SchematicDocumentStore(dividerSchematic);

    const verdict = setRestrictedParameter(
      store,
      R2,
      "resistanceOhms",
      4700,
      catalog,
      restrictions,
    );

    expect(verdict.allowed).toBe(true);
    expect(
      store.getState().schematic.components.find((c) => c.id === R2)
        ?.parameters,
    ).toEqual({ resistanceOhms: 4700 });
  });

  it("refuses to clear a locked part's optional parameter, and clears an unlocked part's", () => {
    const store = new SchematicDocumentStore(rcCharge as Schematic);
    const before = store.getState();

    const locked = clearRestrictedParameter(
      store,
      CAPACITOR,
      "initialVoltageVolts",
      catalog,
      { allowedTypes: null, locked: new Set([CAPACITOR]) },
    );
    expect(locked.allowed).toBe(false);
    expect(store.getState()).toBe(before);

    const open = clearRestrictedParameter(
      store,
      CAPACITOR,
      "initialVoltageVolts",
      catalog,
      restrictions,
    );
    expect(open.allowed).toBe(true);
    expect(
      store.getState().schematic.components.find((c) => c.id === CAPACITOR)
        ?.parameters,
    ).not.toHaveProperty("initialVoltageVolts");
  });
});

describe("the inspector under an assignment's restrictions", () => {
  const ready: SimulationState = {
    ...initialSimulationState(),
    status: "ready",
    catalog,
  };
  const render = (locked?: ReadonlySet<string>): string =>
    renderToStaticMarkup(
      <Inspector
        state={ready}
        schematic={dividerSchematic}
        labels={componentLabels(dividerSchematic)}
        lockedComponents={locked}
        onCommit={() => {}}
      />,
    );

  /** Which of the part's fields are disabled, by the part's label in the field's text. */
  function disabledFields(markup: string): Record<string, boolean[]> {
    const result: Record<string, boolean[]> = {};
    for (const match of markup.matchAll(
      /<label[^>]*>([A-Z0-9]+) [^<]*<\/label><input[^>]*>/g,
    )) {
      const [tag, label] = match;
      (result[label!] ??= []).push(/\bdisabled=""/.test(tag));
    }
    return result;
  }

  it("shows a locked part's parameters but disables them, and says why", () => {
    const markup = render(new Set([R1]));

    expect(markup).toContain(
      "Parts locked by the assignment cannot be edited.",
    );
    expect(disabledFields(markup)).toEqual({
      V1: [false, false],
      R1: [true],
      R2: [false],
    });
    // The values are still shown.
    expect(markup).toContain('value="1 kΩ"');
  });

  it("disables nothing and says nothing without locked parts", () => {
    for (const locked of [undefined, new Set<string>()]) {
      const markup = render(locked);

      expect(markup).not.toContain("locked by the assignment");
      expect(Object.values(disabledFields(markup)).flat()).not.toContain(true);
    }
  });
});
