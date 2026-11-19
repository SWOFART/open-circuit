import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { ParameterId } from "../generated/ParameterId";
import type { StableId } from "../generated/StableId";
import { ChoiceParameterField } from "./ChoiceParameterField";
import { OptionalParameterField } from "./OptionalParameterField";
import { ParameterField } from "./ParameterField";
import { parameterNote } from "./parameterHints";

export interface ComponentParameterFieldsProps {
  readonly catalog: ComponentCatalog;
  readonly component: ComponentSpec;
  /** The part's label, such as R1. */
  readonly label: string;
  /** False when the fields cannot be edited at all (a locked part, a pending operation). */
  readonly editable: boolean;
  readonly onCommit: (
    componentId: StableId,
    parameterId: ParameterId,
    value: number,
  ) => void;
  readonly onClear: (componentId: StableId, parameterId: ParameterId) => void;
}

/**
 * The parameter widgets of one component, from the catalog's metadata. The Inspector and the
 * double-click editor both render exactly this, and both report through the same callbacks, so
 * a value goes through one validation, one SI conversion and one command path.
 */
export function ComponentParameterFields({
  catalog,
  component,
  label,
  editable,
  onCommit,
  onClear,
}: ComponentParameterFieldsProps) {
  const entry = catalog.componentTypes.find(
    (candidate) => candidate.typeId === component.typeId,
  );
  const values = component.parameters as Record<string, number>;

  return (
    <>
      {(entry?.parameters ?? []).map((definition) => {
        const value = values[definition.id];
        const key = `${component.id}:${definition.id}`;

        if (definition.domain === "zero_or_one") {
          return (
            <ChoiceParameterField
              key={key}
              label={label}
              definition={definition}
              committedValue={value}
              disabled={!editable}
              onCommit={(next) => onCommit(component.id, definition.id, next)}
              onClear={() => onClear(component.id, definition.id)}
            />
          );
        }

        if (definition.optional) {
          return (
            <OptionalParameterField
              key={key}
              label={label}
              definition={definition}
              committedValue={value}
              disabled={!editable}
              onCommit={(next) => onCommit(component.id, definition.id, next)}
              onClear={() => onClear(component.id, definition.id)}
            />
          );
        }

        return value === undefined ? null : (
          <ParameterField
            key={key}
            label={label}
            definition={definition}
            committedValue={value}
            disabled={!editable}
            note={parameterNote(definition)}
            onCommit={(next) => onCommit(component.id, definition.id, next)}
          />
        );
      })}
    </>
  );
}
