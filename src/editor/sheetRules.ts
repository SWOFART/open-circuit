import type { SheetSpec } from "../generated/SheetSpec";
import type { Bounds, ScenePoint } from "./scene";

export const SHEET_GRID_SIZE = { a4: [59, 42], a3: [84, 59] } as const;
export const SHEET_INSET = 2;

export function sheetBounds(sheet: SheetSpec, inset = 0): Bounds {
  const [width, height] = SHEET_GRID_SIZE[sheet.size];
  const { x, y } = sheet.origin;
  return {
    minX: x + inset,
    minY: y + inset,
    maxX: x + width - inset,
    maxY: y + height - inset,
  };
}

export function sheetsOverlap(a: SheetSpec, b: SheetSpec): boolean {
  const first = sheetBounds(a);
  const second = sheetBounds(b);
  return (
    first.minX < second.maxX &&
    second.minX < first.maxX &&
    first.minY < second.maxY &&
    second.minY < first.maxY
  );
}

export function containsPoint(bounds: Bounds, point: ScenePoint): boolean {
  return (
    point.x >= bounds.minX &&
    point.x < bounds.maxX &&
    point.y >= bounds.minY &&
    point.y < bounds.maxY
  );
}

/** Clip an orthogonal conductor segment in its original direction. */
export function clipConductor(
  a: ScenePoint,
  b: ScenePoint,
  bounds: Bounds,
): readonly ScenePoint[] {
  if (a.y === b.y && a.y >= bounds.minY && a.y <= bounds.maxY) {
    const left = Math.max(Math.min(a.x, b.x), bounds.minX);
    const right = Math.min(Math.max(a.x, b.x), bounds.maxX);
    if (left < right) {
      const points = [
        { x: left, y: a.y },
        { x: right, y: a.y },
      ];
      return a.x <= b.x ? points : points.reverse();
    }
  }
  if (a.x === b.x && a.x >= bounds.minX && a.x <= bounds.maxX) {
    const top = Math.max(Math.min(a.y, b.y), bounds.minY);
    const bottom = Math.min(Math.max(a.y, b.y), bounds.maxY);
    if (top < bottom) {
      const points = [
        { x: a.x, y: top },
        { x: a.x, y: bottom },
      ];
      return a.y <= b.y ? points : points.reverse();
    }
  }
  return [];
}
