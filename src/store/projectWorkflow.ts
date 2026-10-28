import type { Schematic } from "../generated/Schematic";
import { dividerSchematic } from "./fixtures";
import type { SchematicDocumentStore } from "./document";
import type { ProjectFilePort } from "./projectFiles";

/**
 * `circuit` replaces the document with a circuit of an assignment the teacher is editing;
 * `assignment` with the starting circuit of an assignment the student opens, which is read from
 * a file only after the unsaved-changes prompt.
 */
export type ProjectAction =
  "new" | "open" | "example" | "circuit" | "assignment";

/** Where a replacement circuit comes from: given, or read when the action runs (null: none). */
export type CircuitSource = Schematic | (() => Promise<Schematic | null>);
export type UnsavedChoice = "save" | "discard" | "cancel";

export const blankSchematic: Schematic = {
  schemaVersion: 1,
  components: [],
  wires: [],
  junctions: [],
  referenceMarkers: [],
};

/** Applies file operations only after Rust has returned a validated document. */
export class ProjectWorkflow {
  constructor(
    private readonly document: SchematicDocumentStore,
    private readonly files: ProjectFilePort,
  ) {}

  async save(saveAs = false): Promise<boolean> {
    const schematic = this.document.getState().schematic;
    const saved = saveAs
      ? await this.files.saveAs(schematic)
      : await this.files.save(schematic);
    if (!saved) return false;

    this.document.markSaved(schematic);
    return !this.document.getState().isDirty;
  }

  async perform(
    action: ProjectAction,
    source?: CircuitSource,
  ): Promise<boolean> {
    switch (action) {
      case "new":
        await this.files.clearPath();
        this.document.replaceDocument(blankSchematic);
        return true;
      case "example":
        await this.files.clearPath();
        this.document.replaceDocument(dividerSchematic);
        return true;
      case "circuit":
      case "assignment": {
        const circuit = typeof source === "function" ? await source() : source;
        if (circuit === undefined || circuit === null) return false;
        await this.files.clearPath();
        this.document.replaceDocument(circuit);
        return true;
      }
      case "open": {
        const schematic = await this.files.open();
        if (schematic === null) return false;
        this.document.replaceDocument(schematic);
        return true;
      }
    }
  }
}

/** Runs a deferred New/Open/Example/Close action after the dirty prompt choice. */
export async function resolveUnsavedAction(
  choice: UnsavedChoice,
  save: () => Promise<boolean>,
  proceed: () => Promise<boolean>,
): Promise<boolean> {
  if (choice === "cancel") return false;
  if (choice === "save" && !(await save())) return false;
  return proceed();
}
