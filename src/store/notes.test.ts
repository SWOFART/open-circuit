import { describe, expect, it } from "vitest";
import { MAX_NOTES } from "../editor/noteRules";
import { hasSameElectricalPart } from "./simulation";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";

function newStore(): SchematicDocumentStore {
  let next = 0x100;
  return new SchematicDocumentStore(
    dividerSchematic,
    () => `00000000-0000-4000-8000-${(next++).toString(16).padStart(12, "0")}`,
  );
}

const notes = (store: SchematicDocumentStore) =>
  store.getState().schematic.documentation.notes;

describe("notes in the document", () => {
  it("adds, moves, edits and removes a note, one history entry each", () => {
    const store = newStore();
    const id = store.addNote({ x: 4, y: 6 }, "Main supply");
    expect(notes(store)).toEqual([
      { id, position: { x: 4, y: 6 }, text: "Main supply" },
    ]);

    store.moveNote(id, { x: 8, y: 6 });
    store.setNoteText(id, "Hauptschalter\nNotaus");
    expect(notes(store)[0]).toEqual({
      id,
      position: { x: 8, y: 6 },
      text: "Hauptschalter\nNotaus",
    });

    store.removeNote(id);
    expect(notes(store)).toEqual([]);

    store.undo();
    expect(notes(store)[0]?.text).toBe("Hauptschalter\nNotaus");
    store.undo();
    expect(notes(store)[0]).toMatchObject({ position: { x: 8, y: 6 } });
    store.undo();
    expect(notes(store)[0]).toMatchObject({
      position: { x: 4, y: 6 },
      text: "Main supply",
    });
    store.undo();
    expect(notes(store)).toEqual([]);
    expect(store.getState().isDirty).toBe(false);

    for (let i = 0; i < 4; i += 1) store.redo();
    expect(notes(store)).toEqual([]);
    expect(store.getState().canRedo).toBe(false);
  });

  it("refuses a text outside the rules and leaves the document and history alone", () => {
    const store = newStore();
    const id = store.addNote({ x: 0, y: 0 }, "ok");
    const before = store.getState().schematic;

    expect(() => store.addNote({ x: 0, y: 0 }, "")).toThrow(/empty/);
    expect(() => store.setNoteText(id, "x".repeat(501))).toThrow(/500/);
    expect(() => store.setNoteText(id, "a\n".repeat(11))).toThrow(/lines/);
    expect(() => store.setNoteText(id, "a\u0007")).toThrow(/control/);
    expect(() => store.setNoteText(id, "текст")).not.toThrow();
    store.undo();
    expect(store.getState().schematic).toEqual(before);
  });

  it("refuses a note that does not exist, and a coordinate off the grid", () => {
    const store = newStore();
    const missing = "00000000-0000-4000-8000-0000000000ff";

    expect(() => store.moveNote(missing, { x: 0, y: 0 })).toThrow(
      /does not exist/,
    );
    expect(() => store.setNoteText(missing, "x")).toThrow(/does not exist/);
    expect(() => store.removeNote(missing)).toThrow(/does not exist/);
    expect(() => store.addNote({ x: 0.5, y: 0 }, "x")).toThrow(
      /Grid coordinate/,
    );
  });

  it("holds at most the limit of notes", () => {
    const store = newStore();
    for (let i = 0; i < MAX_NOTES; i += 1) store.addNote({ x: i, y: 0 }, "n");
    const before = store.getState().schematic;

    expect(() => store.addNote({ x: 0, y: 1 }, "one too many")).toThrow(
      /at most 200/,
    );
    expect(store.getState().schematic).toBe(before);
  });

  it("is documentation: no note edit touches the circuit", () => {
    const store = newStore();
    const circuit = store.getState().schematic;
    const id = store.addNote({ x: 1, y: 1 }, "a");
    store.moveNote(id, { x: 2, y: 2 });
    store.setNoteText(id, "b");
    store.removeNote(id);

    expect(hasSameElectricalPart(circuit, store.getState().schematic)).toBe(
      true,
    );
  });

  it("shares the one ID space of the document", () => {
    const store = newStore();
    const id = store.addNote({ x: 0, y: 0 }, "x");
    const others = [
      ...store.getState().schematic.components.map((item) => item.id),
      ...store.getState().schematic.wires.map((item) => item.id),
    ];

    expect(others).not.toContain(id);
    expect(
      () =>
        new SchematicDocumentStore(
          {
            ...dividerSchematic,
            documentation: {
              ...dividerSchematic.documentation,
              notes: [
                {
                  id: dividerSchematic.components[0]!.id,
                  position: { x: 0, y: 0 },
                  text: "x",
                },
              ],
            },
          },
          () => id,
        ),
    ).toThrow(/duplicated/);
  });
});
