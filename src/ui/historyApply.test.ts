import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { copySelection } from "../editor/clipboard";
import { commandForIntent } from "../editor/commands";
import { CanvasInteraction } from "../editor/interaction";
import { buildScene } from "../editor/scene";
import { toScreen } from "../editor/view";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import { SchematicDocumentStore } from "../store/document";
import { dividerSchematic } from "../store/fixtures";
import { symbols } from "../symbols";
import { applyEditorCommand, setRestrictedParameter } from "./editorCommands";

// The labels the editor puts on history entries (docs/contracts/stage-16-editor-improvements.md
// section 6), and the view centering of canvas search (section 7).

const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";

let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xd200 + counter++).toString(16).padStart(12, "0")}`;
const createStore = () => new SchematicDocumentStore(dividerSchematic, nextId);
const lastLabel = (store: SchematicDocumentStore) =>
  store.getState().history.past.at(-1);
const part = (id: string) => ({ kind: "component", id }) as const;

describe("labels on the entries the editor records", () => {
  it("labels a group move, a group delete and a turn with the number of objects", () => {
    const store = createStore();
    const targets = [part(R1), part(R2)];
    const intents = [
      { kind: "moveSelection", targets, delta: { x: 0, y: 2 } },
      { kind: "turnSelection", targets, turn: "half" },
      { kind: "turnSelection", targets, turn: "mirror" },
      { kind: "deleteSelection", targets },
    ] as const;

    for (const intent of intents) {
      const command = commandForIntent(
        intent,
        store.getState().schematic,
        symbols,
      )!;
      applyEditorCommand(store, command, catalog);
    }

    expect(store.getState().history.past).toEqual([
      { key: "history.move", values: { count: 2 } },
      { key: "history.turn", values: { count: 2 } },
      { key: "history.mirror", values: { count: 2 } },
      { key: "history.deleteSelection", values: { count: 2 } },
    ]);
  });

  it("labels a placed part with the name of its type, and a paste with its object count", () => {
    const store = createStore();

    applyEditorCommand(
      store,
      {
        kind: "placeComponent",
        typeId: "resistor",
        position: { x: 0, y: 8 },
        rotationDeg: 0,
      },
      catalog,
    );
    expect(lastLabel(store)).toEqual({
      key: "history.place",
      values: { name: expect.any(String) },
    });

    const copy = copySelection(
      store.getState().schematic,
      [
        part(R1),
        part(R2),
        { kind: "wire", id: "00000000-0000-4000-8000-000000000005" },
      ],
      symbols,
    )!;
    applyEditorCommand(
      store,
      { kind: "paste", schematic: copy, delta: { x: 0, y: 12 } },
      catalog,
    );
    expect(lastLabel(store)).toEqual({
      key: "history.paste",
      values: { count: 3 },
    });
  });

  it("labels a parameter edit with the part and the parameter", () => {
    const store = createStore();

    setRestrictedParameter(store, R1, "resistanceOhms", 220, catalog, null);

    expect(lastLabel(store)).toEqual({
      key: "history.setParameter",
      values: { part: "R1", parameter: "resistanceOhms" },
    });
  });

  it("labels a designator edit", () => {
    const store = createStore();

    store.setDesignator(R1, "R7");

    expect(lastLabel(store)).toEqual({
      key: "history.setDesignator",
      values: { part: "R7" },
    });
  });

  it("leaves a refused command out of the history", () => {
    const store = createStore();

    expect(() =>
      applyEditorCommand(
        store,
        {
          kind: "deleteComponent",
          componentId: "00000000-0000-4000-8000-0000000000ff",
        },
        catalog,
      ),
    ).toThrow();

    expect(store.getState().history.past).toEqual([]);
  });
});

describe("centering the view on a search result", () => {
  function setup() {
    const interaction = new CanvasInteraction();
    interaction.setViewport({ width: 800, height: 600 });
    interaction.setScene(buildScene(dividerSchematic, symbols));
    return interaction;
  }

  it("puts the center of the object at the center of the view and keeps the zoom", () => {
    const interaction = setup();
    const scale = interaction.view.scale;

    interaction.centerOn({ minX: 10, minY: 20, maxX: 14, maxY: 22 });

    expect(interaction.view.scale).toBe(scale);
    const center = toScreen({ x: 12, y: 21 }, interaction.view);
    expect(center.x).toBeCloseTo(400);
    expect(center.y).toBeCloseTo(300);
  });

  it("does not turn on the fit to the circuit again", () => {
    const interaction = setup();

    interaction.centerOn({ minX: 100, minY: 100, maxX: 101, maxY: 101 });
    interaction.setViewport({ width: 900, height: 700 });

    // A resize keeps the centered view: the user's view, not the fitted one.
    expect(toScreen({ x: 100.5, y: 100.5 }, interaction.view).x).toBeCloseTo(
      400,
    );
  });
});
