import type { Schematic } from "../generated/Schematic";
import type { SchematicDocumentStore } from "./document";
import type { RecoveryPort } from "./recoveryFiles";

// Autosave (docs/contracts/stage-16-editor-improvements.md section 9): while the document is
// dirty, a write after 2 seconds without an edit and then at most every 30 seconds; never while
// a save or an open is running. A write that fails never interrupts editing: one notice, a retry
// at the next interval, and the notice goes when a write succeeds.

export const AUTOSAVE_IDLE_MS = 2_000;
export const AUTOSAVE_INTERVAL_MS = 30_000;
/** How long to wait before asking again while a save or an open is running. */
const BLOCKED_RETRY_MS = 1_000;

/** The clock and timers autosave uses, replaced in tests. */
export interface AutosaveClock {
  now(): number;
  setTimeout(callback: () => void, delayMs: number): unknown;
  clearTimeout(handle: unknown): void;
}

const SYSTEM_CLOCK: AutosaveClock = {
  now: () => Date.now(),
  setTimeout: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimeout: (handle) =>
    clearTimeout(handle as ReturnType<typeof setTimeout>),
};

export interface AutosaveOptions {
  /** False while autosave is off (an assignment is open, or the start-up offer is pending). */
  readonly enabled: () => boolean;
  /** True while a save or an open is running. */
  readonly blocked: () => boolean;
  /** Called with the notice to show, or null to clear it. */
  readonly onNotice: (notice: string | null) => void;
  readonly clock?: AutosaveClock;
}

function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Writes the dirty document to the recovery file. It watches the store, never changes it, and
 * never touches the project file: only `RecoveryPort` is written.
 */
export class Autosave {
  private readonly clock: AutosaveClock;
  private timer: unknown = null;
  private unsubscribe: (() => void) | null = null;
  // The document last seen, and the one last written (or null when nothing is on disk for it).
  private seen: Schematic | null = null;
  private written: Schematic | null = null;
  private lastEditAt = 0;
  private lastWriteAt = Number.NEGATIVE_INFINITY;
  private inFlight: Promise<void> | null = null;

  constructor(
    private readonly store: SchematicDocumentStore,
    private readonly port: Pick<RecoveryPort, "write" | "discard">,
    private readonly options: AutosaveOptions,
  ) {
    this.clock = options.clock ?? SYSTEM_CLOCK;
  }

  /** Starts watching the store; the returned function stops it. */
  start(): () => void {
    this.seen = this.store.getState().schematic;
    this.unsubscribe = this.store.subscribe(this.onStoreChange);
    this.evaluate();
    return () => this.stop();
  }

  stop(): void {
    this.unsubscribe?.();
    this.unsubscribe = null;
    this.cancelTimer();
  }

  /**
   * Says that something outside autosave changed what may be written (it was switched on, or a
   * save or an open finished), so the next write can be planned.
   */
  poke(): void {
    this.evaluate();
  }

  /**
   * Deletes the recovery file after the write in flight, if any, has finished, so a late write
   * cannot bring back the file of a document that was just saved or discarded.
   */
  async discard(): Promise<void> {
    this.cancelTimer();
    await this.inFlight;
    this.written = null;
    await this.port.discard();
  }

  private readonly onStoreChange = (): void => {
    const schematic = this.store.getState().schematic;
    if (schematic !== this.seen) {
      this.seen = schematic;
      this.lastEditAt = this.clock.now();
    }
    this.evaluate();
  };

  private cancelTimer(): void {
    if (this.timer !== null) {
      this.clock.clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private evaluate(): void {
    this.cancelTimer();
    const state = this.store.getState();
    if (
      !this.options.enabled() ||
      !state.isDirty ||
      state.schematic === this.written
    ) {
      return;
    }

    const now = this.clock.now();
    const due = Math.max(
      this.lastEditAt + AUTOSAVE_IDLE_MS,
      this.lastWriteAt + AUTOSAVE_INTERVAL_MS,
    );
    if (this.inFlight || this.options.blocked()) {
      this.timer = this.clock.setTimeout(
        () => this.evaluate(),
        Math.max(due - now, BLOCKED_RETRY_MS),
      );
    } else if (now >= due) {
      this.write(state.schematic);
    } else {
      this.timer = this.clock.setTimeout(() => this.evaluate(), due - now);
    }
  }

  private write(schematic: Schematic): void {
    const attempt = this.port
      .write(schematic)
      .then(() => {
        this.written = schematic;
        this.options.onNotice(null);
      })
      .catch((error: unknown) => {
        this.options.onNotice(`Autosave is not working: ${reasonOf(error)}`);
      })
      .then(() => {
        this.lastWriteAt = this.clock.now();
        this.inFlight = null;
        this.evaluate();
      });
    this.inFlight = attempt;
  }
}
