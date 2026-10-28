import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Assignment } from "../generated/Assignment";
import type { AssignmentDecoded } from "../generated/AssignmentDecoded";
import type { AssignmentSaveOutcome } from "../generated/AssignmentSaveOutcome";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CheckResult } from "../generated/CheckResult";
import type { DiagnosisChecked } from "../generated/DiagnosisChecked";
import type { DiagnosisEntry } from "../generated/DiagnosisEntry";

/**
 * The UI's view of assignment files and checks. Paths and filesystem access stay in Rust,
 * every rule is validated by the Engine, and a refused document comes back as the Engine's
 * list of errors with JSON pointers, not as a thrown error. An error is thrown only when the
 * backend cannot be reached or a file cannot be read.
 */
export interface AssignmentPort {
  /** Opens the native file picker. `null` when the user cancels. */
  open(): Promise<AssignmentDecoded | null>;
  /**
   * Validates a document, valid or not, with the Engine's rules and without touching any
   * file. The document is not typed, because a draft under edit may be incomplete.
   */
  validate(document: unknown): Promise<AssignmentDecoded>;
  /** Saves to the active path, or asks for one. Nothing is written for an invalid document. */
  save(assignment: Assignment): Promise<AssignmentSaveOutcome>;
  saveAs(assignment: Assignment): Promise<AssignmentSaveOutcome>;
  /** Writes the student's copy (without the reference); the active path does not change. */
  exportStudentCopy(assignment: Assignment): Promise<AssignmentSaveOutcome>;
  /** Forgets the active path when a new assignment is started. */
  clearPath(): Promise<void>;
  /**
   * Evaluates the assignment's checks on a student's schematic, on an Engine of its own and
   * away from the live run. The result carries the request's `documentRevision`.
   */
  checkAssignment(request: CheckRequest): Promise<CheckResult>;
  /** Compares a diagnosis with the faults of a fault-finding assignment. */
  checkDiagnosis(
    assignment: Assignment,
    diagnosis: DiagnosisEntry[],
  ): Promise<DiagnosisChecked>;
}

async function invokeAssignment<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) {
    throw new Error("Assignment files are available in the desktop app only.");
  }

  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(error instanceof Error ? error.message : String(error), {
      cause: error,
    });
  }
}

/** Calls the native assignment and check commands exposed by `crates/app`. */
export const tauriAssignments: AssignmentPort = {
  open: () => invokeAssignment<AssignmentDecoded | null>("open_assignment"),
  validate: (document) =>
    invokeAssignment<AssignmentDecoded>("validate_assignment", {
      assignment: document,
    }),
  save: (assignment) =>
    invokeAssignment<AssignmentSaveOutcome>("save_assignment", { assignment }),
  saveAs: (assignment) =>
    invokeAssignment<AssignmentSaveOutcome>("save_assignment_as", {
      assignment,
    }),
  exportStudentCopy: (assignment) =>
    invokeAssignment<AssignmentSaveOutcome>("export_student_copy", {
      assignment,
    }),
  clearPath: () => invokeAssignment<void>("clear_assignment_path"),
  checkAssignment: (request) =>
    invokeAssignment<CheckResult>("check_assignment", { request }),
  checkDiagnosis: (assignment, diagnosis) =>
    invokeAssignment<DiagnosisChecked>("check_fault_diagnosis", {
      assignment,
      diagnosis,
    }),
};

/**
 * A check result is shown only while the document it was made for is the current one; any
 * other result is discarded without an error, as a stale frame is.
 */
export function checkResultBelongsTo(
  result: CheckResult,
  documentRevision: string,
): boolean {
  return result.documentRevision === documentRevision;
}
