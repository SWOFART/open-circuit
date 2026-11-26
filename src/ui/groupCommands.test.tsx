import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { commandForIntent, type EditorCommand } from "../editor/commands";
import {
  CanvasInteraction,
  type InteractionIntent,
} from "../editor/interaction";
import { buildScene } from "../editor/scene";
import { toScreen } from "../editor/view";
import { moveSelection } from "../editor/selectionOps";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import { SchematicDocumentStore } from "../store/document";
import { dividerSchematic } from "../store/fixtures";
import type { EditorRestrictions } from "../store/studentAssignment";
import { symbols } from "../symbols";
import { applyEditorCommand, applyRestrictedCommand } from "./editorCommands";
import { SelectionSummary, selectionSummaryText } from "./SelectionSummary";

// A group is one transaction (docs/contracts/stage-16-editor-improvements.md section 3): one
// history entry, all or nothing, and an assignment refuses the whole group when it refuses a part.

const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const WIRE_5 = "00000000-0000-4000-8000-000000000005";
let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xc000 + counter++).toString(16).padStart(12, "0")}`;

function createStore(): SchematicDocumentStore {
  return new SchematicDocumentStore(dividerSchematic, nextId);
}

const groupMove = (store: SchematicDocumentStore): EditorCommand => {
  const move = moveSelection(
    store.getState().schematic,
    [
      { kind: "component", id: R1 },
      { kind: "component", id: R2 },
      { kind: "wire", id: WIRE_5 },
    ],
    { x: 2, y: 3 },
    symbols,
  );
  if (!move) throw new Error("no move");
  return { kind: "batch", commands: move.commands };
};

describe("a group applied to the document store", () => {
  it("is one history entry: one undo restores the document exactly, one redo repeats it", () => {
    const store = createStore();
    const before = store.getState().schematic;

    applyEditorCommand(store, groupMove(store), catalog);

    const moved = store.getState().schematic;
    expect(moved).not.toEqual(before);
    expect(moved.components.find((item) => item.id === R1)!.position).toEqual({
      x: 6,
      y: 3,
    });

    expect(store.undo()).toBe(true);
    expect(store.getState().schematic).toEqual(before);
    // A single entry: nothing is left to undo.
    expect(store.getState().canUndo).toBe(false);

    expect(store.redo()).toBe(true);
    expect(store.getState().schematic).toEqual(moved);
  });

  it("notifies listeners once, after the whole group has been applied", () => {
    const store = createStore();
    let notifications = 0;
    store.subscribe(() => {
      notifications += 1;
    });

    applyEditorCommand(store, groupMove(store), catalog);

    expect(notifications).toBe(1);
  });

  it("is a group delete in one entry too", () => {
    const store = createStore();

    applyEditorCommand(
      store,
      {
        kind: "batch",
        commands: [
          { kind: "deleteComponent", componentId: R1 },
          { kind: "deleteWire", wireId: WIRE_5 },
        ],
      },
      catalog,
    );
    expect(store.getState().schematic.components).toHaveLength(2);

    store.undo();
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
  });

  it("applies nothing, and records nothing, when one part of the group is refused", () => {
    const store = createStore();
    const before = store.getState();

    expect(() =>
      applyEditorCommand(
        store,
        {
          kind: "batch",
          commands: [
            { kind: "deleteComponent", componentId: R1 },
            // The second command names a part that does not exist: the store refuses it.
            {
              kind: "deleteComponent",
              componentId: "00000000-0000-4000-8000-0000000000ff",
            },
          ],
        },
        catalog,
      ),
    ).toThrow();

    expect(store.getState().schematic).toEqual(before.schematic);
    expect(store.getState().canUndo).toBe(false);
    expect(store.getState().isDirty).toBe(false);
  });

  it("keeps the document usable after a refused group", () => {
    const store = createStore();
    expect(() =>
      applyEditorCommand(
        store,
        {
          kind: "batch",
          commands: [
            {
              kind: "deleteComponent",
              componentId: "00000000-0000-4000-8000-0000000000ff",
            },
          ],
        },
        catalog,
      ),
    ).toThrow();

    applyEditorCommand(
      store,
      { kind: "deleteComponent", componentId: R1 },
      catalog,
    );
    expect(store.undo()).toBe(true);
    expect(store.getState().schematic).toEqual(dividerSchematic);
  });

  it("clears the redo stack like any other edit", () => {
    const store = createStore();
    applyEditorCommand(store, groupMove(store), catalog);
    store.undo();
    expect(store.getState().canRedo).toBe(true);

    applyEditorCommand(store, groupMove(store), catalog);

    expect(store.getState().canRedo).toBe(false);
  });

  it("joins an outer transaction when it is nested", () => {
    const store = createStore();

    store.transact(() => {
      applyEditorCommand(store, groupMove(store), catalog);
      store.transact(() => {
        store.removeWire(WIRE_5);
      });
    });

    store.undo();
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
  });
});

describe("a group under an assignment's restrictions", () => {
  const restrictions: EditorRestrictions = {
    allowedTypes: null,
    locked: new Set([R1]),
    findFault: false,
  };

  it("refuses to delete a group that holds a locked part, and says how many", () => {
    const store = createStore();
    const before = store.getState();

    const verdict = applyRestrictedCommand(
      store,
      {
        kind: "batch",
        commands: [
          { kind: "deleteComponent", componentId: R1 },
          { kind: "deleteComponent", componentId: R2 },
        ],
      },
      catalog,
      restrictions,
    );

    expect(verdict).toEqual({
      allowed: false,
      reason:
        "1 selected part is locked by the assignment, so nothing was deleted.",
    });
    // All or nothing: the unlocked part is still there.
    expect(store.getState()).toBe(before);
  });

  it("counts several locked parts", () => {
    const store = createStore();

    const verdict = applyRestrictedCommand(
      store,
      {
        kind: "batch",
        commands: [
          { kind: "deleteComponent", componentId: R1 },
          { kind: "deleteComponent", componentId: R2 },
        ],
      },
      catalog,
      { ...restrictions, locked: new Set([R1, R2]) },
    );

    expect(verdict).toMatchObject({
      allowed: false,
      reason:
        "2 selected parts are locked by the assignment, so nothing was deleted.",
    });
  });

  it("lets a locked part be moved with the group: a lock forbids deleting, not placing", () => {
    const store = createStore();

    const verdict = applyRestrictedCommand(
      store,
      groupMove(store),
      catalog,
      restrictions,
    );

    expect(verdict.allowed).toBe(true);
    expect(
      store.getState().schematic.components.find((item) => item.id === R1)!
        .position,
    ).toEqual({ x: 6, y: 3 });
  });

  it("deletes a group with no locked part", () => {
    const store = createStore();

    const verdict = applyRestrictedCommand(
      store,
      {
        kind: "batch",
        commands: [
          { kind: "deleteComponent", componentId: R2 },
          { kind: "deleteWire", wireId: WIRE_5 },
        ],
      },
      catalog,
      restrictions,
    );

    expect(verdict.allowed).toBe(true);
    expect(store.getState().schematic.components).toHaveLength(2);
  });
});

describe("the selection summary", () => {
  const part = { kind: "component", id: R1 } as const;
  const other = { kind: "component", id: R2 } as const;
  const wire = { kind: "wire", id: WIRE_5 } as const;

  it("says nothing for no object or one", () => {
    expect(selectionSummaryText([])).toBe("");
    expect(selectionSummaryText([part])).toBe("");
  });

  it("counts the objects by kind", () => {
    expect(selectionSummaryText([part, other, wire])).toBe(
      "2 parts, 1 wire selected",
    );
    expect(
      selectionSummaryText([
        wire,
        { kind: "rail", id: R1 },
        { kind: "referenceMarker", id: R2 },
        { kind: "note", id: WIRE_5 },
      ]),
    ).toBe("1 wire, 1 ground marker, 1 rail, 1 note selected");
  });

  it("is a polite live region, present even when empty", () => {
    const empty = renderToStaticMarkup(<SelectionSummary selections={[]} />);
    const several = renderToStaticMarkup(
      <SelectionSummary selections={[part, other]} />,
    );

    expect(empty).toContain('role="status"');
    expect(empty).toContain('aria-live="polite"');
    expect(several).toContain("2 parts selected");
  });
});

describe("a drag of a group, from the pointer to the document", () => {
  it("moves the selected parts together and is undone in one step", () => {
    const store = createStore();
    const intents: InteractionIntent[] = [];
    const interaction = new CanvasInteraction(
      () => {},
      (intent) => intents.push(intent),
    );
    interaction.setViewport({ width: 800, height: 600 });
    interaction.setScene(buildScene(store.getState().schematic, symbols));
    interaction.selectMany([
      { kind: "component", id: R1 },
      { kind: "component", id: R2 },
    ]);

    const pointer = (x: number, y: number) => ({
      ...toScreen({ x, y }, interaction.view),
      button: 0,
      pointerId: 1,
    });
    interaction.pointerDown(pointer(4, 0));
    interaction.pointerMove(pointer(4, 2));
    interaction.pointerUp(pointer(4, 2));

    const [intent] = intents;
    if (!intent) throw new Error("no intent");
    const command = commandForIntent(
      intent,
      store.getState().schematic,
      symbols,
    );
    if (!command) throw new Error("no command");
    applyEditorCommand(store, command, catalog);

    const positions = store
      .getState()
      .schematic.components.map((item) => [item.id, item.position.y]);
    expect(positions).toEqual([
      ["00000000-0000-4000-8000-000000000001", 0],
      [R1, 2],
      [R2, 2],
    ]);

    store.undo();
    expect(store.getState().schematic).toEqual(dividerSchematic);
  });
});
