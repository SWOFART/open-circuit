import type { SheetSize } from "../generated/SheetSize";
import type { SheetSpec } from "../generated/SheetSpec";
import type { Schematic } from "../generated/Schematic";
import type { TitleBlock } from "../generated/TitleBlock";
import {
  boundsOf,
  type Bounds,
  type SceneModel,
  type ScenePoint,
} from "./scene";
import {
  clipConductor,
  containsPoint,
  sheetBounds,
  SHEET_INSET,
} from "./sheetRules";

export type PageMark =
  | { kind: "polyline"; points: readonly ScenePoint[] }
  | { kind: "circle"; center: ScenePoint; radius: number; filled: boolean }
  | { kind: "text"; position: ScenePoint; text: string; size: number };

export interface DrawingPage {
  readonly size: SheetSize;
  readonly marks: readonly PageMark[];
}

export const pageDimensions = {
  a4: {
    width: 297,
    height: 210,
    frameWidth: 295,
    frameHeight: 210,
    pixels: [3508, 2480],
  },
  a3: {
    width: 420,
    height: 297,
    frameWidth: 420,
    frameHeight: 295,
    pixels: [4961, 3508],
  },
} as const;

const GRID_MM = 5;
const INSET_MM = 10;
const BLOCK_WIDTH_MM = 180;
const BLOCK_HEIGHT_MM = 40;

export function fittingPageSize(bounds: Bounds): SheetSize {
  for (const size of ["a4", "a3"] as const) {
    const { frameWidth, frameHeight } = pageDimensions[size];
    if (
      (bounds.maxX - bounds.minX) * GRID_MM <= frameWidth - 2 * INSET_MM &&
      (bounds.maxY - bounds.minY) * GRID_MM <= frameHeight - 2 * INSET_MM
    )
      return size;
  }
  throw new Error(
    "The drawing is too large for A3. Add sheets before exporting.",
  );
}

function rectangle(
  left: number,
  top: number,
  right: number,
  bottom: number,
): PageMark {
  return {
    kind: "polyline",
    points: [
      { x: left, y: top },
      { x: right, y: top },
      { x: right, y: bottom },
      { x: left, y: bottom },
      { x: left, y: top },
    ],
  };
}

function titleBlockMarks(
  title: TitleBlock,
  right: number,
  bottom: number,
  sheetNumber = "1 of 1",
): PageMark[] {
  const left = right - BLOCK_WIDTH_MM;
  const top = bottom - BLOCK_HEIGHT_MM;
  const marks = [rectangle(left, top, right, bottom)];
  const addText = (value: string, x: number, y: number, capacity: number) => {
    const characters = Array.from(value);
    for (let start = 0; start < characters.length; start += capacity) {
      marks.push({
        kind: "text",
        text: characters.slice(start, start + capacity).join(""),
        position: { x, y: y + (start / capacity) * 3.5 },
        size: 2.5,
      });
    }
  };
  const center = left + BLOCK_WIDTH_MM / 2;
  addText(title.title, center, top + 4, 52);
  addText(title.drawingNumber, left + 60, top + 14, 32);
  addText(title.revision, left + 150, top + 14, 16);
  addText(title.author, center, top + 20, 52);
  addText(title.company, center, top + 28, 52);
  addText(title.date, left + 60, top + 36, 32);
  addText(sheetNumber, left + 150, top + 36, 16);
  return marks;
}

function intersectsBlock(
  scene: SceneModel,
  mm: (point: ScenePoint) => ScenePoint,
  blockLeft: number,
  blockTop: number,
): boolean {
  const crosses = (bounds: Bounds) => {
    const { x, y } = mm({ x: bounds.maxX, y: bounds.maxY });
    return x >= blockLeft - 0.125 && y >= blockTop - 0.125;
  };
  return (
    scene.shapes.some((shape) => {
      if (shape.kind !== "polyline") return crosses(boundsOf([shape]));
      return shape.points.some((point, index) => {
        if (index === 0) return false;
        const previous = shape.points[index - 1];
        const a = mm(previous);
        const b = mm(point);
        const left = blockLeft - 0.125;
        const top = blockTop - 0.125;
        if ((a.x >= left && a.y >= top) || (b.x >= left && b.y >= top))
          return true;
        if (a.x < left !== b.x < left) {
          const y = a.y + ((left - a.x) / (b.x - a.x)) * (b.y - a.y);
          return y >= top;
        }
        return false;
      });
    }) ||
    scene.connectionDots.some((dot) => {
      const center = mm(dot);
      return center.x + 0.6 >= blockLeft && center.y + 0.6 >= blockTop;
    })
  );
}

/** Convert the editor's static scene into a printable page in millimetres. */
export function drawingPage(
  scene: SceneModel,
  title: TitleBlock,
  requestedSize?: SheetSize,
): DrawingPage {
  const size = requestedSize ?? fittingPageSize(scene.bounds);
  const { width, height, frameWidth, frameHeight } = pageDimensions[size];
  const left = (width - frameWidth) / 2 + INSET_MM;
  const top = (height - frameHeight) / 2 + INSET_MM;
  const right = width - left;
  const bottom = height - top;
  const { minX, minY, maxX, maxY } = scene.bounds;
  if (
    (maxX - minX) * GRID_MM > right - left ||
    (maxY - minY) * GRID_MM > bottom - top
  ) {
    throw new Error("The drawing does not fit the chosen page size.");
  }
  const xGap = Math.min(2, (right - left - (maxX - minX) * GRID_MM) / 2);
  const yGap = Math.min(2, (bottom - top - (maxY - minY) * GRID_MM) / 2);
  const mm = ({ x, y }: ScenePoint): ScenePoint => ({
    x: left + xGap + (x - minX) * GRID_MM,
    y: top + yGap + (y - minY) * GRID_MM,
  });
  if (
    intersectsBlock(scene, mm, right - BLOCK_WIDTH_MM, bottom - BLOCK_HEIGHT_MM)
  ) {
    if (!requestedSize && size === "a4") return drawingPage(scene, title, "a3");
    throw new Error(
      "The drawing overlaps the title block. Choose a larger page or add sheets.",
    );
  }
  return formattedPage(scene, title, size, mm);
}

export function sheetDrawingPage(
  scene: SceneModel,
  schematic: Schematic,
  sheet: SheetSpec,
  index: number,
): DrawingPage {
  const frame = sheetBounds(sheet);
  const printable = sheetBounds(sheet, SHEET_INSET);
  const { width, height, frameWidth, frameHeight } = pageDimensions[sheet.size];
  const left = (width - frameWidth) / 2;
  const top = (height - frameHeight) / 2;
  const mm = ({ x, y }: ScenePoint): ScenePoint => ({
    x: left + (x - frame.minX) * GRID_MM,
    y: top + (y - frame.minY) * GRID_MM,
  });
  const origins = new Map(
    [
      ...schematic.components,
      ...schematic.junctions,
      ...schematic.referenceMarkers,
      ...schematic.documentation.notes,
      ...schematic.documentation.terminals,
    ].map(({ id, position }) => [id, position]),
  );

  const shapes = scene.shapes.flatMap((shape): SceneModel["shapes"] => {
    if (shape.owner.kind === "wire" || shape.owner.kind === "rail") {
      if (shape.kind === "polyline") {
        return shape.points.slice(1).flatMap((end, i) => {
          const points = clipConductor(shape.points[i], end, printable);
          return points.length ? [{ ...shape, points }] : [];
        });
      }
      const point = shape.kind === "label" ? shape.position : shape.center;
      return containsPoint(printable, point) ? [shape] : [];
    }
    const origin = origins.get(shape.owner.id);
    return origin && containsPoint(frame, origin) ? [shape] : [];
  });
  const forSheet: SceneModel = {
    bounds: printable,
    shapes,
    connectionDots: scene.connectionDots.filter((point) =>
      containsPoint(printable, point),
    ),
  };
  const right = width - left - INSET_MM;
  const bottom = height - top - INSET_MM;
  if (
    intersectsBlock(
      forSheet,
      mm,
      right - BLOCK_WIDTH_MM,
      bottom - BLOCK_HEIGHT_MM,
    )
  ) {
    throw new Error(
      `Drawing marks overlap the title block on sheet ${index + 1} (${sheet.name}). Move the marks or reposition the sheet before exporting.`,
    );
  }
  return formattedPage(
    forSheet,
    schematic.documentation.titleBlock,
    sheet.size,
    mm,
    `${index + 1} of ${schematic.documentation.sheets.length}`,
  );
}

function formattedPage(
  scene: SceneModel,
  title: TitleBlock,
  size: SheetSize,
  mm: (point: ScenePoint) => ScenePoint,
  sheetNumber?: string,
): DrawingPage {
  const { width, height, frameWidth, frameHeight } = pageDimensions[size];
  const left = (width - frameWidth) / 2 + INSET_MM;
  const top = (height - frameHeight) / 2 + INSET_MM;
  const right = width - left;
  const bottom = height - top;
  const marks: PageMark[] = [rectangle(left, top, right, bottom)];
  for (const shape of scene.shapes) {
    if (shape.kind === "polyline") {
      marks.push({ kind: "polyline", points: shape.points.map(mm) });
    } else if (shape.kind === "circle") {
      marks.push({
        kind: "circle",
        center: mm(shape.center),
        radius: shape.radius * GRID_MM,
        filled: shape.role === "junction",
      });
    } else {
      const position = mm(shape.position);
      const lines = shape.text.split("\n");
      for (const [index, text] of lines.entries()) {
        marks.push({
          kind: "text",
          position: {
            x: position.x,
            y: position.y + (index - (lines.length - 1) / 2) * 3,
          },
          text,
          size: 2.5,
        });
      }
    }
  }
  for (const dot of scene.connectionDots) {
    marks.push({ kind: "circle", center: mm(dot), radius: 0.6, filled: true });
  }
  marks.push(...titleBlockMarks(title, right, bottom, sheetNumber));
  return { size, marks };
}
