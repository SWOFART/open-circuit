import { useId } from "react";
import type { ParameterDefinition } from "../generated/ParameterDefinition";
import { message } from "./messages";

export interface ChoiceParameterFieldProps {
  /** The component's display name, such as `S1`. */
  readonly label: string;
  readonly definition: ParameterDefinition;
  /** The stored value (0 or 1), or undefined while the parameter is not stored. */
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

  return (
    <div className="parameter-field">
      <label htmlFor={id}>{name}</label>
      <select
        id={id}
        value={committedValue === 1 ? "closed" : "open"}
        disabled={disabled}
        onChange={(event) => {
          if (event.target.value === "closed") {
            onCommit(1);
          } else if (definition.optional) {
            onClear();
          } else {
            onCommit(0);
          }
        }}
      >
        <option value="open">Open</option>
        <option value="closed">Closed</option>
      </select>
      <p className="parameter-hint">
        {message(`${definition.descriptionKey}Hint`)}
      </p>
    </div>
  );
}
