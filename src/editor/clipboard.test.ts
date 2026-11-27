import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import {
  addedObjects,
  clipboardBounds,
  clipboardText,
  copySelection,
  MAX_CLIPBOARD_CHARS,
  parseClipboardText,
  translateClipboard,
  type ClipboardSchematic,
} from "./clipboard";

// The clipboard of docs/contracts/stage-16-editor-improvements.md section 4 on the divider:
// V1 at (0, 0), R1 at (4, 0), R2 at (8, 0); wire 4 joins V1's pin at -1 to R1's at 3, wire 5
// joins R1's pin at 5 to R2's at 7, wire 6 joins R2's pin at 9 to V1's at 1.

const divider = schematicFixture as Schematic;
const catalog = catalogFixture as ComponentCatalog;
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const WIRE_4 = "00000000-0000-4000-8000-000000000004";
const WIRE_5 = "00000000-0000-4000-8000-000000000005";
const MARKER = "00000000-0000-4000-8000-000000000007";

const component = (id: string) => ({ kind: "component", id }) as const;
const wire = (id: string) => ({ kind: "wire", id }) as const;

const withRailAndNote: Schematic = {
  ...divider,
  rails: [
    {
      id: "00000000-0000-4000-8000-0000000000a2",
      name: "RAIL1",
      start: { x: 20, y: 0 },
      end: { x: 20, y: 10 },
    },
  ],
  junctions: [
    { id: "00000000-0000-4000-8000-0000000000a1", position: { x: 6, y: 0 } },
  ],
  wires: [
    ...divider.wires,
    {
      id: "00000000-0000-4000-8000-0000000000a3",
      points: [
        { x: 20, y: 4 },
        { x: 12, y: 4 },
      ],
    },
    {
      id: "00000000-0000-4000-8000-0000000000a5",
      points: [
        { x: 6, y: 0 },
        { x: 6, y: 6 },
        { x: 20, y: 6 },
      ],
    },
  ],
  documentation: {
    ...divider.documentation,
    designators: [{ componentId: R1, designator: "R1" }],
    notes: [
      {
        id: "00000000-0000-4000-8000-0000000000a4",
        position: { x: 2, y: 8 },
        text: "Hello",
      },
    ],
  },
};

function copyOf(
  schematic: Schematic,
  targets: readonly { kind: string; id: string }[],
) {
  const copy = copySelection(
    schematic,
    targets as Parameters<typeof copySelection>[1],
    symbols,
  );
  if (!copy) throw new Error("nothing copied");
  return copy;
}

describe("what a copy holds", () => {
  it("holds the selected objects and the wire between two copied parts", () => {
    const copy = copyOf(divider, [component(R1), component(R2)]);

    expect(copy.components.map(({ id }) => id)).toEqual([R1, R2]);
    // Wire 5 has both ends on copied parts although it is not selected; wires 4 and 6 end on V1.
    expect(copy.wires.map(({ id }) => id)).toEqual([WIRE_5]);
  });

  it("holds a selected wire whether or not its ends are on copied objects", () => {
    const copy = copyOf(divider, [component(R1), wire(WIRE_4)]);

    expect(copy.wires.map(({ id }) => id)).toEqual([WIRE_4]);
  });

  it("holds rails, junctions, markers and notes, and the wires between them", () => {
    const copy = copyOf(withRailAndNote, [
      { kind: "rail", id: "00000000-0000-4000-8000-0000000000a2" },
      { kind: "junction", id: "00000000-0000-4000-8000-0000000000a1" },
      { kind: "note", id: "00000000-0000-4000-8000-0000000000a4" },
      { kind: "referenceMarker", id: MARKER },
    ]);

    expect(copy.rails).toHaveLength(1);
    expect(copy.junctions).toHaveLength(1);
    expect(copy.notes.map(({ text }) => text)).toEqual(["Hello"]);
    expect(copy.referenceMarkers).toHaveLength(1);
    // The wire from the junction at (6, 0) to the rail at (20, 6) has both ends on copied objects.
    expect(copy.wires.map(({ id }) => id)).toEqual([
      "00000000-0000-4000-8000-0000000000a5",
    ]);
  });

  it("does not copy a wire with only one end on a copied object", () => {
    const copy = copyOf(withRailAndNote, [
      { kind: "rail", id: "00000000-0000-4000-8000-0000000000a2" },
    ]);

    expect(copy.wires).toEqual([]);
  });

  it("holds no designator, wire number or terminal, nor anything derived", () => {
    const text = clipboardText(copyOf(withRailAndNote, [component(R1)]));

    expect(JSON.parse(text)).toEqual({
      format: "open-circuit-clipboard",
      version: 1,
      schematic: {
        components: [divider.components[1]],
        wires: [],
        junctions: [],
        referenceMarkers: [],
        rails: [],
        notes: [],
      },
    });
    expect(text).not.toContain("designator");
  });

  it("is nothing for an empty selection", () => {
    expect(copySelection(divider, [], symbols)).toBeNull();
  });

  it("copies by value: the copy is not the document", () => {
    const copy = copyOf(divider, [component(R1)]);

    expect(copy.components[0]).toEqual(divider.components[1]);
    expect(copy.components[0]).not.toBe(divider.components[1]);
  });
});

describe("reading the clipboard text", () => {
  const good = (): ClipboardSchematic =>
    copyOf(divider, [component(R1), component(R2), wire(WIRE_5)]);
  const parse = (text: string) => parseClipboardText(text, catalog);
  const withBody = (change: (body: Record<string, unknown>) => void) => {
    const body = JSON.parse(clipboardText(good())) as Record<string, unknown>;
    change(body);
    return parse(JSON.stringify(body));
  };
  const withSchematic = (
    change: (schematic: Record<string, unknown[]>) => void,
  ) => withBody((body) => change(body.schematic as Record<string, unknown[]>));

  it("round-trips a copy", () => {
    const result = parse(clipboardText(good()));

    expect(result).toEqual({
      ok: true,
      payload: {
        format: "open-circuit-clipboard",
        version: 1,
        schematic: good(),
      },
    });
  });

  it.each([
    ["text that is not JSON", "hello"],
    ["JSON that is not an object", "[1, 2]"],
    [
      "an object of another format",
      '{"format":"x","version":1,"schematic":{}}',
    ],
  ])("refuses %s", (_name, text) => {
    expect(parse(text)).toMatchObject({ ok: false });
  });

  it("refuses a text longer than the project limit before parsing it", () => {
    const result = parse(" ".repeat(MAX_CLIPBOARD_CHARS + 1));

    expect(result).toEqual({
      ok: false,
      reason: "The clipboard text is too large to paste.",
    });
  });

  it("refuses an unknown version with a message, and a version that is not an integer", () => {
    expect(withBody((body) => (body.version = 2))).toMatchObject({
      ok: false,
      reason: expect.stringContaining("different version"),
    });
    expect(withBody((body) => (body.version = 1.5))).toMatchObject({
      ok: false,
      reason: "The clipboard version is not readable.",
    });
  });

  it("refuses an unknown field anywhere", () => {
    expect(withBody((body) => (body.extra = 1))).toMatchObject({ ok: false });
    expect(withSchematic((schematic) => (schematic.extra = []))).toMatchObject({
      ok: false,
    });
    expect(
      withSchematic((schematic) => {
        (schematic.components![0] as Record<string, unknown>).locked = true;
      }),
    ).toMatchObject({ ok: false });
  });

  it("refuses a missing member", () => {
    expect(
      withSchematic((schematic) => {
        delete schematic.rails;
      }),
    ).toMatchObject({ ok: false });
  });

  it("refuses a part of a type the catalog does not have", () => {
    expect(
      withSchematic((schematic) => {
        (schematic.components![0] as Record<string, unknown>).typeId = "warp";
      }),
    ).toMatchObject({ ok: false });
  });

  it("refuses a rotation, a coordinate or a parameter that is not valid", () => {
    const part = (schematic: Record<string, unknown[]>) =>
      schematic.components![0] as Record<string, unknown>;

    expect(
      withSchematic((schematic) => (part(schematic).rotationDeg = 45)),
    ).toMatchObject({ ok: false });
    expect(
      withSchematic(
        (schematic) => (part(schematic).position = { x: 1.5, y: 0 }),
      ),
    ).toMatchObject({ ok: false });
    expect(
      withSchematic(
        (schematic) => (part(schematic).position = { x: 2 ** 31, y: 0 }),
      ),
    ).toMatchObject({ ok: false });
    expect(
      withSchematic(
        (schematic) => (part(schematic).parameters = { resistanceOhms: "1k" }),
      ),
    ).toMatchObject({ ok: false });
  });

  it("refuses an ID that is not a canonical UUID and an ID that is repeated", () => {
    expect(
      withSchematic((schematic) => {
        (schematic.components![0] as Record<string, unknown>).id = "R1";
      }),
    ).toMatchObject({ ok: false });
    expect(
      withSchematic((schematic) => {
        (schematic.components![1] as Record<string, unknown>).id = R1;
      }),
    ).toMatchObject({ ok: false, reason: expect.stringContaining("repeated") });
  });

  it("refuses a wire that is not a list of at least two grid points", () => {
    expect(
      withSchematic((schematic) => {
        (schematic.wires![0] as Record<string, unknown>).points = [
          { x: 0, y: 0 },
        ];
      }),
    ).toMatchObject({ ok: false });
    expect(
      withSchematic((schematic) => {
        (schematic.wires![0] as Record<string, unknown>).points = "none";
      }),
    ).toMatchObject({ ok: false });
  });

  it("refuses a selection with nothing in it", () => {
    expect(
      parse(
        clipboardText({
          components: [],
          wires: [],
          junctions: [],
          referenceMarkers: [],
          rails: [],
          notes: [],
        }),
      ),
    ).toEqual({ ok: false, reason: "The clipboard selection is empty." });
  });

  it("refuses a prototype key without building anything from it", () => {
    const text =
      '{"format":"open-circuit-clipboard","version":1,"__proto__":{"polluted":true},"schematic":{}}';

    expect(parse(text)).toMatchObject({ ok: false });
    expect(({} as Record<string, unknown>).polluted).toBeUndefined();
  });
});

describe("placing a copy", () => {
  it("moves every point by the offset and keeps the IDs", () => {
    const copy = copyOf(divider, [component(R1), wire(WIRE_5)]);
    const moved = translateClipboard(copy, { x: 2, y: 3 });

    expect(moved.components[0]!.position).toEqual({ x: 6, y: 3 });
    expect(moved.wires[0]!.points).toEqual([
      { x: 7, y: 3 },
      { x: 9, y: 3 },
    ]);
    expect(moved.components[0]!.id).toBe(R1);
  });

  it("has bounds that hold the pins and the bodies", () => {
    const copy = copyOf(divider, [component(R1), component(R2)]);
    const bounds = clipboardBounds(copy, symbols)!;

    expect(bounds.minX).toBeLessThanOrEqual(3);
    expect(bounds.maxX).toBeGreaterThanOrEqual(9);
  });

  it("reports what a command added, and nothing else", () => {
    const after: Schematic = {
      ...divider,
      components: [
        ...divider.components,
        {
          ...divider.components[1]!,
          id: "00000000-0000-4000-8000-0000000000b1",
        },
      ],
    };

    expect(addedObjects(divider, after)).toEqual([
      { kind: "component", id: "00000000-0000-4000-8000-0000000000b1" },
    ]);
    expect(addedObjects(divider, divider)).toEqual([]);
  });
});
