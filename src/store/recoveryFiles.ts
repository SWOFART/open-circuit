import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Schematic } from "../generated/Schematic";

// The native side of autosave and crash recovery (crates/app, `recovery.rs`;
// docs/contracts/stage-16-editor-improvements.md section 9). The interface asks for three
// operations and shows the answers; paths and filesystem access stay in Rust.

/** A recovery file that was read and decoded. */
export interface RecoveryOffer {
  /** When it was written, in milliseconds since the Unix epoch. */
  readonly writtenAtMs: number;
  /** The project the document came from, or null for a document that was never saved. */
  readonly sourcePath: string | null;
  readonly appVersion: string;
  readonly schematic: Schematic;
}

/** What is in the recovery folder at start. */
export type RecoveryProbe =
  | { readonly kind: "none" }
  | ({ readonly kind: "offer" } & RecoveryOffer)
  /** A file that cannot be read or decoded: offered for deletion, with the reason. */
  | { readonly kind: "unreadable"; readonly reason: string };

export interface RecoveryPort {
  /** False when there is no native side (the web build): autosave is then off. */
  readonly available: boolean;
  /** Writes the recovery file; rejects with the reason when it cannot. */
  write(schematic: Schematic): Promise<void>;
  /** Removes files older than 30 days, then says what is left to offer. */
  probe(): Promise<RecoveryProbe>;
  /** Deletes the recovery file. */
  discard(): Promise<void>;
}

async function invokeRecovery<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(error instanceof Error ? error.message : String(error), {
      cause: error,
    });
  }
}

/** Calls the native recovery commands exposed by `crates/app`. */
export const tauriRecoveryFiles: RecoveryPort = {
  get available() {
    return isTauri();
  },
  write: (schematic) => invokeRecovery<void>("write_recovery", { schematic }),
  probe: () => invokeRecovery<RecoveryProbe>("probe_recovery"),
  discard: () => invokeRecovery<void>("discard_recovery"),
};

/** A recovery port for a build with no native side: nothing is kept and nothing is offered. */
export const noRecoveryFiles: RecoveryPort = {
  available: false,
  write: () => Promise.resolve(),
  probe: () => Promise.resolve({ kind: "none" }),
  discard: () => Promise.resolve(),
};
