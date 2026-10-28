import { buildGeometryEnvelope } from "../editor/geometry";
import type { AssignmentError } from "../generated/AssignmentError";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CheckResult } from "../generated/CheckResult";
import type { Schematic } from "../generated/Schematic";
import { symbols, type SymbolRegistry } from "../symbols";
import { checkResultBelongsTo, type AssignmentPort } from "./assignmentBridge";
import {
  assignmentToDraft,
  draftToAssignment,
  type AssignmentDraftStore,
} from "./assignmentDraft";

export const emptySchematic: Schematic = {
  schemaVersion: 1,
  components: [],
  wires: [],
  junctions: [],
  referenceMarkers: [],
};

export type OpenOutcome =
  | { readonly status: "cancelled" }
  | { readonly status: "opened" }
  | { readonly status: "invalid"; readonly errors: AssignmentError[] };

export type SaveOutcome =
  | { readonly status: "saved" | "cancelled" }
  | { readonly status: "invalid"; readonly errors: AssignmentError[] };

export type ValidationOutcome =
  | { readonly status: "valid" }
  /** The draft changed while the Engine was working, so its answer was dropped. */
  | { readonly status: "stale" }
  | { readonly status: "invalid"; readonly errors: AssignmentError[] };

export type PreviewOutcome =
  | { readonly status: "result"; readonly result: CheckResult }
  /** The draft changed while the Engine was working, so its answer was dropped. */
  | { readonly status: "stale" }
  | { readonly status: "notBuildTask" }
  | { readonly status: "needsReference" }
  | { readonly status: "unavailable"; readonly reason: string };

let revisionCounter = 0;

function createPreviewRevision(): string {
  revisionCounter += 1;
  return `preview-${revisionCounter}-${Math.random().toString(36).slice(2, 8)}`;
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * The teacher's file and preview operations. Every rule is the Engine's: the draft is turned
 * into a document and sent to the bridge, and what comes back is shown as it is. A refused
 * file or document never changes the draft, so the teacher's work is never lost.
 */
export class AssignmentAuthoring {
  constructor(
    private readonly draft: AssignmentDraftStore,
    private readonly port: AssignmentPort,
    private readonly registry: SymbolRegistry = symbols,
    private readonly createRevision: () => string = createPreviewRevision,
  ) {}

  /** Starts a new assignment: forgets the file path and empties the draft. */
  async newAssignment(): Promise<void> {
    await this.port.clearPath();
    this.draft.reset();
  }

  /** The draft is replaced only by a file the Engine accepted. */
  async open(): Promise<OpenOutcome> {
    const decoded = await this.port.open();

    if (decoded === null) return { status: "cancelled" };
    if (decoded.status === "invalid") {
      return { status: "invalid", errors: decoded.errors };
    }

    this.draft.replaceWith(assignmentToDraft(decoded.assignment));
    return { status: "opened" };
  }

  async save(saveAs = false): Promise<SaveOutcome> {
    const saved = this.draft.getState().draft;
    const document = draftToAssignment(saved);
    const outcome = saveAs
      ? await this.port.saveAs(document)
      : await this.port.save(document);

    if (outcome.status === "saved") {
      this.draft.markSaved(saved);
    }

    return outcome;
  }

  /** Writes the student's copy (without the reference); the draft stays as it is. */
  exportStudentCopy(): Promise<SaveOutcome> {
    return this.port.exportStudentCopy(
      draftToAssignment(this.draft.getState().draft),
    );
  }

  async validate(): Promise<ValidationOutcome> {
    const { draft, version } = this.draft.getState();
    const decoded = await this.port.validate(draftToAssignment(draft));

    // The errors of an older draft would point at fields that have since changed.
    if (this.draft.getState().version !== version) {
      return { status: "stale" };
    }

    return decoded.status === "valid"
      ? { status: "valid" }
      : { status: "invalid", errors: decoded.errors };
  }

  /**
   * Evaluates the checks on the reference solution, on the Engine, and returns the Engine's
   * structured result. An answer for a draft that has changed meanwhile is dropped.
   */
  async previewChecks(): Promise<PreviewOutcome> {
    const { draft, version } = this.draft.getState();

    if (draft.taskKind !== "build") return { status: "notBuildTask" };
    if (draft.reference === null) return { status: "needsReference" };

    const revision = this.createRevision();
    let result: CheckResult;

    try {
      const request: CheckRequest = {
        checks: [...draft.checks],
        lockedComponents: [...draft.lockedComponents],
        startingCircuit: draft.startingCircuit ?? emptySchematic,
        schematic: draft.reference,
        geometry: buildGeometryEnvelope(draft.reference, this.registry),
        documentRevision: revision,
      };
      result = await this.port.checkAssignment(request);
    } catch (error) {
      return { status: "unavailable", reason: describe(error) };
    }

    if (
      !checkResultBelongsTo(result, revision) ||
      this.draft.getState().version !== version
    ) {
      return { status: "stale" };
    }

    return { status: "result", result };
  }
}
