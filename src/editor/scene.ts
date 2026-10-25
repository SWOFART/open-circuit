import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbolFor, type LocalPoint, type SymbolRegistry } from "../symbols";
import type { EditorGeometry } from "./connectivity";
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

export type SelectableKind =
  "component" | "wire" | "junction" | "referenceMarker";

/** The schematic object a shape belongs to; selection and hit tests use it. */
export interface SceneOwner {
  readonly kind: SelectableKind;
  readonly id: StableId;
}

export type SceneShape =
  | {
      readonly kind: "polyline";
      readonly role: "wire" | "symbol" | "reference";
      readonly owner: SceneOwner;
      readonly points: readonly ScenePoint[];
      /** A marker with no pin or wire at its point; drawn as a warning. */
      readonly detached?: boolean;
    }
  | {
      readonly kind: "circle";
      readonly role: "junction" | "symbol";
      readonly owner: SceneOwner;
      readonly center: ScenePoint;
      readonly radius: number;
      readonly detached?: boolean;
    }
  | {
      readonly kind: "label";
      readonly owner: SceneOwner;
      readonly text: string;
      readonly position: ScenePoint;
    };

/** Everything the renderer draws, in logical grid units. No Pixi types. */
export interface SceneModel {
  readonly shapes: readonly SceneShape[];
  readonly bounds: Bounds;
  /** Derived connection dots (T-junctions and similar). Not selectable. */
  readonly connectionDots: readonly ScenePoint[];
}

export interface SceneOptions {
  readonly labels?: ReadonlyMap<StableId, string>;
  /** Adds derived connection dots and flags detached markers. */
  readonly geometry?: EditorGeometry;
}

export const JUNCTION_RADIUS = 0.12;
// Labels are text, so their extent is only approximated for fitting purposes.
export const LABEL_HALF_WIDTH = 0.8;
export const LABEL_HALF_HEIGHT = 0.3;
const LABEL_OFFSET_Y = -1;

/** The ground glyph of a reference marker whose stem starts at `position`. */
export function referenceMarkerShapes(
  position: GridPoint,
  owner: SceneOwner,
  detached = false,
): SceneShape[] {
  const { x, y } = position;
  const flag = detached ? { detached } : {};
  const bar = (halfWidth: number, offset: number): SceneShape => ({
    kind: "polyline",
    role: "reference",
    owner,
    ...flag,
    points: [
      { x: x - halfWidth, y: y + offset },
      { x: x + halfWidth, y: y + offset },
    ],
  });

  return [
    {
      kind: "polyline",
      role: "reference",
      owner,
      ...flag,
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

export function boundsOf(shapes: readonly SceneShape[]): Bounds {
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
      owner: { kind: "wire", id: wire.id },
      points: wire.points.map((point) => ({ x: point.x, y: point.y })),
    });
  }

  for (const component of schematic.components) {
    const symbol = symbolFor(component.typeId, registry);
    const owner: SceneOwner = { kind: "component", id: component.id };

    for (const primitive of symbol.primitives) {
      if (primitive.kind === "polyline") {
        shapes.push({
          kind: "polyline",
          role: "symbol",
          owner,
          points: primitive.points.map((point) => worldPoint(point, component)),
        });
      } else {
        shapes.push({
          kind: "circle",
          role: "symbol",
          owner,
          center: worldPoint(primitive.center, component),
          radius: primitive.radius,
        });
      }
    }

    const label = options.labels?.get(component.id);

    if (label !== undefined) {
      shapes.push({
        kind: "label",
        owner,
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
      owner: { kind: "junction", id: junction.id },
      ...(options.geometry?.detachedMarkers.has(junction.id)
        ? { detached: true }
        : {}),
      center: { x: junction.position.x, y: junction.position.y },
      radius: JUNCTION_RADIUS,
    });
  }

  for (const marker of schematic.referenceMarkers) {
    shapes.push(
      ...referenceMarkerShapes(
        marker.position,
        { kind: "referenceMarker", id: marker.id },
        options.geometry?.detachedMarkers.has(marker.id) ?? false,
      ),
    );
  }

  return {
    shapes,
    bounds: boundsOf(shapes),
    connectionDots: (options.geometry?.connectionDots ?? []).map((point) => ({
      x: point.x,
      y: point.y,
    })),
  };
}
