import { useMemo, useState, useSyncExternalStore } from "react";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { CheckResult } from "../../generated/CheckResult";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { DiagnosisResult } from "../../generated/DiagnosisResult";
import type { Schematic } from "../../generated/Schematic";
import type { StableId } from "../../generated/StableId";
import type { AssignmentPort } from "../../store/assignmentBridge";
import {
  answerForKey,
  checkStudentCircuit,
  checkStudentDiagnosis,
  type StudentSessionStore,
} from "../../store/studentAssignment";
import { describeDiagnostic } from "../messages";
import { describeAssignmentError, faultKindsFor } from "./assignmentView";
import { CheckResultView } from "./CheckResultView";
import { DiagnosisEditor } from "./DiagnosisEditor";
import { DiagnosisResultView } from "./DiagnosisResultView";
import type { Diagnostic } from "../../generated/Diagnostic";

export interface StudentPanelProps {
  readonly sessionStore: StudentSessionStore;
  readonly port: AssignmentPort;
  readonly catalog: ComponentCatalog | null;
  /** The circuit the student is editing, and the names its parts have. */
  readonly document: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  /** True while a project operation is under way. */
  readonly disabled: boolean;
  /** Why the last file was not opened, as the Engine's errors. */
  readonly fileErrors: readonly AssignmentError[] | null;
  /** The student's circuit as compiled for the workspace: it may have no result. */
  readonly circuit: {
    readonly failed: boolean;
    readonly diagnostics: readonly Diagnostic[];
  };
  readonly onOpen: () => void;
  readonly onClose: () => void;
}

/**
 * The student's panel: the instructions, a Check action for a build task, or a diagnosis to
 * compose and check for a fault-finding task. Every verdict and number comes from the Engine
 * through the bridge; the panel decides only what an answer may be shown for.
 */
export function StudentPanel({
  sessionStore,
  port,
  catalog,
  document,
  labels,
  disabled: projectLocked,
  fileErrors,
  circuit,
  onOpen,
  onClose,
}: StudentPanelProps) {
  const session = useSyncExternalStore(
    sessionStore.subscribe,
    sessionStore.getState,
    sessionStore.getState,
  );
  const { assignment, entries, entriesVersion } = session;
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  // An answer belongs to what it was asked for: a check to the circuit object, a diagnosis to the
  // version of the findings. Once either has changed the answer is not shown.
  const [check, setCheck] = useState<{
    key: Schematic;
    value: CheckResult;
  } | null>(null);
  const [diagnosis, setDiagnosis] = useState<{
    key: number;
    value: DiagnosisResult;
  } | null>(null);
  const [diagnosisErrors, setDiagnosisErrors] = useState<{
    key: number;
    value: AssignmentError[];
  } | null>(null);
  const disabled = busy || projectLocked;
  const shownCheck = answerForKey(check, document);
  const shownDiagnosis = answerForKey(diagnosis, entriesVersion);
  const shownErrors = answerForKey(diagnosisErrors, entriesVersion) ?? [];
  const checkNames = useMemo(
    () =>
      new Map(
        (assignment?.checks ?? []).flatMap((criterion) =>
          criterion.label ? [[criterion.id, criterion.label] as const] : [],
        ),
      ),
    [assignment],
  );

  async function run(work: () => Promise<void>): Promise<void> {
    if (busy) return;
    setBusy(true);
    setNotice(null);
    try {
      await work();
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  const runCheck = (): Promise<void> =>
    run(async () => {
      if (!assignment) return;
      const asked = document;
      const outcome = await checkStudentCircuit(port, assignment, asked);
      if (outcome.status === "result") {
        setCheck({ key: asked, value: outcome.result });
      } else if (outcome.status === "unavailable") {
        setNotice(`The check could not be run: ${outcome.reason}`);
      }
    });

  const runDiagnosis = (): Promise<void> =>
    run(async () => {
      if (!assignment) return;
      const asked = entriesVersion;
      const outcome = await checkStudentDiagnosis(port, assignment, entries);
      if (outcome.status === "checked") {
        setDiagnosis({ key: asked, value: outcome.result });
        setDiagnosisErrors(null);
      } else if (outcome.status === "invalid") {
        setDiagnosisErrors({ key: asked, value: outcome.errors });
      } else {
        setNotice(`The diagnosis could not be checked: ${outcome.reason}`);
      }
    });

  const startingCircuit = assignment?.startingCircuit ?? null;
  const findFault = assignment?.task.kind === "findFault";
  const reveal =
    assignment?.task.kind === "findFault" ? assignment.task.reveal : "never";
  const lockedNames = [...(assignment?.restrictions?.lockedComponents ?? [])]
    .map((id) => labels.get(id))
    .filter((name): name is string => name !== undefined);

  return (
    <aside className="assignment-panel" aria-label="Student assignment">
      <div className="assignment-header">
        <h2>Assignment</h2>
        {assignment !== null && (
          <button
            type="button"
            className="toolbar-button"
            onClick={onClose}
            disabled={busy}
          >
            Close assignment
          </button>
        )}
      </div>

      {assignment === null ? (
        <>
          <p className="assignment-summary">
            Open an assignment file from your teacher. You start from a copy of
            its circuit, and the file is never changed.
          </p>
          <div className="assignment-actions">
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled}
              onClick={onOpen}
            >
              Open an assignment
            </button>
          </div>
        </>
      ) : (
        <>
          <h3 className="student-title">{assignment.title}</h3>
          <p className="student-instructions">{assignment.instructions}</p>
          {lockedNames.length > 0 && (
            <p className="parameter-hint">
              Locked by the assignment: {lockedNames.join(", ")}. You can move
              and wire them but not change or delete them.
            </p>
          )}
          {assignment.restrictions?.allowedComponentTypes != null && (
            <p className="parameter-hint">
              The palette offers only the part types this assignment allows.
            </p>
          )}
          {circuit.failed && (
            <ul
              className="field-error-list"
              role="alert"
              aria-label="Problems in your circuit"
            >
              <li>Your circuit has no result yet:</li>
              {circuit.diagnostics.map((diagnostic, index) => (
                <li key={`${diagnostic.code}-${index}`}>
                  {describeDiagnostic(diagnostic)}
                </li>
              ))}
            </ul>
          )}

          {!findFault && (
            <section className="assignment-section" aria-label="Check">
              <div className="assignment-actions">
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={disabled}
                  onClick={() => void runCheck()}
                >
                  Check my circuit
                </button>
              </div>
              {shownCheck !== null && (
                <CheckResultView
                  result={shownCheck}
                  labels={labels}
                  names={checkNames}
                />
              )}
            </section>
          )}

          {findFault && startingCircuit !== null && (
            <section className="assignment-section" aria-label="Diagnosis">
              <p className="parameter-hint">
                This circuit has a fault. Measure it with the meters and the
                oscilloscope, then say which part is faulty and how.
              </p>
              {entries.map((entry, index) => (
                <DiagnosisEditor
                  key={index}
                  entry={entry}
                  index={index}
                  startingCircuit={startingCircuit}
                  labels={labels}
                  catalog={catalog}
                  errors={shownErrors}
                  disabled={disabled}
                  onChange={(next) => sessionStore.replaceEntry(index, next)}
                  onRemove={() => sessionStore.removeEntry(index)}
                />
              ))}
              {shownErrors.length > 0 && (
                <ul className="field-error-list" role="alert">
                  {shownErrors.map((error, index) => (
                    <li key={`${error.path}-${index}`}>
                      {describeAssignmentError(error)}
                    </li>
                  ))}
                </ul>
              )}
              <div
                className="assignment-actions"
                role="group"
                aria-label="Diagnosis actions"
              >
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={
                    disabled ||
                    entries.length >= 8 ||
                    startingCircuit.components.length === 0
                  }
                  onClick={() => {
                    const first = startingCircuit.components.find(
                      (component) =>
                        faultKindsFor(catalog, component.typeId).length > 0,
                    );
                    if (first) {
                      sessionStore.addEntry({
                        componentId: first.id,
                        kind: faultKindsFor(catalog, first.typeId)[0] ?? "open",
                      });
                    }
                  }}
                >
                  Add a finding
                </button>
                <button
                  type="button"
                  className="toolbar-button"
                  disabled={disabled}
                  onClick={() => void runDiagnosis()}
                >
                  Check my diagnosis
                </button>
              </div>
              {shownDiagnosis !== null && (
                <DiagnosisResultView
                  result={shownDiagnosis}
                  reveal={reveal}
                  labels={labels}
                  // The faults are named only for a policy that shows them.
                  faults={
                    reveal === "afterCheck" ? (assignment.faults ?? []) : []
                  }
                />
              )}
            </section>
          )}

          <p className="parameter-hint">
            Save your work as a project (Save in the toolbar). It holds your
            circuit only.
          </p>
        </>
      )}

      {notice !== null && (
        <p className="assignment-notice" role="status">
          {notice}
        </p>
      )}
      {fileErrors !== null && (
        <ul
          className="field-error-list"
          role="alert"
          aria-label="Problems in the file"
        >
          <li>That file was not opened. Your circuit is unchanged.</li>
          {fileErrors.map((error, index) => (
            <li key={`${error.path}-${index}`}>
              {describeAssignmentError(error)}
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}
