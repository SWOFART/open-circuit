import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { GridPoint } from "../generated/GridPoint";
import type { JunctionSpec } from "../generated/JunctionSpec";
import type { NoteSpec } from "../generated/NoteSpec";
import type { RailSpec } from "../generated/RailSpec";
import type { ReferenceMarkerSpec } from "../generated/ReferenceMarkerSpec";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { WireSpec } from "../generated/WireSpec";
import type { SymbolRegistry } from "../symbols";
import { emptyDocumentation } from "./documentation";
import { resolvePinPositions } from "./geometry";
import type { SelectionTarget } from "./hitTest";
import { bodyBounds } from "./transform";

// Copy, cut, paste and duplicate (docs/contracts/stage-16-editor-improvements.md section 4).
// This module is pure: it builds and reads the clipboard text and knows nothing of the system
// clipboard, the document store or the canvas.

export const CLIPBOARD_FORMAT = "open-circuit-clipboard";
export const CLIPBOARD_VERSION = 1;
/** The project file limit; a longer text is refused before it is parsed. */
export const MAX_CLIPBOARD_CHARS = 16 * 1024 * 1024;
/** The offset of a duplicate, and of a first paste next to its source, in grid units. */
export const PASTE_OFFSET: GridPoint = { x: 2, y: 2 };

/** The drawn objects of a copy, with the shapes of the saved schematic of the same names. */
export interface ClipboardSchematic {
  readonly components: readonly ComponentSpec[];
  readonly wires: readonly WireSpec[];
  readonly junctions: readonly JunctionSpec[];
  readonly referenceMarkers: readonly ReferenceMarkerSpec[];
  readonly rails: readonly RailSpec[];
  readonly notes: readonly NoteSpec[];
}

export interface ClipboardPayload {
  readonly format: typeof CLIPBOARD_FORMAT;
  readonly version: number;
  readonly schematic: ClipboardSchematic;
}

export type ClipboardParse =
  | { readonly ok: true; readonly payload: ClipboardPayload }
  | { readonly ok: false; readonly reason: string };

const BODY_KEYS = ["format", "version", "schematic"];
const SCHEMATIC_KEYS = [
  "components",
  "wires",
  "junctions",
  "referenceMarkers",
  "rails",
  "notes",
];
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ROTATIONS = new Set([0, 90, 180, 270]);

function key(point: GridPoint): string {
  return `${point.x},${point.y}`;
}

/** Whether `payload` holds no object at all. */
export function isEmptyPayload(schematic: ClipboardSchematic): boolean {
  return (
    schematic.components.length +
      schematic.wires.length +
      schematic.junctions.length +
      schematic.referenceMarkers.length +
      schematic.rails.length +
      schematic.notes.length ===
    0
  );
}

/** The selected objects themselves, copied by value, and nothing the selection merely touches. */
export function selectedObjects(
  schematic: Schematic,
  targets: readonly SelectionTarget[],
): ClipboardSchematic {
  const chosen = (kind: SelectionTarget["kind"]): ReadonlySet<StableId> =>
    new Set(
      targets.filter((target) => target.kind === kind).map(({ id }) => id),
    );
  const componentIds = chosen("component");
  const wireIds = chosen("wire");
  const junctionIds = chosen("junction");
  const markerIds = chosen("referenceMarker");
  const railIds = chosen("rail");
  const noteIds = chosen("note");

  return structuredClone({
    components: schematic.components.filter(({ id }) => componentIds.has(id)),
    wires: schematic.wires.filter(({ id }) => wireIds.has(id)),
    junctions: schematic.junctions.filter(({ id }) => junctionIds.has(id)),
    referenceMarkers: schematic.referenceMarkers.filter(({ id }) =>
      markerIds.has(id),
    ),
    rails: schematic.rails.filter(({ id }) => railIds.has(id)),
    notes: schematic.documentation.notes.filter(({ id }) => noteIds.has(id)),
  });
}

/**
 * What a copy holds: every selected component, wire, junction, marker, rail and note, and every
 * unselected wire whose two ends are both on copied objects (a pin of a copied part, a copied
 * junction, or a place on a copied rail), so the connection between two copied parts is kept.
 * Designators, wire numbers and terminal entries are not copied. Null when nothing is copyable.
 */
export function copySelection(
  schematic: Schematic,
  targets: readonly SelectionTarget[],
  registry: SymbolRegistry,
): ClipboardSchematic | null {
  const chosen = selectedObjects(schematic, targets);
  const wireIds = new Set(chosen.wires.map(({ id }) => id));

  const anchors = new Set<string>();
  for (const component of chosen.components) {
    for (const pin of resolvePinPositions(component, registry)) {
      anchors.add(key(pin.position));
    }
  }
  for (const junction of chosen.junctions) {
    anchors.add(key(junction.position));
  }
  const onCopiedRail = (point: GridPoint): boolean =>
    chosen.rails.some(
      (rail) =>
        point.x === rail.start.x &&
        point.y >= Math.min(rail.start.y, rail.end.y) &&
        point.y <= Math.max(rail.start.y, rail.end.y),
    );
  const anchored = (point: GridPoint | undefined): boolean =>
    point !== undefined && (anchors.has(key(point)) || onCopiedRail(point));

  const copy: ClipboardSchematic = {
    ...chosen,
    wires: structuredClone(
      schematic.wires.filter(
        (wire) =>
          wireIds.has(wire.id) ||
          (anchored(wire.points[0]) &&
            anchored(wire.points[wire.points.length - 1])),
      ),
    ),
  };

  return isEmptyPayload(copy) ? null : copy;
}

/** The text a copy puts on the system clipboard. */
export function clipboardText(schematic: ClipboardSchematic): string {
  const payload: ClipboardPayload = {
    format: CLIPBOARD_FORMAT,
    version: CLIPBOARD_VERSION,
    schematic,
  };
  return JSON.stringify(payload);
}

function plainObject(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** Every required key is there and no key is there that is neither required nor optional. */
function hasExactKeys(
  value: Record<string, unknown>,
  keys: string[],
  optional: string[] = [],
): boolean {
  return (
    keys.every((name) => name in value) &&
    Object.keys(value).every(
      (name) => keys.includes(name) || optional.includes(name),
    )
  );
}

function gridPoint(value: unknown): GridPoint | null {
  const point = plainObject(value);
  if (!point || !hasExactKeys(point, ["x", "y"])) return null;
  const { x, y } = point;
  const inRange = (axis: unknown): axis is number =>
    typeof axis === "number" &&
    Number.isInteger(axis) &&
    axis >= -2_147_483_648 &&
    axis <= 2_147_483_647;
  return inRange(x) && inRange(y) ? { x, y } : null;
}

function pointList(value: unknown): GridPoint[] | null {
  if (!Array.isArray(value)) return null;
  const points = value.map(gridPoint);
  return points.every((point): point is GridPoint => point !== null)
    ? points
    : null;
}

class Refusal extends Error {}

function refuse(what: string): never {
  throw new Refusal(`The clipboard holds a ${what} that cannot be pasted.`);
}

function objectList(
  schematic: Record<string, unknown>,
  name: string,
  keys: string[],
  what: string,
  optional: string[] = [],
): Record<string, unknown>[] {
  const list = schematic[name];
  if (!Array.isArray(list)) refuse(what);
  return list.map((item) => {
    const object = plainObject(item);
    if (!object || !hasExactKeys(object, keys, optional)) refuse(what);
    if (typeof object.id !== "string" || !UUID.test(object.id)) refuse(what);
    return object;
  });
}

function decodeSchematic(value: unknown, catalog: ComponentCatalog) {
  const schematic = plainObject(value);
  if (!schematic || !hasExactKeys(schematic, SCHEMATIC_KEYS)) {
    refuse("selection of an unknown shape");
  }
  const types = new Set(catalog.componentTypes.map((entry) => entry.typeId));
  const seen = new Set<string>();
  const unique = (id: unknown): void => {
    if (typeof id !== "string" || seen.has(id)) refuse("repeated object ID");
    seen.add(id);
  };

  const components = objectList(
    schematic,
    "components",
    ["typeId", "id", "position", "rotationDeg", "parameters"],
    "part",
    ["mirrored"],
  ).map((item): ComponentSpec => {
    unique(item.id);
    const position = gridPoint(item.position);
    const parameters = plainObject(item.parameters);
    if (
      !position ||
      !parameters ||
      typeof item.typeId !== "string" ||
      !types.has(item.typeId as ComponentSpec["typeId"]) ||
      (item.mirrored !== undefined && typeof item.mirrored !== "boolean") ||
      typeof item.rotationDeg !== "number" ||
      !ROTATIONS.has(item.rotationDeg) ||
      !Object.values(parameters).every(
        (parameter) =>
          typeof parameter === "number" && Number.isFinite(parameter),
      )
    ) {
      refuse("part");
    }
    return {
      typeId: item.typeId,
      id: item.id,
      position,
      rotationDeg: item.rotationDeg,
      // An unmirrored part has no field, whichever way the text spelled it.
      ...(item.mirrored === true ? { mirrored: true } : {}),
      parameters: { ...parameters },
    } as ComponentSpec;
  });

  const wires = objectList(schematic, "wires", ["id", "points"], "wire").map(
    (item): WireSpec => {
      unique(item.id);
      const points = pointList(item.points);
      if (!points || points.length < 2) refuse("wire");
      return { id: item.id as StableId, points };
    },
  );

  const marker = (name: string, what: string) =>
    objectList(schematic, name, ["id", "position"], what).map((item) => {
      unique(item.id);
      const position = gridPoint(item.position);
      if (!position) refuse(what);
      return { id: item.id as StableId, position };
    });
  const junctions = marker("junctions", "junction");
  const referenceMarkers = marker("referenceMarkers", "ground marker");

  const rails = objectList(
    schematic,
    "rails",
    ["id", "name", "start", "end"],
    "rail",
  ).map((item): RailSpec => {
    unique(item.id);
    const start = gridPoint(item.start);
    const end = gridPoint(item.end);
    if (!start || !end || typeof item.name !== "string") refuse("rail");
    return { id: item.id as StableId, name: item.name, start, end };
  });

  const notes = objectList(
    schematic,
    "notes",
    ["id", "position", "text"],
    "note",
  ).map((item): NoteSpec => {
    unique(item.id);
    const position = gridPoint(item.position);
    if (!position || typeof item.text !== "string") refuse("note");
    return { id: item.id as StableId, position, text: item.text };
  });

  return { components, wires, junctions, referenceMarkers, rails, notes };
}

/**
 * Reads the text of the system clipboard. The text is untrusted input: anything may be there,
 * so every field is checked and the **whole** paste is refused on the first problem. Nothing is
 * repaired, truncated or run. The catalog and range rules the document store applies to every
 * added object (parameter domains, wire geometry, note text, limits) apply again at the paste,
 * inside its transaction, so a refusal there also leaves the document unchanged.
 */
export function parseClipboardText(
  text: string,
  catalog: ComponentCatalog,
): ClipboardParse {
  if (text.length > MAX_CLIPBOARD_CHARS) {
    return { ok: false, reason: "The clipboard text is too large to paste." };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return {
      ok: false,
      reason: "The clipboard does not hold a copied selection.",
    };
  }

  const body = plainObject(parsed);
  if (
    !body ||
    body.format !== CLIPBOARD_FORMAT ||
    !hasExactKeys(body, BODY_KEYS)
  ) {
    return {
      ok: false,
      reason: "The clipboard does not hold a copied selection.",
    };
  }
  if (!Number.isInteger(body.version)) {
    return { ok: false, reason: "The clipboard version is not readable." };
  }
  if (body.version !== CLIPBOARD_VERSION) {
    return {
      ok: false,
      reason:
        "This clipboard was made by a different version of the program and cannot be pasted.",
    };
  }

  try {
    const schematic = decodeSchematic(body.schematic, catalog);
    if (isEmptyPayload(schematic)) {
      return { ok: false, reason: "The clipboard selection is empty." };
    }
    return {
      ok: true,
      payload: {
        format: CLIPBOARD_FORMAT,
        version: CLIPBOARD_VERSION,
        schematic,
      },
    };
  } catch (error) {
    if (error instanceof Refusal) return { ok: false, reason: error.message };
    throw error;
  }
}

function shifted(point: GridPoint, delta: GridPoint): GridPoint {
  return { x: point.x + delta.x, y: point.y + delta.y };
}

/** The objects moved by `delta`, with their IDs unchanged. */
export function translateClipboard(
  schematic: ClipboardSchematic,
  delta: GridPoint,
): ClipboardSchematic {
  return {
    components: schematic.components.map((item) => ({
      ...item,
      position: shifted(item.position, delta),
    })),
    wires: schematic.wires.map((item) => ({
      ...item,
      points: item.points.map((point) => shifted(point, delta)),
    })),
    junctions: schematic.junctions.map((item) => ({
      ...item,
      position: shifted(item.position, delta),
    })),
    referenceMarkers: schematic.referenceMarkers.map((item) => ({
      ...item,
      position: shifted(item.position, delta),
    })),
    rails: schematic.rails.map((item) => ({
      ...item,
      start: shifted(item.start, delta),
      end: shifted(item.end, delta),
    })),
    notes: schematic.notes.map((item) => ({
      ...item,
      position: shifted(item.position, delta),
    })),
  };
}

/** The bounds of everything the copy holds, in grid units, or null for an empty copy. */
export function clipboardBounds(
  schematic: ClipboardSchematic,
  registry: SymbolRegistry,
): { minX: number; minY: number; maxX: number; maxY: number } | null {
  const points: GridPoint[] = [
    ...schematic.components.flatMap((component) => {
      const body = bodyBounds(component, registry);
      return [
        { x: body.minX, y: body.minY },
        { x: body.maxX, y: body.maxY },
        ...resolvePinPositions(component, registry).map((pin) => pin.position),
      ];
    }),
    ...schematic.wires.flatMap((wire) => wire.points),
    ...schematic.junctions.map((item) => item.position),
    ...schematic.referenceMarkers.map((item) => item.position),
    ...schematic.rails.flatMap((rail) => [rail.start, rail.end]),
    ...schematic.notes.map((item) => item.position),
  ];
  if (points.length === 0) return null;

  return {
    minX: Math.min(...points.map((point) => point.x)),
    minY: Math.min(...points.map((point) => point.y)),
    maxX: Math.max(...points.map((point) => point.x)),
    maxY: Math.max(...points.map((point) => point.y)),
  };
}

/** The copy as a drawing, for the translucent preview of a placement. Never stored. */
export function clipboardDrawing(schematic: ClipboardSchematic): Schematic {
  return {
    schemaVersion: 3,
    components: [...schematic.components],
    wires: [...schematic.wires],
    junctions: [...schematic.junctions],
    referenceMarkers: [...schematic.referenceMarkers],
    rails: [...schematic.rails],
    documentation: { ...emptyDocumentation(), notes: [...schematic.notes] },
  };
}

/** The objects of `after` that `before` did not have: what a paste added. */
export function addedObjects(
  before: Schematic,
  after: Schematic,
): SelectionTarget[] {
  const known = new Set<string>([
    ...before.components.map(({ id }) => id),
    ...before.wires.map(({ id }) => id),
    ...before.junctions.map(({ id }) => id),
    ...before.referenceMarkers.map(({ id }) => id),
    ...before.rails.map(({ id }) => id),
    ...before.documentation.notes.map(({ id }) => id),
  ]);
  const fresh = <T extends { id: StableId }>(
    kind: SelectionTarget["kind"],
    items: readonly T[],
  ): SelectionTarget[] =>
    items
      .filter(({ id }) => !known.has(id))
      .map(({ id }) => ({ kind, id }) as SelectionTarget);

  return [
    ...fresh("component", after.components),
    ...fresh("wire", after.wires),
    ...fresh("junction", after.junctions),
    ...fresh("referenceMarker", after.referenceMarkers),
    ...fresh("rail", after.rails),
    ...fresh("note", after.documentation.notes),
  ];
}
