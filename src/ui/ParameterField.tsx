import { useEffect, useId, useState } from "react";
import type { ParameterDefinition } from "../generated/ParameterDefinition";
import { describeInvalidDraft, message } from "./messages";
import { evaluateParameterDraft, formatSi, unitSymbols } from "./units";

export interface ParameterFieldProps {
  readonly label: string;
  readonly definition: ParameterDefinition;
  /** The value last accepted by the engine, in SI units. */
  readonly committedValue: number;
  readonly disabled: boolean;
  /** Called with a validated SI number. Never called for an invalid field. */
  readonly onCommit: (value: number) => void;
}

export function ParameterField({
  label,
  definition,
  committedValue,
  disabled,
  onCommit,
}: ParameterFieldProps) {
  const inputId = useId();
  const messageId = `${inputId}-message`;
  const [draft, setDraft] = useState(() =>
    formatSi(committedValue, definition.unit),
  );

  useEffect(() => {
    setDraft(formatSi(committedValue, definition.unit));
  }, [committedValue, definition.unit]);

  const evaluation = evaluateParameterDraft(draft, definition, committedValue);
  const invalid = evaluation.state === "invalid";

  function commit(): void {
    if (evaluation.state === "commit") {
      onCommit(evaluation.value);
    } else if (evaluation.state === "unchanged") {
      setDraft(formatSi(committedValue, definition.unit));
    }
  }

  return (
    <div className="parameter-field">
      <label htmlFor={inputId}>
        {label} {message(definition.descriptionKey).toLowerCase()}
      </label>
      <input
        id={inputId}
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        value={draft}
        disabled={disabled}
        aria-invalid={invalid}
        aria-describedby={invalid ? messageId : undefined}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
          }
        }}
      />
      {evaluation.state === "invalid" && (
        <p id={messageId} className="field-error" role="alert">
          {describeInvalidDraft(
            evaluation.reason,
            unitSymbols[definition.unit],
          )}
        </p>
      )}
    </div>
  );
}
