import type { EditorCommand } from "../editor/commands";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type {
  NewComponentSpec,
  SchematicDocumentStore,
} from "../store/document";

/** A new component's parameters: every catalog default, explicitly. */
export function defaultParameters(
  catalog: ComponentCatalog,
  typeId: ComponentTypeId,
): Record<string, number> {
  const entry = catalog.componentTypes.find((item) => item.typeId === typeId);
  if (!entry) {
    throw new Error(`Component type ${typeId} is not in the catalog.`);
  }

  return Object.fromEntries(
    entry.parameters.map((parameter) => [parameter.id, parameter.defaultValue]),
  );
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
    case "undo":
      store.undo();
      return;
    case "redo":
      store.redo();
      return;
  }
}
