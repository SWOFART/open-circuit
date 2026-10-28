import {
  useId,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { CheckResult } from "../../generated/CheckResult";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { Diagnostic } from "../../generated/Diagnostic";
import type { ResultLayout } from "../../generated/ResultLayout";
import type { RevealPolicy } from "../../generated/RevealPolicy";
import type { Schematic } from "../../generated/Schematic";
import type { StableId } from "../../generated/StableId";
import {
  emptySchematic,
  type AssignmentAuthoring,
} from "../../store/assignmentAuthoring";
import type {
  AssignmentDraftStore,
  TaskKind,
} from "../../store/assignmentDraft";
import { ComponentLabels } from "../../store/labels";
import { describeDiagnostic, message } from "../messages";
import {
  answerFor,
  describeAssignmentError,
  deviceStatesFor,
  errorsAt,
  hasErrorAt,
  locateError,
  newCountCheck,
  newDeviceStateCheck,
  newRangeCheck,
  newValueCheck,
  signalOptions,
} from "./assignmentView";
import { CheckEditor } from "./CheckEditor";
import { CheckResultView } from "./CheckResultView";
import { FaultEditor } from "./FaultEditor";

export interface FaultView {
  /** The workspace shows the starting circuit with the draft's faults applied. */
  readonly shown: boolean;
  /** Why that faulted circuit has no result, if it does not compile. */
  readonly diagnostics: readonly Diagnostic[];
  readonly failed: boolean;
}

export interface AssignmentPanelProps {
  readonly draftStore: AssignmentDraftStore;
  readonly authoring: AssignmentAuthoring;
  readonly catalog: ComponentCatalog | null;
  /** The circuit in the editor, and the layout the Engine reported for it. */
  readonly document: Schematic;
  readonly layout: ResultLayout | null;
  /** True while a project operation is under way. */
  readonly disabled: boolean;
  /** Shows a circuit of the assignment in the editor. */
  readonly onShowCircuit: (schematic: Schematic) => void;
  readonly faultView: FaultView;
  readonly onFaultViewChange: (shown: boolean) => void;
  readonly onClose: () => void;
}

type Confirmation = "new" | "open";

const REVEAL_WORDS: Readonly<Record<RevealPolicy, string>> = {
  never: "Never: the student sees only whether the diagnosis passed",
  afterCheck: "After a check: the student sees each verdict and the faults",
};

function Section({
  title,
  children,
}: {
  readonly title: string;
  readonly children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="assignment-section" aria-labelledby={id}>
      <h3 id={id}>{title}</h3>
      {children}
    </section>
  );
}

/**
 * The teacher's authoring panel. Its state is the draft store's, apart from the editor and the
 * schematic document; it holds copies of circuits, never the document itself. It validates
 * nothing: the Engine does, through the bridge, and its errors are shown next to the fields.
 */
export function AssignmentPanel({
  draftStore,
  authoring,
  catalog,
  document,
  layout,
  disabled: projectLocked,
  onShowCircuit,
  faultView,
  onFaultViewChange,
  onClose,
}: AssignmentPanelProps) {
  const { draft, version, isDirty } = useSyncExternalStore(
    draftStore.subscribe,
    draftStore.getState,
    draftStore.getState,
  );
  const [busy, setBusy] = useState(false);
  const [confirmation, setConfirmation] = useState<Confirmation | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  // Answers carry the version of the draft they were asked for: one for an older draft is
  // not shown, because its pointers would no longer mean the same fields.
  const [problems, setProblems] = useState<{
    version: number;
    value: AssignmentError[];
  } | null>(null);
  const [fileProblems, setFileProblems] = useState<AssignmentError[] | null>(
    null,
  );
  const [preview, setPreview] = useState<{
    version: number;
    value: CheckResult;
  } | null>(null);
  const disabled = busy || projectLocked;
  const errors = answerFor(problems, version) ?? [];
  const shownPreview = answerFor(preview, version);

  // Named the way a student's document will be: the starting circuit in file order, then
  // the parts only the reference solution has.
  const labels = useMemo(() => {
    const names = new ComponentLabels();
    const starting = names.labelsFor(draft.startingCircuit ?? emptySchematic);
    return draft.reference ? names.labelsFor(draft.reference) : starting;
  }, [draft.startingCircuit, draft.reference]);
  const signals = useMemo(
    () => signalOptions(layout, draft.startingCircuit, labels),
    [layout, draft.startingCircuit, labels],
  );
  const parts = draft.startingCircuit?.components ?? [];
  const checkNames = useMemo(
    () =>
      new Map(
        draft.checks.flatMap((check) =>
          check.label ? [[check.id, check.label] as const] : [],
        ),
      ),
    [draft.checks],
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

  const startNew = (): Promise<void> =>
    run(async () => {
      await authoring.newAssignment();
      setFileProblems(null);
      setNotice("Started a new assignment.");
    });

  const startOpen = (): Promise<void> =>
    run(async () => {
      const outcome = await authoring.open();
      if (outcome.status === "invalid") {
        setFileProblems(outcome.errors);
        setNotice("That file was not opened. Your assignment is unchanged.");
      } else if (outcome.status === "opened") {
        setFileProblems(null);
        setNotice("Opened the assignment.");
      }
    });

  const askBeforeReplacing = (action: Confirmation): void => {
    if (busy) return;
    if (isDirty) setConfirmation(action);
    else void (action === "new" ? startNew() : startOpen());
  };

  const save = (saveAs: boolean): Promise<void> =>
    run(async () => {
      const askedFor = draftStore.getState().version;
      const outcome = await authoring.save(saveAs);
      if (outcome.status === "invalid") {
        setProblems({ version: askedFor, value: outcome.errors });
        setNotice("The assignment was not saved. See the problems below.");
      } else if (outcome.status === "saved") {
        setProblems(null);
        setNotice("Saved.");
      }
    });

  const exportCopy = (): Promise<void> =>
    run(async () => {
      const askedFor = draftStore.getState().version;
      const outcome = await authoring.exportStudentCopy();
      if (outcome.status === "invalid") {
        setProblems({ version: askedFor, value: outcome.errors });
        setNotice("The student copy was not written. See the problems below.");
      } else if (outcome.status === "saved") {
        setNotice("Wrote the student copy, without the reference solution.");
      }
    });

  const validate = (): Promise<void> =>
    run(async () => {
      const askedFor = draftStore.getState().version;
      const outcome = await authoring.validate();
      if (outcome.status === "invalid") {
        setProblems({ version: askedFor, value: outcome.errors });
      } else if (outcome.status === "valid") {
        setProblems(null);
        setNotice("The assignment is valid.");
      }
    });

  const previewChecks = (): Promise<void> =>
    run(async () => {
      const askedFor = draftStore.getState().version;
      const outcome = await authoring.previewChecks();
      switch (outcome.status) {
        case "result":
          setPreview({ version: askedFor, value: outcome.result });
          break;
        case "needsReference":
          setNotice("Set the reference solution first.");
          break;
        case "unavailable":
          setNotice(`The checks could not be run: ${outcome.reason}`);
          break;
        case "stale":
        case "notBuildTask":
          break;
      }
    });

  const generalErrors = errors.filter((error) => {
    const section = locateError(error.path).section;
    return section !== "checks" && section !== "faults";
  });
  const findFault = draft.taskKind === "findFault";
  const lockedIds = new Set<StableId>(draft.lockedComponents);
  const allowed = draft.allowedComponentTypes;
  const titleInvalid = hasErrorAt(errors, "details", "title");
  const instructionsInvalid = hasErrorAt(errors, "details", "instructions");
  const canAddCheck = draft.startingCircuit !== null;
  const firstDevice = parts.find(
    (component) => deviceStatesFor(component.typeId).length > 0,
  );

  return (
    <aside className="assignment-panel" aria-label="Assignment authoring">
      <div className="assignment-header">
        <h2>Assignment</h2>
        <button
          type="button"
          className="toolbar-button"
          onClick={onClose}
          disabled={busy}
        >
          Close
        </button>
      </div>

      {confirmation !== null && (
        <div
          className="assignment-confirm"
          role="alertdialog"
          aria-label="Unsaved assignment"
        >
          <p>
            This assignment has changes that are not saved. Replace it
            {confirmation === "new" ? " with a new one" : " with another file"}?
          </p>
          <button
            type="button"
            className="toolbar-button"
            onClick={() => {
              const action = confirmation;
              setConfirmation(null);
              void (action === "new" ? startNew() : startOpen());
            }}
          >
            Replace
          </button>
          <button
            type="button"
            className="toolbar-button"
            onClick={() => setConfirmation(null)}
          >
            Keep editing
          </button>
        </div>
      )}

      <div
        className="assignment-actions"
        role="group"
        aria-label="Assignment file"
      >
        <button
          type="button"
          className="toolbar-button"
          disabled={disabled}
          onClick={() => askBeforeReplacing("new")}
        >
          New assignment
        </button>
        <button
          type="button"
          className="toolbar-button"
          disabled={disabled}
          onClick={() => askBeforeReplacing("open")}
        >
          Open assignment
        </button>
        <button
          type="button"
          className="toolbar-button"
          disabled={disabled}
          onClick={() => void save(false)}
        >
          Save assignment
        </button>
        <button
          type="button"
          className="toolbar-button"
          disabled={disabled}
          onClick={() => void save(true)}
        >
          Save assignment as
        </button>
        <button
          type="button"
          className="toolbar-button"
          disabled={disabled}
          onClick={() => void exportCopy()}
        >
          Export student copy
        </button>
        <button
          type="button"
          className="toolbar-button"
          disabled={disabled}
          onClick={() => void validate()}
        >
          Check assignment
        </button>
      </div>
      <p className="assignment-state">
        {isDirty ? "Unsaved changes" : "Saved"}
      </p>
      {notice !== null && (
        <p className="assignment-notice" role="status">
          {notice}
        </p>
      )}
      {fileProblems !== null && (
        <ul
          className="field-error-list"
          role="alert"
          aria-label="Problems in the file"
        >
          {fileProblems.map((error, index) => (
            <li key={`${error.path}-${index}`}>
              {locateError(error.path).where}: {describeAssignmentError(error)}
            </li>
          ))}
        </ul>
      )}
      {generalErrors.length > 0 && (
        <ul
          className="field-error-list"
          role="alert"
          aria-label="Problems in the assignment"
        >
          {generalErrors.map((error, index) => (
            <li key={`${error.path}-${index}`}>
              {locateError(error.path).where}: {describeAssignmentError(error)}
            </li>
          ))}
        </ul>
      )}

      <Section title="Task">
        <div className="parameter-field">
          <label htmlFor="assignment-title">Title</label>
          <input
            id="assignment-title"
            type="text"
            maxLength={120}
            value={draft.title}
            disabled={disabled}
            aria-invalid={titleInvalid}
            onChange={(event) => draftStore.setTitle(event.target.value)}
          />
        </div>
        <div className="parameter-field">
          <label htmlFor="assignment-instructions">
            Instructions for the student (plain text)
          </label>
          <textarea
            id="assignment-instructions"
            rows={4}
            maxLength={8000}
            value={draft.instructions}
            disabled={disabled}
            aria-invalid={instructionsInvalid}
            onChange={(event) => draftStore.setInstructions(event.target.value)}
          />
        </div>
        <div className="parameter-field">
          <label htmlFor="assignment-kind">Kind of task</label>
          <select
            id="assignment-kind"
            value={draft.taskKind}
            disabled={disabled}
            onChange={(event) =>
              draftStore.setTaskKind(event.target.value as TaskKind)
            }
          >
            <option value="build">Build a circuit that meets checks</option>
            <option value="findFault">Find the fault in a circuit</option>
          </select>
        </div>
        {findFault && (
          <div className="parameter-field">
            <label htmlFor="assignment-reveal">
              What the student sees of the result
            </label>
            <select
              id="assignment-reveal"
              value={draft.reveal}
              disabled={disabled}
              onChange={(event) =>
                draftStore.setReveal(event.target.value as RevealPolicy)
              }
            >
              {(Object.keys(REVEAL_WORDS) as RevealPolicy[]).map((policy) => (
                <option key={policy} value={policy}>
                  {REVEAL_WORDS[policy]}
                </option>
              ))}
            </select>
          </div>
        )}
        <p className="parameter-hint">
          The file is plain text on the student&apos;s computer. This is a
          classroom aid, not secure examination software.
        </p>
      </Section>

      <Section title="Circuits">
        <p className="assignment-summary">
          Starting circuit:{" "}
          {draft.startingCircuit
            ? `${draft.startingCircuit.components.length} parts`
            : "not set"}
          {findFault ? "" : ". Reference solution: "}
          {findFault
            ? ""
            : draft.reference
              ? `${draft.reference.components.length} parts`
              : "not set"}
        </p>
        <div className="assignment-actions">
          <button
            type="button"
            className="toolbar-button"
            disabled={disabled}
            onClick={() => draftStore.setStartingCircuit(document)}
          >
            Use the circuit in the editor as the starting circuit
          </button>
          {!findFault && (
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled}
              onClick={() => draftStore.setReference(document)}
            >
              Use the circuit in the editor as the reference solution
            </button>
          )}
          <button
            type="button"
            className="toolbar-button"
            disabled={disabled || draft.startingCircuit === null}
            onClick={() =>
              draft.startingCircuit && onShowCircuit(draft.startingCircuit)
            }
          >
            Show the starting circuit in the editor
          </button>
          {!findFault && (
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled || draft.reference === null}
              onClick={() => draft.reference && onShowCircuit(draft.reference)}
            >
              Show the reference solution in the editor
            </button>
          )}
        </div>
        <p className="parameter-hint">
          {findFault
            ? "The student starts from a copy of the starting circuit and finds the faults you add below."
            : "The student starts from a copy of the starting circuit. The reference solution stays in your file only and is left out of the student copy."}
        </p>
      </Section>

      <Section title="Restrictions">
        <fieldset className="assignment-fieldset">
          <legend>
            Locked parts (the student cannot change or delete them)
          </legend>
          {parts.length === 0 && (
            <p className="parameter-hint">Set the starting circuit first.</p>
          )}
          {parts.map((component) => (
            <label key={component.id} className="assignment-check">
              <input
                type="checkbox"
                checked={lockedIds.has(component.id)}
                disabled={disabled}
                onChange={(event) =>
                  draftStore.setComponentLocked(
                    component.id,
                    event.target.checked,
                  )
                }
              />
              Lock {labels.get(component.id) ?? component.typeId}
            </label>
          ))}
        </fieldset>
        <fieldset className="assignment-fieldset">
          <legend>Part types the student can add</legend>
          <label className="assignment-check">
            <input
              type="checkbox"
              checked={allowed === null}
              disabled={disabled || catalog === null}
              onChange={(event) =>
                draftStore.setAllowedComponentTypes(
                  event.target.checked
                    ? null
                    : (catalog?.componentTypes.map((entry) => entry.typeId) ??
                        []),
                )
              }
            />
            Every part type
          </label>
          {allowed !== null &&
            catalog?.componentTypes.map((entry) => (
              <label key={entry.typeId} className="assignment-check">
                <input
                  type="checkbox"
                  checked={allowed.includes(entry.typeId)}
                  disabled={disabled}
                  onChange={(event) =>
                    draftStore.setAllowedComponentTypes(
                      event.target.checked
                        ? [...allowed, entry.typeId]
                        : allowed.filter((type) => type !== entry.typeId),
                    )
                  }
                />
                {message(entry.displayNameKey)}
              </label>
            ))}
        </fieldset>
      </Section>

      {!findFault && (
        <Section title="Checks">
          {draft.checks.map((criterion, index) => (
            <CheckEditor
              key={criterion.id}
              criterion={criterion}
              index={index}
              errors={errorsAt(errors, "checks", index)}
              allErrors={errors}
              signals={signals}
              startingCircuit={draft.startingCircuit}
              labels={labels}
              catalog={catalog}
              disabled={disabled}
              onChange={(next) => draftStore.replaceCheck(next)}
              onRemove={() => draftStore.removeCheck(criterion.id)}
            />
          ))}
          <div
            className="assignment-actions"
            role="group"
            aria-label="Add a check"
          >
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled || signals.length === 0}
              onClick={() => {
                const first = signals[0];
                if (first)
                  draftStore.addCheck(newValueCheck(draftStore.newId(), first));
              }}
            >
              Add a value check
            </button>
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled || signals.length === 0}
              onClick={() => {
                const first = signals[0];
                if (first)
                  draftStore.addCheck(newRangeCheck(draftStore.newId(), first));
              }}
            >
              Add a range check
            </button>
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled || firstDevice === undefined}
              onClick={() => {
                if (firstDevice) {
                  draftStore.addCheck(
                    newDeviceStateCheck(
                      draftStore.newId(),
                      firstDevice.id,
                      firstDevice.typeId,
                    ),
                  );
                }
              }}
            >
              Add a device state check
            </button>
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled || !canAddCheck || catalog === null}
              onClick={() => {
                const first = catalog?.componentTypes[0];
                if (first)
                  draftStore.addCheck(
                    newCountCheck(draftStore.newId(), first.typeId),
                  );
              }}
            >
              Add a part count check
            </button>
          </div>
          {signals.length === 0 && (
            <p className="parameter-hint">
              {draft.startingCircuit === null
                ? "Set the starting circuit to choose signals to check."
                : "Signals come from the Engine's result for the circuit in the editor. It has none for the starting circuit's parts: does it compile?"}
            </p>
          )}
          <div className="assignment-actions">
            <button
              type="button"
              className="toolbar-button"
              disabled={disabled || draft.reference === null}
              onClick={() => void previewChecks()}
            >
              Preview the checks on the reference solution
            </button>
          </div>
          {shownPreview !== null && (
            <CheckResultView
              result={shownPreview}
              labels={labels}
              names={checkNames}
            />
          )}
        </Section>
      )}

      {findFault && (
        <Section title="Faults">
          {draft.faults.map((fault, index) => (
            <FaultEditor
              key={fault.id}
              fault={fault}
              index={index}
              errors={errorsAt(errors, "faults", index)}
              allErrors={errors}
              startingCircuit={draft.startingCircuit}
              labels={labels}
              catalog={catalog}
              disabled={disabled}
              onChange={(next) => draftStore.replaceFault(next)}
              onRemove={() => draftStore.removeFault(fault.id)}
            />
          ))}
          <div className="assignment-actions">
            <button
              type="button"
              className="toolbar-button"
              disabled={
                disabled || parts.length === 0 || draft.faults.length >= 8
              }
              onClick={() => {
                const first = parts[0];
                if (first) {
                  draftStore.addFault({
                    id: draftStore.newId(),
                    kind: "open",
                    componentId: first.id,
                  });
                }
              }}
            >
              Add a fault
            </button>
          </div>
          <label className="assignment-check">
            <input
              type="checkbox"
              checked={faultView.shown}
              disabled={disabled || draft.faults.length === 0}
              onChange={(event) => onFaultViewChange(event.target.checked)}
            />
            Show the faulted circuit in the workspace
          </label>
          <p className="parameter-hint">
            The circuit in the editor should be the starting circuit. Faults are
            applied by the Engine and are never written into the circuit.
          </p>
          {faultView.shown && faultView.failed && (
            <ul
              className="field-error-list"
              role="alert"
              aria-label="Faulted circuit problems"
            >
              <li>
                The faulted circuit has no result, so a student could not use
                it:
              </li>
              {faultView.diagnostics.map((diagnostic, index) => (
                <li key={`${diagnostic.code}-${index}`}>
                  {describeDiagnostic(diagnostic)}
                </li>
              ))}
            </ul>
          )}
        </Section>
      )}
    </aside>
  );
}
