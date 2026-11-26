import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SymbolRegistry } from "../symbols";
import type { EditorCommand } from "./commands";
import type { SelectionTarget } from "./hitTest";
import { railExtentProblem } from "./railRules";
import {
  applyComponentTransform,
  followingGroupWireEdits,
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

/**
 * Moves every selected object by `delta` grid units.
 *
 * - A **selected wire** is translated with its whole path.
 * - A **component** moves with its pins; an **unselected wire** that ends on one of its pins,
 *   on a moved junction or on a moved rail keeps its other end and has its last leg re-routed
 *   (a wire that is not selected is never translated, so a selection never changes a wire it
 *   does not contain). Wire ends are matched against the original drawing and each wire is
 *   re-routed once, so a wire between two moved parts follows both ends.
 * - Junctions, reference markers, notes and rails move by `delta`.
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
    const points = wire.points.map((point) => shifted(point, delta));
    commands.push({ kind: "editWire", wireId: wire.id, points });
    scratch = withWirePaths(scratch, [{ id: wire.id, points }]);
  }

  // Rails: each must stay a legal rail.
  const railIds = selected("rail");
  const movedRails = schematic.rails.filter((rail) => railIds.has(rail.id));
  for (const rail of movedRails) {
    const x = rail.start.x + delta.x;
    const top = rail.start.y + delta.y;
    const bottom = rail.end.y + delta.y;
    if (railExtentProblem(x, top, bottom) !== null) {
      return null;
    }
    commands.push({
      kind: "editRail",
      railId: rail.id,
      patch: { x, top, bottom },
    });
  }

  // Junctions, markers and notes.
  const junctionIds = selected("junction");
  const markerIds = selected("referenceMarker");
  const noteIds = selected("note");
  const movedPoints = new Map<string, GridPoint>();
  for (const junction of schematic.junctions) {
    if (junctionIds.has(junction.id)) {
      const position = shifted(junction.position, delta);
      commands.push({ kind: "moveJunction", id: junction.id, position });
      movedPoints.set(key(junction.position), position);
    }
  }
  for (const marker of schematic.referenceMarkers) {
    if (markerIds.has(marker.id)) {
      commands.push({
        kind: "moveReferenceMarker",
        id: marker.id,
        position: shifted(marker.position, delta),
      });
    }
  }
  for (const note of schematic.documentation.notes) {
    if (noteIds.has(note.id)) {
      commands.push({
        kind: "moveNote",
        id: note.id,
        position: shifted(note.position, delta),
      });
    }
  }

  // Components and everything that follows them or a moved junction or rail. Every wire end is
  // matched against the original drawing and each unselected wire is re-routed exactly once.
  const componentIds = selected("component");
  const movedComponents = schematic.components
    .filter((component) => componentIds.has(component.id))
    .map((component) => ({
      componentId: component.id,
      placement: {
        position: shifted(component.position, delta),
        rotationDeg: component.rotationDeg,
      },
    }));
  const followRails = movedRails.map((rail) => ({
    x: rail.start.x,
    top: rail.start.y,
    bottom: rail.end.y,
  }));
  const following = followingGroupWireEdits(
    schematic,
    movedComponents,
    registry,
    (point) => {
      const junction = movedPoints.get(key(point));
      if (junction) {
        return junction;
      }
      return followRails.some(
        (rail) =>
          point.x === rail.x && point.y >= rail.top && point.y <= rail.bottom,
      )
        ? shifted(point, delta)
        : undefined;
    },
    wireIds,
  );
  for (const { componentId, placement } of movedComponents) {
    commands.push({
      kind: "transformComponent",
      componentId,
      position: placement.position,
      rotationDeg: placement.rotationDeg,
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

  // The drawing the commands leave: moved markers, notes, junctions and rails as well.
  const final: Schematic = {
    ...scratch,
    junctions: scratch.junctions.map((junction) =>
      junctionIds.has(junction.id)
        ? { ...junction, position: shifted(junction.position, delta) }
        : junction,
    ),
    referenceMarkers: scratch.referenceMarkers.map((marker) =>
      markerIds.has(marker.id)
        ? { ...marker, position: shifted(marker.position, delta) }
        : marker,
    ),
    rails: scratch.rails.map((rail) =>
      railIds.has(rail.id)
        ? {
            ...rail,
            start: shifted(rail.start, delta),
            end: shifted(rail.end, delta),
          }
        : rail,
    ),
    documentation: {
      ...scratch.documentation,
      notes: scratch.documentation.notes.map((note) =>
        noteIds.has(note.id)
          ? { ...note, position: shifted(note.position, delta) }
          : note,
      ),
    },
  };

  return { commands, schematic: final };
}
