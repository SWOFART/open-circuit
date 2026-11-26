import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import {
  CanvasInteraction,
  type CanvasPointer,
  type InteractionIntent,
} from "./interaction";
import { buildScene } from "./scene";
import { toScreen } from "./view";

// The multi-selection of docs/contracts/stage-16-editor-improvements.md section 2, on the
// divider: V1 at (0, 0), R1 at (4, 0), R2 at (8, 0), the wire between R1 and R2 along y = 0
// from x = 5 to 7, the two long wires above and below, and a ground marker at (1, 0).

const schematic = schematicFixture as Schematic;
const scene = buildScene(schematic, symbols);
const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const MIDDLE_WIRE = "00000000-0000-4000-8000-000000000005";
const MARKER = "00000000-0000-4000-8000-000000000007";
const VIEWPORT = { width: 800, height: 600 };

const component = (id: string) => ({ kind: "component", id }) as const;

function setup() {
  const intents: InteractionIntent[] = [];
  const interaction = new CanvasInteraction(
    () => {},
    (intent) => intents.push(intent),
  );
  interaction.setViewport(VIEWPORT);
  interaction.setScene(scene);
  return { interaction, intents };
}

function at(
  interaction: CanvasInteraction,
  grid: { x: number; y: number },
  extra: Partial<CanvasPointer> = {},
): CanvasPointer {
  return {
    ...toScreen(grid, interaction.view),
    button: 0,
    pointerId: 1,
    ...extra,
  };
}

/** Presses, moves and releases from `from` to `to` on the grid. */
function drag(
  interaction: CanvasInteraction,
  from: { x: number; y: number },
  to: { x: number; y: number },
  extra: Partial<CanvasPointer> = {},
): void {
  interaction.pointerDown(at(interaction, from, extra));
  interaction.pointerMove(at(interaction, to, extra));
  interaction.pointerUp(at(interaction, to, extra));
}

describe("the selection set", () => {
  it("is an ordered set whose last member is the primary one", () => {
    const { interaction } = setup();

    interaction.select(component(R1));
    interaction.toggleSelection(component(R2));
    interaction.toggleSelection({ kind: "wire", id: MIDDLE_WIRE });

    expect(interaction.selections).toEqual([
      component(R1),
      component(R2),
      { kind: "wire", id: MIDDLE_WIRE },
    ]);
    expect(interaction.selection).toEqual({ kind: "wire", id: MIDDLE_WIRE });
  });

  it("reports a single selection to the panels and nothing for several", () => {
    const { interaction } = setup();

    interaction.select(component(R1));
    expect(interaction.status.selection).toEqual(component(R1));

    interaction.toggleSelection(component(R2));
    expect(interaction.status.selection).toBeNull();
    expect(interaction.status.selections).toHaveLength(2);
  });

  it("never holds a duplicate or an object that is not drawn", () => {
    const { interaction } = setup();

    interaction.selectMany([
      component(R1),
      component(R1),
      { kind: "wire", id: R1 },
    ]);

    expect(interaction.selections).toEqual([component(R1)]);
  });

  it("removes a member when it is toggled again", () => {
    const { interaction } = setup();
    interaction.selectMany([component(R1), component(R2)]);

    interaction.toggleSelection(component(R1));

    expect(interaction.selections).toEqual([component(R2)]);
  });

  it("drops the members that vanish from the drawing and keeps the others", () => {
    const { interaction } = setup();
    interaction.selectMany([component(R1), component(R2)]);

    interaction.setScene(
      buildScene(
        {
          ...schematic,
          components: schematic.components.filter((item) => item.id !== R1),
        },
        symbols,
      ),
    );

    expect(interaction.selections).toEqual([component(R2)]);
  });

  it("selects every selectable object with select all", () => {
    const { interaction } = setup();

    interaction.selectAll();

    const kinds = interaction.selections.map(({ kind }) => kind).sort();
    expect(kinds).toEqual([
      "component",
      "component",
      "component",
      "referenceMarker",
      "wire",
      "wire",
      "wire",
    ]);
  });
});

describe("Shift-click", () => {
  it("adds an object to the selection and starts no drag", () => {
    const { interaction, intents } = setup();
    interaction.select(component(R1));

    expect(
      interaction.pointerDown(
        at(interaction, { x: 8, y: 0 }, { shiftKey: true }),
      ),
    ).toBe(false);

    expect(interaction.selections).toEqual([component(R1), component(R2)]);
    expect(intents).toEqual([]);
  });

  it("removes a member that is clicked again", () => {
    const { interaction } = setup();
    interaction.selectMany([component(R1), component(R2)]);

    interaction.pointerDown(
      at(interaction, { x: 8, y: 0 }, { shiftKey: true }),
    );

    expect(interaction.selections).toEqual([component(R1)]);
  });

  it("keeps the selection when empty canvas is clicked with Shift", () => {
    const { interaction } = setup();
    interaction.selectMany([component(R1), component(R2)]);

    interaction.pointerDown(
      at(interaction, { x: 6, y: 3 }, { shiftKey: true }),
    );
    interaction.pointerUp(at(interaction, { x: 6, y: 3 }, { shiftKey: true }));

    expect(interaction.selections).toHaveLength(2);
  });
});

describe("box selection", () => {
  // The two resistors and the wire between them are entirely inside this rectangle; the long
  // wires and the voltage source reach outside it.
  const FROM = { x: 2.4, y: -1.6 };
  const TO = { x: 9.6, y: 1.6 };

  it("selects the objects entirely inside, replacing the selection", () => {
    const { interaction } = setup();
    interaction.select(component(V1));

    drag(interaction, FROM, TO);

    expect(interaction.selections.map(({ id }) => id).sort()).toEqual(
      [R1, R2, MIDDLE_WIRE].sort(),
    );
  });

  it("does not select an object it only touches", () => {
    const { interaction } = setup();

    // Reaches into R2 but not past its right pin at x = 9: R1 and the wire are inside, R2 is
    // only touched.
    drag(interaction, FROM, { x: 8.2, y: 1.6 });

    const ids = interaction.selections.map(({ id }) => id).sort();
    expect(ids).toEqual([R1, MIDDLE_WIRE].sort());
    expect(ids).not.toContain(R2);
  });

  it("adds to the selection with Shift held at the press", () => {
    const { interaction } = setup();
    interaction.select(component(V1));

    drag(interaction, FROM, TO, { shiftKey: true });

    expect(interaction.selections).toHaveLength(4);
    expect(interaction.selections[0]).toEqual(component(V1));
  });

  it("draws a preview while the pointer moves and nothing is selected until release", () => {
    const { interaction } = setup();

    interaction.pointerDown(at(interaction, FROM));
    interaction.pointerMove(at(interaction, TO));

    expect(interaction.boxPreview).toEqual({
      minX: expect.closeTo(FROM.x, 5),
      minY: expect.closeTo(FROM.y, 5),
      maxX: expect.closeTo(TO.x, 5),
      maxY: expect.closeTo(TO.y, 5),
    });
    expect(interaction.selections).toEqual([]);

    interaction.pointerUp(at(interaction, TO));
    expect(interaction.boxPreview).toBeNull();
  });

  it("is a click, and selects nothing, when the rectangle is no bigger than a click", () => {
    const { interaction } = setup();
    interaction.select(component(R1));

    drag(interaction, FROM, { x: FROM.x + 0.05, y: FROM.y + 0.05 });

    expect(interaction.selections).toEqual([]);
  });

  it("is canceled by Escape, which selects nothing", () => {
    const { interaction } = setup();

    interaction.pointerDown(at(interaction, FROM));
    interaction.pointerMove(at(interaction, TO));
    interaction.escape();

    expect(interaction.boxPreview).toBeNull();
    expect(interaction.selections).toEqual([]);
  });
});

describe("moving a selection", () => {
  it("drags the whole group as one intent", () => {
    const { interaction, intents } = setup();
    interaction.selectMany([component(R1), component(R2)]);

    drag(interaction, { x: 4, y: 0 }, { x: 4, y: 2 });

    expect(intents).toEqual([
      {
        kind: "moveSelection",
        targets: [component(R1), component(R2)],
        delta: { x: 0, y: 2 },
      },
    ]);
    // The selection is kept, so the next drag moves the same group.
    expect(interaction.selections).toHaveLength(2);
  });

  it("previews the group while it is dragged", () => {
    const { interaction } = setup();
    interaction.selectMany([component(R1), component(R2)]);

    interaction.pointerDown(at(interaction, { x: 4, y: 0 }));
    interaction.pointerMove(at(interaction, { x: 5, y: 0 }));

    expect(interaction.dragPreview).toEqual({
      kind: "group",
      targets: [component(R1), component(R2)],
      delta: { x: 1, y: 0 },
    });
  });

  it("selects just the object that was clicked when a member is clicked without a drag", () => {
    const { interaction, intents } = setup();
    interaction.selectMany([component(R1), component(R2)]);

    interaction.pointerDown(at(interaction, { x: 4, y: 0 }));
    interaction.pointerUp(at(interaction, { x: 4, y: 0 }));

    expect(interaction.selections).toEqual([component(R1)]);
    expect(intents).toEqual([]);
  });

  it("moves one selected object as the group of one, by the arrow keys", () => {
    const { interaction, intents } = setup();
    interaction.select(component(R1));

    interaction.moveSelectionBy({ x: -1, y: 0 });

    expect(intents).toEqual([
      {
        kind: "moveSelection",
        targets: [component(R1)],
        delta: { x: -1, y: 0 },
      },
    ]);
  });

  it("does nothing for an arrow key when nothing is selected", () => {
    const { interaction, intents } = setup();

    interaction.moveSelectionBy({ x: 1, y: 0 });

    expect(intents).toEqual([]);
  });

  it("does nothing while a gesture is in progress", () => {
    const { interaction, intents } = setup();
    interaction.selectMany([component(R1), component(R2)]);
    interaction.pointerDown(at(interaction, { x: 4, y: 0 }));
    interaction.pointerMove(at(interaction, { x: 5, y: 0 }));

    interaction.moveSelectionBy({ x: 1, y: 0 });

    expect(intents).toEqual([]);
  });
});

describe("deleting a selection", () => {
  it("asks for one delete of everything selected when there are several objects", () => {
    const { interaction, intents } = setup();
    interaction.selectMany([component(R1), { kind: "wire", id: MIDDLE_WIRE }]);

    interaction.deleteSelection();

    expect(intents).toEqual([
      {
        kind: "deleteSelection",
        targets: [component(R1), { kind: "wire", id: MIDDLE_WIRE }],
      },
    ]);
  });

  it("keeps the single-object delete for one selected object", () => {
    const { interaction, intents } = setup();
    interaction.select({ kind: "referenceMarker", id: MARKER });

    interaction.deleteSelection();

    expect(intents).toEqual([
      { kind: "delete", target: { kind: "referenceMarker", id: MARKER } },
    ]);
  });
});

describe("Escape", () => {
  it("cancels a gesture first, then a tool, then clears the selection, one at a time", () => {
    const { interaction } = setup();
    interaction.selectMany([component(R1), component(R2)]);
    interaction.setTool({ kind: "wire" });
    interaction.setTool({ kind: "select" });
    interaction.selectMany([component(R1), component(R2)]);

    // A drag in progress: the first Escape cancels it and the selection stays.
    interaction.pointerDown(at(interaction, { x: 4, y: 0 }));
    interaction.pointerMove(at(interaction, { x: 5, y: 0 }));
    interaction.escape();
    expect(interaction.dragPreview).toBeNull();
    expect(interaction.selections).toHaveLength(2);

    // Nothing else is in the way: the next one clears the selection.
    interaction.escape();
    expect(interaction.selections).toEqual([]);
  });

  it("returns a drawing tool to Select before it touches the selection", () => {
    const { interaction } = setup();
    interaction.setTool({ kind: "wire" });
    interaction.selectMany([component(R1)]);

    interaction.escape();

    expect(interaction.tool).toEqual({ kind: "select" });
  });
});
