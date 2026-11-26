import type { ComponentSpec } from "../generated/ComponentSpec";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbolFor, type SymbolRegistry } from "../symbols";
import { resolvePinPositions, toWorld, type Rotation } from "./geometry";
import type { Bounds } from "./scene";

/** Where a component sits on the grid and how it is turned. */
export interface ComponentPlacement {
  readonly position: GridPoint;
  readonly rotationDeg: Rotation;
}

/** A replacement path for a wire, applied in the same transaction. */
export interface WireEdit {
  readonly id: StableId;
  readonly points: readonly GridPoint[];
}

export function rotateClockwise(rotation: Rotation): Rotation {
  return ((rotation + 90) % 360) as Rotation;
}

function samePoint(a: GridPoint, b: GridPoint): boolean {
  return a.x === b.x && a.y === b.y;
}

function key(point: GridPoint): string {
  return `${point.x},${point.y}`;
}

function strictlyBetween(value: number, a: number, b: number): boolean {
  return Math.min(a, b) < value && value < Math.max(a, b);
}

/** True when `middle` lies strictly inside the straight run from a to b. */
function passesThrough(a: GridPoint, middle: GridPoint, b: GridPoint): boolean {
  return (
    (a.x === middle.x &&
      middle.x === b.x &&
      strictlyBetween(middle.y, a.y, b.y)) ||
    (a.y === middle.y &&
      middle.y === b.y &&
      strictlyBetween(middle.x, a.x, b.x))
  );
}

/**
 * Removes repeated points and points that lie strictly inside a straight run,
 * so the path has no zero-length segments. A turning point where the path
 * doubles back is kept, so no drawn extent, and no contact along it, is lost.
 */
export function simplifyPath(points: readonly GridPoint[]): GridPoint[] {
  const path: GridPoint[] = [];

  for (const point of points) {
    const last = path[path.length - 1];
    if (last && samePoint(last, point)) {
      continue;
    }

    const beforeLast = path[path.length - 2];
    if (last && beforeLast && passesThrough(beforeLast, last, point)) {
      path[path.length - 1] = { x: point.x, y: point.y };
      continue;
    }

    path.push({ x: point.x, y: point.y });
  }

  return path;
}

/** The extent of a placed component's symbol drawing, in grid units. */
export function bodyBounds(
  component: ComponentSpec,
  registry: SymbolRegistry,
): Bounds {
  const points = symbolFor(component.typeId, registry).primitives.flatMap(
    (primitive) =>
      primitive.kind === "polyline"
        ? primitive.points
        : [
            {
              x: primitive.center.x - primitive.radius,
              y: primitive.center.y - primitive.radius,
            },
            {
              x: primitive.center.x + primitive.radius,
              y: primitive.center.y + primitive.radius,
            },
          ],
  );
  const world = points.map((point) => toWorld(point, component));

  return {
    minX: Math.min(...world.map((point) => point.x)),
    minY: Math.min(...world.map((point) => point.y)),
    maxX: Math.max(...world.map((point) => point.x)),
    maxY: Math.max(...world.map((point) => point.y)),
  };
}

function onSegment(point: GridPoint, a: GridPoint, b: GridPoint): boolean {
  return (
    (a.x === b.x &&
      point.x === a.x &&
      point.y >= Math.min(a.y, b.y) &&
      point.y <= Math.max(a.y, b.y)) ||
    (a.y === b.y &&
      point.y === a.y &&
      point.x >= Math.min(a.x, b.x) &&
      point.x <= Math.max(a.x, b.x))
  );
}

/** True when an axis-aligned segment runs through the inside of `box`. */
function crossesInterior(a: GridPoint, b: GridPoint, box: Bounds): boolean {
  if (a.x === b.x) {
    return (
      box.minX < a.x &&
      a.x < box.maxX &&
      Math.max(a.y, b.y) > box.minY &&
      Math.min(a.y, b.y) < box.maxY
    );
  }
  return (
    box.minY < a.y &&
    a.y < box.maxY &&
    Math.max(a.x, b.x) > box.minX &&
    Math.min(a.x, b.x) < box.maxX
  );
}

/** Where the moved component is, so a rubber band can route around it. */
interface Obstacle {
  readonly body: Bounds;
  readonly pins: readonly GridPoint[];
}

/**
 * How badly the segments `start → bend → end` run into the moved component:
 * through its body, or over one of its other pins (which would connect them).
 */
function routeCost(
  start: GridPoint,
  bend: GridPoint,
  end: GridPoint,
  obstacle: Obstacle,
): number {
  let cost = 0;

  for (const [a, b] of [
    [start, bend],
    [bend, end],
  ] as const) {
    if (samePoint(a, b)) {
      continue;
    }
    if (crossesInterior(a, b, obstacle.body)) {
      cost += 1;
    }
    cost += obstacle.pins.filter(
      (pin) => !samePoint(pin, start) && onSegment(pin, a, b),
    ).length;
  }

  return cost;
}

/**
 * Moves the first point of an orthogonal path to `target` and bends once to
 * rejoin the old second point (an L-shaped rubber band). Of the two possible
 * bends, the one that avoids the moved component's body and other pins wins;
 * on a tie the path leaves in the direction its old first segment had.
 */
function moveStart(
  points: readonly GridPoint[],
  target: GridPoint,
  obstacle: Obstacle,
): GridPoint[] {
  const [first, second, ...rest] = points;
  if (!first || !second) {
    return [target];
  }

  const horizontalFirst = { x: second.x, y: target.y };
  const verticalFirst = { x: target.x, y: second.y };
  const [preferred, other] =
    first.y === second.y
      ? [horizontalFirst, verticalFirst]
      : [verticalFirst, horizontalFirst];
  const bend =
    routeCost(target, other, second, obstacle) <
    routeCost(target, preferred, second, obstacle)
      ? other
      : preferred;

  // Only the new bend and the joint after it are tidied; the untouched rest
  // of the wire stays as drawn.
  const [next, ...tail] = rest;
  return [
    ...simplifyPath(
      next ? [target, bend, second, next] : [target, bend, second],
    ),
    ...tail,
  ];
}

/**
 * Re-routes every wire whose first or last point lies exactly on a pin of the
 * component, so that endpoint follows the pin to its new position. Pins that
 * only touch a wire's interior have no stored attachment and are not followed
 * (docs/contracts/stage-2-editor.md section 3.2). A wire that would collapse
 * to a single point is left unchanged.
 */
export function followingWireEdits(
  schematic: Schematic,
  componentId: StableId,
  placement: ComponentPlacement,
  registry: SymbolRegistry,
): WireEdit[] {
  const component = schematic.components.find(
    (item) => item.id === componentId,
  );
  if (!component) {
    return [];
  }

  const moved = { ...component, ...placement } as ComponentSpec;
  const before = resolvePinPositions(component, registry);
  const movedPins = resolvePinPositions(moved, registry);
  const after = new Map(movedPins.map((pin) => [pin.pinId, pin.position]));
  const obstacle: Obstacle = {
    body: bodyBounds(moved, registry),
    pins: movedPins.map((pin) => pin.position),
  };
  const targets = new Map<string, GridPoint>();
  for (const pin of before) {
    const target = after.get(pin.pinId);
    if (target && !samePoint(target, pin.position)) {
      targets.set(key(pin.position), target);
    }
  }

  return rerouteEnds(schematic, (point) => targets.get(key(point)), obstacle);
}

/** A point that holds nothing: a rubber band that has no part to avoid. */
const NO_OBSTACLE: Obstacle = {
  body: { minX: 0, minY: 0, maxX: 0, maxY: 0 },
  pins: [],
};

/**
 * Re-routes the wires whose first or last point `targetFor` moves, as an L-shaped rubber band
 * (the endpoint follows, the other end stays). Wires in `skip` are left alone. A wire that
 * would collapse to a single point is left unchanged.
 */
function rerouteEnds(
  schematic: Schematic,
  targetFor: (point: GridPoint) => GridPoint | undefined,
  obstacle: Obstacle,
  skip: ReadonlySet<StableId> = new Set(),
): WireEdit[] {
  const edits: WireEdit[] = [];
  for (const wire of schematic.wires) {
    const first = wire.points[0];
    const last = wire.points[wire.points.length - 1];
    if (!first || !last || skip.has(wire.id)) {
      continue;
    }

    const startTarget = targetFor(first);
    const endTarget = targetFor(last);
    if (!startTarget && !endTarget) {
      continue;
    }

    let path: GridPoint[] = [...wire.points];
    if (startTarget) {
      path = moveStart(path, startTarget, obstacle);
    }
    if (endTarget) {
      path = moveStart(path.reverse(), endTarget, obstacle).reverse();
    }

    // Adjacent points are distinct here, so two points make a valid path.
    if (path.length >= 2) {
      edits.push({ id: wire.id, points: path });
    }
  }

  return edits;
}

/** One component of a group move, and where it goes. */
export interface GroupComponentMove {
  readonly componentId: StableId;
  readonly placement: ComponentPlacement;
}

/**
 * Re-routes, once each, the wires that end on a pin of a moved component or on a point that
 * `extraTarget` moves (a junction, a place on a moved rail), for a move of several things at
 * once. Every wire end is matched against the **original** drawing, so a wire that is also
 * translated or re-routed for another part is never matched a second time by a pin it only
 * happens to land on. Wires in `skip` (the ones moved whole) are left alone. Each edit names the
 * component whose pin its first matched end sat on (nothing for an end that follows a junction
 * or a rail), so the caller can apply it in that component's own transaction.
 */
export function followingGroupWireEdits(
  schematic: Schematic,
  moves: readonly GroupComponentMove[],
  registry: SymbolRegistry,
  extraTarget: (point: GridPoint) => GridPoint | undefined,
  skip: ReadonlySet<StableId>,
): { id: StableId; points: GridPoint[]; owner: StableId | null }[] {
  interface EndTarget {
    readonly to: GridPoint;
    readonly obstacle: Obstacle;
    readonly owner: StableId | null;
  }
  const pinTargets = new Map<string, EndTarget>();
  for (const { componentId, placement } of moves) {
    const component = schematic.components.find(
      (item) => item.id === componentId,
    );
    if (!component) {
      continue;
    }

    const moved = { ...component, ...placement } as ComponentSpec;
    const movedPins = resolvePinPositions(moved, registry);
    const after = new Map(movedPins.map((pin) => [pin.pinId, pin.position]));
    const obstacle: Obstacle = {
      body: bodyBounds(moved, registry),
      pins: movedPins.map((pin) => pin.position),
    };
    for (const pin of resolvePinPositions(component, registry)) {
      const to = after.get(pin.pinId);
      // Pins on one point move together; the first part to claim the point keeps it.
      if (
        to &&
        !samePoint(to, pin.position) &&
        !pinTargets.has(key(pin.position))
      ) {
        pinTargets.set(key(pin.position), { to, obstacle, owner: componentId });
      }
    }
  }

  const targetFor = (point: GridPoint): EndTarget | undefined => {
    const pin = pinTargets.get(key(point));
    if (pin) {
      return pin;
    }
    const to = extraTarget(point);
    return to ? { to, obstacle: NO_OBSTACLE, owner: null } : undefined;
  };

  const edits: { id: StableId; points: GridPoint[]; owner: StableId | null }[] =
    [];
  for (const wire of schematic.wires) {
    const first = wire.points[0];
    const last = wire.points[wire.points.length - 1];
    if (!first || !last || skip.has(wire.id)) {
      continue;
    }

    const start = targetFor(first);
    const end = targetFor(last);
    if (!start && !end) {
      continue;
    }

    let path: GridPoint[] = [...wire.points];
    if (start) {
      path = moveStart(path, start.to, start.obstacle);
    }
    if (end) {
      // The first move can collapse a short wire onto the old place of its other end, which is
      // itself moving: then the wire is simply re-drawn between the two new places.
      path =
        path.length >= 2
          ? moveStart(path.reverse(), end.to, end.obstacle).reverse()
          : simplifyPath([
              start?.to ?? first,
              ...(start && start.to.x !== end.to.x && start.to.y !== end.to.y
                ? [{ x: end.to.x, y: start.to.y }]
                : []),
              end.to,
            ]);
    }

    // Adjacent points are distinct here, so two points make a valid path.
    if (path.length >= 2) {
      edits.push({
        id: wire.id,
        points: path,
        owner: start?.owner ?? end?.owner ?? null,
      });
    }
  }

  return edits;
}

/** The schematic as it would look after the transform; used for previews. */
export function applyComponentTransform(
  schematic: Schematic,
  componentId: StableId,
  placement: ComponentPlacement,
  wireEdits: readonly WireEdit[],
): Schematic {
  const paths = new Map(wireEdits.map((edit) => [edit.id, edit.points]));

  return {
    ...schematic,
    components: schematic.components.map((component) =>
      component.id === componentId
        ? ({ ...component, ...placement } as ComponentSpec)
        : component,
    ),
    wires: schematic.wires.map((wire) => {
      const points = paths.get(wire.id);
      return points ? { ...wire, points: [...points] } : wire;
    }),
  };
}
