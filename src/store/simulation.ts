import { buildGeometryEnvelope } from "../editor/geometry";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ParameterId } from "../generated/ParameterId";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultQuantity } from "../generated/ResultQuantity";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { Schematic } from "../generated/Schematic";
import type { SetParameterResult } from "../generated/SetParameterResult";
import type { StableId } from "../generated/StableId";
import { symbols, type SymbolRegistry } from "../symbols";
import type { EngineAdapter } from "./engineAdapter";

/**
 * - `idle`: not started.
 * - `loading`: waiting for the catalog or the first compile.
 * - `ready`: a valid result is available (warnings may accompany it).
 * - `failed`: the engine rejected the circuit; there is no usable result.
 * - `unavailable`: the backend could not be reached or answered incoherently.
 */
export type SimulationStatus =
  "idle" | "loading" | "ready" | "failed" | "unavailable";

/** What an incoming frame is checked against. */
export interface ActiveRun {
  readonly runId: StableId;
  readonly layout: ResultLayout;
  readonly documentRevision: string;
}

export interface SimulationState {
  readonly status: SimulationStatus;
  readonly catalog: ComponentCatalog | null;
  readonly run: ActiveRun | null;
  readonly snapshot: ResultSnapshot | null;
  readonly diagnostics: readonly Diagnostic[];
  /** A transport failure, as opposed to engine diagnostics. */
  readonly error: string | null;
  /** A parameter update is waiting for the engine. */
  readonly updating: boolean;
  /** Frames rejected by `isSnapshotCurrent`. */
  readonly discardedFrames: number;
}

export function initialSimulationState(): SimulationState {
  return {
    status: "idle",
    catalog: null,
    run: null,
    snapshot: null,
    diagnostics: [],
    error: null,
    updating: false,
    discardedFrames: 0,
  };
}

/**
 * The frame acceptance rule from docs/contracts/stage-1.md section 5: the run
 * ID, document revision, and layout version must match the active ones, the
 * payload must fit the layout and be finite, and the sequence must be newer
 * than the last applied frame. Anything else is stale and must be discarded.
 */
export function isSnapshotCurrent(
  run: ActiveRun | null,
  lastApplied: ResultSnapshot | null,
  snapshot: ResultSnapshot,
): boolean {
  return (
    matchesRun(run, snapshot) &&
    (lastApplied === null ||
      snapshot.metadata.frameSequence > lastApplied.metadata.frameSequence)
  );
}

/** Identity, layout, shape, and finiteness: everything but the sequence rule. */
function matchesRun(run: ActiveRun | null, snapshot: ResultSnapshot): boolean {
  if (!run) {
    return false;
  }

  const { metadata, values } = snapshot;

  return (
    metadata.runId === run.runId &&
    metadata.documentRevision === run.documentRevision &&
    metadata.layoutVersion === run.layout.layoutVersion &&
    values.length === run.layout.valueCount &&
    values.every((value) => Number.isFinite(value)) &&
    Number.isFinite(metadata.simulationTimeSeconds)
  );
}

/**
 * A reply that arrived after a newer frame of the same revision was already
 * applied. Frames and invoke replies travel separately, so a frame produced right
 * after an update can overtake the reply to that update. The reply is then
 * confirmed by the newer frame instead of being stale.
 */
export function isSnapshotSuperseded(
  run: ActiveRun | null,
  lastApplied: ResultSnapshot | null,
  snapshot: ResultSnapshot,
): boolean {
  return (
    lastApplied !== null &&
    matchesRun(run, snapshot) &&
    matchesRun(run, lastApplied) &&
    lastApplied.metadata.frameSequence >= snapshot.metadata.frameSequence
  );
}

/**
 * Contract section 5: `valueCount` is the number of slots, and the offsets are
 * unique and fill 0 to `valueCount - 1`. A layout that breaks this cannot be
 * used to read a snapshot safely.
 */
export function isLayoutCoherent(layout: ResultLayout): boolean {
  const offsets = new Set(layout.slots.map((slot) => slot.offset));

  return (
    layout.slots.length === layout.valueCount &&
    offsets.size === layout.valueCount &&
    layout.slots.every(
      (slot) =>
        Number.isInteger(slot.offset) &&
        slot.offset >= 0 &&
        slot.offset < layout.valueCount,
    )
  );
}

/** Reads one value through the layout, never by assuming a slot position. */
export function readSlot(
  run: ActiveRun | null,
  snapshot: ResultSnapshot | null,
  componentId: StableId,
  quantity: ResultQuantity,
): number | undefined {
  if (!run || !snapshot) {
    return undefined;
  }

  const slot = run.layout.slots.find(
    (candidate) =>
      candidate.componentId === componentId && candidate.quantity === quantity,
  );

  return slot ? snapshot.values[slot.offset] : undefined;
}

let revisionCounter = 0;

/** An opaque token; the engine only compares revisions for equality. */
function createRevisionToken(): string {
  revisionCounter += 1;
  return `rev-${revisionCounter}-${Math.random().toString(36).slice(2, 8)}`;
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const UNCONFIRMED_UPDATE =
  "The engine's answer did not match the current document and was ignored, so the last update is unconfirmed.";

interface ParameterUpdate {
  readonly componentId: StableId;
  readonly parameterId: ParameterId;
  readonly value: number;
}

/**
 * Owns the active run and the latest result. Plain TypeScript
 * with no React or Pixi: React subscribes through `subscribe`/`getState`.
 *
 * Parameter edits are sent one at a time and coalesced, so the engine's state
 * always corresponds to the newest response and an older one can never be
 * applied over a newer one.
 */
export class SimulationController {
  private state: SimulationState;
  private readonly listeners = new Set<() => void>();
  private started = false;
  private disposed = false;
  private readonly pending = new Map<string, ParameterUpdate>();
  private drain: Promise<void> | null = null;
  private stopFrames: (() => void) | null = null;

  constructor(
    private readonly adapter: EngineAdapter,
    private readonly registry: SymbolRegistry = symbols,
    private readonly createRevision: () => string = createRevisionToken,
  ) {
    this.state = initialSimulationState();
  }

  readonly getState = (): SimulationState => this.state;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** Loads the catalog and compiles the document. Safe to call once. */
  async start(schematic: Schematic): Promise<void> {
    if (this.started || this.disposed) {
      return;
    }

    this.started = true;
    this.setState({ status: "loading", error: null });

    let catalog: ComponentCatalog;

    try {
      catalog = await this.adapter.getCatalog();
    } catch (error) {
      this.markUnavailable(error);
      return;
    }

    if (this.disposed) {
      return;
    }

    this.setState({ catalog });

    const documentRevision = this.createRevision();
    let result: CompileResult;

    try {
      result = await this.adapter.compile({
        schematic,
        geometry: buildGeometryEnvelope(schematic, this.registry),
        documentRevision,
      });
    } catch (error) {
      this.markUnavailable(error);
      return;
    }

    if (this.disposed) {
      return;
    }

    try {
      if (result.status === "failure") {
        this.setState({
          status: "failed",
          diagnostics: result.diagnostics,
          error: null,
        });
        return;
      }

      const run: ActiveRun = {
        runId: result.runId,
        layout: result.layout,
        documentRevision,
      };

      if (
        !isLayoutCoherent(run.layout) ||
        !isSnapshotCurrent(run, null, result.initialSnapshot)
      ) {
        this.setState({
          status: "unavailable",
          error:
            "The engine returned a result that does not match the compiled run.",
          discardedFrames: this.state.discardedFrames + 1,
        });
        return;
      }

      this.setState({
        status: "ready",
        run,
        snapshot: result.initialSnapshot,
        diagnostics: result.diagnostics,
        error: null,
      });
    } catch (error) {
      // A malformed payload must surface as an error, never as a crash.
      this.markUnavailable(
        new Error(
          `The engine returned an unreadable result: ${describeError(error)}`,
        ),
      );
      return;
    }

    await this.openFrameStream(result.runId);
  }

  /**
   * Requests a parameter change in SI units. Resolves once this edit, and any
   * edit made while it was in flight, has been answered.
   */
  setParameter(
    componentId: StableId,
    parameterId: ParameterId,
    value: number,
  ): Promise<void> {
    if (this.disposed) {
      return Promise.resolve();
    }

    // A newer edit of the same parameter replaces the older one; edits of
    // different parameters are all kept, in the order they were last requested.
    const key = `${componentId}:${parameterId}`;
    this.pending.delete(key);
    this.pending.set(key, { componentId, parameterId, value });
    this.drain ??= this.drainUpdates();
    return this.drain;
  }

  dispose(): void {
    this.disposed = true;
    this.pending.clear();
    this.listeners.clear();
    this.stopFrames?.();
    this.stopFrames = null;
  }

  /**
   * A live frame from the backend. It is applied only if it passes the contract's
   * acceptance rule. Stale frames (an older revision, another run, a sequence that
   * is not newer) are expected during edits and are dropped without an error.
   */
  private readonly handleFrame = (snapshot: ResultSnapshot): void => {
    if (this.disposed || this.state.status !== "ready") {
      return;
    }

    if (isSnapshotCurrent(this.state.run, this.state.snapshot, snapshot)) {
      this.setState({ snapshot });
    } else {
      this.setState({ discardedFrames: this.state.discardedFrames + 1 });
    }
  };

  private readonly handleFrameError = (error: Error): void => {
    if (this.disposed) {
      return;
    }

    this.setState({
      discardedFrames: this.state.discardedFrames + 1,
      error: `A frame from the engine was unusable: ${error.message}`,
    });
  };

  private async openFrameStream(runId: StableId): Promise<void> {
    try {
      const stop = await this.adapter.startFrames(runId, {
        onFrame: this.handleFrame,
        onError: this.handleFrameError,
      });

      if (this.disposed) {
        stop();
      } else {
        this.stopFrames = stop;
      }
    } catch (error) {
      // The results returned by compile and setParameter stay valid; only the live
      // updates are missing, and the user is told so.
      if (!this.disposed) {
        this.setState({
          error: `Live updates are unavailable: ${describeError(error)}`,
        });
      }
    }
  }

  private async drainUpdates(): Promise<void> {
    try {
      while (!this.disposed) {
        const next = this.pending.entries().next();

        if (next.done) {
          break;
        }

        const [key, update] = next.value;
        this.pending.delete(key);
        await this.applyUpdate(update);
      }
    } finally {
      this.drain = null;
    }
  }

  private async applyUpdate(update: ParameterUpdate): Promise<void> {
    const { run } = this.state;

    if (this.state.status !== "ready" || !run) {
      return;
    }

    const revision = this.createRevision();
    const sentRun: ActiveRun = { ...run, documentRevision: revision };
    let result: SetParameterResult;

    this.setState({ updating: true, run: sentRun });

    try {
      result = await this.adapter.setParameter({
        runId: run.runId,
        // The engine checks this against its active snapshot, then adopts the
        // new revision; both are compared by equality only.
        expectedDocumentRevision: run.documentRevision,
        documentRevision: revision,
        componentId: update.componentId,
        parameterId: update.parameterId,
        value: update.value,
      });
    } catch (error) {
      this.setState({
        updating: false,
        run,
        error: describeError(error),
      });
      return;
    }

    if (this.disposed) {
      return;
    }

    try {
      if (result.status === "failure") {
        // A rejected update leaves the previous valid state and result in place.
        this.setState({
          updating: false,
          run,
          diagnostics: result.diagnostics,
          error: null,
        });
        return;
      }

      if (isSnapshotSuperseded(sentRun, this.state.snapshot, result.snapshot)) {
        // A newer frame of this revision is already on screen, so the update is
        // confirmed; only the document has to follow.
        this.setState({
          updating: false,
          run: sentRun,
          diagnostics: result.diagnostics,
          error: null,
        });
        return;
      }

      if (!isSnapshotCurrent(sentRun, this.state.snapshot, result.snapshot)) {
        // The engine may have applied the change, but its answer cannot be
        // trusted for this document, so the update is reported as unconfirmed.
        this.setState({
          updating: false,
          run,
          error: UNCONFIRMED_UPDATE,
          discardedFrames: this.state.discardedFrames + 1,
        });
        return;
      }

      this.setState({
        updating: false,
        run: sentRun,
        snapshot: result.snapshot,
        diagnostics: result.diagnostics,
        error: null,
      });
    } catch (error) {
      // A malformed payload must surface as an error, never as a crash.
      this.setState({
        updating: false,
        run,
        error: `The engine returned an unreadable result: ${describeError(error)}`,
      });
    }
  }

  private markUnavailable(error: unknown): void {
    if (this.disposed) {
      return;
    }

    this.setState({ status: "unavailable", error: describeError(error) });
  }

  private setState(patch: Partial<SimulationState>): void {
    this.state = { ...this.state, ...patch };

    for (const listener of [...this.listeners]) {
      listener();
    }
  }
}
