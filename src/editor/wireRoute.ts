import type { GridPoint } from "../generated/GridPoint";
import { simplifyPath } from "./transform";

/**
 * The orthogonal leg from `from` to `to`. Points that already share a row or
 * column are joined directly; otherwise one bend is inserted, horizontal-first
 * by default and vertical-first when `verticalFirst` is set.
 */
export function leg(
  from: GridPoint,
  to: GridPoint,
  verticalFirst: boolean,
): GridPoint[] {
  if (from.x === to.x || from.y === to.y) {
    return [to];
  }

  const bend = verticalFirst ? { x: from.x, y: to.y } : { x: to.x, y: from.y };

  return [bend, to];
}

/** Appends the leg to `to` and tidies the path around the new bend. */
export function extendRoute(
  path: readonly GridPoint[],
  to: GridPoint,
  verticalFirst: boolean,
): GridPoint[] {
  const last = path[path.length - 1];
  if (!last) {
    return [{ x: to.x, y: to.y }];
  }

  return simplifyPath([...path, ...leg(last, to, verticalFirst)]);
}

/**
 * A committed wire has at least two distinct points, no repeated adjacent
 * points, and only horizontal or vertical segments.
 */
export function isValidWirePath(points: readonly GridPoint[]): boolean {
  if (points.length < 2) {
    return false;
  }

  return points.every((point, index) => {
    const previous = points[index - 1];
    return (
      previous === undefined ||
      (previous.x === point.x) !== (previous.y === point.y)
    );
  });
}

function copy(point: GridPoint): GridPoint {
  return { x: point.x, y: point.y };
}

/**
 * Moves vertex `index` to `to`, keeping both neighbouring segments orthogonal.
 * Each neighbour keeps its old direction: it leaves the old neighbour the way
 * the old segment did, then bends once to reach the moved vertex. Returns null
 * when no valid path results, so the edit is not committed.
 */
export function moveVertex(
  points: readonly GridPoint[],
  index: number,
  to: GridPoint,
): GridPoint[] | null {
  const vertex = points[index];
  if (!vertex) {
    return null;
  }

  const previous = points[index - 1];
  const next = points[index + 1];
  const path: GridPoint[] = points.slice(0, index).map(copy);

  if (previous && previous.x !== to.x && previous.y !== to.y) {
    // The old segment was horizontal when both ends shared a row.
    path.push(
      previous.y === vertex.y
        ? { x: to.x, y: previous.y }
        : { x: previous.x, y: to.y },
    );
  }

  path.push(copy(to));

  if (next && next.x !== to.x && next.y !== to.y) {
    path.push(
      vertex.y === next.y ? { x: next.x, y: to.y } : { x: to.x, y: next.y },
    );
  }

  path.push(...points.slice(index + 1).map(copy));
  const result = simplifyPath(path);

  return isValidWirePath(result) ? result : null;
}

export function samePath(
  a: readonly GridPoint[],
  b: readonly GridPoint[],
): boolean {
  return (
    a.length === b.length &&
    a.every((point, index) => {
      const other = b[index];
      return other !== undefined && point.x === other.x && point.y === other.y;
    })
  );
}
