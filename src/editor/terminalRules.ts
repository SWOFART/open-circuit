import type { GridPoint } from "../generated/GridPoint";

export const MAX_TERMINALS = 4096;

export function validTerminalName(strip: string, label: string): boolean {
  return (
    /^[A-Z]/.test(strip) &&
    strip.length <= 8 &&
    !/[^A-Z0-9]/.test(strip) &&
    label.length >= 1 &&
    label.length <= 8 &&
    !/[^A-Za-z0-9+._-]/.test(label)
  );
}

export function validWireNumber(number: string): boolean {
  return (
    number.length >= 1 &&
    number.length <= 16 &&
    !/[^A-Za-z0-9+._-]/.test(number)
  );
}

export function samePoint(a: GridPoint, b: GridPoint): boolean {
  return a.x === b.x && a.y === b.y;
}

const distance = (a: GridPoint, b: GridPoint) =>
  Math.abs(a.x - b.x) + Math.abs(a.y - b.y);

/** Distance from the route's first point, or null when detached. */
export function wireOffset(
  points: readonly GridPoint[],
  point: GridPoint,
): number | null {
  let travelled = 0;
  for (let index = 1; index < points.length; index += 1) {
    const a = points[index - 1];
    const b = points[index];
    const length = distance(a, b);
    if (distance(a, point) + distance(point, b) === length) {
      return travelled + distance(a, point);
    }
    travelled += length;
  }
  return null;
}

export function pointAtWireOffset(
  points: readonly GridPoint[],
  offset: number,
): GridPoint {
  let remaining = offset;
  for (let index = 1; index < points.length; index += 1) {
    const a = points[index - 1];
    const b = points[index];
    const length = distance(a, b);
    if (remaining <= length) {
      return {
        x: a.x + Math.sign(b.x - a.x) * remaining,
        y: a.y + Math.sign(b.y - a.y) * remaining,
      };
    }
    remaining -= length;
  }
  return { ...points[points.length - 1] };
}
