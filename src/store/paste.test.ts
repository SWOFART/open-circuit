import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { copySelection, type ClipboardSchematic } from "../editor/clipboard";
import type { SelectionTarget } from "../editor/hitTest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";

// Paste (docs/contracts/stage-16-editor-improvements.md section 4.3): fresh IDs, one history
// entry, the whole paste or nothing.

const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const WIRE_4 = "00000000-0000-4000-8000-000000000004";
const RAIL = "00000000-0000-4000-8000-0000000000a2";
const NOTE = "00000000-0000-4000-8000-0000000000a4";

let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xe000 + counter++).toString(16).padStart(12, "0")}`;

const withExtras: Schematic = {
  ...dividerSchematic,
  rails: [
    {
      id: RAIL,
      name: "RAIL1",
      start: { x: 20, y: 0 },
      end: { x: 20, y: 10 },
    },
  ],
  documentation: {
    ...dividerSchematic.documentation,
    notes: [{ id: NOTE, position: { x: 2, y: 8 }, text: "Hello" }],
  },
};

const createStore = (schematic: Schematic = withExtras) =>
  new SchematicDocumentStore(schematic, nextId);

function copyOf(
  schematic: Schematic,
  targets: readonly SelectionTarget[],
): ClipboardSchematic {
  const copy = copySelection(schematic, targets, symbols);
  if (!copy) throw new Error("nothing copied");
  return copy;
}

const part = (id: string) => ({ kind: "component", id }) as const;

function idsOf(schematic: Schematic): string[] {
  return [
    ...schematic.components,
    ...schematic.wires,
    ...schematic.junctions,
    ...schematic.referenceMarkers,
    ...schematic.rails,
    ...schematic.documentation.notes,
  ].map(({ id }) => id);
}

describe("SchematicDocumentStore.pasteClipboard", () => {
  it("adds the parts and the wire between them moved by the offset, with fresh IDs", () => {
    const store = createStore();
    const copy = copyOf(dividerSchematic, [part(R1), part(R2)]);

    store.pasteClipboard(copy, { x: 0, y: 6 }, catalog);

    const after = store.getState().schematic;
    expect(after.components).toHaveLength(5);
    const added = after.components.slice(3);
    expect(added.map(({ position }) => position)).toEqual([
      { x: 4, y: 6 },
      { x: 8, y: 6 },
    ]);
    expect(added.map(({ typeId }) => typeId)).toEqual(["resistor", "resistor"]);
    // The wire between R1 and R2 came along, moved with them.
    expect(after.wires).toHaveLength(4);
    expect(after.wires[3]!.points).toEqual([
      { x: 5, y: 6 },
      { x: 7, y: 6 },
    ]);
  });

  it("never keeps an ID of its source, and a document never holds a duplicate", () => {
    const store = createStore();
    const copy = copyOf(dividerSchematic, [part(R1), part(R2)]);

    store.pasteClipboard(copy, { x: 0, y: 6 }, catalog);
    store.pasteClipboard(copy, { x: 0, y: 12 }, catalog);

    const ids = idsOf(store.getState().schematic);
    expect(new Set(ids).size).toBe(ids.length);
    const sourceIds = [R1, R2, "00000000-0000-4000-8000-000000000005"];
    const added = ids.slice(idsOf(withExtras).length);
    expect(added.some((id) => sourceIds.includes(id))).toBe(false);
  });

  it("gives a paste into another document IDs of that document, not the source's", () => {
    const source = createStore();
    const other = createStore({
      ...dividerSchematic,
      components: [],
      wires: [],
    });
    const copy = copyOf(source.getState().schematic, [part(R1)]);

    other.pasteClipboard(copy, { x: 1, y: 1 }, catalog);

    const [pasted] = other.getState().schematic.components;
    expect(pasted!.id).not.toBe(R1);
    expect(pasted!.position).toEqual({ x: 5, y: 1 });
  });

  it("is one history entry: one undo removes exactly the pasted objects", () => {
    const store = createStore();
    const before = store.getState().schematic;
    const copy = copyOf(withExtras, [
      part(R1),
      part(R2),
      { kind: "rail", id: RAIL },
      { kind: "note", id: NOTE },
    ]);

    store.pasteClipboard(copy, { x: 3, y: 3 }, catalog);

    expect(store.getState().schematic).not.toEqual(before);
    expect(store.undo()).toBe(true);
    expect(store.getState().schematic).toEqual(before);
    expect(store.getState().canUndo).toBe(false);
    expect(store.redo()).toBe(true);
    expect(store.getState().schematic.components).toHaveLength(5);
  });

  it("copies a rail, naming it afresh when its name is taken, and a note's text", () => {
    const store = createStore();
    const copy = copyOf(withExtras, [
      { kind: "rail", id: RAIL },
      { kind: "note", id: NOTE },
    ]);

    store.pasteClipboard(copy, { x: 4, y: 1 }, catalog);

    const { rails, documentation } = store.getState().schematic;
    expect(rails).toHaveLength(2);
    expect(rails[1]!.start).toEqual({ x: 24, y: 1 });
    expect(rails[1]!.end).toEqual({ x: 24, y: 11 });
    expect(rails[1]!.name).not.toBe("RAIL1");
    expect(documentation.notes.map(({ text }) => text)).toEqual([
      "Hello",
      "Hello",
    ]);
    expect(documentation.notes[1]!.position).toEqual({ x: 6, y: 9 });
  });

  it("names pasted parts by the automatic numbering, and copies no designator", () => {
    const store = createStore();
    store.setDesignator(R1, "R1");
    const copy = copyOf(store.getState().schematic, [part(R1)]);

    store.pasteClipboard(copy, { x: 0, y: 6 }, catalog, { designate: true });

    const { designators } = store.getState().schematic.documentation;
    expect(designators.map(({ designator }) => designator)).toEqual([
      "R1",
      "R2",
    ]);
  });

  it("leaves parts unnamed when the document holds no documentation", () => {
    const store = createStore();
    const copy = copyOf(withExtras, [part(R1)]);

    store.pasteClipboard(copy, { x: 0, y: 6 }, catalog);

    expect(store.getState().schematic.documentation.designators).toEqual([]);
  });

  it("refuses the whole paste when one part is invalid, and records nothing", () => {
    const store = createStore();
    const copy = copyOf(withExtras, [part(R1), part(R2)]);
    const broken: ClipboardSchematic = {
      ...copy,
      components: [
        copy.components[0]!,
        {
          ...copy.components[1]!,
          parameters: { resistanceOhms: -5 },
        } as ClipboardSchematic["components"][number],
      ],
    };

    expect(() =>
      store.pasteClipboard(broken, { x: 0, y: 6 }, catalog),
    ).toThrow();

    expect(store.getState().schematic).toEqual(withExtras);
    expect(store.getState().canUndo).toBe(false);
  });

  it("refuses the whole paste for a wire that is not orthogonal", () => {
    const store = createStore();
    const copy = copyOf(withExtras, [part(R1)]);
    const broken: ClipboardSchematic = {
      ...copy,
      wires: [
        {
          id: WIRE_4,
          points: [
            { x: 0, y: 0 },
            { x: 3, y: 3 },
          ],
        },
      ],
    };

    expect(() =>
      store.pasteClipboard(broken, { x: 0, y: 6 }, catalog),
    ).toThrow();
    expect(store.getState().schematic).toEqual(withExtras);
  });

  it("refuses a paste that would leave the 32-bit grid", () => {
    const store = createStore();
    const copy = copyOf(withExtras, [part(R1)]);

    expect(() =>
      store.pasteClipboard(copy, { x: 2147483647, y: 0 }, catalog),
    ).toThrow();
    expect(store.getState().schematic).toEqual(withExtras);
  });

  it("notifies listeners once", () => {
    const store = createStore();
    let notifications = 0;
    store.subscribe(() => (notifications += 1));

    store.pasteClipboard(
      copyOf(withExtras, [part(R1), part(R2)]),
      { x: 0, y: 6 },
      catalog,
    );

    expect(notifications).toBe(1);
  });
});
