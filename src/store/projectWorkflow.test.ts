import { describe, expect, it, vi } from "vitest";
import type { Schematic } from "../generated/Schematic";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";
import {
  blankSchematic,
  ProjectWorkflow,
  resolveUnsavedAction,
} from "./projectWorkflow";

const R1 = "00000000-0000-4000-8000-000000000002";

function createFiles() {
  return {
    open: vi.fn<() => Promise<Schematic | null>>().mockResolvedValue(null),
    save: vi.fn<(_: Schematic) => Promise<boolean>>().mockResolvedValue(true),
    saveAs: vi.fn<(_: Schematic) => Promise<boolean>>().mockResolvedValue(true),
    clearPath: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
  };
}

describe("ProjectWorkflow", () => {
  it("creates a clean blank document after clearing the current project path", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const workflow = new ProjectWorkflow(store, files);

    await expect(workflow.perform("new")).resolves.toBe(true);

    expect(files.clearPath).toHaveBeenCalledOnce();
    expect(store.getState()).toMatchObject({
      schematic: blankSchematic,
      canUndo: false,
      canRedo: false,
      isDirty: false,
    });
  });

  it("keeps the current document when opening is cancelled or fails", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const before = store.getState();
    const workflow = new ProjectWorkflow(store, files);

    await expect(workflow.perform("open")).resolves.toBe(false);
    expect(store.getState()).toBe(before);

    files.open.mockRejectedValueOnce(new Error("Invalid project file."));
    await expect(workflow.perform("open")).rejects.toThrow(
      "Invalid project file",
    );
    expect(store.getState()).toBe(before);
  });

  it("replaces the document only after a validated file is returned", async () => {
    const opened = {
      ...dividerSchematic,
      wires: [],
    } satisfies Schematic;
    const files = createFiles();
    files.open.mockResolvedValue(opened);
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const workflow = new ProjectWorkflow(store, files);

    await expect(workflow.perform("open")).resolves.toBe(true);

    expect(store.getState().schematic).toEqual(opened);
    expect(store.getState().isDirty).toBe(false);
    expect(store.getState().canUndo).toBe(false);
  });

  it("marks a document saved only when the file operation succeeds", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    const workflow = new ProjectWorkflow(store, files);
    store.moveComponent(R1, { x: 40, y: 40 });

    files.save.mockResolvedValueOnce(false);
    await expect(workflow.save()).resolves.toBe(false);
    expect(store.getState().isDirty).toBe(true);

    await expect(workflow.save()).resolves.toBe(true);
    expect(store.getState().isDirty).toBe(false);
  });

  it("keeps edits made during an async save dirty and cancels a pending action", async () => {
    const files = createFiles();
    let finishSaving: (saved: boolean) => void = () => {
      throw new Error("The save should have started before it is completed.");
    };
    files.save.mockReturnValueOnce(
      new Promise<boolean>((resolve) => {
        finishSaving = resolve;
      }),
    );
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const workflow = new ProjectWorkflow(store, files);
    const savedSnapshot = store.getState().schematic;
    const proceed = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);

    const pendingAction = resolveUnsavedAction(
      "save",
      () => workflow.save(),
      proceed,
    );
    store.moveComponent(R1, { x: 50, y: 50 });
    finishSaving(true);

    await expect(pendingAction).resolves.toBe(false);
    expect(files.save).toHaveBeenCalledWith(savedSnapshot);
    expect(store.getState().isDirty).toBe(true);
    expect(proceed).not.toHaveBeenCalled();
  });

  it("uses Save As and marks the document clean after it succeeds", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const workflow = new ProjectWorkflow(store, files);

    await expect(workflow.save(true)).resolves.toBe(true);

    expect(files.saveAs).toHaveBeenCalledOnce();
    expect(files.save).not.toHaveBeenCalled();
    expect(store.getState().isDirty).toBe(false);
  });

  it("clears the project path before opening the built-in example", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(blankSchematic);
    store.addJunction({ x: 1, y: 2 });
    const workflow = new ProjectWorkflow(store, files);

    await expect(workflow.perform("example")).resolves.toBe(true);

    expect(files.clearPath).toHaveBeenCalledOnce();
    expect(store.getState().schematic).toEqual(dividerSchematic);
    expect(store.getState().isDirty).toBe(false);
    expect(store.getState().canUndo).toBe(false);
  });
});

describe("unsaved-change decision", () => {
  it("does not save or proceed when the user cancels", async () => {
    const save = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);
    const proceed = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);

    await expect(resolveUnsavedAction("cancel", save, proceed)).resolves.toBe(
      false,
    );
    expect(save).not.toHaveBeenCalled();
    expect(proceed).not.toHaveBeenCalled();
  });

  it("does not proceed when Save is cancelled or fails", async () => {
    const save = vi.fn<() => Promise<boolean>>().mockResolvedValue(false);
    const proceed = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);

    await expect(resolveUnsavedAction("save", save, proceed)).resolves.toBe(
      false,
    );
    expect(proceed).not.toHaveBeenCalled();
  });

  it("proceeds without saving after Discard", async () => {
    const save = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);
    const proceed = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);

    await expect(resolveUnsavedAction("discard", save, proceed)).resolves.toBe(
      true,
    );
    expect(save).not.toHaveBeenCalled();
    expect(proceed).toHaveBeenCalledOnce();
  });
});

describe("ProjectWorkflow with a circuit of an assignment", () => {
  it("replaces the document with the circuit after clearing the project path", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const workflow = new ProjectWorkflow(store, files);

    await expect(workflow.perform("circuit", blankSchematic)).resolves.toBe(
      true,
    );

    expect(files.clearPath).toHaveBeenCalledOnce();
    expect(store.getState()).toMatchObject({
      schematic: blankSchematic,
      canUndo: false,
      isDirty: false,
    });
  });

  it("leaves the document alone when there is no circuit to show", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const before = store.getState();
    const workflow = new ProjectWorkflow(store, files);

    await expect(workflow.perform("circuit")).resolves.toBe(false);

    expect(store.getState()).toBe(before);
    expect(files.clearPath).not.toHaveBeenCalled();
  });
});

describe("ProjectWorkflow with a student's assignment", () => {
  it("reads the starting circuit only when the action runs, and replaces the document with it", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const workflow = new ProjectWorkflow(store, files);
    const load = vi
      .fn<() => Promise<Schematic | null>>()
      .mockResolvedValue(blankSchematic);

    expect(load).not.toHaveBeenCalled();
    await expect(workflow.perform("assignment", load)).resolves.toBe(true);

    expect(load).toHaveBeenCalledOnce();
    expect(files.clearPath).toHaveBeenCalledOnce();
    expect(store.getState()).toMatchObject({
      schematic: blankSchematic,
      canUndo: false,
      isDirty: false,
    });
  });

  it("keeps the document and the project path when the file is cancelled or refused", async () => {
    const files = createFiles();
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 40, y: 40 });
    const before = store.getState();
    const workflow = new ProjectWorkflow(store, files);

    await expect(
      workflow.perform("assignment", () => Promise.resolve(null)),
    ).resolves.toBe(false);
    expect(store.getState()).toBe(before);
    expect(files.clearPath).not.toHaveBeenCalled();

    await expect(
      workflow.perform("assignment", () =>
        Promise.reject(new Error("Could not read assignment file.")),
      ),
    ).rejects.toThrow("Could not read assignment file.");
    expect(store.getState()).toBe(before);
    expect(files.clearPath).not.toHaveBeenCalled();
  });
});
