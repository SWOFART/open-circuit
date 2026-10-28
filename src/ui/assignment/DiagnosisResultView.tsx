import type { DiagnosisResult } from "../../generated/DiagnosisResult";
import type { FaultSpec } from "../../generated/FaultSpec";
import type { RevealPolicy } from "../../generated/RevealPolicy";
import type { StableId } from "../../generated/StableId";
import { revealDiagnosis } from "../../store/studentAssignment";
import { message } from "../messages";
import { faultKindWords } from "./assignmentView";

export interface DiagnosisResultViewProps {
  readonly result: DiagnosisResult;
  readonly reveal: RevealPolicy;
  readonly labels: ReadonlyMap<StableId, string>;
  /** The assignment's faults: read only to name a fault after a check the policy allows. */
  readonly faults: readonly FaultSpec[];
}

/**
 * The Engine's comparison of a diagnosis, as much of it as the task's reveal policy shows. With
 * `never` there is the outcome and nothing else: no verdict of a finding, and no fault.
 */
export function DiagnosisResultView({
  result,
  reveal,
  labels,
  faults,
}: DiagnosisResultViewProps) {
  const shown = revealDiagnosis(result, reveal);

  return (
    <section className="check-result" aria-label="Diagnosis result">
      <p
        className={`check-outcome check-outcome-${result.outcome}`}
        role="status"
      >
        {message(shown.outcomeKey)}
      </p>
      {shown.entries !== null && shown.entries.length > 0 && (
        <ul className="check-result-list" aria-label="Your findings">
          {shown.entries.map((entry, index) => (
            <li
              key={`${entry.entry.componentId}-${entry.entry.kind}-${index}`}
              className={`check-row check-row-${entry.verdict === "correct" ? "pass" : "fail"}`}
            >
              <strong>Finding {index + 1}:</strong> {message(entry.messageKey)}
              <span className="check-detail">
                {labels.get(entry.entry.componentId) ?? "A part"},{" "}
                {faultKindWords[entry.entry.kind]}
              </span>
            </li>
          ))}
        </ul>
      )}
      {shown.faults !== null && (
        <ul className="check-result-list" aria-label="The faults">
          {shown.faults.map((fault, index) => {
            const spec = faults.find(
              (candidate) => candidate.id === fault.faultId,
            );
            return (
              <li
                key={fault.faultId}
                className={`check-row check-row-${fault.verdict === "found" ? "pass" : "fail"}`}
              >
                <strong>Fault {index + 1}:</strong> {message(fault.messageKey)}
                {spec !== undefined && (
                  <span className="check-detail">
                    {labels.get(spec.componentId) ?? "A part"},{" "}
                    {faultKindWords[spec.kind]}
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
