import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import connectivity from "../../tests/golden/stage-2-connectivity.json";
import drawings from "../../tests/golden/stage-2-drawings.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import { SchematicDocumentStore } from "../store/document";
import { dividerSchematic } from "../store/fixtures";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { commandForIntent } from "./commands";
import { analyzeGeometry } from "./connectivity";
import { CanvasInteraction } from "./interaction";
import { buildScene } from "./scene";
import { toScreen } from "./view";

const catalog = catalogFixture as ComponentCatalog;

type Step =
  | {
      tool: "wire";
      clicks: number[][];
      finish?: "doubleClick" | "enter";
    }
  | { tool: "junction" | "reference"; clicks: number[][] }
  | { tool: "delete"; click: number[] };

interface Drawing {
  ids?: string[];
  steps: Step[];
}

const drawingsByName = drawings.drawings as unknown as Record<string, Drawing>;

/**
 * Drives the real tools with grid clicks, applies each completed action to the
 * document store as the application does, and keeps the canvas model in step.
 */
function editor(ids: readonly string[]) {
  const queue = [...ids];
  const store = new SchematicDocumentStore(
    {
      ...dividerSchematic,
      wires: [],
      junctions: [],
      referenceMarkers: [],
    },
    () => {
      const next = queue.shift();
      if (!next) throw new Error("The drawing needed more IDs than listed.");
      return next;
    },
  );

  const refresh = (): void => {
    const schematic = store.getState().schematic;
    const geometry = analyzeGeometry(schematic, symbols);
    interaction.setScene(
      buildScene(schematic, symbols, { geometry }),
      geometry,
    );
  };
  const interaction: CanvasInteraction = new CanvasInteraction(
    () => {},
    (intent) => {
      const command = commandForIntent(
        intent,
        store.getState().schematic,
        symbols,
      );
      if (!command) throw new Error(`The editor ignored ${intent.kind}.`);
      applyEditorCommand(store, command, catalog);
      refresh();
    },
  );
  interaction.setViewport({ width: 800, height: 600 });
  refresh();

  const click = ([x, y]: number[]): void => {
    const pointer = {
      ...toScreen({ x: x ?? 0, y: y ?? 0 }, interaction.view),
      button: 0,
      pointerId: 1,
    };
    interaction.pointerDown(pointer);
    interaction.pointerUp(pointer);
  };

  return {
    store,
    interaction,
    run(step: Step): void {
      if (step.tool === "delete") {
        interaction.setTool({ kind: "select" });
        click(step.click);
        interaction.deleteSelection();
        return;
      }

      interaction.setTool({ kind: step.tool });
      step.clicks.forEach(click);

      if (step.tool === "wire" && step.finish === "doubleClick") {
        const last = step.clicks[step.clicks.length - 1];
        if (last) click(last);
      } else if (step.tool === "wire" && step.finish === "enter") {
        interaction.finishRoute();
      }
    },
  };
}

function schematicOf(drawing: (typeof connectivity.cases)[number]): Schematic {
  return {
    ...dividerSchematic,
    wires: drawing.wires,
    junctions: drawing.junctions,
    referenceMarkers: drawing.referenceMarkers,
  } as Schematic;
}

describe("drawings made with the editor tools", () => {
  it("has a recorded drawing for every connectivity case", () => {
    expect(Object.keys(drawingsByName).sort()).toEqual(
      connectivity.cases.map((drawing) => drawing.name).sort(),
    );
  });

  it.each(connectivity.cases)(
    "reproduces $name exactly, so the Engine's verdict on the fixture applies",
    (drawing) => {
      const recorded = drawingsByName[drawing.name];
      if (!recorded) throw new Error(`No recorded drawing for ${drawing.name}`);
      const ids = recorded.ids ?? [
        ...drawing.wires.map((wire) => wire.id),
        ...drawing.junctions.map((junction) => junction.id),
        ...drawing.referenceMarkers.map((marker) => marker.id),
      ];
      const { store, interaction, run } = editor(ids);

      for (const step of recorded.steps) {
        run(step);
        expect(interaction.status.routing).toBe(false);
      }

      expect(store.getState().schematic).toEqual(schematicOf(drawing));
    },
  );

  it("undoes a whole drawing one finished wire at a time", () => {
    const { store, run } = editor([
      "00000000-0000-4000-8000-000000000101",
      "00000000-0000-4000-8000-000000000102",
    ]);

    run({
      tool: "wire",
      clicks: [
        [5, 0],
        [7, 0],
      ],
    });
    run({
      tool: "wire",
      clicks: [
        [3, 0],
        [3, -3],
        [6, -3],
        [6, 3],
        [9, 3],
        [9, 0],
      ],
    });
    expect(store.getState().schematic.wires).toHaveLength(2);

    store.undo();
    expect(store.getState().schematic.wires).toHaveLength(1);
    store.undo();
    expect(store.getState().schematic.wires).toHaveLength(0);
    expect(store.getState().isDirty).toBe(false);
  });

  it("leaves the document alone when a junction is refused", () => {
    const { store, interaction, run } = editor([
      "00000000-0000-4000-8000-000000000101",
    ]);
    run({
      tool: "wire",
      clicks: [
        [5, 0],
        [7, 0],
      ],
    });
    const before = store.getState().schematic;

    run({ tool: "junction", clicks: [[6, 0]] });

    expect(store.getState().schematic).toBe(before);
    expect(interaction.status.rejected).toBe("crossing");
  });
});
