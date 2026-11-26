import type { AssignmentGrading } from "../../generated/AssignmentGrading";
import type { CheckCriterion } from "../../generated/CheckCriterion";
import type { CriterionGrading } from "../../generated/CriterionGrading";
import { NumberField } from "./NumberField";

interface GradingEditorProps {
  readonly grading: AssignmentGrading | null;
  readonly checks: readonly CheckCriterion[];
  readonly disabled: boolean;
  readonly onEnable: (enabled: boolean) => void;
  readonly onCriterion: (row: CriterionGrading) => void;
  readonly onHint: (key: string, text: string) => void;
}

export function GradingEditor({
  grading,
  checks,
  disabled,
  onEnable,
  onCriterion,
  onHint,
}: GradingEditorProps) {
  const rows = new Map(grading?.criteria.map((row) => [row.criterionId, row]));
  const possible = grading?.criteria.reduce((sum, row) => sum + row.points, 0);
  return (
    <fieldset disabled={disabled}>
      <legend>Scoring and hints</legend>
      <label>
        <input
          type="checkbox"
          checked={grading !== null}
          onChange={(event) => onEnable(event.target.checked)}
        />
        Enable scoring
      </label>
      {grading && (
        <>
          <p className="parameter-hint">{possible} points possible.</p>
          {possible === 0 && (
            <p className="field-error">
              Assign at least one point across the checks.
            </p>
          )}
          {checks.map((check, index) => {
            const row = rows.get(check.id);
            if (!row) return null;
            const key = row.hintKey ?? "";
            const validKey = /^[a-zA-Z0-9_-]{1,64}$/.test(key);
            const hint = Object.hasOwn(grading.hints, key)
              ? (grading.hints[key] ?? "")
              : "";
            const checkNumber = index + 1;
            return (
              <fieldset key={check.id}>
                <legend>{check.label || `Check ${checkNumber}`}</legend>
                <NumberField
                  label={`Points for check ${checkNumber}`}
                  value={row.points}
                  parse={(text) => {
                    const digits = text.trim();
                    const value = Number(digits);
                    return /^\d+$/.test(digits) && value <= 1000
                      ? { ok: true, value }
                      : { ok: false };
                  }}
                  format={String}
                  hint="Enter a whole number from 0 to 1000."
                  disabled={disabled}
                  onCommit={(points) => onCriterion({ ...row, points })}
                />
                <label>
                  Hint key for check {checkNumber}
                  <input
                    value={key}
                    aria-invalid={key !== "" && !validKey}
                    onChange={(event) => {
                      const hintKey = event.target.value;
                      onCriterion({
                        criterionId: row.criterionId,
                        points: row.points,
                        ...(hintKey.length > 0 ? { hintKey } : {}),
                      });
                    }}
                  />
                </label>
                {key !== "" && !validKey && (
                  <p className="field-error">
                    Use 1 to 64 letters, digits, hyphens or underscores.
                  </p>
                )}
                {validKey && (
                  <label>
                    Hint text for check {checkNumber}
                    <textarea
                      value={hint}
                      aria-invalid={
                        hint.trim().length === 0 || [...hint].length > 500
                      }
                      onChange={(event) => onHint(key, event.target.value)}
                    />
                  </label>
                )}
              </fieldset>
            );
          })}
        </>
      )}
    </fieldset>
  );
}
