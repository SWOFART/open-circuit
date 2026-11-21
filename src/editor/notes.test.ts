import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { commandForIntent, dragPreviewSchematic } from "./commands";
import { analyzeGeometry } from "./connectivity";
import { hitTest } from "./hitTest";
import {
  CanvasInteraction,
  type CanvasPointer,
  type InteractionIntent,
} from "./interaction";
import {
  DEFAULT_NOTE_TEXT,
  MAX_NOTES,
  NOTE_MAX_CHARACTERS,
  NOTE_MAX_LINES,
  noteExtent,
  noteProblem,
} from "./noteRules";
import { buildScene } from "./scene";
import { toScreen } from "./view";

const NOTE = "00000000-0000-4000-8000-000000000a01";
const R1 = "00000000-0000-4000-8000-000000000002";

const base = schematicFixture as unknown as Schematic;
const schematic: Schematic = {
  ...base,
  schemaVersion: 3,
  rails: base.rails ?? [],
  documentation: {
    designators: [],
    wireNumbers: [],
    terminals: [],
    notes: [{ id: NOTE, position: { x: 20, y: 10 }, text: "Main supply" }],
    titleBlock: {
      title: "",
      drawingNumber: "",
      revision: "",
      author: "",
      company: "",
      date: "",
    },
    sheets: [],
  },
};

describe("the note text rules", () => {
  it("accepts text in any script, with line breaks", () => {
    for (const text of [
      "x",
      "Main supply",
      "Hauptschalter\nNotaus",
      "Главный автомат",
      "a\n".repeat(NOTE_MAX_LINES),
    ]) {
      expect(noteProblem(text), text).toBeNull();
    }
  });

  it("names the problem otherwise, counting as the Engine does", () => {
    expect(noteProblem("")).toBe("empty");
    expect(noteProblem("x".repeat(NOTE_MAX_CHARACTERS + 1))).toBe("tooLong");
    expect(noteProblem("x".repeat(NOTE_MAX_CHARACTERS))).toBeNull();
    expect(noteProblem("a\n".repeat(NOTE_MAX_LINES + 1))).toBe("tooManyLines");
    expect(noteProblem("a\r\nb")).toBe("control");
    expect(noteProblem("a\u0007b")).toBe("control");
    expect(noteProblem("a\tb")).toBe("control");
    expect(noteProblem("a\u0085b")).toBe("control");
  });

  it("estimates a larger box for more text", () => {
    const small = noteExtent("a");
    const wide = noteExtent("a".repeat(40));
    const tall = noteExtent("a\nb\nc");

    expect(wide.width).toBeGreaterThan(small.width);
    expect(tall.height).toBeGreaterThan(small.height);
  });
});

describe("a note in the scene", () => {
  const scene = buildScene(schematic, symbols);

  it("is drawn as text owned by the note, on top of the circuit", () => {
    const shape = scene.shapes.at(-1);

    expect(shape).toMatchObject({
      kind: "label",
      owner: { kind: "note", id: NOTE },
      text: "Main supply",
    });
  });

  it("is picked inside its text and only where nothing electrical is", () => {
    const { width, height } = noteExtent("Main supply");

    expect(
      hitTest(scene, { x: 20 + width / 2, y: 10 + height / 2 }, 0.2),
    ).toEqual({ kind: "note", id: NOTE });
    expect(hitTest(scene, { x: 20 + width + 5, y: 10 }, 0.2)).toBeNull();

    // A note over a part does not take the click away from the part.
    const covering: Schematic = {
      ...schematic,
      documentation: {
        ...schematic.documentation,
        notes: [{ id: NOTE, position: { x: -2, y: -1 }, text: "x".repeat(60) }],
      },
    };
    expect(
      hitTest(buildScene(covering, symbols), { x: 0, y: 0 }, 0.2),
    ).toMatchObject({ kind: "component" });
  });
});

describe("note commands", () => {
  it("turns the intents into commands, and ignores a note that is gone", () => {
    expect(
      commandForIntent(
        { kind: "addNote", position: { x: 3, y: 4 } },
        schematic,
        symbols,
      ),
    ).toEqual({ kind: "addNote", position: { x: 3, y: 4 } });
    expect(
      commandForIntent(
        { kind: "moveNote", id: NOTE, position: { x: 5, y: 6 } },
        schematic,
        symbols,
      ),
    ).toEqual({ kind: "moveNote", id: NOTE, position: { x: 5, y: 6 } });
    expect(
      commandForIntent(
        { kind: "moveNote", id: R1, position: { x: 5, y: 6 } },
        schematic,
        symbols,
      ),
    ).toBeNull();
    expect(
      commandForIntent(
        { kind: "delete", target: { kind: "note", id: NOTE } },
        schematic,
        symbols,
      ),
    ).toEqual({ kind: "deleteNote", id: NOTE });
    expect(
      commandForIntent(
        { kind: "delete", target: { kind: "note", id: R1 } },
        schematic,
        symbols,
      ),
    ).toBeNull();
  });

  it("shows a dragged note at its new place without touching the circuit", () => {
    const moved = dragPreviewSchematic(
      schematic,
      { kind: "note", id: NOTE, to: { x: 30, y: 12 } },
      symbols,
    );

    expect(moved.documentation.notes[0]?.position).toEqual({ x: 30, y: 12 });
    expect(moved.components).toBe(schematic.components);
    expect(moved.wires).toBe(schematic.wires);
  });

  it("has a limit and a default text the interface can rely on", () => {
    expect(MAX_NOTES).toBe(200);
    expect(noteProblem(DEFAULT_NOTE_TEXT)).toBeNull();
  });
});

describe("the Note tool and note gestures", () => {
  function setUp() {
    const intents: InteractionIntent[] = [];
    const interaction = new CanvasInteraction(
      () => {},
      (intent) => intents.push(intent),
    );
    interaction.setViewport({ width: 1600, height: 600 });
    interaction.setScene(
      buildScene(schematic, symbols),
      analyzeGeometry(schematic, symbols),
    );
    const point = (
      grid: { x: number; y: number },
      time: number,
    ): CanvasPointer => ({
      ...toScreen(grid, interaction.view),
      button: 0,
      pointerId: 1,
      time,
    });
    return { interaction, intents, point };
  }

  it("places a note at the clicked grid point and stays active", () => {
    const { interaction, intents, point } = setUp();
    interaction.setTool({ kind: "note" });

    interaction.pointerDown(point({ x: 7, y: -4 }, 1));
    interaction.pointerUp(point({ x: 7, y: -4 }, 2));
    interaction.pointerDown(point({ x: 9, y: -4 }, 3));

    expect(intents).toEqual([
      { kind: "addNote", position: { x: 7, y: -4 } },
      { kind: "addNote", position: { x: 9, y: -4 } },
    ]);
    expect(interaction.tool).toEqual({ kind: "note" });
    interaction.escape();
    expect(interaction.tool).toEqual({ kind: "select" });
  });

  it("selects a note by a click and moves it by dragging, as one move", () => {
    const { interaction, intents, point } = setUp();
    const inside = { x: 20.4, y: 10.2 };

    interaction.pointerDown(point(inside, 1));
    expect(interaction.status.selection).toEqual({ kind: "note", id: NOTE });
    interaction.pointerMove(point({ x: 24.4, y: 12.2 }, 2));
    expect(interaction.dragPreview).toEqual({
      kind: "note",
      id: NOTE,
      to: { x: 24, y: 12 },
    });
    interaction.pointerUp(point({ x: 24.4, y: 12.2 }, 3));

    expect(intents).toEqual([
      { kind: "moveNote", id: NOTE, position: { x: 24, y: 12 } },
    ]);
  });

  it("opens the note's editor on a double press and on Enter", () => {
    const { interaction, intents, point } = setUp();
    const inside = { x: 20.4, y: 10.2 };

    interaction.pointerDown(point(inside, 1000));
    interaction.pointerUp(point(inside, 1050));
    interaction.pointerDown(point(inside, 1200));
    expect(intents).toEqual([
      { kind: "openEditor", target: { kind: "note", id: NOTE } },
    ]);

    intents.length = 0;
    interaction.pointerUp(point(inside, 1250));
    expect(interaction.openEditorForSelection()).toBe(true);
    expect(intents).toEqual([
      { kind: "openEditor", target: { kind: "note", id: NOTE } },
    ]);
  });

  it("requests a delete of the selected note", () => {
    const { interaction, intents, point } = setUp();
    interaction.pointerDown(point({ x: 20.4, y: 10.2 }, 1));
    interaction.pointerUp(point({ x: 20.4, y: 10.2 }, 2));

    interaction.deleteSelection();

    expect(intents).toContainEqual({
      kind: "delete",
      target: { kind: "note", id: NOTE },
    });
  });
});
