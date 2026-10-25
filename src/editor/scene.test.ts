import { describe, expect, it } from "vitest";
import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { componentLabels } from "../store/fixtures";
import { buildScene, type Bounds, type SceneShape } from "./scene";
import { analyzeGeometry } from "./connectivity";
import { fitView, MAX_SCALE_PX_PER_UNIT, toScreen } from "./view";

const schematic = schematicFixture as Schematic;

function count(
  shapes: readonly SceneShape[],
  kind: SceneShape["kind"],
  role?: string,
): number {
  return shapes.filter(
    (shape) =>
      shape.kind === kind &&
      (role === undefined || ("role" in shape && shape.role === role)),
  ).length;
}

describe("scene from the canonical fixture", () => {
  const scene = buildScene(schematic, symbols, {
    labels: componentLabels(schematic),
  });

  it("draws the three wires, the symbols, the labels, and the ground marker", () => {
    expect(count(scene.shapes, "polyline", "wire")).toBe(3);
    expect(count(scene.shapes, "polyline", "symbol")).toBe(
      symbols.dc_voltage_source.primitives.length +
        2 * symbols.resistor.primitives.length,
    );
    expect(count(scene.shapes, "label")).toBe(3);
    expect(count(scene.shapes, "polyline", "reference")).toBe(4);
    expect(count(scene.shapes, "circle")).toBe(0);
  });

  it("labels the components V1, R1, and R2 in file order", () => {
    const labels = scene.shapes.flatMap((shape) =>
      shape.kind === "label" ? [shape.text] : [],
    );

    expect(labels).toEqual(["V1", "R1", "R2"]);
  });

  it("covers every wire point and pin within its bounds", () => {
    const { bounds } = scene;

    for (const wire of schematic.wires) {
      for (const point of wire.points) {
        expect(point.x).toBeGreaterThanOrEqual(bounds.minX);
        expect(point.x).toBeLessThanOrEqual(bounds.maxX);
        expect(point.y).toBeGreaterThanOrEqual(bounds.minY);
        expect(point.y).toBeLessThanOrEqual(bounds.maxY);
      }
    }
  });

  it("tags every shape with the schematic object it belongs to", () => {
    const owners = new Set(
      scene.shapes.map((shape) => `${shape.owner.kind}:${shape.owner.id}`),
    );

    expect([...owners].sort()).toEqual(
      [
        ...schematic.components.map((item) => `component:${item.id}`),
        ...schematic.wires.map((item) => `wire:${item.id}`),
        ...schematic.referenceMarkers.map(
          (item) => `referenceMarker:${item.id}`,
        ),
      ].sort(),
    );
    for (const shape of scene.shapes) {
      if (shape.kind === "label") {
        expect(shape.owner.kind).toBe("component");
      }
    }
  });

  it("draws a junction as a filled dot", () => {
    const withJunction: Schematic = {
      ...schematic,
      junctions: [
        {
          id: "00000000-0000-4000-8000-0000000000f2",
          position: { x: 3, y: 4 },
        },
      ],
    };
    const junctionScene = buildScene(withJunction, symbols);

    expect(count(junctionScene.shapes, "circle", "junction")).toBe(1);
  });
});

describe("fitting the scene into the window", () => {
  const { bounds } = buildScene(schematic, symbols);
  const corners = [
    { x: bounds.minX, y: bounds.minY },
    { x: bounds.maxX, y: bounds.maxY },
  ];

  it.each([
    { width: 1280, height: 800 },
    { width: 900, height: 600 },
    { width: 400, height: 300 },
    { width: 300, height: 900 },
    { width: 3000, height: 500 },
    { width: 90, height: 90 },
  ])(
    "keeps the schematic visible in a $width x $height viewport",
    (viewport) => {
      const view = fitView(bounds, viewport);

      for (const corner of corners) {
        const screen = toScreen(corner, view);

        expect(screen.x).toBeGreaterThanOrEqual(0);
        expect(screen.x).toBeLessThanOrEqual(viewport.width);
        expect(screen.y).toBeGreaterThanOrEqual(0);
        expect(screen.y).toBeLessThanOrEqual(viewport.height);
      }
    },
  );

  it("centers the schematic in the viewport", () => {
    const viewport = { width: 1000, height: 700 };
    const view = fitView(bounds, viewport);
    const center = toScreen(
      {
        x: (bounds.minX + bounds.maxX) / 2,
        y: (bounds.minY + bounds.maxY) / 2,
      },
      view,
    );

    expect(center.x).toBeCloseTo(viewport.width / 2, 6);
    expect(center.y).toBeCloseTo(viewport.height / 2, 6);
  });

  it("does not enlarge a small circuit beyond the maximum scale", () => {
    const tiny: Bounds = { minX: 0, minY: 0, maxX: 1, maxY: 1 };

    expect(fitView(tiny, { width: 4000, height: 4000 }).scale).toBe(
      MAX_SCALE_PX_PER_UNIT,
    );
  });

  it("stays finite for an empty scene and a collapsed viewport", () => {
    const empty = buildScene(
      { ...schematic, components: [], wires: [], referenceMarkers: [] },
      symbols,
    );
    const view = fitView(empty.bounds, { width: 0, height: 0 });

    expect(Number.isFinite(view.scale)).toBe(true);
    expect(view.scale).toBeGreaterThan(0);
    expect(Number.isFinite(view.offsetX)).toBe(true);
    expect(Number.isFinite(view.offsetY)).toBe(true);
  });
});

describe("scene of drawn connections", () => {
  const geometrySchematic: Schematic = {
    ...schematic,
    wires: [
      {
        id: "00000000-0000-4000-8000-000000000101",
        points: [
          { x: 3, y: 4 },
          { x: 9, y: 4 },
        ],
      },
      {
        id: "00000000-0000-4000-8000-000000000102",
        points: [
          { x: 6, y: 1 },
          { x: 6, y: 4 },
        ],
      },
    ],
    junctions: [
      {
        id: "00000000-0000-4000-8000-0000000001a1",
        position: { x: 20, y: 20 },
      },
    ],
    referenceMarkers: [
      {
        id: "00000000-0000-4000-8000-0000000001b1",
        position: { x: 6, y: 4 },
      },
      {
        id: "00000000-0000-4000-8000-0000000001b2",
        position: { x: 30, y: 30 },
      },
    ],
  };

  it("adds nothing derived when no geometry is supplied", () => {
    const scene = buildScene(geometrySchematic, symbols);

    expect(scene.connectionDots).toEqual([]);
    expect(
      scene.shapes.some((shape) => "detached" in shape && shape.detached),
    ).toBe(false);
  });

  it("carries the derived connection dots, which are not selectable", () => {
    const geometry = analyzeGeometry(geometrySchematic, symbols);
    const scene = buildScene(geometrySchematic, symbols, { geometry });

    expect(scene.connectionDots).toEqual([{ x: 6, y: 4 }]);
    // Only the one explicit junction is a shape; the dot is not an object.
    expect(
      scene.shapes.filter(
        (shape) => shape.kind === "circle" && shape.role === "junction",
      ),
    ).toHaveLength(1);
  });

  it("flags markers with nothing under them, and only those", () => {
    const geometry = analyzeGeometry(geometrySchematic, symbols);
    const scene = buildScene(geometrySchematic, symbols, { geometry });
    const detachedOwners = new Set(
      scene.shapes.flatMap((shape) =>
        "detached" in shape && shape.detached ? [shape.owner.id] : [],
      ),
    );

    expect(detachedOwners).toEqual(
      new Set([
        "00000000-0000-4000-8000-0000000001a1",
        "00000000-0000-4000-8000-0000000001b2",
      ]),
    );
  });
});
