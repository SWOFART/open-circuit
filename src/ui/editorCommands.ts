import type { EditorCommand } from "../editor/commands";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { ParameterId } from "../generated/ParameterId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type {
  NewComponentSpec,
  SchematicDocumentStore,
} from "../store/document";
import type { ParameterUpdate } from "../store/simulation";
import {
  commandVerdict,
  parameterVerdict,
  type EditorRestrictions,
  type Verdict,
} from "../store/studentAssignment";

/**
 * A new component's parameters: every required catalog parameter at its
 * default, explicitly. Optional parameters, such as initial conditions, stay
 * absent: a default offered by the editor is never applied implicitly.
 */
export function defaultParameters(
  catalog: ComponentCatalog,
  typeId: ComponentTypeId,
): Record<string, number> {
  const entry = catalog.componentTypes.find((item) => item.typeId === typeId);
  if (!entry) {
    throw new Error(`Component type ${typeId} is not in the catalog.`);
  }

  return Object.fromEntries(
    entry.parameters
      .filter((parameter) => !parameter.optional)
      .map((parameter) => [parameter.id, parameter.defaultValue]),
  );
}

/**
 * The lightweight Engine update for a committed parameter edit, or undefined
 * when the edit must recompile the circuit. An initial condition only sets the
 * state at time zero, so setting or clearing one always starts a new run; the
 * catalog's `initialCondition` flag decides, not the parameter's name.
 */
export function liveParameterUpdate(
  catalog: ComponentCatalog,
  schematic: Schematic,
  componentId: StableId,
  parameterId: ParameterId,
  value: number,
): ParameterUpdate | undefined {
  const component = schematic.components.find(
    (item) => item.id === componentId,
  );
  const definition = catalog.componentTypes
    .find((entry) => entry.typeId === component?.typeId)
    ?.parameters.find((item) => item.id === parameterId);

  return definition && !definition.initialCondition
    ? { componentId, parameterId, value }
    : undefined;
}

/**
 * Applies a canvas command to the document store as one history entry. The
 * store validates it against the catalog and throws, leaving the document
 * unchanged, when it is invalid.
 */
export function applyEditorCommand(
  store: SchematicDocumentStore,
  command: EditorCommand,
  catalog: ComponentCatalog | null,
): void {
  switch (command.kind) {
    case "placeComponent": {
      if (!catalog) {
        throw new Error("The component catalog has not loaded yet.");
      }
      store.addComponent(
        {
          typeId: command.typeId,
          position: command.position,
          rotationDeg: command.rotationDeg,
          parameters: defaultParameters(catalog, command.typeId),
        } as NewComponentSpec,
        catalog,
      );
      return;
    }
    case "transformComponent":
      store.transformComponent(
        command.componentId,
        command.position,
        command.rotationDeg,
        command.wires,
      );
      return;
    case "deleteComponent":
      store.removeComponent(command.componentId);
      return;
    case "addWire":
      store.addWire({ points: [...command.points] });
      return;
    case "editWire":
      store.editWire(command.wireId, command.points);
      return;
    case "deleteWire":
      store.removeWire(command.wireId);
      return;
    case "addJunction":
      store.addJunction(command.position);
      return;
    case "moveJunction":
      store.moveJunction(command.id, command.position);
      return;
    case "deleteJunction":
      store.removeJunction(command.id);
      return;
    case "addReferenceMarker":
      store.addReferenceMarker(command.position);
      return;
    case "moveReferenceMarker":
      store.moveReferenceMarker(command.id, command.position);
      return;
    case "deleteReferenceMarker":
      store.removeReferenceMarker(command.id);
      return;
    case "undo":
      store.undo();
      return;
    case "redo":
      store.redo();
      return;
  }
}

/**
 * Applies a canvas command unless the open assignment's restrictions refuse it. A refused
 * command leaves the document untouched and says why; the document store's own rules are not
 * changed and still apply to a command that is allowed (it throws, as `applyEditorCommand`
 * does, when the store refuses it).
 */
export function applyRestrictedCommand(
  store: SchematicDocumentStore,
  command: EditorCommand,
  catalog: ComponentCatalog | null,
  restrictions: EditorRestrictions | null,
): Verdict {
  const verdict = commandVerdict(command, restrictions);

  if (verdict.allowed) {
    applyEditorCommand(store, command, catalog);
  }

  return verdict;
}

/** Sets a parameter unless the assignment locks the part. */
export function setRestrictedParameter(
  store: SchematicDocumentStore,
  componentId: StableId,
  parameterId: ParameterId,
  value: number,
  catalog: ComponentCatalog,
  restrictions: EditorRestrictions | null,
): Verdict {
  const verdict = parameterVerdict(componentId, restrictions);

  if (verdict.allowed) {
    store.setParameter(componentId, parameterId, value, catalog);
  }

  return verdict;
}

/** Clears an optional parameter unless the assignment locks the part. */
export function clearRestrictedParameter(
  store: SchematicDocumentStore,
  componentId: StableId,
  parameterId: ParameterId,
  catalog: ComponentCatalog,
  restrictions: EditorRestrictions | null,
): Verdict {
  const verdict = parameterVerdict(componentId, restrictions);

  if (verdict.allowed) {
    store.clearParameter(componentId, parameterId, catalog);
  }

  return verdict;
}
