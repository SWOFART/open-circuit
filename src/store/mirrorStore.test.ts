import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { copySelection } from "../editor/clipboard";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";

// The saved `mirrored` flag (docs/contracts/stage-16-editor-improvements.md section 5.2): optional,
// omitted when false, a boolean when present, and kept by every edit and by a paste.

const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";

let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xc100 + counter++).toString(16).padStart(12, "0")}`;
const createStore = (schematic: Schematic = dividerSchematic) =>
  new SchematicDocumentStore(schematic, nextId);
const r1 = (store: SchematicDocumentStore) =>
  store.getState().schematic.components.find(({ id }) => id === R1)!;

describe("the mirror flag in the document store", () => {
  it("is set by a transform, cleared again, and never written as false", () => {
    const store = createStore();
    const at = r1(store).position;

    store.transformComponent(R1, at, 0, [], true);
    expect(r1(store).mirrored).toBe(true);
    expect(JSON.stringify(store.getState().schematic)).toContain(
      '"mirrored":true',
    );

    store.transformComponent(R1, at, 0, [], false);
    expect(Object.keys(r1(store))).not.toContain("mirrored");
    expect(JSON.stringify(store.getState().schematic)).not.toContain(
      "mirrored",
    );
  });

  it("leaves the flag as it is when a transform does not name it", () => {
    const store = createStore();
    const at = r1(store).position;
    store.transformComponent(R1, at, 0, [], true);

    store.transformComponent(R1, { x: at.x + 1, y: at.y }, 90);

    expect(r1(store).mirrored).toBe(true);
    expect(r1(store).rotationDeg).toBe(90);
  });

  it("refuses a flag that is not a boolean, and records nothing", () => {
    const store = createStore();

    expect(() =>
      store.transformComponent(
        R1,
        r1(store).position,
        0,
        [],
        "yes" as unknown as boolean,
      ),
    ).toThrow("mirror flag");
    expect(store.getState().canUndo).toBe(false);
  });

  it("is one history entry that undo takes back", () => {
    const store = createStore();
    const before = store.getState().schematic;

    store.transformComponent(R1, r1(store).position, 0, [], true);
    store.undo();

    expect(store.getState().schematic).toEqual(before);
    expect(store.getState().canUndo).toBe(false);
  });

  it("refuses a loaded document whose flag is not a boolean", () => {
    const broken = {
      ...dividerSchematic,
      components: dividerSchematic.components.map((item) => ({
        ...item,
        mirrored: 1,
      })),
    } as unknown as Schematic;

    expect(() => createStore(broken)).toThrow("mirror flag");
  });

  it("keeps a mirrored document as it is, and an unmirrored one byte for byte", () => {
    const mirrored: Schematic = {
      ...dividerSchematic,
      components: dividerSchematic.components.map((item) =>
        item.id === R1 ? { ...item, mirrored: true } : item,
      ),
    };

    expect(createStore(mirrored).getState().schematic).toEqual(mirrored);
    expect(JSON.stringify(createStore().getState().schematic)).toBe(
      JSON.stringify(dividerSchematic),
    );
  });
});

describe("the flag and the clipboard", () => {
  it("a copy holds the flag and a paste restores it with a fresh ID", () => {
    const source = createStore();
    source.transformComponent(R1, r1(source).position, 0, [], true);
    const copy = copySelection(
      source.getState().schematic,
      [{ kind: "component", id: R1 }],
      symbols,
    )!;

    const target = createStore();
    target.pasteClipboard(copy, { x: 0, y: 6 }, catalog);

    const pasted = target.getState().schematic.components.at(-1)!;
    expect(pasted.mirrored).toBe(true);
    expect(pasted.id).not.toBe(R1);
  });

  it("a paste of an unmirrored part writes no flag", () => {
    const store = createStore();
    const copy = copySelection(
      store.getState().schematic,
      [{ kind: "component", id: R1 }],
      symbols,
    )!;

    store.pasteClipboard(copy, { x: 0, y: 6 }, catalog);

    expect(JSON.stringify(store.getState().schematic)).not.toContain(
      "mirrored",
    );
  });
});
