import type { ComponentSpec } from "../generated/ComponentSpec";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SymbolRegistry } from "../symbols";
import type { Rotation } from "./geometry";
import type { SelectionTarget } from "./hitTest";
import type { DragPreview, InteractionIntent } from "./interaction";
import { moveVertex } from "./wireRoute";
import {
  applyComponentTransform,
  followingWireEdits,
  rotateClockwise,
  type ComponentPlacement,
  type WireEdit,
} from "./transform";

/**
 * A document operation requested by the canvas. The host applies it to the
 * document store, which validates it and records one history entry.
 */
export type EditorCommand =
  | {
      readonly kind: "placeComponent";
      readonly typeId: ComponentTypeId;
      readonly position: GridPoint;
      readonly rotationDeg: Rotation;
    }
  | {
      readonly kind: "transformComponent";
      readonly componentId: StableId;
      readonly position: GridPoint;
      readonly rotationDeg: Rotation;
      readonly wires: readonly WireEdit[];
    }
  | { readonly kind: "deleteComponent"; readonly componentId: StableId }
  | { readonly kind: "addWire"; readonly points: readonly GridPoint[] }
  | {
      readonly kind: "editWire";
      readonly wireId: StableId;
      readonly points: readonly GridPoint[];
    }
  | { readonly kind: "deleteWire"; readonly wireId: StableId }
  | { readonly kind: "addJunction"; readonly position: GridPoint }
  | {
      readonly kind: "moveJunction";
      readonly id: StableId;
      readonly position: GridPoint;
    }
  | { readonly kind: "deleteJunction"; readonly id: StableId }
  | { readonly kind: "addReferenceMarker"; readonly position: GridPoint }
  | {
      readonly kind: "moveReferenceMarker";
      readonly id: StableId;
      readonly position: GridPoint;
    }
  | { readonly kind: "deleteReferenceMarker"; readonly id: StableId }
  | { readonly kind: "undo" }
  | { readonly kind: "redo" };

// Never stored: the placement preview is drawn, not added to the document.
const PREVIEW_ID: StableId = "00000000-0000-4000-8000-000000000000";

function transformCommand(
  schematic: Schematic,
  component: ComponentSpec,
  placement: ComponentPlacement,
  registry: SymbolRegistry,
): EditorCommand {
  return {
    kind: "transformComponent",
    componentId: component.id,
    position: placement.position,
    rotationDeg: placement.rotationDeg,
    wires: followingWireEdits(schematic, component.id, placement, registry),
  };
}

/**
 * Turns a canvas intent into a document command. Returns null when the intent
 * names an object that no longer exists or is not editable by these tools.
 */
export function commandForIntent(
  intent: InteractionIntent,
  schematic: Schematic,
  registry: SymbolRegistry,
): EditorCommand | null {
  const find = (id: StableId) =>
    schematic.components.find((component) => component.id === id);

  switch (intent.kind) {
    case "place":
      return {
        kind: "placeComponent",
        typeId: intent.typeId,
        position: intent.position,
        rotationDeg: intent.rotationDeg,
      };
    case "move": {
      const component = find(intent.componentId);
      return component
        ? transformCommand(
            schematic,
            component,
            {
              position: {
                x: component.position.x + intent.delta.x,
                y: component.position.y + intent.delta.y,
              },
              rotationDeg: component.rotationDeg,
            },
            registry,
          )
        : null;
    }
    case "rotate": {
      const component = find(intent.componentId);
      return component
        ? transformCommand(
            schematic,
            component,
            {
              position: component.position,
              rotationDeg: rotateClockwise(component.rotationDeg),
            },
            registry,
          )
        : null;
    }
    case "delete":
      return deleteCommand(intent.target, schematic);
    case "addWire":
      return { kind: "addWire", points: intent.points };
    case "editWire":
      return exists(schematic.wires, intent.wireId)
        ? { kind: "editWire", wireId: intent.wireId, points: intent.points }
        : null;
    case "addJunction":
    case "addReferenceMarker":
      return { kind: intent.kind, position: intent.position };
    case "moveJunction":
      return exists(schematic.junctions, intent.id)
        ? { kind: "moveJunction", id: intent.id, position: intent.position }
        : null;
    case "moveReferenceMarker":
      return exists(schematic.referenceMarkers, intent.id)
        ? {
            kind: "moveReferenceMarker",
            id: intent.id,
            position: intent.position,
          }
        : null;
    case "undo":
    case "redo":
      return { kind: intent.kind };
  }
}

function exists(items: readonly { id: StableId }[], id: StableId): boolean {
  return items.some((item) => item.id === id);
}

/**
 * Deleting removes exactly the selected object and nothing connected to it;
 * the Engine re-derives connectivity from what remains.
 */
function deleteCommand(
  target: SelectionTarget,
  schematic: Schematic,
): EditorCommand | null {
  switch (target.kind) {
    case "component":
      return exists(schematic.components, target.id)
        ? { kind: "deleteComponent", componentId: target.id }
        : null;
    case "wire":
      return exists(schematic.wires, target.id)
        ? { kind: "deleteWire", wireId: target.id }
        : null;
    case "junction":
      return exists(schematic.junctions, target.id)
        ? { kind: "deleteJunction", id: target.id }
        : null;
    case "referenceMarker":
      return exists(schematic.referenceMarkers, target.id)
        ? { kind: "deleteReferenceMarker", id: target.id }
        : null;
  }
}

/**
 * The schematic as it would be if the current drag were released now. A drag
 * that would be canceled on release (an invalid vertex path or marker target)
 * leaves the drawing as it is.
 */
export function dragPreviewSchematic(
  schematic: Schematic,
  drag: DragPreview,
  registry: SymbolRegistry,
): Schematic {
  switch (drag.kind) {
    case "component": {
      const command = commandForIntent(
        { kind: "move", componentId: drag.componentId, delta: drag.delta },
        schematic,
        registry,
      );

      return command?.kind === "transformComponent"
        ? applyComponentTransform(
            schematic,
            drag.componentId,
            command,
            command.wires,
          )
        : schematic;
    }
    case "vertex": {
      const wire = schematic.wires.find((item) => item.id === drag.wireId);
      const points = wire && moveVertex(wire.points, drag.index, drag.to);

      return points
        ? {
            ...schematic,
            wires: schematic.wires.map((item) =>
              item.id === drag.wireId ? { ...item, points } : item,
            ),
          }
        : schematic;
    }
    case "marker": {
      if (!drag.valid) {
        return schematic;
      }

      const move = <T extends { id: StableId }>(items: readonly T[]): T[] =>
        items.map((item) =>
          item.id === drag.id ? { ...item, position: drag.to } : item,
        );

      return drag.marker === "junction"
        ? { ...schematic, junctions: move(schematic.junctions) }
        : {
            ...schematic,
            referenceMarkers: move(schematic.referenceMarkers),
          };
    }
  }
}

/** A one-component schematic that draws the placement preview. */
export function placementPreviewSchematic(
  typeId: ComponentTypeId,
  position: GridPoint,
  rotationDeg: Rotation,
): Schematic {
  // Parameters are irrelevant to drawing; the placed component gets the
  // catalog defaults from the host.
  const component = {
    id: PREVIEW_ID,
    typeId,
    position,
    rotationDeg,
    parameters: {},
  } as unknown as ComponentSpec;

  return {
    schemaVersion: 1,
    components: [component],
    wires: [],
    junctions: [],
    referenceMarkers: [],
  };
}
