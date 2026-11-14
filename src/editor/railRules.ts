import type { GridPoint } from "../generated/GridPoint";

// The rules of docs/contracts/stage-8-supply-rails.md sections 2, 3.3 and 7.1 that the editor
// checks before it asks the document to change. The Engine validates every one of them again
// at every boundary; these only keep an invalid gesture or field from reaching the document.

/** At most this many rails in one schematic. */
export const MAX_RAILS = 256;
/** The shortest and the longest legal rail, in grid units. */
export const MIN_RAIL_EXTENT = 2;
export const MAX_RAIL_EXTENT = 1_000_000;
/** The line previewed before the first click of the rail tool. */
export const DEFAULT_RAIL_LENGTH = 20;
/** Coordinates are signed 32-bit integers. */
const COORDINATE_MIN = -2_147_483_648;
const COORDINATE_MAX = 2_147_483_647;

/** 1–32 ASCII bytes: `[A-Za-z0-9][A-Za-z0-9_+-]{0,31}`; compared byte for byte. */
const RAIL_NAME = /^[A-Za-z0-9][A-Za-z0-9_+-]{0,31}$/;

export type RailNameProblem = "empty" | "grammar" | "duplicate";

function isCoordinate(value: number): boolean {
  return (
    Number.isInteger(value) &&
    value >= COORDINATE_MIN &&
    value <= COORDINATE_MAX
  );
}

/** Why `x`, `top` and `bottom` are not a legal rail, or null when they are. */
export function railExtentProblem(
  x: number,
  top: number,
  bottom: number,
): "coordinate" | "extent" | null {
  if (![x, top, bottom].every(isCoordinate)) {
    return "coordinate";
  }

  // Signed 64-bit arithmetic in the Engine; a double holds it exactly here.
  const extent = bottom - top;
  return extent >= MIN_RAIL_EXTENT && extent <= MAX_RAIL_EXTENT
    ? null
    : "extent";
}

export function isLegalRail(x: number, top: number, bottom: number): boolean {
  return railExtentProblem(x, top, bottom) === null;
}

/**
 * Why `name` cannot name a rail next to `others` (the other rails' names), or null. The name is
 * never trimmed or case-folded: `L1` and `l1` are different, and so are ` L1` and `L1`.
 */
export function railNameProblem(
  name: string,
  others: readonly string[],
): RailNameProblem | null {
  if (name.length === 0) {
    return "empty";
  }
  if (!RAIL_NAME.test(name)) {
    return "grammar";
  }

  return others.includes(name) ? "duplicate" : null;
}

/** The smallest unused `RAIL<n>`, for positive integer n. */
export function nextRailName(names: readonly string[]): string {
  const used = new Set(names);

  for (let n = 1; ; n += 1) {
    const candidate = `RAIL${n}`;
    if (!used.has(candidate)) {
      return candidate;
    }
  }
}

/** The top and bottom of a rail drawn between two clicks at one x, in order. */
export function orderedExtent(
  first: GridPoint,
  second: GridPoint,
): { readonly top: number; readonly bottom: number } {
  return first.y <= second.y
    ? { top: first.y, bottom: second.y }
    : { top: second.y, bottom: first.y };
}
