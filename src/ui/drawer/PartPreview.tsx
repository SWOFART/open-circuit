import type { SymbolDefinition, SymbolPrimitive } from "../../symbols";

/** The preview box, in CSS pixels. */
export const PREVIEW_WIDTH = 44;
export const PREVIEW_HEIGHT = 28;
const PADDING = 0.15;

export interface Bounds {
  readonly minX: number;
  readonly minY: number;
  readonly maxX: number;
  readonly maxY: number;
}

/** The box that contains every point and circle of a drawing; null for an empty drawing. */
export function symbolBounds(
  primitives: readonly SymbolPrimitive[],
): Bounds | null {
  let bounds: Bounds | null = null;
  const include = (x: number, y: number) => {
    bounds = bounds
      ? {
          minX: Math.min(bounds.minX, x),
          minY: Math.min(bounds.minY, y),
          maxX: Math.max(bounds.maxX, x),
          maxY: Math.max(bounds.maxY, y),
        }
      : { minX: x, minY: y, maxX: x, maxY: y };
  };

  for (const primitive of primitives) {
    if (primitive.kind === "polyline") {
      for (const point of primitive.points) {
        include(point.x, point.y);
      }
    } else {
      include(
        primitive.center.x - primitive.radius,
        primitive.center.y - primitive.radius,
      );
      include(
        primitive.center.x + primitive.radius,
        primitive.center.y + primitive.radius,
      );
    }
  }

  return bounds;
}

/** The SVG `viewBox` for a drawing: its bounds with a margin, in the preview's aspect ratio. */
export function previewViewBox(bounds: Bounds): string {
  const width = Math.max(bounds.maxX - bounds.minX, 0.5);
  const height = Math.max(bounds.maxY - bounds.minY, 0.5);
  const centerX = (bounds.minX + bounds.maxX) / 2;
  const centerY = (bounds.minY + bounds.maxY) / 2;
  const aspect = PREVIEW_WIDTH / PREVIEW_HEIGHT;
  let viewWidth = width * (1 + 2 * PADDING);
  let viewHeight = height * (1 + 2 * PADDING);

  if (viewWidth / viewHeight < aspect) {
    viewWidth = viewHeight * aspect;
  } else {
    viewHeight = viewWidth / aspect;
  }

  const round = (value: number) => Math.round(value * 1000) / 1000;
  return [
    round(centerX - viewWidth / 2),
    round(centerY - viewHeight / 2),
    round(viewWidth),
    round(viewHeight),
  ].join(" ");
}

/** Up to three capital letters naming a part, for the placeholder: "DC voltage source" is "DVS". */
export function placeholderInitials(name: string): string {
  const letters = name
    .split(/[^\p{L}\p{N}]+/u)
    .filter((word) => word.length > 0)
    .map((word) => word.charAt(0).toUpperCase())
    .slice(0, 3)
    .join("");

  return letters === "" ? "?" : letters;
}

function primitiveElement(primitive: SymbolPrimitive, index: number) {
  return primitive.kind === "polyline" ? (
    <polyline
      key={index}
      points={primitive.points.map(({ x, y }) => `${x},${y}`).join(" ")}
    />
  ) : (
    <circle
      key={index}
      cx={primitive.center.x}
      cy={primitive.center.y}
      r={primitive.radius}
    />
  );
}

export interface PartPreviewProps {
  /** The part's symbol, or undefined when the editor has none yet. */
  readonly symbol: SymbolDefinition | undefined;
  /** The part's display name, used for the placeholder's initials. */
  readonly name: string;
}

/**
 * The picture of a part in the drawer, drawn from the same resting primitives as the canvas
 * (docs/contracts/stage-11-drawer-and-editing.md section 4.1), so a preview can never differ from
 * the placed part. It draws in the current text color and so follows the theme. A part without a
 * symbol shows its initials. The picture is decorative: the option's text names the part.
 */
export function PartPreview({ symbol, name }: PartPreviewProps) {
  const bounds = symbol ? symbolBounds(symbol.primitives) : null;

  if (!symbol || !bounds) {
    return (
      <span
        className="part-preview part-preview-placeholder"
        aria-hidden="true"
      >
        {placeholderInitials(name)}
      </span>
    );
  }

  return (
    <svg
      className="part-preview"
      width={PREVIEW_WIDTH}
      height={PREVIEW_HEIGHT}
      viewBox={previewViewBox(bounds)}
      aria-hidden="true"
      focusable="false"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      {symbol.primitives.map(primitiveElement)}
    </svg>
  );
}
