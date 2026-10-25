import type { Schematic } from "../generated/Schematic";
import { dividerSchematic } from "./fixtures";
import type { SchematicDocumentStore } from "./document";
import type { ProjectFilePort } from "./projectFiles";

export type ProjectAction = "new" | "open" | "example";
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

  async perform(action: ProjectAction): Promise<boolean> {
    switch (action) {
      case "new":
        await this.files.clearPath();
        this.document.replaceDocument(blankSchematic);
        return true;
      case "example":
        await this.files.clearPath();
        this.document.replaceDocument(dividerSchematic);
        return true;
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
