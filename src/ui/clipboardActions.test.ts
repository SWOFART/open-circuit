import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import {
  clipboardText,
  copySelection,
  type ClipboardSchematic,
} from "../editor/clipboard";
import type { EditorCommand } from "../editor/commands";
import type { SelectionTarget } from "../editor/hitTest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import { SchematicDocumentStore } from "../store/document";
import { dividerSchematic } from "../store/fixtures";
import {
  commandVerdict,
  type EditorRestrictions,
} from "../store/studentAssignment";
import { symbols } from "../symbols";
import { runClipboardAction, type ClipboardHost } from "./clipboardActions";
import { applyRestrictedCommand } from "./editorCommands";

// Copy, cut, paste and duplicate with the system clipboard and an assignment's restrictions
// (docs/contracts/stage-16-editor-improvements.md sections 4.4 and 4.5).

const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const WIRE_5 = "00000000-0000-4000-8000-000000000005";

let counter = 0;
const nextId = () =>
  `00000000-0000-4000-8000-${(0xf000 + counter++).toString(16).padStart(12, "0")}`;

const part = (id: string): SelectionTarget => ({ kind: "component", id });

function setup(
  options: {
    restrictions?: EditorRestrictions | null;
    text?: string;
    readFails?: boolean;
    writeFails?: boolean;
  } = {},
) {
  const store = new SchematicDocumentStore(dividerSchematic, nextId);
  const restrictions = options.restrictions ?? null;
  const clip = { text: options.text ?? "" };
  const log = {
    placed: [] as ClipboardSchematic[],
    selected: [] as (readonly SelectionTarget[])[],
  };
  const host: ClipboardHost = {
    schematic: () => store.getState().schematic,
    registry: symbols,
    catalog: () => catalog,
    readText: async () => {
      if (options.readFails) throw new Error("denied");
      return clip.text;
    },
    writeText: async (text) => {
      if (options.writeFails) throw new Error("denied");
      clip.text = text;
    },
    refusal: (command: EditorCommand) => {
      const verdict = commandVerdict(command, restrictions);
      return verdict.allowed ? null : verdict.reason;
    },
    apply: (command: EditorCommand) => {
      try {
        const verdict = applyRestrictedCommand(
          store,
          command,
          catalog,
          restrictions,
        );
        return verdict.allowed ? null : verdict.reason;
      } catch (error) {
        return error instanceof Error ? error.message : String(error);
      }
    },
    startPaste: (schematic) => {
      log.placed.push(schematic);
      return true;
    },
    select: (targets) => log.selected.push(targets),
  };
  return { store, clip, host, log };
}

const copyOfParts = (): ClipboardSchematic =>
  copySelection(dividerSchematic, [part(R1), part(R2)], symbols)!;

describe("copy", () => {
  it("puts the selection on the clipboard and changes nothing else", async () => {
    const { store, clip, host } = setup();

    const message = await runClipboardAction(
      "copy",
      [part(R1), part(R2)],
      host,
    );

    expect(message).toBeNull();
    expect(clip.text).toBe(clipboardText(copyOfParts()));
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
  });

  it("says so when the clipboard cannot be written", async () => {
    const { host } = setup({ writeFails: true });

    expect(await runClipboardAction("copy", [part(R1)], host)).toMatch(
      /clipboard could not be written/,
    );
  });

  it("copies nothing when only things that cannot be copied are selected", async () => {
    const { host, clip } = setup();

    expect(
      await runClipboardAction("copy", [{ kind: "terminal", id: R1 }], host),
    ).toBe("Nothing in the selection can be copied.");
    expect(clip.text).toBe("");
  });
});

describe("cut", () => {
  it("copies the selection, then deletes it as one history entry", async () => {
    const { store, clip, host } = setup();

    const message = await runClipboardAction("cut", [part(R1)], host);

    expect(message).toBeNull();
    expect(JSON.parse(clip.text).schematic.components).toHaveLength(1);
    expect(store.getState().schematic.components).toHaveLength(2);
    store.undo();
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
  });

  it("deletes nothing when the clipboard cannot be written", async () => {
    const { store, host } = setup({ writeFails: true });

    const message = await runClipboardAction("cut", [part(R1)], host);

    expect(message).toMatch(/nothing was cut/);
    expect(store.getState().schematic).toEqual(dividerSchematic);
  });

  it("is refused whole, and leaves the clipboard alone, when a selected part is locked", async () => {
    const { store, clip, host } = setup({
      text: "earlier",
      restrictions: {
        allowedTypes: null,
        locked: new Set([R1]),
        findFault: false,
      },
    });

    const message = await runClipboardAction("cut", [part(R1), part(R2)], host);

    expect(message).toBe(
      "1 selected part is locked by the assignment, so nothing was deleted.",
    );
    expect(clip.text).toBe("earlier");
    expect(store.getState().schematic).toEqual(dividerSchematic);
  });

  it("lets a locked part be copied", async () => {
    const { clip, host } = setup({
      restrictions: {
        allowedTypes: null,
        locked: new Set([R1]),
        findFault: false,
      },
    });

    expect(await runClipboardAction("copy", [part(R1)], host)).toBeNull();
    expect(clip.text).not.toBe("");
  });
});

describe("paste", () => {
  it("starts a placement of what the clipboard holds", async () => {
    const { host, log } = setup({ text: clipboardText(copyOfParts()) });

    expect(await runClipboardAction("paste", [], host)).toBeNull();

    expect(log.placed).toEqual([copyOfParts()]);
  });

  it("refuses text that is not a copy, with a message, and places nothing", async () => {
    const { host, log } = setup({ text: "just words" });

    expect(await runClipboardAction("paste", [], host)).toBe(
      "The clipboard does not hold a copied selection.",
    );
    expect(log.placed).toEqual([]);
  });

  it("says so when the clipboard cannot be read", async () => {
    const { host } = setup({ readFails: true });

    expect(await runClipboardAction("paste", [], host)).toBe(
      "The clipboard could not be read.",
    );
  });

  it("is refused before placement for a type an assignment does not allow", async () => {
    const { host, log } = setup({
      text: clipboardText(copyOfParts()),
      restrictions: {
        allowedTypes: ["dc_voltage_source"],
        locked: new Set(),
        findFault: false,
      },
    });

    expect(await runClipboardAction("paste", [], host)).toBe(
      "This assignment does not allow resistor",
    );
    expect(log.placed).toEqual([]);
  });
});

describe("duplicate", () => {
  it("pastes a copy at (+2, +2) as one entry, selects it, and keeps the clipboard", async () => {
    const { store, clip, host, log } = setup({ text: "earlier" });

    const message = await runClipboardAction(
      "duplicate",
      [part(R1), part(R2), { kind: "wire", id: WIRE_5 }],
      host,
    );

    expect(message).toBeNull();
    const after = store.getState().schematic;
    expect(after.components.slice(3).map(({ position }) => position)).toEqual([
      { x: 6, y: 2 },
      { x: 10, y: 2 },
    ]);
    expect(clip.text).toBe("earlier");
    expect(log.selected).toHaveLength(1);
    expect(log.selected[0]!.map(({ kind }) => kind).sort()).toEqual([
      "component",
      "component",
      "wire",
    ]);
    store.undo();
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().canUndo).toBe(false);
  });

  it("steps along a diagonal when pressed again on the new selection", async () => {
    const { store, host, log } = setup();
    await runClipboardAction("duplicate", [part(R1)], host);

    await runClipboardAction("duplicate", log.selected[0]!, host);

    expect(
      store
        .getState()
        .schematic.components.slice(3)
        .map(({ position }) => position),
    ).toEqual([
      { x: 6, y: 2 },
      { x: 8, y: 4 },
    ]);
  });

  it("is refused whole in an assignment that does not allow the type", async () => {
    const { store, host } = setup({
      restrictions: {
        allowedTypes: ["dc_voltage_source"],
        locked: new Set(),
        findFault: false,
      },
    });

    expect(await runClipboardAction("duplicate", [part(R1)], host)).toBe(
      "This assignment does not allow resistor",
    );
    expect(store.getState().schematic).toEqual(dividerSchematic);
  });
});

describe("the assignment's verdict on a paste", () => {
  const restricted: EditorRestrictions = {
    allowedTypes: null,
    locked: new Set(),
    findFault: false,
  };
  const empty = {
    components: [],
    wires: [],
    junctions: [],
    referenceMarkers: [],
    rails: [],
    notes: [],
  };
  const paste = (schematic: ClipboardSchematic): EditorCommand => ({
    kind: "paste",
    schematic,
    delta: { x: 0, y: 0 },
  });

  it("allows everything outside an assignment", () => {
    expect(
      commandVerdict(
        paste({
          ...empty,
          rails: [
            {
              id: R1,
              name: "L1",
              start: { x: 0, y: 0 },
              end: { x: 0, y: 4 },
            },
          ],
        }),
        null,
      ),
    ).toEqual({ allowed: true });
  });

  it("refuses a rail in the payload, whole", () => {
    expect(
      commandVerdict(
        paste({
          ...copyOfParts(),
          rails: [
            {
              id: R1,
              name: "L1",
              start: { x: 0, y: 0 },
              end: { x: 0, y: 4 },
            },
          ],
        }),
        restricted,
      ),
    ).toMatchObject({
      allowed: false,
      reason: expect.stringContaining("DC rails are not available"),
    });
  });

  it("refuses a note in the payload, because an assignment holds no documentation", () => {
    expect(
      commandVerdict(
        paste({
          ...empty,
          notes: [{ id: R1, position: { x: 0, y: 0 }, text: "x" }],
        }),
        restricted,
      ),
    ).toMatchObject({
      allowed: false,
      reason: expect.stringContaining("Notes are not available"),
    });
  });

  it("allows a paste of allowed types, and its parts are never locked", () => {
    const store = new SchematicDocumentStore(dividerSchematic, nextId);
    const verdict = applyRestrictedCommand(
      store,
      paste(copyOfParts()),
      catalog,
      { ...restricted, locked: new Set([R1, R2]) },
    );

    expect(verdict.allowed).toBe(true);
    const added = store.getState().schematic.components.slice(3);
    expect(added).toHaveLength(2);
    expect(added.some(({ id }) => id === R1 || id === R2)).toBe(false);
  });
});
