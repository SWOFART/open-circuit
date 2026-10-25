import { useId } from "react";
import type { ParameterDefinition } from "../generated/ParameterDefinition";
import type { Unit } from "../generated/Unit";
import { message } from "./messages";
import { ParameterField } from "./ParameterField";

export interface OptionalParameterFieldProps {
  /** The component's display name, such as `C1`. */
  readonly label: string;
  readonly definition: ParameterDefinition;
  /** The stored SI value, or undefined while the parameter is not set. */
  readonly committedValue: number | undefined;
  readonly disabled: boolean;
  /** Called with the value to store: the validated entry, or the offered one. */
  readonly onCommit: (value: number) => void;
  /** Called when the user chooses not to set the parameter any more. */
  readonly onClear: () => void;
}

/** What turning the checkbox on or off does to the stored parameter. */
export type OptionalToggle =
  { readonly kind: "set"; readonly value: number } | { readonly kind: "clear" };

/** Turning it on stores the catalog's offered value; turning it off removes it. */
export function optionalToggle(
  checked: boolean,
  definition: ParameterDefinition,
): OptionalToggle {
  return checked
    ? { kind: "set", value: definition.defaultValue }
    : { kind: "clear" };
}

/** What the sign of an initial value refers to, from the parameter's unit. */
function signReference(unit: Unit): string {
  switch (unit) {
    case "volt":
      return "Voltage V(pin1) − V(pin2) at time zero.";
    case "amp":
      return "Current entering pin1 at time zero.";
    default:
      return "Value at time zero.";
  }
}

/**
 * A parameter that may be absent, such as an initial voltage or current. The
 * user chooses explicitly whether to set it: an unset parameter is not stored,
 * and the circuit starts from its operating point instead. The value offered
 * when the user turns it on is the catalog's `defaultValue`.
 */
export function OptionalParameterField({
  label,
  definition,
  committedValue,
  disabled,
  onCommit,
  onClear,
}: OptionalParameterFieldProps) {
  const checkboxId = useId();
  const isSet = committedValue !== undefined;
  const name = `${label} ${message(definition.descriptionKey).toLowerCase()}`;

  return (
    <div className="optional-parameter">
      <label className="optional-parameter-toggle" htmlFor={checkboxId}>
        <input
          id={checkboxId}
          type="checkbox"
          checked={isSet}
          disabled={disabled}
          onChange={(event) => {
            const toggle = optionalToggle(event.target.checked, definition);

            if (toggle.kind === "set") {
              onCommit(toggle.value);
            } else {
              onClear();
            }
          }}
        />
        Set {name}
      </label>
      {isSet ? (
        <ParameterField
          label={label}
          definition={definition}
          committedValue={committedValue}
          disabled={disabled}
          onCommit={onCommit}
        />
      ) : (
        <p className="parameter-hint">
          Not set: starts from the operating point.
        </p>
      )}
      <p className="parameter-hint">{signReference(definition.unit)}</p>
    </div>
  );
}
