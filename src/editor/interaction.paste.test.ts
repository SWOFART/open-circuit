import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import type { ClipboardSchematic } from "./clipboard";
import {
  CanvasInteraction,
  type CanvasPointer,
  type InteractionIntent,
} from "./interaction";
import { buildScene } from "./scene";
import { toScreen } from "./view";

// Placing a paste (docs/contracts/stage-16-editor-improvements.md section 4.4): a preview that
// follows the pointer, committed by a click or Enter, canceled by Escape.

const schematic = schematicFixture as Schematic;
const R1 = "00000000-0000-4000-8000-000000000002";
const copy: ClipboardSchematic = {
  components: [schematic.components[1]!],
  wires: [],
  junctions: [],
  referenceMarkers: [],
  rails: [],
  notes: [],
};
const PLACEMENT = {
  schematic: copy,
  anchor: { x: 3, y: -1 },
  delta: { x: 2, y: 2 },
};

function setup() {
  const intents: InteractionIntent[] = [];
  const interaction = new CanvasInteraction(
    () => {},
    (intent) => intents.push(intent),
  );
  interaction.setViewport({ width: 800, height: 600 });
  interaction.setScene(buildScene(schematic, symbols));
  return { interaction, intents };
}

const at = (
  interaction: CanvasInteraction,
  grid: { x: number; y: number },
): CanvasPointer => ({
  ...toScreen(grid, interaction.view),
  button: 0,
  pointerId: 1,
});

describe("placing a paste", () => {
  it("shows the preview at the starting offset until the pointer moves", () => {
    const { interaction } = setup();

    interaction.startPaste(PLACEMENT);

    expect(interaction.tool).toEqual({ kind: "paste" });
    expect(interaction.pastePreview).toEqual({
      schematic: copy,
      delta: { x: 2, y: 2 },
    });
  });

  it("puts the top-left corner of the bounds on the pointer's grid point", () => {
    const { interaction } = setup();
    interaction.startPaste(PLACEMENT);

    interaction.pointerMove(at(interaction, { x: 10, y: 5 }));

    // The corner was at (3, -1), so a corner at (10, 5) is a move of (7, 6).
    expect(interaction.pastePreview?.delta).toEqual({ x: 7, y: 6 });
  });

  it("commits where the click is, as one intent, and returns to Select", () => {
    const { interaction, intents } = setup();
    interaction.startPaste(PLACEMENT);

    interaction.pointerDown(at(interaction, { x: 6, y: 3 }));

    expect(intents).toEqual([
      { kind: "paste", schematic: copy, delta: { x: 3, y: 4 } },
    ]);
    expect(interaction.tool).toEqual({ kind: "select" });
    expect(interaction.pastePreview).toBeNull();
  });

  it("commits at the preview's place with Enter", () => {
    const { interaction, intents } = setup();
    interaction.startPaste(PLACEMENT);
    interaction.pointerMove(at(interaction, { x: 5, y: 1 }));

    expect(interaction.commitPaste()).toBe(true);

    expect(intents).toEqual([
      { kind: "paste", schematic: copy, delta: { x: 2, y: 2 } },
    ]);
    // A second Enter has nothing to commit.
    expect(interaction.commitPaste()).toBe(false);
  });

  it("is canceled by Escape, which commits nothing and keeps the selection empty", () => {
    const { interaction, intents } = setup();
    interaction.startPaste(PLACEMENT);

    interaction.escape();

    expect(intents).toEqual([]);
    expect(interaction.tool).toEqual({ kind: "select" });
    expect(interaction.pastePreview).toBeNull();
  });

  it("is dropped when another tool is chosen", () => {
    const { interaction, intents } = setup();
    interaction.startPaste(PLACEMENT);

    interaction.setTool({ kind: "wire" });

    expect(interaction.pastePreview).toBeNull();
    expect(intents).toEqual([]);
  });

  it("does not move the preview for a right-button press", () => {
    const { interaction, intents } = setup();
    interaction.startPaste(PLACEMENT);

    interaction.pointerDown({ ...at(interaction, { x: 6, y: 3 }), button: 2 });

    expect(intents).toEqual([]);
    expect(interaction.tool).toEqual({ kind: "paste" });
  });
});

describe("asking for a clipboard action", () => {
  it("passes the selection to the host", () => {
    const { interaction, intents } = setup();
    interaction.select({ kind: "component", id: R1 });

    interaction.clipboard("copy");

    expect(intents).toEqual([
      {
        kind: "clipboard",
        action: "copy",
        targets: [{ kind: "component", id: R1 }],
      },
    ]);
  });

  it("does nothing during a gesture", () => {
    const { interaction, intents } = setup();
    interaction.select({ kind: "component", id: R1 });
    interaction.pointerDown(at(interaction, { x: 4, y: 0 }));
    interaction.pointerMove(at(interaction, { x: 5, y: 0 }));

    interaction.clipboard("cut");

    expect(intents).toEqual([]);
  });
});
