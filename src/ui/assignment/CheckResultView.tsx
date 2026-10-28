import type { CheckResult } from "../../generated/CheckResult";
import type { CheckVerdict } from "../../generated/CheckVerdict";
import type { CriterionResult } from "../../generated/CriterionResult";
import type { DiagnosticReference } from "../../generated/DiagnosticReference";
import type { StableId } from "../../generated/StableId";
import { describeDiagnostic, message } from "../messages";
import { formatSi } from "../units";
import { deviceStateWords, formatSeconds } from "./assignmentView";

const verdictWords: Readonly<Record<CheckVerdict, string>> = {
  pass: "Passed",
  fail: "Failed",
  notEvaluated: "Not checked",
};

function partNames(
  references: readonly DiagnosticReference[],
  labels: ReadonlyMap<StableId, string>,
): string {
  return references
    .flatMap((reference) =>
      reference.kind === "component"
        ? [
            labels.get(reference.componentId) ??
              "a part that is no longer there",
          ]
        : [],
    )
    .join(", ");
}

/** The numbers of a criterion, as the Engine reported them, with their units. */
function details(
  result: CriterionResult,
  labels: ReadonlyMap<StableId, string>,
): string[] {
  const lines: string[] = [];

  switch (result.kind) {
    case "value": {
      const tolerance = [
        result.toleranceAbs > 0
          ? `±${formatSi(result.toleranceAbs, result.unit)}`
          : "",
        result.toleranceRel > 0 ? `±${result.toleranceRel * 100} %` : "",
      ]
        .filter((part) => part !== "")
        .join(" and ");
      lines.push(
        `At ${formatSeconds(result.atSeconds)}: expected ${formatSi(result.expected, result.unit)}${tolerance === "" ? "" : `, ${tolerance}`}.`,
      );
      if (result.measured !== undefined) {
        lines.push(`Measured ${formatSi(result.measured, result.unit)}.`);
      }
      break;
    }
    case "range": {
      lines.push(
        `From ${formatSeconds(result.fromSeconds)} to ${formatSeconds(result.toSeconds)}: between ${formatSi(result.min, result.unit)} and ${formatSi(result.max, result.unit)}.`,
      );
      if (result.measured !== undefined && result.atSeconds !== undefined) {
        lines.push(
          `Closest to a limit: ${formatSi(result.measured, result.unit)} at ${formatSeconds(result.atSeconds)}.`,
        );
      }
      break;
    }
    case "deviceState": {
      lines.push(
        `At ${formatSeconds(result.atSeconds)}: expected ${deviceStateWords[result.expected]}.`,
      );
      if (result.measured !== undefined) {
        lines.push(`Found ${deviceStateWords[result.measured]}.`);
      }
      break;
    }
    case "componentCount": {
      const limit =
        result.max === undefined
          ? `at least ${result.min}`
          : result.max === result.min
            ? `exactly ${result.min}`
            : `from ${result.min} to ${result.max}`;
      lines.push(`Expected ${limit}.`);
      if (result.measured !== undefined) {
        lines.push(`Found ${result.measured}.`);
      }
      break;
    }
  }

  const parts = partNames(result.references, labels);
  if (parts !== "" && result.kind !== "componentCount") {
    lines.push(`Part: ${parts}.`);
  }

  return lines;
}

export interface CheckResultViewProps {
  readonly result: CheckResult;
  readonly labels: ReadonlyMap<StableId, string>;
  /** The authored names of the criteria, by ID. */
  readonly names?: ReadonlyMap<StableId, string>;
}

/**
 * The Engine's structured check result: the outcome, the locked parts, and each criterion with
 * its verdict, measured and expected values, tolerance, and message. Nothing here is computed.
 */
export function CheckResultView({
  result,
  labels,
  names,
}: CheckResultViewProps) {
  return (
    <section className="check-result" aria-label="Check result">
      <p
        className={`check-outcome check-outcome-${result.outcome}`}
        role="status"
      >
        {message(result.outcomeKey)}{" "}
        <span className="check-counts">
          {result.counts.pass} passed, {result.counts.fail} failed,{" "}
          {result.counts.notEvaluated} not checked.
        </span>
      </p>
      <ul className="check-result-list">
        <li className={`check-row check-row-${result.locked.verdict}`}>
          <strong>Locked parts: {verdictWords[result.locked.verdict]}.</strong>{" "}
          {message(result.locked.messageKey)}
          {partNames(result.locked.references, labels) !== "" &&
            ` (${partNames(result.locked.references, labels)})`}
        </li>
        {result.criteria.map((criterion, index) => (
          <li
            key={criterion.id}
            className={`check-row check-row-${criterion.verdict}`}
          >
            <strong>
              Check {index + 1}
              {names?.get(criterion.id)
                ? ` (${names.get(criterion.id)})`
                : ""}: {verdictWords[criterion.verdict]}.
            </strong>{" "}
            {message(criterion.messageKey)}
            {details(criterion, labels).map((line) => (
              <span key={line} className="check-detail">
                {line}
              </span>
            ))}
          </li>
        ))}
      </ul>
      {result.diagnostics.length > 0 && (
        <ul className="field-error-list" aria-label="Circuit problems">
          {result.diagnostics.map((diagnostic, index) => (
            <li key={`${diagnostic.code}-${index}`}>
              {describeDiagnostic(diagnostic)}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
