import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Schematic } from "../generated/Schematic";

/** The UI works with document values; paths and filesystem access stay in Rust. */
export interface ProjectFilePort {
  open(): Promise<Schematic | null>;
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
  open: () => invokeProject<Schematic | null>("open_project"),
  save: (schematic) => invokeProject<boolean>("save_project", { schematic }),
  saveAs: (schematic) =>
    invokeProject<boolean>("save_project_as", { schematic }),
  clearPath: () => invokeProject<void>("clear_project_path"),
};
