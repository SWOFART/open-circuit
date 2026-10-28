import type { EditorCommand } from "../editor/commands";
import { buildGeometryEnvelope } from "../editor/geometry";
import type { Assignment } from "../generated/Assignment";
import type { AssignmentError } from "../generated/AssignmentError";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CheckResult } from "../generated/CheckResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DiagnosisEntry } from "../generated/DiagnosisEntry";
import type { DiagnosisResult } from "../generated/DiagnosisResult";
import type { FaultSpec } from "../generated/FaultSpec";
import type { RevealPolicy } from "../generated/RevealPolicy";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols, type SymbolRegistry } from "../symbols";
import { emptySchematic } from "./assignmentAuthoring";
import { checkResultBelongsTo, type AssignmentPort } from "./assignmentBridge";

// The student's side of an assignment. It holds the assignment as the student's copy (the
// teacher's reference solution is dropped the moment a file is opened), the diagnosis being
// composed, and nothing about any circuit. A check or a diagnosis is answered by the Engine
// through the bridge; this module only builds the requests, enforces the editor restrictions
// of the file (contract section 3), and decides what an answer may be shown for.

/** The editor restrictions of an assignment (contract section 3). */
export interface EditorRestrictions {
  /** `null`: every catalog type. */
  readonly allowedTypes: readonly ComponentTypeId[] | null;
  readonly locked: ReadonlySet<StableId>;
}

export function restrictionsOf(assignment: Assignment): EditorRestrictions {
  return {
    allowedTypes: assignment.restrictions?.allowedComponentTypes ?? null,
    locked: new Set(assignment.restrictions?.lockedComponents ?? []),
  };
}

export type Verdict =
  | { readonly allowed: true }
  | { readonly allowed: false; readonly reason: string };

const ALLOWED: Verdict = { allowed: true };

/**
 * Whether the assignment lets the student apply a canvas command. A locked part cannot be
 * deleted; moving, rotating, and wiring to it are allowed. A part type outside the allowed list
 * cannot be placed. The document store's own rules are not changed: this only refuses a command
 * before it is applied, and the Engine's `locked` check verifies the result independently.
 */
export function commandVerdict(
  command: EditorCommand,
  restrictions: EditorRestrictions | null,
): Verdict {
  if (restrictions === null) return ALLOWED;

  switch (command.kind) {
    case "placeComponent":
      return restrictions.allowedTypes === null ||
        restrictions.allowedTypes.includes(command.typeId)
        ? ALLOWED
        : {
            allowed: false,
            reason: "The assignment does not allow adding that kind of part.",
          };
    case "deleteComponent":
      return restrictions.locked.has(command.componentId)
        ? {
            allowed: false,
            reason:
              "That part is locked by the assignment and cannot be deleted.",
          }
        : ALLOWED;
    default:
      return ALLOWED;
  }
}

/** Whether the assignment lets the student change a parameter of a part. */
export function parameterVerdict(
  componentId: StableId,
  restrictions: EditorRestrictions | null,
): Verdict {
  return restrictions?.locked.has(componentId)
    ? {
        allowed: false,
        reason: "That part is locked by the assignment and cannot be edited.",
      }
    : ALLOWED;
}

/** The catalog the palette offers: only the part types the assignment allows. */
export function allowedCatalog(
  catalog: ComponentCatalog | null,
  restrictions: EditorRestrictions | null,
): ComponentCatalog | null {
  if (catalog === null || restrictions?.allowedTypes == null) return catalog;

  const allowed = restrictions.allowedTypes;
  return {
    ...catalog,
    componentTypes: catalog.componentTypes.filter((entry) =>
      allowed.includes(entry.typeId),
    ),
  };
}

/** The assignment as a student keeps it: without the teacher's reference solution. */
export function studentCopyOf(assignment: Assignment): Assignment {
  const copy = { ...assignment };
  delete copy.reference;
  return copy;
}

export type StudentOpenOutcome =
  | { readonly status: "cancelled" }
  | { readonly status: "invalid"; readonly errors: AssignmentError[] }
  | {
      readonly status: "opened";
      readonly assignment: Assignment;
      /** The new, unsaved document the student starts from: a copy of the starting circuit. */
      readonly circuit: Schematic;
    };

/** Opens a file through the bridge. Only a file the Engine accepted yields a circuit. */
export async function openStudentAssignment(
  port: AssignmentPort,
): Promise<StudentOpenOutcome> {
  const decoded = await port.open();

  if (decoded === null) return { status: "cancelled" };
  if (decoded.status === "invalid") {
    return { status: "invalid", errors: decoded.errors };
  }

  // The bridge made the file the active assignment path. A student's file must never become
  // the target of the teacher's Save, so the path is forgotten again.
  await port.clearPath();

  const assignment = studentCopyOf(decoded.assignment);
  return {
    status: "opened",
    assignment,
    circuit: structuredClone(assignment.startingCircuit ?? emptySchematic),
  };
}

/** The faults the student's runs carry: the faults of a fault-finding task, never shown. */
export function faultsOf(
  assignment: Assignment | null,
): readonly FaultSpec[] | null {
  return assignment?.task.kind === "findFault"
    ? (assignment.faults ?? null)
    : null;
}

export type StudentCheckOutcome =
  | { readonly status: "result"; readonly result: CheckResult }
  /** The answer is not for this request, so it is not shown. */
  | { readonly status: "stale" }
  | { readonly status: "notBuildTask" }
  | { readonly status: "unavailable"; readonly reason: string };

let revisionCounter = 0;

function createCheckRevision(): string {
  revisionCounter += 1;
  return `check-${revisionCounter}-${Math.random().toString(36).slice(2, 8)}`;
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Asks the Engine to check the student's circuit against the assignment. The request is made
 * for the circuit as it is now; the caller shows the result only while the document is still
 * that circuit (the result carries the request's revision, and `answerFor` keys it by the
 * circuit itself).
 */
export async function checkStudentCircuit(
  port: AssignmentPort,
  assignment: Assignment,
  schematic: Schematic,
  registry: SymbolRegistry = symbols,
  createRevision: () => string = createCheckRevision,
): Promise<StudentCheckOutcome> {
  if (assignment.task.kind !== "build") return { status: "notBuildTask" };

  const revision = createRevision();
  let result: CheckResult;

  try {
    const request: CheckRequest = {
      checks: [...(assignment.checks ?? [])],
      lockedComponents: [...(assignment.restrictions?.lockedComponents ?? [])],
      startingCircuit: assignment.startingCircuit ?? emptySchematic,
      schematic,
      geometry: buildGeometryEnvelope(schematic, registry),
      documentRevision: revision,
    };
    result = await port.checkAssignment(request);
  } catch (error) {
    return { status: "unavailable", reason: describe(error) };
  }

  return checkResultBelongsTo(result, revision)
    ? { status: "result", result }
    : { status: "stale" };
}

export type StudentDiagnosisOutcome =
  | { readonly status: "checked"; readonly result: DiagnosisResult }
  | { readonly status: "invalid"; readonly errors: AssignmentError[] }
  | { readonly status: "unavailable"; readonly reason: string };

/** Compares the student's diagnosis with the faults, through the Engine. */
export async function checkStudentDiagnosis(
  port: AssignmentPort,
  assignment: Assignment,
  entries: readonly DiagnosisEntry[],
): Promise<StudentDiagnosisOutcome> {
  try {
    const checked = await port.checkDiagnosis(assignment, [...entries]);
    return checked.status === "checked"
      ? { status: "checked", result: checked.result }
      : { status: "invalid", errors: checked.errors };
  } catch (error) {
    return { status: "unavailable", reason: describe(error) };
  }
}

/** An answer is shown only for the very thing it was asked for (the same circuit object). */
export function answerForKey<T, K>(
  answer: { readonly key: K; readonly value: T } | null,
  key: K,
): T | null {
  return answer !== null && Object.is(answer.key, key) ? answer.value : null;
}

// --- the session -----------------------------------------------------------------------------

export interface StudentSession {
  /** The student's copy of the open assignment, or none. */
  readonly assignment: Assignment | null;
  /** The diagnosis being composed, in the order the student added it. */
  readonly entries: readonly DiagnosisEntry[];
  /** Changes with every edit of the diagnosis, so an answer for an older one is stale. */
  readonly entriesVersion: number;
}

/** Plain TypeScript with no React: React subscribes through `subscribe` and `getState`. */
export class StudentSessionStore {
  private state: StudentSession = {
    assignment: null,
    entries: [],
    entriesVersion: 0,
  };
  private readonly listeners = new Set<() => void>();

  readonly getState = (): StudentSession => this.state;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** Starts working on `assignment`: a new session with an empty diagnosis. */
  start(assignment: Assignment): void {
    this.publish({
      assignment: studentCopyOf(assignment),
      entries: [],
    });
  }

  /** Ends the session: no assignment, no restrictions, no faults, no diagnosis. */
  close(): void {
    this.publish({ assignment: null, entries: [] });
  }

  addEntry(entry: DiagnosisEntry): void {
    this.publish({ entries: [...this.state.entries, entry] });
  }

  replaceEntry(index: number, entry: DiagnosisEntry): void {
    this.publish({
      entries: this.state.entries.map((existing, position) =>
        position === index ? entry : existing,
      ),
    });
  }

  removeEntry(index: number): void {
    this.publish({
      entries: this.state.entries.filter((_, position) => position !== index),
    });
  }

  private publish(
    changes: Partial<Pick<StudentSession, "assignment" | "entries">>,
  ): void {
    this.state = {
      ...this.state,
      ...changes,
      entriesVersion: this.state.entriesVersion + 1,
    };
    for (const listener of [...this.listeners]) {
      listener();
    }
  }
}

/** What the student may see of a diagnosis result (contract section 5.7). */
export interface RevealedDiagnosis {
  readonly outcomeKey: string;
  /** Present only when the task's policy lets the student see the verdicts. */
  readonly entries: DiagnosisResult["entries"] | null;
  readonly faults: DiagnosisResult["faults"] | null;
}

/**
 * The Engine's answer carries the full detail; the task's `reveal` policy decides how much of it
 * the interface shows. `never` shows the outcome and nothing else, `afterCheck` the verdict of
 * each entry and each fault. It is a presentation policy of the local application, not a
 * protection (contract section 2.4).
 */
export function revealDiagnosis(
  result: DiagnosisResult,
  reveal: RevealPolicy,
): RevealedDiagnosis {
  return reveal === "afterCheck"
    ? {
        outcomeKey: result.outcomeKey,
        entries: result.entries,
        faults: result.faults,
      }
    : { outcomeKey: result.outcomeKey, entries: null, faults: null };
}
