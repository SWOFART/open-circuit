import { useEffect, useId, useRef, useState } from "react";

export type NumberParse =
  { readonly ok: true; readonly value: number } | { readonly ok: false };

export interface NumberFieldProps {
  readonly label: string;
  readonly value: number;
  /** Text to number. Anything it refuses is shown as invalid and never committed. */
  readonly parse: (text: string) => NumberParse;
  /** The number as text, for display. */
  readonly format: (value: number) => string;
  /** Changes when `format` and `parse` mean something else, such as another unit. */
  readonly formatKey?: string;
  /** What to type, shown when the text is not accepted. */
  readonly hint: string;
  /** The Engine refused this field: marks it invalid until the next edit. */
  readonly engineError?: boolean;
  readonly disabled?: boolean;
  readonly onCommit: (value: number) => void;
}

/**
 * A number typed with units, such as `2.5`, `20m`, or `4.7 kΩ`. The text is parsed into a
 * plain SI number before it crosses to the draft; a field that does not parse is never
 * committed. Whether the number is allowed (a tolerance cannot be negative, a time must fall
 * on the 1 ms step) is the Engine's to say, and it says so when the document is validated.
 */
export function NumberField({
  label,
  value,
  parse,
  format,
  formatKey = "",
  hint,
  engineError = false,
  disabled = false,
  onCommit,
}: NumberFieldProps) {
  const inputId = useId();
  const messageId = `${inputId}-message`;
  const [draft, setDraft] = useState(() => format(value));
  // Callers may pass a new function every render; only a new value or unit resets the text.
  const formatRef = useRef(format);
  formatRef.current = format;

  useEffect(() => {
    setDraft(formatRef.current(value));
  }, [value, formatKey]);

  const parsed = parse(draft);
  const unparsable = !parsed.ok && draft.trim() !== format(value);

  function commit(): void {
    // A value is shown rounded, so text that is the display of the value is not an edit.
    if (draft.trim() === format(value)) return;

    if (parsed.ok) {
      if (parsed.value !== value) onCommit(parsed.value);
    } else {
      setDraft(format(value));
    }
  }

  return (
    <div className="parameter-field">
      <label htmlFor={inputId}>{label}</label>
      <input
        id={inputId}
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        value={draft}
        disabled={disabled}
        aria-invalid={unparsable || engineError}
        aria-describedby={unparsable ? messageId : undefined}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") commit();
        }}
      />
      {unparsable && (
        <p id={messageId} className="field-error" role="alert">
          {hint}
        </p>
      )}
    </div>
  );
}
