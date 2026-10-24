import type { StableId } from "../generated/StableId";
import {
  boundsOf,
  LABEL_HALF_HEIGHT,
  LABEL_HALF_WIDTH,
  type Bounds,
  type SceneModel,
  type SceneOwner,
  type ScenePoint,
  type SceneShape,
  type SelectableKind,
} from "./scene";

/** The one selected schematic object. Multi-selection is deferred. */
export type SelectionTarget = SceneOwner;

// Lower wins. Matches the drawing order: point markers are drawn over
// components, and components over wires.
const PRIORITY: Readonly<Record<SelectableKind, number>> = {
  junction: 0,
  referenceMarker: 0,
  component: 1,
  wire: 2,
};

function distance(a: ScenePoint, b: ScenePoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function distanceToSegment(
  point: ScenePoint,
  start: ScenePoint,
  end: ScenePoint,
): number {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSquared = dx * dx + dy * dy;

  if (lengthSquared === 0) {
    return distance(point, start);
  }

  const t = Math.min(
    Math.max(
      ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSquared,
      0,
    ),
    1,
  );

  return distance(point, { x: start.x + t * dx, y: start.y + t * dy });
}

/** Zero inside the box, otherwise the distance to its edge. */
function distanceToBounds(point: ScenePoint, bounds: Bounds): number {
  const dx = Math.max(bounds.minX - point.x, 0, point.x - bounds.maxX);
  const dy = Math.max(bounds.minY - point.y, 0, point.y - bounds.maxY);

  return Math.hypot(dx, dy);
}

function shapeDistance(shape: SceneShape, point: ScenePoint): number {
  switch (shape.kind) {
    case "polyline": {
      const [first, ...rest] = shape.points;
      if (!first) {
        return Infinity;
      }

      let best = distance(point, first);
      let previous = first;
      for (const next of rest) {
        best = Math.min(best, distanceToSegment(point, previous, next));
        previous = next;
      }
      return best;
    }
    case "circle":
      // Junction dots are filled, and a symbol circle is a body to click on.
      return Math.max(distance(point, shape.center) - shape.radius, 0);
    case "label":
      return distanceToBounds(point, {
        minX: shape.position.x - LABEL_HALF_WIDTH,
        minY: shape.position.y - LABEL_HALF_HEIGHT,
        maxX: shape.position.x + LABEL_HALF_WIDTH,
        maxY: shape.position.y + LABEL_HALF_HEIGHT,
      });
  }
}

/** The area of each component's symbol, so clicks inside its body hit it. */
function componentBodies(scene: SceneModel): Map<StableId, Bounds> {
  const shapesById = new Map<StableId, SceneShape[]>();

  for (const shape of scene.shapes) {
    if (shape.owner.kind === "component" && shape.kind !== "label") {
      const list = shapesById.get(shape.owner.id) ?? [];
      list.push(shape);
      shapesById.set(shape.owner.id, list);
    }
  }

  return new Map(
    [...shapesById].map(([id, shapes]) => [id, boundsOf(shapes)] as const),
  );
}

/**
 * Finds the visible object under `point` (grid units). `tolerance` is the
 * pick radius in grid units. Point markers win over components and
 * components over wires; within a class the nearest wins, and on a tie the
 * object drawn last (on top) wins.
 */
export function hitTest(
  scene: SceneModel,
  point: ScenePoint,
  tolerance: number,
): SelectionTarget | null {
  const bodies = componentBodies(scene);
  let best: { owner: SceneOwner; priority: number; distance: number } | null =
    null;

  for (const shape of scene.shapes) {
    const body =
      shape.owner.kind === "component" && shape.kind !== "label"
        ? bodies.get(shape.owner.id)
        : undefined;
    const shapeDistanceToPoint = body
      ? distanceToBounds(point, body)
      : shapeDistance(shape, point);

    if (shapeDistanceToPoint > tolerance) {
      continue;
    }

    const priority = PRIORITY[shape.owner.kind];
    if (
      best === null ||
      priority < best.priority ||
      (priority === best.priority && shapeDistanceToPoint <= best.distance)
    ) {
      best = { owner: shape.owner, priority, distance: shapeDistanceToPoint };
    }
  }

  return best ? { kind: best.owner.kind, id: best.owner.id } : null;
}

/** True when `target` still names an object drawn in `scene`. */
export function sceneContains(
  scene: SceneModel,
  target: SelectionTarget,
): boolean {
  return scene.shapes.some(
    (shape) => shape.owner.kind === target.kind && shape.owner.id === target.id,
  );
}
