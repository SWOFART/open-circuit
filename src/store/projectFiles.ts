import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Schematic } from "../generated/Schematic";

/** The UI works with document values; paths and filesystem access stay in Rust. */
/** What a native Open is asked to refuse. */
export interface OpenOptions {
  /**
   * An assignment is open: a project with DC rails or with documentation beyond designators is
   * refused by the native side before the project path changes, and designators alone are
   * dropped, because assignments support neither.
   */
  readonly assignmentOpen?: boolean;
}

export type ExampleEntry = readonly [
  id: string,
  title: string,
  description: string,
];

export interface ProjectFilePort {
  listExamples?(): Promise<readonly ExampleEntry[]>;
  readExample?(id: string, options?: OpenOptions): Promise<Schematic>;
  open(options?: OpenOptions): Promise<Schematic | null>;
  save(schematic: Schematic): Promise<boolean>;
  saveAs(schematic: Schematic): Promise<boolean>;
  clearPath(): Promise<void>;
}

async function invokeProject<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!isTauri()) {
    throw new Error("Project files are available in the desktop app only.");
  }

  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(error instanceof Error ? error.message : String(error), {
      cause: error,
    });
  }
}

/** Calls the native project commands exposed by `crates/app`. */
export const tauriProjectFiles: ProjectFilePort = {
  listExamples: () => invokeProject("list_examples"),
  readExample: (id, options) =>
    invokeProject("read_example", {
      id,
      assignmentOpen: options?.assignmentOpen ?? false,
    }),
  open: (options) =>
    invokeProject<Schematic | null>("open_project", {
      assignmentOpen: options?.assignmentOpen ?? false,
    }),
  save: (schematic) => invokeProject<boolean>("save_project", { schematic }),
  saveAs: (schematic) =>
    invokeProject<boolean>("save_project_as", { schematic }),
  clearPath: () => invokeProject<void>("clear_project_path"),
};

export function saveDrawing(
  format: "svg" | "png" | "pdf",
  data: unknown,
): Promise<boolean> {
  return invokeProject<boolean>("export_drawing", { format, data });
}

export function saveSheetDrawings(
  format: "svg" | "png",
  pages: readonly unknown[],
): Promise<string | null> {
  return invokeProject<string | null>("export_sheet_drawings", {
    format,
    pages,
  });
}
