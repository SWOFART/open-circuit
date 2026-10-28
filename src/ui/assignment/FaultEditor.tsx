import { useId } from "react";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { FaultKind } from "../../generated/FaultKind";
import type { FaultSpec } from "../../generated/FaultSpec";
import type { ParameterId } from "../../generated/ParameterId";
import type { Schematic } from "../../generated/Schematic";
import type { StableId } from "../../generated/StableId";
import { message } from "../messages";
import { formatSi, parseSi, unitSymbols } from "../units";
import {
  describeAssignmentError,
  faultKindsFor,
  faultKindWords,
  faultParametersFor,
  hasErrorAt,
} from "./assignmentView";
import { NumberField } from "./NumberField";

export interface FaultEditorProps {
  readonly fault: FaultSpec;
  readonly index: number;
  readonly errors: readonly AssignmentError[];
  readonly allErrors: readonly AssignmentError[];
  readonly startingCircuit: Schematic | null;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly catalog: ComponentCatalog | null;
  readonly disabled: boolean;
  readonly onChange: (fault: FaultSpec) => void;
  readonly onRemove: () => void;
}

/** A fault of a fault-finding task. The catalog says which kinds and parameters a part has. */
export function FaultEditor({
  fault,
  index,
  errors,
  allErrors,
  startingCircuit,
  labels,
  catalog,
  disabled,
  onChange,
  onRemove,
}: FaultEditorProps) {
  const id = useId();
  const invalid = (field: string): boolean =>
    hasErrorAt(allErrors, "faults", field, index);
  const parts = startingCircuit?.components ?? [];
  const part = parts.find((component) => component.id === fault.componentId);
  const kinds = part ? faultKindsFor(catalog, part.typeId) : [fault.kind];
  const parameters = part ? faultParametersFor(catalog, part.typeId) : [];
  const parameter = parameters.find(
    (definition) => definition.id === fault.parameterId,
  );

  /** The fault with the kind another part or kind needs: only a parameter fault has a value. */
  function withKind(kind: FaultKind, componentId: StableId): FaultSpec {
    const base = { id: fault.id, kind, componentId };
    if (kind !== "parameter") return base;

    const next = parts.find((component) => component.id === componentId);
    const first = next
      ? faultParametersFor(catalog, next.typeId)[0]
      : undefined;
    return first
      ? {
          ...base,
          parameterId: first.id,
          value: first.defaultValue,
        }
      : base;
  }

  function changePart(componentId: StableId): void {
    const next = parts.find((component) => component.id === componentId);
    const supported = next ? faultKindsFor(catalog, next.typeId) : [];
    const kind = supported.includes(fault.kind)
      ? fault.kind
      : (supported[0] ?? fault.kind);
    onChange(withKind(kind, componentId));
  }

  return (
    <fieldset className="check-editor" aria-labelledby={`${id}-legend`}>
      <legend id={`${id}-legend`}>Fault {index + 1}</legend>
      <div className="parameter-field">
        <label htmlFor={`${id}-part`}>Faulty part</label>
        <select
          id={`${id}-part`}
          value={fault.componentId}
          disabled={disabled}
          aria-invalid={invalid("componentId")}
          onChange={(event) => changePart(event.target.value)}
        >
          {part === undefined && (
            <option value={fault.componentId}>
              (not in the starting circuit)
            </option>
          )}
          {parts.map((component) => (
            <option key={component.id} value={component.id}>
              {labels.get(component.id) ?? component.typeId}
            </option>
          ))}
        </select>
      </div>
      <div className="parameter-field">
        <label htmlFor={`${id}-kind`}>Kind of fault</label>
        <select
          id={`${id}-kind`}
          value={fault.kind}
          disabled={disabled}
          aria-invalid={invalid("kind")}
          onChange={(event) =>
            onChange(
              withKind(event.target.value as FaultKind, fault.componentId),
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
      {fault.kind === "parameter" && (
        <>
          <div className="parameter-field">
            <label htmlFor={`${id}-parameter`}>Parameter</label>
            <select
              id={`${id}-parameter`}
              value={fault.parameterId ?? ""}
              disabled={disabled}
              aria-invalid={invalid("parameterId")}
              onChange={(event) => {
                const next = parameters.find(
                  (definition) => definition.id === event.target.value,
                );
                if (!next) return;
                onChange({
                  ...fault,
                  parameterId: next.id as ParameterId,
                  value: next.defaultValue,
                });
              }}
            >
              {parameter === undefined && (
                <option value={fault.parameterId ?? ""}>
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
          {parameter !== undefined && (
            <NumberField
              label={`Wrong value (${unitSymbols[parameter.unit]})`}
              value={fault.value ?? parameter.defaultValue}
              parse={(text) => parseSi(text, parameter.unit)}
              format={(value) => formatSi(value, parameter.unit)}
              formatKey={parameter.id}
              hint="Enter a number, optionally with a prefix, for example 4.7k."
              engineError={invalid("value")}
              disabled={disabled}
              onCommit={(value) => onChange({ ...fault, value })}
            />
          )}
        </>
      )}
      {errors.length > 0 && (
        <ul className="field-error-list" role="alert">
          {errors.map((error, position) => (
            <li key={`${error.path}-${position}`}>
              {describeAssignmentError(error)}
            </li>
          ))}
        </ul>
      )}
      <button
        type="button"
        className="toolbar-button"
        disabled={disabled}
        onClick={onRemove}
      >
        Remove fault {index + 1}
      </button>
    </fieldset>
  );
}
