import type { ComponentSpec } from "../generated/ComponentSpec";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SymbolRegistry } from "../symbols";
import type { EditorCommand } from "./commands";
import type { SelectionTarget } from "./hitTest";
import { clipboardBounds, selectedObjects } from "./clipboard";
import { railExtentProblem } from "./railRules";
import {
  applyComponentTransform,
  followingGroupWireEdits,
  rotateClockwise,
  type ComponentPlacement,
  type WireEdit,
} from "./transform";

/**
 * What a group move does, as the commands that make it and the drawing it leaves
 * (docs/contracts/stage-16-editor-improvements.md section 3). The drawing is what a drag
 * previews; the commands are applied together as one history entry.
 */
export interface SelectionMove {
  readonly commands: readonly EditorCommand[];
  readonly schematic: Schematic;
}

function key(point: GridPoint): string {
  return `${point.x},${point.y}`;
}

function shifted(point: GridPoint, delta: GridPoint): GridPoint {
  return { x: point.x + delta.x, y: point.y + delta.y };
}

function withWirePaths(
  schematic: Schematic,
  edits: readonly WireEdit[],
): Schematic {
  const paths = new Map(edits.map((edit) => [edit.id, edit.points]));

  return {
    ...schematic,
    wires: schematic.wires.map((wire) => {
      const points = paths.get(wire.id);
      return points ? { ...wire, points: [...points] } : wire;
    }),
  };
}

/** How a selection is carried: where each kind of object goes. */
interface SelectionMap {
  /** Where a point of the drawing goes: a wire's points, a junction, a marker, a note. */
  readonly point: (point: GridPoint) => GridPoint;
  /** How a selected component is placed afterwards. */
  readonly component: (component: ComponentSpec) => ComponentPlacement;
  /** Where a selected rail is afterwards: its x and its ends. */
  readonly rail: (rail: Schematic["rails"][number]) => {
    x: number;
    top: number;
    bottom: number;
  };
}

type Carried = SelectionMove | { readonly refusal: string } | null;

/**
 * Carries every selected object by `map`.
 *
 * - A **selected wire** has every point of its path carried.
 * - A **component** moves with its pins; an **unselected wire** that ends on one of its pins,
 *   on a carried junction or on a carried rail keeps its other end and has its last leg
 *   re-routed (a wire that is not selected is never carried, so a selection never changes a wire
 *   it does not contain). Wire ends are matched against the original drawing and each wire is
 *   re-routed once, so a wire between two carried parts follows both ends.
 * - Junctions, reference markers and notes have their point carried, and rails their extent.
 *
 * Returns null when nothing changes, and a refusal when a rail would leave the legal grid.
 */
function carrySelection(
  schematic: Schematic,
  targets: readonly SelectionTarget[],
  map: SelectionMap,
  registry: SymbolRegistry,
): Carried {
  const selected = (kind: SelectionTarget["kind"]): ReadonlySet<StableId> =>
    new Set(
      targets.filter((target) => target.kind === kind).map((item) => item.id),
    );
  const wireIds = selected("wire");
  const commands: EditorCommand[] = [];
  let scratch = schematic;

  // Selected wires travel whole, before anything follows.
  for (const wire of schematic.wires) {
    if (!wireIds.has(wire.id)) {
      continue;
    }
    const points = wire.points.map(map.point);
    commands.push({ kind: "editWire", wireId: wire.id, points });
    scratch = withWirePaths(scratch, [{ id: wire.id, points }]);
  }

  // Rails: each must stay a legal rail.
  const railIds = selected("rail");
  const carriedRails = schematic.rails
    .filter((rail) => railIds.has(rail.id))
    .map((rail) => ({ rail, after: map.rail(rail) }));
  for (const { rail, after } of carriedRails) {
    if (railExtentProblem(after.x, after.top, after.bottom) !== null) {
      return { refusal: "A rail cannot be moved off the grid." };
    }
    commands.push({ kind: "editRail", railId: rail.id, patch: after });
  }

  // Junctions, markers and notes.
  const junctionIds = selected("junction");
  const markerIds = selected("referenceMarker");
  const noteIds = selected("note");
  const carriedPoints = new Map<string, GridPoint>();
  for (const junction of schematic.junctions) {
    if (junctionIds.has(junction.id)) {
      const position = map.point(junction.position);
      commands.push({ kind: "moveJunction", id: junction.id, position });
      carriedPoints.set(key(junction.position), position);
    }
  }
  for (const marker of schematic.referenceMarkers) {
    if (markerIds.has(marker.id)) {
      commands.push({
        kind: "moveReferenceMarker",
        id: marker.id,
        position: map.point(marker.position),
      });
    }
  }
  for (const note of schematic.documentation.notes) {
    if (noteIds.has(note.id)) {
      commands.push({
        kind: "moveNote",
        id: note.id,
        position: map.point(note.position),
      });
    }
  }

  // Components and everything that follows them or a carried junction or rail. Every wire end is
  // matched against the original drawing and each unselected wire is re-routed exactly once.
  const componentIds = selected("component");
  const carriedComponents = schematic.components
    .filter((component) => componentIds.has(component.id))
    .map((component) => ({
      componentId: component.id,
      placement: map.component(component),
    }));
  const following = followingGroupWireEdits(
    schematic,
    carriedComponents,
    registry,
    (point) => {
      const junction = carriedPoints.get(key(point));
      if (junction) {
        return junction;
      }
      return carriedRails.some(
        ({ rail }) =>
          point.x === rail.start.x &&
          point.y >= Math.min(rail.start.y, rail.end.y) &&
          point.y <= Math.max(rail.start.y, rail.end.y),
      )
        ? map.point(point)
        : undefined;
    },
    wireIds,
  );
  for (const { componentId, placement } of carriedComponents) {
    commands.push({
      kind: "transformComponent",
      componentId,
      position: placement.position,
      rotationDeg: placement.rotationDeg,
      ...(placement.mirrored === undefined
        ? {}
        : { mirrored: placement.mirrored }),
      wires: following
        .filter((edit) => edit.owner === componentId)
        .map(({ id, points }) => ({ id, points })),
    });
    scratch = applyComponentTransform(scratch, componentId, placement, []);
  }
  for (const edit of following) {
    if (edit.owner === null) {
      commands.push({ kind: "editWire", wireId: edit.id, points: edit.points });
    }
  }
  scratch = withWirePaths(scratch, following);

  if (commands.length === 0) {
    return null;
  }

  // The drawing the commands leave: carried markers, notes, junctions and rails as well.
  const final: Schematic = {
    ...scratch,
    junctions: scratch.junctions.map((junction) =>
      junctionIds.has(junction.id)
        ? { ...junction, position: map.point(junction.position) }
        : junction,
    ),
    referenceMarkers: scratch.referenceMarkers.map((marker) =>
      markerIds.has(marker.id)
        ? { ...marker, position: map.point(marker.position) }
        : marker,
    ),
    rails: scratch.rails.map((rail) => {
      const carried = carriedRails.find((item) => item.rail.id === rail.id);
      return carried
        ? {
            ...rail,
            start: { x: carried.after.x, y: carried.after.top },
            end: { x: carried.after.x, y: carried.after.bottom },
          }
        : rail;
    }),
    documentation: {
      ...scratch.documentation,
      notes: scratch.documentation.notes.map((note) =>
        noteIds.has(note.id)
          ? { ...note, position: map.point(note.position) }
          : note,
      ),
    },
  };

  return { commands, schematic: final };
}

/**
 * Moves every selected object by `delta` grid units (see `carrySelection`).
 *
 * Returns null when nothing would move, or when a rail would leave the legal grid.
 */
export function moveSelection(
  schematic: Schematic,
  targets: readonly SelectionTarget[],
  delta: GridPoint,
  registry: SymbolRegistry,
): SelectionMove | null {
  if (targets.length === 0 || (delta.x === 0 && delta.y === 0)) {
    return null;
  }

  const carried = carrySelection(
    schematic,
    targets,
    {
      point: (point) => shifted(point, delta),
      component: (component) => ({
        position: shifted(component.position, delta),
        rotationDeg: component.rotationDeg,
      }),
      rail: (rail) => ({
        x: rail.start.x + delta.x,
        top: rail.start.y + delta.y,
        bottom: rail.end.y + delta.y,
      }),
    },
    registry,
  );

  return carried && "commands" in carried ? carried : null;
}

/** A turn or a mirror of a selection (stage 16 contract, section 5). */
export type SelectionTurn = "quarter" | "half" | "mirror";

/**
 * The point a selection turns about: one selected part turns about its own origin, and several
 * objects about the center of their bounds, each coordinate rounded toward positive infinity so
 * the result is on the grid. Null for a selection with no object in it.
 */
export function selectionPivot(
  schematic: Schematic,
  targets: readonly SelectionTarget[],
  registry: SymbolRegistry,
): GridPoint | null {
  const [only] = targets;
  if (targets.length === 1 && only?.kind === "component") {
    return (
      schematic.components.find(({ id }) => id === only.id)?.position ?? null
    );
  }

  const bounds = clipboardBounds(selectedObjects(schematic, targets), registry);
  return bounds
    ? {
        x: Math.ceil((bounds.minX + bounds.maxX) / 2),
        y: Math.ceil((bounds.minY + bounds.maxY) / 2),
      }
    : null;
}

type Angle = ComponentSpec["rotationDeg"];

/**
 * Turns a quarter clockwise or a half, or mirrors, every selected object about the pivot of
 * `selectionPivot`, as the commands that do it and the drawing they leave. Every member is
 * carried by the same map of the plane:
 *
 * - half turn `(x, y)` to `(2px - x, 2py - y)`; quarter `(px - (y - py), py + (x - px))`;
 *   mirror `(2px - x, y)`;
 * - a component moves its origin by the map and adds the angle to its rotation; a mirror toggles
 *   its `mirrored` flag, and for a part that is carried with others it also negates the rotation,
 *   so the whole selection is the true mirror image (a part turned 90 degrees is flipped along
 *   the other axis of its symbol). A lone part is mirrored about the vertical axis of its own
 *   symbol, which only toggles the flag;
 * - a rail stays vertical, so a quarter turn of a selection that holds one is refused whole.
 *
 * Returns null for an empty selection, and a refusal that names the reason otherwise.
 */
export function turnSelection(
  schematic: Schematic,
  targets: readonly SelectionTarget[],
  turn: SelectionTurn,
  registry: SymbolRegistry,
): SelectionMove | { readonly refusal: string } | null {
  const pivot = selectionPivot(schematic, targets, registry);
  if (!pivot) {
    return null;
  }
  if (turn === "quarter" && targets.some((target) => target.kind === "rail")) {
    return { refusal: "A rail cannot be turned a quarter" };
  }

  const alone = targets.length === 1;
  const mirrorX = (x: number): number => 2 * pivot.x - x;
  const flipY = (y: number): number => 2 * pivot.y - y;
  const map: SelectionMap =
    turn === "mirror"
      ? {
          point: ({ x, y }) => ({ x: mirrorX(x), y }),
          component: (component) => ({
            position: {
              x: mirrorX(component.position.x),
              y: component.position.y,
            },
            rotationDeg: alone
              ? component.rotationDeg
              : (((360 - component.rotationDeg) % 360) as Angle),
            mirrored: component.mirrored !== true,
          }),
          rail: (rail) => ({
            x: mirrorX(rail.start.x),
            top: rail.start.y,
            bottom: rail.end.y,
          }),
        }
      : turn === "half"
        ? {
            point: ({ x, y }) => ({ x: mirrorX(x), y: flipY(y) }),
            component: (component) => ({
              position: {
                x: mirrorX(component.position.x),
                y: flipY(component.position.y),
              },
              rotationDeg: ((component.rotationDeg + 180) % 360) as Angle,
            }),
            rail: (rail) => ({
              x: mirrorX(rail.start.x),
              top: flipY(rail.end.y),
              bottom: flipY(rail.start.y),
            }),
          }
        : {
            point: ({ x, y }) => ({
              x: pivot.x - (y - pivot.y),
              y: pivot.y + (x - pivot.x),
            }),
            component: (component) => ({
              position: {
                x: pivot.x - (component.position.y - pivot.y),
                y: pivot.y + (component.position.x - pivot.x),
              },
              rotationDeg: rotateClockwise(component.rotationDeg),
            }),
            // A quarter turn of a rail is refused above.
            rail: (rail) => ({
              x: rail.start.x,
              top: rail.start.y,
              bottom: rail.end.y,
            }),
          };

  return carrySelection(schematic, targets, map, registry);
}
