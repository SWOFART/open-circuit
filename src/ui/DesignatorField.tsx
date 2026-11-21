import { useEffect, useId, useState } from "react";
import { DESIGNATOR_MESSAGES, designatorProblem } from "../editor/designators";

export interface DesignatorFieldProps {
  /** The name shown for the part now: the designator, or the automatic name of an unnamed part. */
  readonly label: string;
  /** The stored designator, or undefined for a part that was never named. */
  readonly committed: string | undefined;
  /** The designators of the other parts, for the immediate check. */
  readonly others: readonly string[];
  readonly disabled: boolean;
  /** Stores the name; returns why it was refused, or null once it is stored. */
  readonly onCommit: (text: string) => string | null;
}

/**
 * The Designator text field of the double-click editor and the Inspector (contract section 3.5).
 * Enter or leaving the field commits, Escape restores, and a refused value stays in the field
 * with its reason. The form check here mirrors the Engine's, which is the authority.
 */
export function DesignatorField({
  label,
  committed,
  others,
  disabled,
  onCommit,
}: DesignatorFieldProps) {
  const inputId = useId();
  const messageId = `${inputId}-message`;
  const [draft, setDraft] = useState(committed ?? "");
  const [refusal, setRefusal] = useState<string | null>(null);

  useEffect(() => {
    setDraft(committed ?? "");
    setRefusal(null);
  }, [committed]);

  const unchanged = draft === (committed ?? "");
  const problem = unchanged ? null : designatorProblem(draft, others);
  const reason =
    refusal ?? (problem === null ? null : DESIGNATOR_MESSAGES[problem]);

  function commit(): void {
    if (unchanged || problem !== null) return;
    setRefusal(onCommit(draft));
  }

  return (
    <div className="parameter-field">
      <label htmlFor={inputId}>{label} designator</label>
      <input
        id={inputId}
        type="text"
        autoComplete="off"
        spellCheck={false}
        maxLength={7}
        value={draft}
        placeholder={label}
        disabled={disabled}
        aria-invalid={reason !== null}
        aria-describedby={reason !== null ? messageId : undefined}
        onChange={(event) => {
          setDraft(event.target.value);
          setRefusal(null);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
          } else if (event.key === "Escape" && !unchanged) {
            // Restores the stored name. A panel that closes on Escape sees only the Escape of
            // a field that has nothing left to restore.
            event.stopPropagation();
            setDraft(committed ?? "");
            setRefusal(null);
          }
        }}
      />
      {reason !== null && (
        <p id={messageId} className="field-error" role="alert">
          {reason}
        </p>
      )}
    </div>
  );
}
