import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SymbolRegistry } from "../symbols";
import { resolvePinPositions } from "./geometry";

/**
 * Editor affordances derived from the drawn geometry: where a route may end,
 * where an explicit junction is useful, where to draw a connection dot, and
 * which markers hang in the air.
 *
 * These are only hints for drawing. The Rust Engine owns the electrical
 * connection rules (docs/contracts/stage-1.md section 2.1) and the diagnostics;
 * `tests/golden/stage-2-connectivity.json` pins both sides to the same cases.
 */
export interface EditorGeometry {
  /** Every wire path by ID. */
  readonly wires: ReadonlyMap<StableId, readonly GridPoint[]>;
  readonly junctions: ReadonlyMap<StableId, GridPoint>;
  readonly referenceMarkers: ReadonlyMap<StableId, GridPoint>;
  /** Points where the Junction tool may place a marker. */
  readonly junctionSpots: readonly GridPoint[];
  /** Derived connection dots: T-junctions and similar joins. Never saved. */
  readonly connectionDots: readonly GridPoint[];
  /** Junctions and reference markers with no pin or wire at their point. */
  readonly detachedMarkers: ReadonlySet<StableId>;
  /** A pin or a point of a wire (ends included) is exactly at `point`. */
  isAttachment(point: GridPoint): boolean;
  /** The Junction tool may place a marker at `point`. */
  isJunctionSpot(point: GridPoint): boolean;
  /** A reference marker already sits at `point`. */
  hasReferenceMarkerAt(point: GridPoint): boolean;
}

export const EMPTY_GEOMETRY: EditorGeometry = {
  wires: new Map(),
  junctions: new Map(),
  referenceMarkers: new Map(),
  junctionSpots: [],
  connectionDots: [],
  detachedMarkers: new Set(),
  isAttachment: () => false,
  isJunctionSpot: () => false,
  hasReferenceMarkerAt: () => false,
};

interface Segment {
  readonly wireId: StableId;
  readonly a: GridPoint;
  readonly b: GridPoint;
}

function keyOf(point: GridPoint): string {
  return `${point.x},${point.y}`;
}

function between(value: number, a: number, b: number): boolean {
  return Math.min(a, b) <= value && value <= Math.max(a, b);
}

function samePoint(a: GridPoint, b: GridPoint): boolean {
  return a.x === b.x && a.y === b.y;
}

/** `point` lies on the axis-aligned segment, its ends included. */
function onSegment(point: GridPoint, segment: Segment): boolean {
  const { a, b } = segment;

  return a.x === b.x
    ? point.x === a.x && between(point.y, a.y, b.y)
    : point.y === a.y && between(point.x, a.x, b.x);
}

function isEnd(point: GridPoint, segment: Segment): boolean {
  return samePoint(point, segment.a) || samePoint(point, segment.b);
}

function strictlyInside(point: GridPoint, segment: Segment): boolean {
  return onSegment(point, segment) && !isEnd(point, segment);
}

function isHorizontal(segment: Segment): boolean {
  return segment.a.y === segment.b.y && segment.a.x !== segment.b.x;
}

function isVertical(segment: Segment): boolean {
  return segment.a.x === segment.b.x && segment.a.y !== segment.b.y;
}

function wireSegments(
  wires: ReadonlyMap<StableId, readonly GridPoint[]>,
): Segment[] {
  const segments: Segment[] = [];

  for (const [wireId, points] of wires) {
    for (let index = 1; index < points.length; index += 1) {
      const a = points[index - 1];
      const b = points[index];
      if (a && b) {
        segments.push({ wireId, a, b });
      }
    }
  }

  return segments;
}

function countPoints(points: readonly GridPoint[]): Map<string, number> {
  const counts = new Map<string, number>();

  for (const point of points) {
    counts.set(keyOf(point), (counts.get(keyOf(point)) ?? 0) + 1);
  }

  return counts;
}

/**
 * Derives the editor geometry of `schematic`. Pin positions come from the
 * symbol registry, exactly as for the Engine's compile request.
 */
export function analyzeGeometry(
  schematic: Schematic,
  registry: SymbolRegistry,
): EditorGeometry {
  const wires = new Map(
    schematic.wires.map((wire) => [wire.id, wire.points] as const),
  );
  const junctions = new Map(
    schematic.junctions.map((item) => [item.id, item.position] as const),
  );
  const referenceMarkers = new Map(
    schematic.referenceMarkers.map((item) => [item.id, item.position] as const),
  );
  const pins = schematic.components.flatMap((component) =>
    resolvePinPositions(component, registry).map((pin) => pin.position),
  );
  const segments = wireSegments(wires);

  const pinCounts = countPoints(pins);
  const endCounts = countPoints(
    segments.flatMap((segment) => [segment.a, segment.b]),
  );
  const junctionKeys = new Set([...junctions.values()].map(keyOf));
  const referenceKeys = new Set([...referenceMarkers.values()].map(keyOf));

  const isAttachment = (point: GridPoint): boolean =>
    pinCounts.has(keyOf(point)) ||
    segments.some((segment) => onSegment(point, segment));

  // A spot is a place where two different wire paths cross with the crossing
  // strictly inside both segments and nothing else already connects there.
  // A path crossing itself is one conductor already.
  const spots = new Map<string, GridPoint>();
  for (const [index, first] of segments.entries()) {
    for (const second of segments.slice(index + 1)) {
      if (first.wireId === second.wireId) {
        continue;
      }

      const horizontal = isHorizontal(first) ? first : second;
      const vertical = isVertical(first) ? first : second;
      if (!isHorizontal(horizontal) || !isVertical(vertical)) {
        continue;
      }

      const crossing = { x: vertical.a.x, y: horizontal.a.y };
      const key = keyOf(crossing);
      if (
        strictlyInside(crossing, horizontal) &&
        strictlyInside(crossing, vertical) &&
        !endCounts.has(key) &&
        !pinCounts.has(key) &&
        !junctionKeys.has(key) &&
        !referenceKeys.has(key)
      ) {
        spots.set(key, crossing);
      }
    }
  }

  // A dot marks a join of at least three arms: a segment that ends at the point
  // is one arm, one that passes through is two, and a pin is one. Two arms are
  // a plain corner or a lead-in. A bare crossing never has an end, pin, or
  // marker, so it is not a candidate; an explicit junction draws its own dot.
  const arms = (point: GridPoint): number =>
    (pinCounts.get(keyOf(point)) ?? 0) +
    segments.reduce(
      (total, segment) =>
        total +
        (isEnd(point, segment) ? 1 : strictlyInside(point, segment) ? 2 : 0),
      0,
    );
  const candidates = new Map<string, GridPoint>();
  for (const point of [
    ...pins,
    ...segments.flatMap((segment) => [segment.a, segment.b]),
    ...referenceMarkers.values(),
  ]) {
    candidates.set(keyOf(point), point);
  }
  const connectionDots = [...candidates.values()].filter(
    (point) => !junctionKeys.has(keyOf(point)) && arms(point) >= 3,
  );

  const detachedMarkers = new Set<StableId>();
  for (const [id, position] of [...junctions, ...referenceMarkers]) {
    if (!isAttachment(position)) {
      detachedMarkers.add(id);
    }
  }

  return {
    wires,
    junctions,
    referenceMarkers,
    junctionSpots: [...spots.values()],
    connectionDots,
    detachedMarkers,
    isAttachment,
    isJunctionSpot: (point) => spots.has(keyOf(point)),
    hasReferenceMarkerAt: (point) => referenceKeys.has(keyOf(point)),
  };
}
