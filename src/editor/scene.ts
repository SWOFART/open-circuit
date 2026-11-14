import type { DeviceState } from "../generated/DeviceState";
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
  "component" | "wire" | "junction" | "referenceMarker" | "rail";

/** The schematic object a shape belongs to; selection and hit tests use it. */
export interface SceneOwner {
  readonly kind: SelectableKind;
  readonly id: StableId;
}

export type SceneShape =
  | {
      readonly kind: "polyline";
      readonly role: "wire" | "symbol" | "reference" | "rail";
      readonly owner: SceneOwner;
      readonly points: readonly ScenePoint[];
      /** A marker with no pin or wire at its point; drawn as a warning. */
      readonly detached?: boolean;
      /** A rail that a diagnostic names; drawn as a warning, like a detached marker. */
      readonly flagged?: boolean;
    }
  | {
      readonly kind: "circle";
      readonly role: "junction" | "symbol" | "railEnd";
      readonly owner: SceneOwner;
      readonly center: ScenePoint;
      readonly radius: number;
      readonly detached?: boolean;
      readonly flagged?: boolean;
    }
  | {
      readonly kind: "label";
      readonly owner: SceneOwner;
      readonly text: string;
      readonly position: ScenePoint;
      /** Half the text's extent in grid units; the default is `LABEL_HALF_WIDTH`. */
      readonly halfWidth?: number;
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
  /**
   * The device states of the run on screen, by component ID. A part without one is drawn
   * at rest. The scene draws what the Engine reported and decides nothing.
   */
  readonly states?: ReadonlyMap<StableId, DeviceState>;
  /** Adds derived connection dots and flags detached markers. */
  readonly geometry?: EditorGeometry;
  /** Rails that diagnostics name, drawn as warnings. */
  readonly flaggedRails?: ReadonlySet<StableId>;
}

export const JUNCTION_RADIUS = 0.12;
// Labels are text, so their extent is only approximated for fitting purposes.
export const LABEL_HALF_WIDTH = 0.8;
export const LABEL_HALF_HEIGHT = 0.3;
const LABEL_OFFSET_Y = -1;
// A rail name sits above the line's top end, centred on it. Its extent follows its length:
// the rendered text is about this many grid units wide per character at the default zoom.
export const RAIL_LABEL_OFFSET_Y = -0.8;
export const RAIL_END_RADIUS = 0.1;
const RAIL_LABEL_HALF_WIDTH_PER_CHARACTER = 0.2;

/** Half the extent of a rail's name, in grid units. */
export function railLabelHalfWidth(name: string): number {
  return Math.max(
    LABEL_HALF_WIDTH / 2,
    name.length * RAIL_LABEL_HALF_WIDTH_PER_CHARACTER,
  );
}

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
    case "label": {
      const halfWidth = shape.halfWidth ?? LABEL_HALF_WIDTH;
      return [
        {
          x: shape.position.x - halfWidth,
          y: shape.position.y - LABEL_HALF_HEIGHT,
        },
        {
          x: shape.position.x + halfWidth,
          y: shape.position.y + LABEL_HALF_HEIGHT,
        },
      ];
    }
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

  for (const rail of schematic.rails) {
    const owner: SceneOwner = { kind: "rail", id: rail.id };
    const flagged = options.flaggedRails?.has(rail.id) ? { flagged: true } : {};
    const [start, end] = [rail.start, rail.end].map((point) => ({
      x: point.x,
      y: point.y,
    })) as [ScenePoint, ScenePoint];

    shapes.push({
      kind: "polyline",
      role: "rail",
      owner,
      ...flagged,
      points: [start, end],
    });
    for (const center of [start, end]) {
      shapes.push({
        kind: "circle",
        role: "railEnd",
        owner,
        ...flagged,
        center,
        radius: RAIL_END_RADIUS,
      });
    }
    shapes.push({
      kind: "label",
      owner,
      // Plain text: never markup, and never an identity.
      text: rail.name,
      position: { x: start.x, y: start.y + RAIL_LABEL_OFFSET_Y },
      halfWidth: railLabelHalfWidth(rail.name),
    });
  }

  for (const component of schematic.components) {
    const symbol = symbolFor(component.typeId, registry);
    const owner: SceneOwner = { kind: "component", id: component.id };

    const primitives = symbol.drawn
      ? symbol.drawn(options.states?.get(component.id))
      : symbol.primitives;

    for (const primitive of primitives) {
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
