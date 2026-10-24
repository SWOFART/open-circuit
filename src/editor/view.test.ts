import { describe, expect, it } from "vitest";
import {
  MAX_ZOOM_PX_PER_UNIT,
  MIN_ZOOM_PX_PER_UNIT,
  panBy,
  screenToGridPoint,
  snapToGrid,
  toGrid,
  toScreen,
  zoomAt,
  type ViewTransform,
} from "./view";

const view: ViewTransform = { scale: 24, offsetX: 100, offsetY: -40 };

describe("view transform", () => {
  it("maps grid points to pixels as screen = grid * scale + offset", () => {
    expect(toScreen({ x: 0, y: 0 }, view)).toEqual({ x: 100, y: -40 });
    expect(toScreen({ x: 2, y: -3 }, view)).toEqual({ x: 148, y: -112 });
  });

  it("inverts exactly back to grid coordinates", () => {
    for (const point of [
      { x: 0, y: 0 },
      { x: 7, y: -12 },
      { x: -3.5, y: 0.25 },
    ]) {
      const back = toGrid(toScreen(point, view), view);

      expect(back.x).toBeCloseTo(point.x, 12);
      expect(back.y).toBeCloseTo(point.y, 12);
    }
  });

  it("returns the integer grid point under a pixel", () => {
    expect(screenToGridPoint({ x: 148, y: -112 }, view)).toEqual({
      x: 2,
      y: -3,
    });
    // 11 px is less than half a 24 px cell away from (2, -3).
    expect(screenToGridPoint({ x: 159, y: -101 }, view)).toEqual({
      x: 2,
      y: -3,
    });
    // 13 px is more than half a cell, so the next point wins.
    expect(screenToGridPoint({ x: 161, y: -99 }, view)).toEqual({
      x: 3,
      y: -2,
    });
  });
});

describe("grid snapping", () => {
  it("rounds to the nearest integer grid point", () => {
    expect(snapToGrid({ x: 1.49, y: -1.51 })).toEqual({ x: 1, y: -2 });
    expect(snapToGrid({ x: 4, y: -7 })).toEqual({ x: 4, y: -7 });
  });

  it("rounds halves toward positive infinity on both axes", () => {
    expect(snapToGrid({ x: 0.5, y: -0.5 })).toEqual({ x: 1, y: 0 });
    expect(snapToGrid({ x: -1.5, y: 2.5 })).toEqual({ x: -1, y: 3 });
  });

  it("never produces negative zero", () => {
    const snapped = snapToGrid({ x: -0.2, y: -0.4 });

    expect(Object.is(snapped.x, 0)).toBe(true);
    expect(Object.is(snapped.y, 0)).toBe(true);
  });

  it("clamps to the signed 32-bit grid range", () => {
    expect(snapToGrid({ x: 1e12, y: -1e12 })).toEqual({
      x: 2_147_483_647,
      y: -2_147_483_648,
    });
  });
});

describe("pan and zoom", () => {
  it("pans by moving the offset in pixels", () => {
    expect(panBy(view, 15, -5)).toEqual({
      scale: 24,
      offsetX: 115,
      offsetY: -45,
    });
  });

  it("keeps the grid position under the anchor fixed while zooming", () => {
    const anchor = { x: 333, y: 217 };
    const before = toGrid(anchor, view);

    for (const factor of [0.5, 0.9, 1.1, 3]) {
      const zoomed = zoomAt(view, anchor, factor);
      const after = toGrid(anchor, zoomed);

      expect(zoomed.scale).toBeCloseTo(view.scale * factor, 12);
      expect(after.x).toBeCloseTo(before.x, 9);
      expect(after.y).toBeCloseTo(before.y, 9);
    }
  });

  it("clamps the scale to the zoom limits and keeps the anchor fixed", () => {
    const anchor = { x: 40, y: 60 };
    const before = toGrid(anchor, view);
    const zoomedIn = zoomAt(view, anchor, 1e6);
    const zoomedOut = zoomAt(view, anchor, 1e-6);

    expect(zoomedIn.scale).toBe(MAX_ZOOM_PX_PER_UNIT);
    expect(zoomedOut.scale).toBe(MIN_ZOOM_PX_PER_UNIT);
    for (const zoomed of [zoomedIn, zoomedOut]) {
      const after = toGrid(anchor, zoomed);
      expect(after.x).toBeCloseTo(before.x, 9);
      expect(after.y).toBeCloseTo(before.y, 9);
    }
  });

  it("does not jump when a fitted view is already outside the limits", () => {
    const tiny: ViewTransform = { scale: 1, offsetX: 0, offsetY: 0 };

    expect(zoomAt(tiny, { x: 0, y: 0 }, 0.5).scale).toBe(1);
    expect(zoomAt(tiny, { x: 0, y: 0 }, 2).scale).toBe(2);
  });

  it("ignores a non-finite or non-positive factor", () => {
    for (const factor of [0, -1, Number.NaN, Infinity]) {
      expect(zoomAt(view, { x: 1, y: 1 }, factor)).toBe(view);
    }
  });
});
