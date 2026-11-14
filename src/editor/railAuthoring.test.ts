import { describe, expect, it } from "vitest";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import { SchematicDocumentStore } from "../store/document";
import { blankSchematic } from "../store/projectWorkflow";
import { commandVerdict } from "../store/studentAssignment";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { commandForIntent, dragPreviewSchematic } from "./commands";
import { analyzeGeometry } from "./connectivity";
import {
  CanvasInteraction,
  type InteractionChange,
  type InteractionIntent,
} from "./interaction";
import { buildScene } from "./scene";
import { toScreen } from "./view";

const RAIL = "00000000-0000-4000-8000-000000009001";
const MARKER = "00000000-0000-4000-8000-000000009011";

const p = (x: number, y: number): GridPoint => ({ x, y });

function drawing(parts: Partial<Schematic> = {}): Schematic {
  return {
    ...blankSchematic,
    rails: [{ id: RAIL, name: "L+", start: p(5, 0), end: p(5, 10) }],
    ...parts,
  };
}

function open(schematic: Schematic = drawing()) {
  const intents: InteractionIntent[] = [];
  const changes: InteractionChange[] = [];
  const interaction = new CanvasInteraction(
    (change) => changes.push(change),
    (intent) => intents.push(intent),
  );
  interaction.setViewport({ width: 800, height: 600 });
  const geometry = analyzeGeometry(schematic, symbols);
  interaction.setScene(buildScene(schematic, symbols, { geometry }), geometry);
  const pointer = (x: number, y: number) => ({
    ...toScreen({ x, y }, interaction.view),
    button: 0,
    pointerId: 1,
  });

  return {
    interaction,
    intents,
    schematic,
    click(x: number, y: number) {
      interaction.pointerDown(pointer(x, y));
      interaction.pointerUp(pointer(x, y));
    },
    move(x: number, y: number) {
      interaction.pointerMove(pointer(x, y));
    },
    press(x: number, y: number) {
      return interaction.pointerDown(pointer(x, y));
    },
    release(x: number, y: number) {
      interaction.pointerUp(pointer(x, y));
    },
  };
}

describe("the DC rail tool", () => {
  it("previews a 20-unit line downward before the first click", () => {
    const { interaction, move } = open();
    interaction.setTool({ kind: "rail" });

    move(12, 3);

    expect(interaction.railPreview).toEqual({
      x: 12,
      top: 3,
      bottom: 23,
      valid: true,
    });
    expect(interaction.status.railStarted).toBe(false);
  });

  it("fixes x and one y on the first click and orders the two on the second", () => {
    const { interaction, intents, click, move } = open();
    interaction.setTool({ kind: "rail" });

    click(12, 8);
    expect(interaction.status.railStarted).toBe(true);
    // The other end above the first click: the preview is ordered, and x stays.
    move(15, 2);
    expect(interaction.railPreview).toEqual({
      x: 12,
      top: 2,
      bottom: 8,
      valid: true,
    });

    click(15, 2);

    expect(intents).toEqual([{ kind: "addRail", x: 12, top: 2, bottom: 8 }]);
    expect(interaction.status.railStarted).toBe(false);
    // The tool stays for the next rail.
    expect(interaction.tool).toEqual({ kind: "rail" });
  });

  it("commits nothing for a second click that makes no legal rail", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "rail" });

    click(12, 8);
    click(12, 8);
    click(30, 9);

    // Same y, then a y one unit away: a rail is at least 2 units long.
    expect(intents).toEqual([]);
    expect(interaction.status.railStarted).toBe(true);
    expect(interaction.railPreview?.valid).toBe(false);
  });

  it("cancels the rail on Escape, then leaves the tool on the next Escape", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "rail" });
    click(12, 8);

    interaction.escape();
    expect(interaction.status.railStarted).toBe(false);
    expect(interaction.tool).toEqual({ kind: "rail" });

    interaction.escape();
    expect(interaction.tool).toEqual({ kind: "select" });
    expect(intents).toEqual([]);
  });

  it("is not rotated by R, and drops an unfinished rail when the tool changes or on Undo", () => {
    const { interaction, intents, click } = open();
    interaction.setTool({ kind: "rail" });
    click(12, 8);

    interaction.rotate();
    expect(interaction.status.railStarted).toBe(true);

    // Undo during a preview cancels the preview and is not sent on.
    interaction.undo();
    expect(interaction.status.railStarted).toBe(false);
    expect(intents).toEqual([]);

    click(12, 8);
    interaction.setTool({ kind: "select" });
    expect(interaction.status.railStarted).toBe(false);
  });

  it("has no preview in another tool or off the canvas", () => {
    const { interaction } = open();

    expect(interaction.railPreview).toBeNull();
    interaction.setTool({ kind: "rail" });
    expect(interaction.railPreview).toBeNull();
  });
});

describe("selecting and dragging a rail", () => {
  it("moves the whole rail by the snapped offset and commits one edit on release", () => {
    const { interaction, intents, press, move, release } = open();

    expect(press(5, 5)).toBe(true);
    expect(interaction.selection).toEqual({ kind: "rail", id: RAIL });
    move(7, 8);
    expect(interaction.dragPreview).toMatchObject({
      kind: "rail",
      handle: "line",
      x: 7,
      top: 3,
      bottom: 13,
      valid: true,
    });
    release(7, 8);

    expect(intents).toEqual([
      { kind: "editRail", railId: RAIL, x: 7, top: 3, bottom: 13 },
    ]);
  });

  it("resizes vertically only, at the existing x, from either end", () => {
    const { interaction, intents, click, press, move, release } = open();
    click(5, 5);
    expect(interaction.selection?.kind).toBe("rail");

    // The bottom handle, dragged sideways and down: only y changes.
    press(5, 10);
    move(9, 14);
    expect(interaction.dragPreview).toMatchObject({
      handle: "end",
      x: 5,
      top: 0,
      bottom: 14,
    });
    release(9, 14);

    // The top handle, dragged up.
    press(5, 0);
    move(5, -3);
    release(5, -3);

    expect(intents).toEqual([
      { kind: "editRail", railId: RAIL, x: 5, top: 0, bottom: 14 },
      { kind: "editRail", railId: RAIL, x: 5, top: -3, bottom: 10 },
    ]);
  });

  it("cancels a drag that would leave an illegal rail, and shows the drawing unchanged", () => {
    const { interaction, intents, schematic, click, press, move, release } =
      open();
    click(5, 5);

    // The bottom end dragged past the top: a reversed rail.
    press(5, 10);
    move(5, -2);
    const preview = interaction.dragPreview;
    expect(preview).toMatchObject({ kind: "rail", valid: false });
    expect(preview && dragPreviewSchematic(schematic, preview, symbols)).toBe(
      schematic,
    );
    release(5, -2);

    expect(intents).toEqual([]);
  });

  it("changes nothing on Escape, on a pointer cancel, or when released where it began", () => {
    const { interaction, intents, press, move, release } = open();

    press(5, 5);
    move(8, 5);
    interaction.escape();
    release(8, 5);
    expect(intents).toEqual([]);

    press(5, 5);
    move(8, 8);
    interaction.pointerCancel(1);
    expect(intents).toEqual([]);

    press(5, 5);
    move(8, 8);
    move(5, 5);
    release(5, 5);
    expect(intents).toEqual([]);
  });

  it("previews a legal drag in the drawing", () => {
    const { interaction, schematic, press, move } = open();
    press(5, 5);
    move(6, 7);

    const preview = interaction.dragPreview;
    const shown = preview && dragPreviewSchematic(schematic, preview, symbols);

    expect(shown?.rails[0]).toMatchObject({
      id: RAIL,
      start: p(6, 2),
      end: p(6, 12),
    });
    // The document itself is untouched.
    expect(schematic.rails[0]?.start).toEqual(p(5, 0));
  });

  it("leaves a reference marker on the rail directly selectable", () => {
    const withMarker = drawing({
      referenceMarkers: [{ id: MARKER, position: p(5, 6) }],
    });
    const { interaction, click } = open(withMarker);

    click(5, 3);
    expect(interaction.selection).toEqual({ kind: "rail", id: RAIL });
    click(5, 6);

    expect(interaction.selection).toEqual({
      kind: "referenceMarker",
      id: MARKER,
    });
  });

  it("asks to delete the selected rail", () => {
    const { interaction, intents, click } = open();
    click(5, 5);

    interaction.deleteSelection();

    expect(intents).toEqual([
      { kind: "delete", target: { kind: "rail", id: RAIL } },
    ]);
  });

  it("tells the host when the selection changes, through the status", () => {
    const { interaction, click } = open();

    expect(interaction.status.selection).toBeNull();
    click(5, 5);

    expect(interaction.status.selection).toEqual({ kind: "rail", id: RAIL });
  });
});

describe("rail commands", () => {
  const schematic = drawing();

  it("turns the canvas intents into rail commands, and drops one for a rail that is gone", () => {
    expect(
      commandForIntent(
        { kind: "addRail", x: 3, top: 1, bottom: 9 },
        schematic,
        symbols,
      ),
    ).toEqual({ kind: "addRail", x: 3, top: 1, bottom: 9 });
    expect(
      commandForIntent(
        { kind: "editRail", railId: RAIL, x: 6, top: 0, bottom: 10 },
        schematic,
        symbols,
      ),
    ).toEqual({
      kind: "editRail",
      railId: RAIL,
      patch: { x: 6, top: 0, bottom: 10 },
    });
    expect(
      commandForIntent(
        {
          kind: "editRail",
          railId: "00000000-0000-4000-8000-0000000000ee",
          x: 1,
          top: 0,
          bottom: 5,
        },
        schematic,
        symbols,
      ),
    ).toBeNull();
    expect(
      commandForIntent(
        { kind: "delete", target: { kind: "rail", id: RAIL } },
        schematic,
        symbols,
      ),
    ).toEqual({ kind: "deleteRail", railId: RAIL });
  });

  it("applies each command to the document as one history entry", () => {
    const document = new SchematicDocumentStore(blankSchematic);

    applyEditorCommand(
      document,
      { kind: "addRail", x: 2, top: 0, bottom: 10 },
      null,
    );
    const id = document.getState().schematic.rails[0]!.id;
    applyEditorCommand(
      document,
      { kind: "editRail", railId: id, patch: { name: "L+" } },
      null,
    );
    applyEditorCommand(document, { kind: "deleteRail", railId: id }, null);

    expect(document.getState().schematic.rails).toEqual([]);
    document.undo();
    document.undo();
    expect(document.getState().schematic.rails[0]).toMatchObject({
      name: "RAIL1",
    });
  });

  it("is refused for an invalid name and leaves the document alone", () => {
    const document = new SchematicDocumentStore(blankSchematic);
    applyEditorCommand(
      document,
      { kind: "addRail", name: "L+", x: 2, top: 0, bottom: 10 },
      null,
    );
    const before = document.getState();

    expect(() =>
      applyEditorCommand(
        document,
        { kind: "addRail", name: "L+", x: 4, top: 0, bottom: 10 },
        null,
      ),
    ).toThrow(/already named/);
    expect(document.getState()).toBe(before);
  });

  it("is not available in an assignment", () => {
    const command = { kind: "addRail", x: 0, top: 0, bottom: 10 } as const;

    expect(commandVerdict(command, null)).toEqual({ allowed: true });
    expect(
      commandVerdict(command, { allowedTypes: null, locked: new Set() }),
    ).toMatchObject({ allowed: false });
  });
});
