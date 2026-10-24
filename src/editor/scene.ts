import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { LocalPoint, SymbolRegistry } from "../symbols";
import { toWorld } from "./geometry";

export interface ScenePoint {
  readonly x: number;
  readonly y: number;
}

export interface Bounds {
  readonly minX: number;
  readonly minY: number;
  readonly maxX: number;
  readonly maxY: number;
}

export type SceneShape =
  | {
      readonly kind: "polyline";
      readonly role: "wire" | "symbol" | "reference";
      readonly points: readonly ScenePoint[];
    }
  | {
      readonly kind: "circle";
      readonly role: "junction" | "symbol";
      readonly center: ScenePoint;
      readonly radius: number;
    }
  | {
      readonly kind: "label";
      readonly componentId: StableId;
      readonly text: string;
      readonly position: ScenePoint;
    };

/** Everything the renderer draws, in logical grid units. No Pixi types. */
export interface SceneModel {
  readonly shapes: readonly SceneShape[];
  readonly bounds: Bounds;
}

export interface SceneOptions {
  readonly labels?: ReadonlyMap<StableId, string>;
}

const JUNCTION_RADIUS = 0.12;
// Labels are text, so their extent is only approximated for fitting purposes.
const LABEL_HALF_WIDTH = 0.8;
const LABEL_HALF_HEIGHT = 0.3;
const LABEL_OFFSET_Y = -1;

function referenceMarkerShapes(position: GridPoint): SceneShape[] {
  const { x, y } = position;
  const bar = (halfWidth: number, offset: number): SceneShape => ({
    kind: "polyline",
    role: "reference",
    points: [
      { x: x - halfWidth, y: y + offset },
      { x: x + halfWidth, y: y + offset },
    ],
  });

  return [
    {
      kind: "polyline",
      role: "reference",
      points: [
        { x, y },
        { x, y: y + 0.6 },
      ],
    },
    bar(0.4, 0.6),
    bar(0.25, 0.8),
    bar(0.1, 1),
  ];
}

function shapePoints(shape: SceneShape): ScenePoint[] {
  switch (shape.kind) {
    case "polyline":
      return [...shape.points];
    case "circle":
      return [
        { x: shape.center.x - shape.radius, y: shape.center.y - shape.radius },
        { x: shape.center.x + shape.radius, y: shape.center.y + shape.radius },
      ];
    case "label":
      return [
        {
          x: shape.position.x - LABEL_HALF_WIDTH,
          y: shape.position.y - LABEL_HALF_HEIGHT,
        },
        {
          x: shape.position.x + LABEL_HALF_WIDTH,
          y: shape.position.y + LABEL_HALF_HEIGHT,
        },
      ];
  }
}

function boundsOf(shapes: readonly SceneShape[]): Bounds {
  const points = shapes.flatMap(shapePoints);

  if (points.length === 0) {
    return { minX: 0, minY: 0, maxX: 0, maxY: 0 };
  }

  return {
    minX: Math.min(...points.map((point) => point.x)),
    minY: Math.min(...points.map((point) => point.y)),
    maxX: Math.max(...points.map((point) => point.x)),
    maxY: Math.max(...points.map((point) => point.y)),
  };
}

function worldPoint(
  local: LocalPoint,
  component: Schematic["components"][number],
): ScenePoint {
  return toWorld(local, component);
}

/** Builds the drawable scene from the saved schematic and symbol registry. */
export function buildScene(
  schematic: Schematic,
  registry: SymbolRegistry,
  options: SceneOptions = {},
): SceneModel {
  const shapes: SceneShape[] = [];

  for (const wire of schematic.wires) {
    shapes.push({
      kind: "polyline",
      role: "wire",
      points: wire.points.map((point) => ({ x: point.x, y: point.y })),
    });
  }

  for (const component of schematic.components) {
    const symbol = registry[component.typeId];

    for (const primitive of symbol.primitives) {
      if (primitive.kind === "polyline") {
        shapes.push({
          kind: "polyline",
          role: "symbol",
          points: primitive.points.map((point) => worldPoint(point, component)),
        });
      } else {
        shapes.push({
          kind: "circle",
          role: "symbol",
          center: worldPoint(primitive.center, component),
          radius: primitive.radius,
        });
      }
    }

    const label = options.labels?.get(component.id);

    if (label !== undefined) {
      shapes.push({
        kind: "label",
        componentId: component.id,
        text: label,
        position: {
          x: component.position.x,
          y: component.position.y + LABEL_OFFSET_Y,
        },
      });
    }
  }

  for (const junction of schematic.junctions) {
    shapes.push({
      kind: "circle",
      role: "junction",
      center: { x: junction.position.x, y: junction.position.y },
      radius: JUNCTION_RADIUS,
    });
  }

  for (const marker of schematic.referenceMarkers) {
    shapes.push(...referenceMarkerShapes(marker.position));
  }

  return { shapes, bounds: boundsOf(shapes) };
}
