import { useId } from "react";
import type { ParameterDefinition } from "../generated/ParameterDefinition";
import { message } from "./messages";

export interface ChoiceParameterFieldProps {
  /** The component's display name, such as `S1`. */
  readonly label: string;
  readonly definition: ParameterDefinition;
  /** The stored value, or undefined while the parameter is not stored. */
  readonly committedValue: number | undefined;
  readonly disabled: boolean;
  readonly onCommit: (value: number) => void;
  /** Stops storing an optional parameter. */
  readonly onClear: () => void;
}

/**
 * A parameter with the domain `zero_or_one` (docs/contracts/stage-6-control-circuits.md
 * section 2), shown as a choice between open and closed instead of a number field. An
 * optional one, such as a switch's initial position, is not stored when it is open: the
 * saved file then says nothing, which means open.
 */
export function ChoiceParameterField({
  label,
  definition,
  committedValue,
  disabled,
  onCommit,
  onClear,
}: ChoiceParameterFieldProps) {
  const id = useId();
  const name = `${label} ${message(definition.descriptionKey).toLowerCase()}`;

  if (definition.id === "initialPosition") {
    const positions =
      definition.domain === "zero_one_or_two"
        ? ["Left", "Center", "Right"]
        : ["Off", "On"];
    return (
      <div className="parameter-field">
        <label htmlFor={id}>{name}</label>
        <select
          id={id}
          value={committedValue ?? definition.defaultValue}
          disabled={disabled}
          onChange={(event) => {
            const next = Number(event.currentTarget.value);
            if (next === definition.defaultValue) onClear();
            else onCommit(next);
          }}
        >
          {positions.map((position, index) => (
            <option key={position} value={index}>
              {position}
            </option>
          ))}
        </select>
      </div>
    );
  }

  const isPhaseSequence = definition.id === "reversedSequence";
  const off = isPhaseSequence ? "standard" : "open";
  const on = isPhaseSequence ? "reversed" : "closed";
  const hint = isPhaseSequence
    ? "Reversing the sequence swaps L2 and L3."
    : message(`${definition.descriptionKey}Hint`);

  return (
    <div className="parameter-field">
      <label htmlFor={id}>{name}</label>
      <select
        id={id}
        value={committedValue === 1 ? on : off}
        disabled={disabled}
        onChange={(event) => {
          if (event.target.value === on) {
            onCommit(1);
          } else if (definition.optional) {
            onClear();
          } else {
            onCommit(0);
          }
        }}
      >
        <option value={off}>{isPhaseSequence ? "Standard" : "Open"}</option>
        <option value={on}>{isPhaseSequence ? "Reversed" : "Closed"}</option>
      </select>
      <p className="parameter-hint">{hint}</p>
    </div>
  );
}
