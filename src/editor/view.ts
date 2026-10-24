import type { GridPoint } from "../generated/GridPoint";
import type { Bounds, ScenePoint } from "./scene";

export interface Viewport {
  readonly width: number;
  readonly height: number;
}

/** Maps grid units to screen pixels: `screen = grid * scale + offset`. */
export interface ViewTransform {
  readonly scale: number;
  readonly offsetX: number;
  readonly offsetY: number;
}

export const VIEW_PADDING_PX = 40;
// Keeps a small circuit from being blown up to an absurd size in a big window.
export const MAX_SCALE_PX_PER_UNIT = 64;
// Interactive zoom limits. A fitted view may lie outside them; zooming then
// only moves the scale back toward this range, never jumps across it.
export const MIN_ZOOM_PX_PER_UNIT = 4;
export const MAX_ZOOM_PX_PER_UNIT = 200;

// Stage 1 grid coordinates are signed 32-bit integers.
const GRID_MIN = -2_147_483_648;
const GRID_MAX = 2_147_483_647;

/** Scales and centers `bounds` so the whole scene stays visible. */
export function fitView(
  bounds: Bounds,
  viewport: Viewport,
  paddingPx: number = VIEW_PADDING_PX,
): ViewTransform {
  const availableWidth = Math.max(viewport.width - 2 * paddingPx, 1);
  const availableHeight = Math.max(viewport.height - 2 * paddingPx, 1);
  const sceneWidth = Math.max(bounds.maxX - bounds.minX, 1e-6);
  const sceneHeight = Math.max(bounds.maxY - bounds.minY, 1e-6);
  const scale = Math.min(
    availableWidth / sceneWidth,
    availableHeight / sceneHeight,
    MAX_SCALE_PX_PER_UNIT,
  );

  return {
    scale,
    offsetX: (viewport.width - sceneWidth * scale) / 2 - bounds.minX * scale,
    offsetY: (viewport.height - sceneHeight * scale) / 2 - bounds.minY * scale,
  };
}

export function toScreen(point: ScenePoint, view: ViewTransform): ScenePoint {
  return {
    x: point.x * view.scale + view.offsetX,
    y: point.y * view.scale + view.offsetY,
  };
}

/** Inverse of `toScreen`: the exact, unsnapped grid position of a pixel. */
export function toGrid(point: ScenePoint, view: ViewTransform): ScenePoint {
  return {
    x: (point.x - view.offsetX) / view.scale,
    y: (point.y - view.offsetY) / view.scale,
  };
}

function snapCoordinate(value: number): number {
  // `+ 0` turns -0 into +0 so snapped points compare equal to plain integers.
  return Math.min(Math.max(Math.round(value), GRID_MIN), GRID_MAX) + 0;
}

/**
 * Snaps a grid position to the nearest integer grid point. Halves round toward
 * positive infinity, so every position has exactly one snap target.
 */
export function snapToGrid(point: ScenePoint): GridPoint {
  return { x: snapCoordinate(point.x), y: snapCoordinate(point.y) };
}

/** The snapped grid point under a canvas pixel. */
export function screenToGridPoint(
  point: ScenePoint,
  view: ViewTransform,
): GridPoint {
  return snapToGrid(toGrid(point, view));
}

export function panBy(
  view: ViewTransform,
  deltaX: number,
  deltaY: number,
): ViewTransform {
  return {
    scale: view.scale,
    offsetX: view.offsetX + deltaX,
    offsetY: view.offsetY + deltaY,
  };
}

/**
 * Multiplies the scale by `factor`, keeping the grid position under `anchor`
 * (a canvas pixel) fixed on screen. The result stays within the zoom limits.
 */
export function zoomAt(
  view: ViewTransform,
  anchor: ScenePoint,
  factor: number,
): ViewTransform {
  if (!Number.isFinite(factor) || factor <= 0) {
    return view;
  }

  const minScale = Math.min(MIN_ZOOM_PX_PER_UNIT, view.scale);
  const maxScale = Math.max(MAX_ZOOM_PX_PER_UNIT, view.scale);
  const scale = Math.min(Math.max(view.scale * factor, minScale), maxScale);
  if (scale === view.scale) {
    return view;
  }

  const fixed = toGrid(anchor, view);

  return {
    scale,
    offsetX: anchor.x - fixed.x * scale,
    offsetY: anchor.y - fixed.y * scale,
  };
}
