import type { ComponentSpec } from "../generated/ComponentSpec";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SymbolRegistry } from "../symbols";
import type { Rotation } from "./geometry";
import type { DragPreview, InteractionIntent } from "./interaction";
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
      // Wires, junctions, and reference markers get their own tools (#31).
      return intent.target.kind === "component" && find(intent.target.id)
        ? { kind: "deleteComponent", componentId: intent.target.id }
        : null;
    case "undo":
    case "redo":
      return { kind: intent.kind };
  }
}

/** The schematic as it would be if the current drag were released now. */
export function dragPreviewSchematic(
  schematic: Schematic,
  drag: DragPreview,
  registry: SymbolRegistry,
): Schematic {
  const command = commandForIntent(
    { kind: "move", ...drag },
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
