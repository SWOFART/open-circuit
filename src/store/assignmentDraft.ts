import type { Assignment } from "../generated/Assignment";
import type { CheckCriterion } from "../generated/CheckCriterion";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { FaultSpec } from "../generated/FaultSpec";
import type { RevealPolicy } from "../generated/RevealPolicy";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";

// The teacher's work in progress. It is authoring state of its own: it is never part of the
// schematic document or of the editor, and it holds a snapshot of the circuits it was given,
// not a live reference to the document. Every rule (what is a valid check, fault, or
// restriction) belongs to the Engine, which validates the document built from this draft.

export type TaskKind = "build" | "findFault";

export interface AssignmentDraft {
  readonly id: StableId;
  readonly title: string;
  readonly instructions: string;
  readonly taskKind: TaskKind;
  /** Kept while the task is a `build` task, so switching back loses nothing. */
  readonly reveal: RevealPolicy;
  readonly startingCircuit: Schematic | null;
  readonly reference: Schematic | null;
  /** `null`: every catalog type. */
  readonly allowedComponentTypes: readonly ComponentTypeId[] | null;
  readonly lockedComponents: readonly StableId[];
  /** Used by a `build` task only. */
  readonly checks: readonly CheckCriterion[];
  /** Used by a `findFault` task only. */
  readonly faults: readonly FaultSpec[];
}

export interface AssignmentDraftState {
  readonly draft: AssignmentDraft;
  /** Changes with every edit; an answer that was asked for an older one is stale. */
  readonly version: number;
  /** The draft differs from the last file it was opened from or saved to. */
  readonly isDirty: boolean;
}

export type StableIdFactory = () => StableId;

export function createStableId(): StableId {
  if (
    typeof crypto === "undefined" ||
    typeof crypto.randomUUID !== "function"
  ) {
    throw new Error("This runtime cannot generate stable object IDs.");
  }

  return crypto.randomUUID();
}

export function emptyDraft(
  createId: StableIdFactory = createStableId,
): AssignmentDraft {
  return {
    id: createId(),
    title: "",
    instructions: "",
    taskKind: "build",
    reveal: "afterCheck",
    startingCircuit: null,
    reference: null,
    allowedComponentTypes: null,
    lockedComponents: [],
    checks: [],
    faults: [],
  };
}

/**
 * The document the Engine validates and the file stores (contract section 2.1): a `build`
 * task has `checks` and no `faults`, a `findFault` task the other way round. A draft that is
 * not complete yields a document the Engine refuses, with errors that say what is missing.
 */
export function draftToAssignment(draft: AssignmentDraft): Assignment {
  const findFault = draft.taskKind === "findFault";

  return {
    schemaVersion: 1,
    id: draft.id,
    title: draft.title,
    instructions: draft.instructions,
    task: findFault
      ? { kind: "findFault", reveal: draft.reveal }
      : { kind: "build" },
    ...(draft.startingCircuit === null
      ? {}
      : { startingCircuit: structuredClone(draft.startingCircuit) }),
    restrictions: {
      ...(draft.allowedComponentTypes === null
        ? {}
        : { allowedComponentTypes: [...draft.allowedComponentTypes] }),
      lockedComponents: [...draft.lockedComponents],
    },
    ...(findFault
      ? { faults: structuredClone([...draft.faults]) }
      : { checks: structuredClone([...draft.checks]) }),
    ...(draft.reference === null
      ? {}
      : { reference: structuredClone(draft.reference) }),
  };
}

export function assignmentToDraft(assignment: Assignment): AssignmentDraft {
  const findFault = assignment.task.kind === "findFault";

  return {
    id: assignment.id,
    title: assignment.title,
    instructions: assignment.instructions,
    taskKind: assignment.task.kind,
    reveal:
      assignment.task.kind === "findFault"
        ? assignment.task.reveal
        : "afterCheck",
    startingCircuit: assignment.startingCircuit
      ? structuredClone(assignment.startingCircuit)
      : null,
    reference: assignment.reference
      ? structuredClone(assignment.reference)
      : null,
    allowedComponentTypes:
      assignment.restrictions?.allowedComponentTypes == null
        ? null
        : [...assignment.restrictions.allowedComponentTypes],
    lockedComponents: [...(assignment.restrictions?.lockedComponents ?? [])],
    checks: findFault ? [] : structuredClone(assignment.checks ?? []),
    faults: findFault ? structuredClone(assignment.faults ?? []) : [],
  };
}

function signature(draft: AssignmentDraft): string {
  return JSON.stringify(draft);
}

/** Plain TypeScript with no React: React subscribes through `subscribe` and `getState`. */
export class AssignmentDraftStore {
  private state: AssignmentDraftState;
  private savedSignature: string;
  private readonly listeners = new Set<() => void>();

  constructor(
    initial: AssignmentDraft = emptyDraft(),
    private readonly createId: StableIdFactory = createStableId,
  ) {
    this.state = { draft: initial, version: 0, isDirty: false };
    this.savedSignature = signature(initial);
  }

  readonly getState = (): AssignmentDraftState => this.state;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** A fresh ID for a criterion or a fault. */
  newId(): StableId {
    return this.createId();
  }

  setTitle(title: string): void {
    this.edit({ title });
  }

  setInstructions(instructions: string): void {
    this.edit({ instructions });
  }

  setTaskKind(taskKind: TaskKind): void {
    this.edit({ taskKind });
  }

  setReveal(reveal: RevealPolicy): void {
    this.edit({ reveal });
  }

  /**
   * Takes a copy of `schematic` as the starting circuit. Locks on parts it does not have
   * are dropped, because a lock needs a part to lock; checks and faults stay, and the
   * Engine's validation reports the ones that no longer fit.
   */
  setStartingCircuit(schematic: Schematic | null): void {
    const copy = schematic === null ? null : structuredClone(schematic);
    const ids = new Set(copy?.components.map((component) => component.id));
    this.edit({
      startingCircuit: copy,
      lockedComponents: this.state.draft.lockedComponents.filter((id) =>
        ids.has(id),
      ),
    });
  }

  setReference(schematic: Schematic | null): void {
    this.edit({
      reference: schematic === null ? null : structuredClone(schematic),
    });
  }

  setComponentLocked(componentId: StableId, locked: boolean): void {
    const current = this.state.draft.lockedComponents;
    if (current.includes(componentId) === locked) return;

    this.edit({
      lockedComponents: locked
        ? [...current, componentId]
        : current.filter((id) => id !== componentId),
    });
  }

  /** `null` allows every type. */
  setAllowedComponentTypes(types: readonly ComponentTypeId[] | null): void {
    this.edit({
      allowedComponentTypes: types === null ? null : [...new Set(types)],
    });
  }

  addCheck(criterion: CheckCriterion): void {
    this.edit({ checks: [...this.state.draft.checks, criterion] });
  }

  replaceCheck(criterion: CheckCriterion): void {
    this.edit({
      checks: this.state.draft.checks.map((existing) =>
        existing.id === criterion.id ? criterion : existing,
      ),
    });
  }

  removeCheck(id: StableId): void {
    this.edit({
      checks: this.state.draft.checks.filter((existing) => existing.id !== id),
    });
  }

  addFault(fault: FaultSpec): void {
    this.edit({ faults: [...this.state.draft.faults, fault] });
  }

  replaceFault(fault: FaultSpec): void {
    this.edit({
      faults: this.state.draft.faults.map((existing) =>
        existing.id === fault.id ? fault : existing,
      ),
    });
  }

  removeFault(id: StableId): void {
    this.edit({
      faults: this.state.draft.faults.filter((existing) => existing.id !== id),
    });
  }

  /** Starts over with `draft`, which counts as saved (a file was opened). */
  replaceWith(draft: AssignmentDraft): void {
    this.savedSignature = signature(draft);
    this.publish(draft);
  }

  /**
   * `saved` is what the file now holds: the draft as it was when the save began. An edit made
   * while the save was in flight therefore keeps the draft dirty.
   */
  markSaved(saved: AssignmentDraft = this.state.draft): void {
    this.savedSignature = signature(saved);
    this.state = {
      ...this.state,
      isDirty: signature(this.state.draft) !== this.savedSignature,
    };
    this.notify();
  }

  reset(): void {
    this.replaceWith(emptyDraft(this.createId));
  }

  private edit(changes: Partial<AssignmentDraft>): void {
    this.publish({ ...this.state.draft, ...changes });
  }

  private publish(draft: AssignmentDraft): void {
    this.state = {
      draft,
      version: this.state.version + 1,
      isDirty: signature(draft) !== this.savedSignature,
    };
    this.notify();
  }

  private notify(): void {
    for (const listener of [...this.listeners]) {
      listener();
    }
  }
}
