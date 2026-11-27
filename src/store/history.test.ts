import { describe, expect, it } from "vitest";
import { HISTORY_LIMIT } from "../editor/historyLabel";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";

// The history of docs/contracts/stage-16-editor-improvements.md section 6: a label with each
// entry, going to an entry by ordinary undo and redo steps, and a bound of 1000 entries.

const R1 = "00000000-0000-4000-8000-000000000002";

let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xd100 + counter++).toString(16).padStart(12, "0")}`;
const createStore = () => new SchematicDocumentStore(dividerSchematic, nextId);
const labelOf = (key: string, values?: Record<string, string | number>) => ({
  key,
  ...(values ? { values } : {}),
});

/** Three edits, each a move of R1, with labels A, B and C. */
function threeMoves(store: SchematicDocumentStore): void {
  for (const [name, y] of [
    ["A", 1],
    ["B", 2],
    ["C", 3],
  ] as const) {
    store.labelled(labelOf(`history.${name}`), () =>
      store.moveComponent(R1, { x: 4, y }),
    );
  }
}
const yOf = (store: SchematicDocumentStore) =>
  store.getState().schematic.components.find(({ id }) => id === R1)!.position.y;

describe("history labels", () => {
  it("gives an edit the label it was made under, and none otherwise", () => {
    const store = createStore();

    store.labelled(labelOf("history.move", { count: 2 }), () =>
      store.moveComponent(R1, { x: 4, y: 1 }),
    );
    store.moveComponent(R1, { x: 4, y: 2 });

    expect(store.getState().history.past).toEqual([
      { key: "history.move", values: { count: 2 } },
      null,
    ]);
  });

  it("gives a transaction one label, and an inner label does not replace the outer one", () => {
    const store = createStore();

    store.labelled(labelOf("history.outer"), () =>
      store.transact(() => {
        store.labelled(labelOf("history.inner"), () =>
          store.moveComponent(R1, { x: 4, y: 1 }),
        );
        store.moveComponent(R1, { x: 4, y: 2 });
      }),
    );

    expect(store.getState().history.past).toEqual([labelOf("history.outer")]);
  });

  it("does not let a label outlive its edit", () => {
    const store = createStore();
    store.labelled(labelOf("history.first"), () =>
      store.moveComponent(R1, { x: 4, y: 1 }),
    );

    store.moveComponent(R1, { x: 4, y: 2 });

    expect(store.getState().history.past[1]).toBeNull();
  });

  it("never saves a label", () => {
    const store = createStore();
    store.labelled(labelOf("history.move"), () =>
      store.moveComponent(R1, { x: 4, y: 1 }),
    );

    expect(JSON.stringify(store.getState().schematic)).not.toContain("history");
  });

  it("records no entry for an edit the store refuses", () => {
    const store = createStore();

    expect(() =>
      store.labelled(labelOf("history.delete"), () =>
        store.removeComponent("00000000-0000-4000-8000-0000000000ff"),
      ),
    ).toThrow();

    expect(store.getState().history).toEqual({
      past: [],
      future: [],
      dropped: false,
    });
  });
});

describe("the history summary", () => {
  it("lists the past oldest first and the undone entries with the next redo first", () => {
    const store = createStore();
    threeMoves(store);
    store.undo();
    store.undo();

    const { past, future } = store.getState().history;
    expect(past.map((label) => label?.key)).toEqual(["history.A"]);
    expect(future.map((label) => label?.key)).toEqual([
      "history.B",
      "history.C",
    ]);
  });

  it("forgets the undone entries when a new edit is made", () => {
    const store = createStore();
    threeMoves(store);
    store.undo();

    store.moveComponent(R1, { x: 4, y: 9 });

    expect(store.getState().history.future).toEqual([]);
  });
});

describe("going to an entry", () => {
  it("undoes back to the state just after an older entry, by ordinary steps", () => {
    const store = createStore();
    threeMoves(store);
    let notifications = 0;
    store.subscribe(() => (notifications += 1));

    expect(store.goToHistory(0)).toBe(true);

    expect(yOf(store)).toBe(1);
    expect(notifications).toBe(2);
    expect(store.getState().history.past).toHaveLength(1);
    expect(store.getState().history.future).toHaveLength(2);
    expect(store.getState().canRedo).toBe(true);
  });

  it("redoes forward to an undone entry", () => {
    const store = createStore();
    threeMoves(store);
    store.undo();
    store.undo();
    store.undo();

    store.goToHistory(1);

    expect(yOf(store)).toBe(2);
    expect(store.getState().history.past).toHaveLength(2);
  });

  it("does nothing for the entry that is already current", () => {
    const store = createStore();
    threeMoves(store);
    let notifications = 0;
    store.subscribe(() => (notifications += 1));

    expect(store.goToHistory(2)).toBe(true);

    expect(yOf(store)).toBe(3);
    expect(notifications).toBe(0);
  });

  it("refuses an index that is not in the list", () => {
    const store = createStore();
    threeMoves(store);

    expect(store.goToHistory(-1)).toBe(false);
    expect(store.goToHistory(3)).toBe(false);
    expect(store.goToHistory(1.5)).toBe(false);
    expect(yOf(store)).toBe(3);
  });

  it("leaves the dirty flag to the comparison with the saved document", () => {
    const store = createStore();
    threeMoves(store);
    store.markSaved();
    expect(store.getState().isDirty).toBe(false);

    store.goToHistory(0);
    expect(store.getState().isDirty).toBe(true);

    store.goToHistory(2);
    expect(store.getState().isDirty).toBe(false);
  });
});

describe("the bound of the history", () => {
  function fill(store: SchematicDocumentStore, count: number): void {
    for (let step = 1; step <= count; step += 1) {
      store.labelled(labelOf("history.step", { count: step }), () =>
        store.moveComponent(R1, { x: 4, y: step }),
      );
    }
  }

  it("keeps 1000 entries and drops the oldest first", () => {
    const store = createStore();

    fill(store, HISTORY_LIMIT + 5);

    const { past, dropped } = store.getState().history;
    expect(past).toHaveLength(HISTORY_LIMIT);
    expect(dropped).toBe(true);
    // Entry 6 is now the oldest: five were dropped.
    expect(past[0]).toEqual(labelOf("history.step", { count: 6 }));
    expect(past[HISTORY_LIMIT - 1]).toEqual(
      labelOf("history.step", { count: HISTORY_LIMIT + 5 }),
    );
  });

  it("does not say entries were dropped before the bound is reached", () => {
    const store = createStore();

    fill(store, HISTORY_LIMIT);

    expect(store.getState().history.dropped).toBe(false);
  });

  it("keeps a document clean or dirty by its content, not by the stack, when the save point is dropped", () => {
    const store = createStore();
    fill(store, 3);
    store.markSaved();

    // Move away and then back to the saved place: the save point's entry is dropped, yet the
    // document equals the saved one.
    fill(store, HISTORY_LIMIT + 10);
    store.moveComponent(R1, { x: 4, y: 3 });
    expect(store.getState().isDirty).toBe(false);

    store.moveComponent(R1, { x: 4, y: 4 });
    expect(store.getState().isDirty).toBe(true);
  });

  it("starts afresh with a new document", () => {
    const store = createStore();
    fill(store, HISTORY_LIMIT + 1);

    store.replaceDocument(dividerSchematic);

    expect(store.getState().history).toEqual({
      past: [],
      future: [],
      dropped: false,
    });
  });
});
