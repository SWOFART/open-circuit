import { describe, expect, it } from "vitest";
import type { GridPoint } from "../generated/GridPoint";
import {
  extendRoute,
  isValidWirePath,
  leg,
  moveVertex,
  samePath,
} from "./wireRoute";

function path(...points: [number, number][]): GridPoint[] {
  return points.map(([x, y]) => ({ x, y }));
}

describe("orthogonal legs", () => {
  it("joins points in one row or column directly", () => {
    expect(leg({ x: 0, y: 0 }, { x: 5, y: 0 }, false)).toEqual([
      { x: 5, y: 0 },
    ]);
    expect(leg({ x: 2, y: 0 }, { x: 2, y: -4 }, true)).toEqual([
      { x: 2, y: -4 },
    ]);
  });

  it("bends horizontal-first by default", () => {
    expect(leg({ x: 0, y: 0 }, { x: 4, y: 3 }, false)).toEqual([
      { x: 4, y: 0 },
      { x: 4, y: 3 },
    ]);
  });

  it("bends vertical-first when asked", () => {
    expect(leg({ x: 0, y: 0 }, { x: 4, y: 3 }, true)).toEqual([
      { x: 0, y: 3 },
      { x: 4, y: 3 },
    ]);
  });

  it("works in every direction", () => {
    expect(leg({ x: 4, y: 3 }, { x: 0, y: 0 }, false)).toEqual([
      { x: 0, y: 3 },
      { x: 0, y: 0 },
    ]);
  });
});

describe("extending a route", () => {
  it("starts a route at the first point", () => {
    expect(extendRoute([], { x: 3, y: 4 }, false)).toEqual([{ x: 3, y: 4 }]);
  });

  it("adds the bend and the new point", () => {
    expect(extendRoute(path([0, 0]), { x: 4, y: 3 }, false)).toEqual(
      path([0, 0], [4, 0], [4, 3]),
    );
  });

  it("keeps an earlier bend and merges what continues straight", () => {
    const first = extendRoute(path([0, 0]), { x: 4, y: 3 }, false);
    const second = extendRoute(first, { x: 8, y: 6 }, true);

    // The straight continuation down column 4 is one segment.
    expect(second).toEqual(path([0, 0], [4, 0], [4, 6], [8, 6]));
  });

  it("merges a straight continuation into one segment", () => {
    const first = extendRoute(path([0, 0]), { x: 4, y: 0 }, false);

    expect(extendRoute(first, { x: 9, y: 0 }, false)).toEqual(
      path([0, 0], [9, 0]),
    );
  });

  it("does not repeat the last point", () => {
    expect(extendRoute(path([0, 0], [4, 0]), { x: 4, y: 0 }, false)).toEqual(
      path([0, 0], [4, 0]),
    );
  });
});

describe("valid wire paths", () => {
  it("needs two distinct points and only horizontal or vertical segments", () => {
    expect(isValidWirePath(path([0, 0], [3, 0]))).toBe(true);
    expect(isValidWirePath(path([0, 0], [3, 0], [3, 5]))).toBe(true);
    expect(isValidWirePath(path([0, 0]))).toBe(false);
    expect(isValidWirePath([])).toBe(false);
    expect(isValidWirePath(path([0, 0], [0, 0]))).toBe(false);
    expect(isValidWirePath(path([0, 0], [3, 4]))).toBe(false);
    expect(isValidWirePath(path([0, 0], [3, 0], [3, 0], [3, 5]))).toBe(false);
  });
});

describe("moving a vertex", () => {
  it("keeps both neighbouring segments orthogonal with one bend each", () => {
    expect(moveVertex(path([0, 0], [4, 0], [4, 3]), 1, { x: 6, y: 1 })).toEqual(
      path([0, 0], [6, 0], [6, 3], [4, 3]),
    );
  });

  it("moves an end of a wire and bends once to keep it orthogonal", () => {
    expect(moveVertex(path([0, 0], [4, 0]), 1, { x: 4, y: 2 })).toEqual(
      path([0, 0], [4, 0], [4, 2]),
    );
    expect(moveVertex(path([0, 0], [4, 0]), 0, { x: 0, y: 3 })).toEqual(
      path([0, 3], [4, 3], [4, 0]),
    );
  });

  it("slides a vertex along its segment without a new bend", () => {
    expect(moveVertex(path([0, 0], [4, 0], [4, 3]), 1, { x: 7, y: 0 })).toEqual(
      path([0, 0], [7, 0], [7, 3], [4, 3]),
    );
    expect(moveVertex(path([0, 0], [4, 0], [4, 3]), 1, { x: 4, y: 2 })).toEqual(
      path([0, 0], [4, 0], [4, 2], [4, 3]).filter((_, index) => index !== 2),
    );
  });

  it("returns the same path when the vertex does not move", () => {
    const original = path([0, 0], [4, 0], [4, 3]);

    expect(
      samePath(moveVertex(original, 1, { x: 4, y: 0 }) ?? [], original),
    ).toBe(true);
  });

  it("refuses a move that would collapse the wire to a point", () => {
    expect(moveVertex(path([0, 0], [4, 0]), 1, { x: 0, y: 0 })).toBeNull();
  });

  it("refuses a vertex that does not exist", () => {
    expect(moveVertex(path([0, 0], [4, 0]), 5, { x: 1, y: 1 })).toBeNull();
    expect(moveVertex(path([0, 0], [4, 0]), -1, { x: 1, y: 1 })).toBeNull();
  });

  it("always returns an orthogonal path or nothing", () => {
    const original = path([0, 0], [4, 0], [4, 3], [8, 3], [8, -2]);

    for (let index = 0; index < original.length; index += 1) {
      for (const to of [
        { x: -3, y: 5 },
        { x: 4, y: 3 },
        { x: 9, y: 0 },
        { x: 0, y: 0 },
        { x: 2, y: 2 },
      ]) {
        const moved = moveVertex(original, index, to);

        if (moved) {
          expect(isValidWirePath(moved)).toBe(true);
        }
      }
    }
  });
});

describe("comparing paths", () => {
  it("compares points, not references", () => {
    expect(samePath(path([0, 0], [1, 0]), path([0, 0], [1, 0]))).toBe(true);
    expect(samePath(path([0, 0], [1, 0]), path([0, 0], [2, 0]))).toBe(false);
    expect(samePath(path([0, 0]), path([0, 0], [1, 0]))).toBe(false);
  });
});
