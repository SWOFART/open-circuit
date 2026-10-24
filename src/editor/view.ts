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
