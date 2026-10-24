import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import {
  CanvasInteraction,
  EMPTY_VIEW,
  type CanvasPointer,
  type InteractionChange,
} from "./interaction";
import { buildScene } from "./scene";
import { fitView, snapToGrid, toGrid, toScreen } from "./view";

const schematic = schematicFixture as Schematic;
const scene = buildScene(schematic, symbols);
const R1 = "00000000-0000-4000-8000-000000000002";
const MIDDLE_WIRE = "00000000-0000-4000-8000-000000000005";
const VIEWPORT = { width: 800, height: 600 };

function setup(): {
  interaction: CanvasInteraction;
  changes: InteractionChange[];
} {
  const changes: InteractionChange[] = [];
  const interaction = new CanvasInteraction((change) => changes.push(change));
  interaction.setViewport(VIEWPORT);
  interaction.setScene(scene);
  return { interaction, changes };
}

function at(
  interaction: CanvasInteraction,
  grid: { x: number; y: number },
  button = 0,
  pointerId = 1,
): CanvasPointer {
  return { ...toScreen(grid, interaction.view), button, pointerId };
}

describe("initial view", () => {
  it("shows the empty-canvas view before a scene is set", () => {
    const interaction = new CanvasInteraction();
    interaction.setViewport(VIEWPORT);

    expect(interaction.view).toEqual(EMPTY_VIEW);
  });

  it("fits the circuit and follows resizes until the user pans or zooms", () => {
    const { interaction } = setup();

    expect(interaction.view).toEqual(fitView(scene.bounds, VIEWPORT));

    interaction.setViewport({ width: 500, height: 400 });
    expect(interaction.view).toEqual(
      fitView(scene.bounds, { width: 500, height: 400 }),
    );
  });

  it("does not refit when the schematic changes", () => {
    const { interaction } = setup();
    const before = interaction.view;
    const moved = buildScene(
      {
        ...schematic,
        components: schematic.components.map((component) => ({
          ...component,
          position: { x: component.position.x + 30, y: component.position.y },
        })),
      },
      symbols,
    );

    interaction.setScene(moved);

    expect(interaction.view).toEqual(before);
  });
});

describe("selection", () => {
  it("selects the object under a left click and clears it on empty canvas", () => {
    const { interaction, changes } = setup();

    expect(interaction.pointerDown(at(interaction, { x: 4, y: 0 }))).toBe(
      false,
    );
    expect(interaction.selection).toEqual({ kind: "component", id: R1 });
    expect(changes).toContain("selection");

    interaction.pointerUp(at(interaction, { x: 4, y: 0 }));
    interaction.pointerDown(at(interaction, { x: 6, y: 0 }));
    expect(interaction.selection).toEqual({ kind: "wire", id: MIDDLE_WIRE });

    interaction.pointerUp(at(interaction, { x: 6, y: 0 }));
    interaction.pointerDown(at(interaction, { x: 6, y: 2 }));
    expect(interaction.selection).toBeNull();
  });

  it("uses a pick radius in screen pixels, independent of zoom", () => {
    const { interaction } = setup();
    // 4 px beside the middle wire at the current zoom.
    const pointer = at(interaction, { x: 6, y: 0 });

    interaction.pointerDown({ ...pointer, y: pointer.y + 4 });
    expect(interaction.selection).toEqual({ kind: "wire", id: MIDDLE_WIRE });

    interaction.pointerUp(pointer);
    interaction.pointerDown({ ...pointer, y: pointer.y + 9 });
    expect(interaction.selection).toBeNull();
  });

  it("does not notify when the same object is selected again", () => {
    const { interaction, changes } = setup();

    interaction.select({ kind: "component", id: R1 });
    changes.length = 0;
    interaction.select({ kind: "component", id: R1 });

    expect(changes).toEqual([]);
  });

  it("ignores right-button clicks", () => {
    const { interaction } = setup();
    interaction.select({ kind: "component", id: R1 });

    interaction.pointerDown(at(interaction, { x: 6, y: 2 }, 2));

    expect(interaction.selection).toEqual({ kind: "component", id: R1 });
  });

  it("keeps the selection by ID when the schematic is redrawn", () => {
    const { interaction } = setup();
    interaction.select({ kind: "component", id: R1 });
    const edited: Schematic = {
      ...schematic,
      components: schematic.components.map((component) =>
        component.id === R1 && component.typeId === "resistor"
          ? { ...component, parameters: { resistanceOhms: 3000 } }
          : component,
      ),
    };

    interaction.setScene(buildScene(edited, symbols));

    expect(interaction.selection).toEqual({ kind: "component", id: R1 });
  });

  it("clears the selection when its object is removed", () => {
    const { interaction, changes } = setup();
    interaction.select({ kind: "component", id: R1 });
    changes.length = 0;

    interaction.setScene(
      buildScene(
        {
          ...schematic,
          components: schematic.components.filter((item) => item.id !== R1),
        },
        symbols,
      ),
    );

    expect(interaction.selection).toBeNull();
    expect(changes).toEqual(["selection"]);
  });

  it("refuses to select an object that is not drawn", () => {
    const { interaction } = setup();

    interaction.select({ kind: "wire", id: R1 });

    expect(interaction.selection).toBeNull();
  });
});

describe("pointer snapping", () => {
  it("tracks the snapped grid point under the pointer", () => {
    const { interaction, changes } = setup();
    const pointer = at(interaction, { x: 2.3, y: -1.6 });

    interaction.pointerMove(pointer);

    expect(interaction.pointerGridPoint).toEqual({ x: 2, y: -2 });
    expect(changes).toContain("pointer");
  });

  it("only notifies when the snapped point changes", () => {
    const { interaction, changes } = setup();
    interaction.pointerMove(at(interaction, { x: 2.1, y: 0 }));
    changes.length = 0;

    interaction.pointerMove(at(interaction, { x: 2.2, y: 0.1 }));

    expect(changes).toEqual([]);
  });

  it("re-snaps the pointer when the view is refitted", () => {
    const { interaction } = setup();
    const pixel = { x: 400, y: 300, button: 0, pointerId: 1 };
    interaction.pointerMove(pixel);

    interaction.setViewport({ width: 300, height: 200 });

    expect(interaction.pointerGridPoint).toEqual(
      snapToGrid(toGrid(pixel, interaction.view)),
    );
  });

  it("forgets the pointer when it leaves the canvas", () => {
    const { interaction } = setup();
    interaction.pointerMove(at(interaction, { x: 2, y: 0 }));

    interaction.pointerLeave();

    expect(interaction.pointerGridPoint).toBeNull();
  });
});

describe("panning", () => {
  it("pans with a middle-button drag and keeps the selection", () => {
    const { interaction, changes } = setup();
    interaction.select({ kind: "component", id: R1 });
    // Park the pointer first, so only the pan's own notifications remain.
    interaction.pointerMove({ x: 100, y: 100, button: 1, pointerId: 7 });
    const start = interaction.view;
    changes.length = 0;

    expect(
      interaction.pointerDown({ x: 100, y: 100, button: 1, pointerId: 7 }),
    ).toBe(true);
    expect(interaction.isPanning).toBe(true);
    interaction.pointerMove({ x: 130, y: 90, button: 1, pointerId: 7 });
    interaction.pointerMove({ x: 150, y: 120, button: 1, pointerId: 7 });
    interaction.pointerUp({ x: 150, y: 120, button: 1, pointerId: 7 });

    expect(interaction.isPanning).toBe(false);
    expect(changes.filter((change) => change === "pointer")).toHaveLength(2);
    expect(interaction.view).toEqual({
      scale: start.scale,
      offsetX: start.offsetX + 50,
      offsetY: start.offsetY + 20,
    });
    expect(interaction.selection).toEqual({ kind: "component", id: R1 });
  });

  it("pans with Space plus a left-button drag instead of selecting", () => {
    const { interaction } = setup();
    const onR1 = at(interaction, { x: 4, y: 0 });
    const start = interaction.view;

    interaction.setPanModifier(true);
    expect(interaction.pointerDown(onR1)).toBe(true);
    interaction.pointerMove({ ...onR1, x: onR1.x - 40 });
    interaction.pointerUp({ ...onR1, x: onR1.x - 40 });

    expect(interaction.selection).toBeNull();
    expect(interaction.view.offsetX).toBe(start.offsetX - 40);
  });

  it("keeps the user's view on resize after panning, until Fit to circuit", () => {
    const { interaction } = setup();
    interaction.pointerDown({ x: 0, y: 0, button: 1, pointerId: 1 });
    interaction.pointerMove({ x: 25, y: 0, button: 1, pointerId: 1 });
    interaction.pointerUp({ x: 25, y: 0, button: 1, pointerId: 1 });
    const panned = interaction.view;

    interaction.setViewport({ width: 500, height: 400 });
    expect(interaction.view).toEqual(panned);

    interaction.fitToCircuit();
    expect(interaction.view).toEqual(
      fitView(scene.bounds, { width: 500, height: 400 }),
    );
  });

  it("recovers when the release of a pan was missed", () => {
    const { interaction } = setup();
    interaction.pointerDown({ x: 0, y: 0, button: 1, pointerId: 1 });

    // The same pointer presses again without a pointerup in between.
    interaction.pointerDown(at(interaction, { x: 4, y: 0 }, 0, 1));

    expect(interaction.isPanning).toBe(false);
    expect(interaction.selection).toEqual({ kind: "component", id: R1 });
  });

  it("ignores moves from another pointer and ends on cancel", () => {
    const { interaction } = setup();
    const start = interaction.view;
    interaction.pointerDown({ x: 0, y: 0, button: 1, pointerId: 1 });

    interaction.pointerMove({ x: 50, y: 50, button: 0, pointerId: 2 });
    expect(interaction.view).toEqual(start);
    expect(
      interaction.pointerDown({ x: 0, y: 0, button: 1, pointerId: 2 }),
    ).toBe(false);

    interaction.pointerCancel(1);
    expect(interaction.isPanning).toBe(false);
    interaction.pointerMove({ x: 80, y: 80, button: 1, pointerId: 1 });
    expect(interaction.view).toEqual(start);
  });
});

describe("wheel zoom", () => {
  it("zooms in and out around the pointer", () => {
    const { interaction } = setup();
    const pointer = { x: 523, y: 211 };
    const anchor = toGrid(pointer, interaction.view);
    const startScale = interaction.view.scale;

    interaction.wheel({ ...pointer, deltaY: -100, deltaMode: 0 });
    expect(interaction.view.scale).toBeGreaterThan(startScale);
    let after = toGrid(pointer, interaction.view);
    expect(after.x).toBeCloseTo(anchor.x, 9);
    expect(after.y).toBeCloseTo(anchor.y, 9);

    interaction.wheel({ ...pointer, deltaY: 300, deltaMode: 0 });
    expect(interaction.view.scale).toBeLessThan(startScale);
    after = toGrid(pointer, interaction.view);
    expect(after.x).toBeCloseTo(anchor.x, 9);
    expect(after.y).toBeCloseTo(anchor.y, 9);
  });

  it("treats line-mode deltas like 16 px per line", () => {
    const pixels = setup().interaction;
    const lines = setup().interaction;

    pixels.wheel({ x: 10, y: 10, deltaY: 48, deltaMode: 0 });
    lines.wheel({ x: 10, y: 10, deltaY: 3, deltaMode: 1 });

    expect(lines.view.scale).toBeCloseTo(pixels.view.scale, 12);
  });

  it("stops following resizes after a zoom", () => {
    const { interaction } = setup();
    interaction.wheel({ x: 10, y: 10, deltaY: -100, deltaMode: 0 });
    const zoomed = interaction.view;

    interaction.setViewport({ width: 300, height: 300 });

    expect(interaction.view).toEqual(zoomed);
  });

  it("does not report a change when zoom is already at its limit", () => {
    const { interaction, changes } = setup();
    for (let index = 0; index < 50; index += 1) {
      interaction.wheel({ x: 10, y: 10, deltaY: -1000, deltaMode: 0 });
    }
    changes.length = 0;

    interaction.wheel({ x: 10, y: 10, deltaY: -1000, deltaMode: 0 });

    expect(changes).toEqual([]);
  });
});
