import { useId } from "react";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { DiagnosisEntry } from "../../generated/DiagnosisEntry";
import type { FaultKind } from "../../generated/FaultKind";
import type { Schematic } from "../../generated/Schematic";
import type { StableId } from "../../generated/StableId";
import { message } from "../messages";
import {
  faultKindsFor,
  faultKindWords,
  faultParametersFor,
} from "./assignmentView";

export interface DiagnosisEditorProps {
  readonly entry: DiagnosisEntry;
  readonly index: number;
  /** The parts of the starting circuit, which are the only parts a fault can be on. */
  readonly startingCircuit: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly catalog: ComponentCatalog | null;
  readonly errors: readonly AssignmentError[];
  readonly disabled: boolean;
  readonly onChange: (entry: DiagnosisEntry) => void;
  readonly onRemove: () => void;
}

/** One finding of the student's diagnosis: a part, and what is wrong with it. */
export function DiagnosisEditor({
  entry,
  index,
  startingCircuit,
  labels,
  catalog,
  errors,
  disabled,
  onChange,
  onRemove,
}: DiagnosisEditorProps) {
  const id = useId();
  const part = startingCircuit.components.find(
    (component) => component.id === entry.componentId,
  );
  const kinds = part ? faultKindsFor(catalog, part.typeId) : [entry.kind];
  const parameters = part ? faultParametersFor(catalog, part.typeId) : [];
  const invalid = (field: string): boolean =>
    errors.some((error) => error.path === `/${index}/${field}`);

  function entryFor(componentId: StableId, kind: FaultKind): DiagnosisEntry {
    const next = startingCircuit.components.find(
      (component) => component.id === componentId,
    );
    const first = next
      ? faultParametersFor(catalog, next.typeId)[0]
      : undefined;
    return kind === "parameter" && first
      ? { componentId, kind, parameterId: first.id }
      : { componentId, kind };
  }

  return (
    <fieldset className="check-editor" aria-labelledby={`${id}-legend`}>
      <legend id={`${id}-legend`}>Finding {index + 1}</legend>
      <div className="parameter-field">
        <label htmlFor={`${id}-part`}>Part</label>
        <select
          id={`${id}-part`}
          value={entry.componentId}
          disabled={disabled}
          aria-invalid={invalid("componentId")}
          onChange={(event) => {
            const componentId = event.target.value;
            const next = startingCircuit.components.find(
              (component) => component.id === componentId,
            );
            const supported = next ? faultKindsFor(catalog, next.typeId) : [];
            const kind = supported.includes(entry.kind)
              ? entry.kind
              : (supported[0] ?? entry.kind);
            onChange(entryFor(componentId, kind));
          }}
        >
          {part === undefined && (
            <option value={entry.componentId}>(not in the circuit)</option>
          )}
          {startingCircuit.components.map((component) => (
            <option key={component.id} value={component.id}>
              {labels.get(component.id) ?? component.typeId}
            </option>
          ))}
        </select>
      </div>
      <div className="parameter-field">
        <label htmlFor={`${id}-kind`}>What is wrong with it</label>
        <select
          id={`${id}-kind`}
          value={entry.kind}
          disabled={disabled}
          aria-invalid={invalid("kind")}
          onChange={(event) =>
            onChange(
              entryFor(entry.componentId, event.target.value as FaultKind),
            )
          }
        >
          {kinds.map((kind) => (
            <option key={kind} value={kind}>
              {faultKindWords[kind]}
            </option>
          ))}
        </select>
      </div>
      {entry.kind === "parameter" && (
        <div className="parameter-field">
          <label htmlFor={`${id}-parameter`}>Which parameter is wrong</label>
          <select
            id={`${id}-parameter`}
            value={entry.parameterId ?? ""}
            disabled={disabled}
            aria-invalid={invalid("parameterId")}
            onChange={(event) => {
              const next = parameters.find(
                (definition) => definition.id === event.target.value,
              );
              if (next) onChange({ ...entry, parameterId: next.id });
            }}
          >
            {!parameters.some(
              (definition) => definition.id === entry.parameterId,
            ) && (
              <option value={entry.parameterId ?? ""}>
                (choose a parameter)
              </option>
            )}
            {parameters.map((definition) => (
              <option key={definition.id} value={definition.id}>
                {message(definition.descriptionKey)}
              </option>
            ))}
          </select>
        </div>
      )}
      <button
        type="button"
        className="toolbar-button"
        disabled={disabled}
        onClick={onRemove}
      >
        Remove finding {index + 1}
      </button>
    </fieldset>
  );
}
