import { describe, expect, it } from "vitest";
import { resolvePinPositions } from "../editor/geometry";
import { moveSelection } from "../editor/selectionOps";
import { symbols } from "../symbols";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";

// Transactions (docs/contracts/stage-16-editor-improvements.md section 3) on the divider:
// R1 at (4, 0) has pins at 3 and 5, R2 at (8, 0) has pins at 7 and 9.

const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const WIRE_5 = "00000000-0000-4000-8000-000000000005";

let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xd000 + counter++).toString(16).padStart(12, "0")}`;

const createStore = () => new SchematicDocumentStore(dividerSchematic, nextId);

describe("SchematicDocumentStore.transact", () => {
  it("records several edits as one entry, undone and redone together", () => {
    const store = createStore();

    store.transact(() => {
      store.moveComponent(R1, { x: 4, y: 3 });
      store.moveComponent(R2, { x: 8, y: 3 });
    });
    const moved = store.getState().schematic;

    expect(store.undo()).toBe(true);
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
    expect(store.redo()).toBe(true);
    expect(store.getState().schematic).toEqual(moved);
  });

  it("restores the document and records nothing when an edit throws", () => {
    const store = createStore();

    expect(() =>
      store.transact(() => {
        store.moveComponent(R1, { x: 4, y: 3 });
        store.removeWire("00000000-0000-4000-8000-0000000000ff");
      }),
    ).toThrow();

    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
    store.moveComponent(R1, { x: 4, y: 1 });
    expect(store.getState().canUndo).toBe(true);
  });

  it("joins an outer transaction when nested", () => {
    const store = createStore();

    store.transact(() => {
      store.transact(() => store.moveComponent(R1, { x: 4, y: 3 }));
      store.moveComponent(R2, { x: 8, y: 3 });
    });

    store.undo();
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
  });

  it("records nothing for a transaction that changes nothing", () => {
    const store = createStore();

    store.transact(() => {});

    expect(store.getState().canUndo).toBe(false);
  });
});

describe("terminals in a transaction", () => {
  const pinOf = (store: SchematicDocumentStore, id: string, index: number) => {
    const component = store
      .getState()
      .schematic.components.find((item) => item.id === id)!;
    return resolvePinPositions(component, symbols)[index]!.position;
  };

  it("follow once, after the last edit, so a part can move onto another's terminal", () => {
    const store = createStore();
    // R1's first pin is at 3, R2's at 7: a move of 4 puts R1's pin where R2's was.
    const a = store.addTerminal(pinOf(store, R1, 0), "X1", "1");
    const b = store.addTerminal(pinOf(store, R2, 0), "X1", "2");
    const move = moveSelection(
      store.getState().schematic,
      [
        { kind: "component", id: R1 },
        { kind: "component", id: R2 },
        { kind: "wire", id: WIRE_5 },
      ],
      { x: 4, y: 0 },
      symbols,
    )!;

    store.transact(() => {
      move.commands.forEach((command) => {
        if (command.kind !== "transformComponent") return;
        store.transformComponent(
          command.componentId,
          command.position,
          command.rotationDeg,
          command.wires,
        );
      });
    });

    const terminals = store.getState().schematic.documentation.terminals;
    expect(terminals.find((item) => item.id === a)!.position).toEqual(
      pinOf(store, R1, 0),
    );
    expect(terminals.find((item) => item.id === b)!.position).toEqual(
      pinOf(store, R2, 0),
    );

    // One undo reverses the whole move; the two added terminals stay.
    store.undo();
    expect(store.getState().schematic.components).toEqual(
      dividerSchematic.components,
    );
    expect(store.getState().schematic.documentation.terminals).toHaveLength(2);
  });

  it("still refuse a transaction that really leaves two terminals on one point", () => {
    const store = createStore();
    store.addTerminal({ x: 5, y: 0 }, "X1", "1");
    store.addTerminal({ x: 7, y: 0 }, "X1", "2");

    // Only R1 moves, onto R2's first pin: the final drawing has the collision.
    expect(() =>
      store.transact(() => store.moveComponent(R1, { x: 6, y: 0 })),
    ).toThrow("two terminals");
    expect(store.getState().schematic.components).toEqual(
      dividerSchematic.components,
    );
    expect(store.getState().canUndo).toBe(true);
  });
});
