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
  /** A parameter update or a recompile of an edit is waiting for the engine. */
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
  schematic?: Schematic,
): boolean {
  return (
    matchesRun(run, snapshot) &&
    (schematic === undefined || statesMatchSchematic(snapshot, schematic)) &&
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

/** Checks that the discrete state side table is complete and belongs to this schematic. */
function statesMatchSchematic(
  snapshot: ResultSnapshot,
  schematic: Schematic,
): boolean {
  const expected = new Map<string, "relay" | "fuse">();
  for (const component of schematic.components) {
    const typeId: string = component.typeId;
    if (typeId === "relay_spdt") expected.set(component.id, "relay");
    if (typeId === "fuse") expected.set(component.id, "fuse");
  }

  if (
    !Array.isArray(snapshot.states) ||
    snapshot.states.length !== expected.size
  ) {
    return false;
  }
  const stateIds: string[] = [];
  for (const state of snapshot.states) {
    const expectedKind = expected.get(state.componentId);
    if (!expectedKind || state.kind !== expectedKind) return false;
    if (
      state.kind === "relay" &&
      state.state !== "released" &&
      state.state !== "picked"
    ) {
      return false;
    }
    if (
      state.kind === "fuse" &&
      state.state !== "intact" &&
      state.state !== "tripped"
    ) {
      return false;
    }
    if (
      state.lastTransitionTimeSeconds !== undefined &&
      (!Number.isFinite(state.lastTransitionTimeSeconds) ||
        state.lastTransitionTimeSeconds < 0 ||
        state.lastTransitionTimeSeconds >
          snapshot.metadata.simulationTimeSeconds)
    ) {
      return false;
    }
    stateIds.push(state.componentId);
  }
  const expectedIds = [...expected.keys()].sort();
  return stateIds.every((id, index) => id === expectedIds[index]);
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
  schematic?: Schematic,
): boolean {
  return (
    lastApplied !== null &&
    matchesRun(run, snapshot) &&
    matchesRun(run, lastApplied) &&
    (schematic === undefined ||
      (statesMatchSchematic(snapshot, schematic) &&
        statesMatchSchematic(lastApplied, schematic))) &&
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

export interface ParameterUpdate {
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
 * applied over a newer one. Any other document change recompiles the whole
 * document; compiles run one at a time and only the newest document is
 * compiled next.
 */
export class SimulationController {
  private state: SimulationState;
  private readonly listeners = new Set<() => void>();
  private started = false;
  private disposed = false;
  private readonly pending = new Map<string, ParameterUpdate>();
  private drain: Promise<void> | null = null;
  private stopFrames: (() => void) | null = null;
  /** The newest document handed to the controller. */
  private syncedSchematic: Schematic | null = null;
  /** The newest document the engine was asked to adopt. */
  private engineSchematic: Schematic | null = null;
  private compiling: Promise<void> | null = null;
  /**
   * Queued updates come from `syncDocument`, so a failed one must bring the
   * engine back to the document. Updates requested directly through
   * `setParameter` are not tied to a document and are never resynced.
   */
  private documentBackedUpdates = false;

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
    // A document synced before start is newer than the one start was given.
    this.syncedSchematic ??= schematic;
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
    await this.requestCompile();
  }

  /**
   * Brings the engine to `schematic`, the current document. Pass `parameter`
   * when that parameter edit is the only change since the previous document:
   * it is then sent as a lightweight update. Any other change recompiles the
   * whole document. Resolves once the engine has answered.
   */
  syncDocument(
    schematic: Schematic,
    parameter?: ParameterUpdate,
  ): Promise<void> {
    if (this.disposed || schematic === this.syncedSchematic) {
      return Promise.resolve();
    }

    const previous = this.syncedSchematic;
    this.syncedSchematic = schematic;

    if (!this.started || this.state.catalog === null) {
      // start() compiles the newest synced document once the catalog arrives.
      return Promise.resolve();
    }

    if (
      parameter &&
      this.state.status === "ready" &&
      this.compiling === null &&
      this.engineSchematic === previous
    ) {
      this.engineSchematic = schematic;
      this.documentBackedUpdates = true;
      return this.setParameter(
        parameter.componentId,
        parameter.parameterId,
        parameter.value,
      );
    }

    return this.requestCompile();
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

  private requestCompile(): Promise<void> {
    // A recompile carries every parameter, so queued updates are obsolete.
    this.pending.clear();
    this.compiling ??= this.compileNewest().finally(() => {
      this.compiling = null;
    });
    return this.compiling;
  }

  private async compileNewest(): Promise<void> {
    while (
      !this.disposed &&
      this.syncedSchematic !== null &&
      this.syncedSchematic !== this.engineSchematic
    ) {
      // An update already in flight belongs to the run being replaced; let it
      // finish so its answer cannot land on the new run.
      if (this.drain) {
        await this.drain;
        continue;
      }

      const target = this.syncedSchematic;
      this.engineSchematic = target;
      if (!(await this.compileDocument(target))) {
        // The backend could not be reached; the next document change retries.
        this.engineSchematic = null;
        return;
      }
    }
  }

  /** Returns false when the engine could not be reached. */
  private async compileDocument(schematic: Schematic): Promise<boolean> {
    const documentRevision = this.createRevision();
    let result: CompileResult;

    if (this.state.status === "ready") {
      this.setState({ updating: true });
    }

    try {
      result = await this.adapter.compile({
        schematic,
        geometry: buildGeometryEnvelope(schematic, this.registry),
        documentRevision,
      });
    } catch (error) {
      this.stopFrameStream();
      this.markUnavailable(error);
      this.setState({ updating: false });
      return false;
    }

    if (this.disposed || schematic !== this.syncedSchematic) {
      // A newer document is waiting; the loop compiles it next.
      return true;
    }

    try {
      if (result.status === "failure") {
        // The document stays editable; only the result is withdrawn.
        this.stopFrameStream();
        this.setState({
          status: "failed",
          run: null,
          snapshot: null,
          diagnostics: result.diagnostics,
          error: null,
          updating: false,
        });
        return true;
      }

      const run: ActiveRun = {
        runId: result.runId,
        layout: result.layout,
        documentRevision,
      };

      if (
        !isLayoutCoherent(run.layout) ||
        !isSnapshotCurrent(run, null, result.initialSnapshot, schematic)
      ) {
        this.stopFrameStream();
        this.setState({
          status: "unavailable",
          error:
            "The engine returned a result that does not match the compiled run.",
          discardedFrames: this.state.discardedFrames + 1,
          updating: false,
        });
        return true;
      }

      this.stopFrameStream();
      this.setState({
        status: "ready",
        run,
        snapshot: result.initialSnapshot,
        diagnostics: result.diagnostics,
        error: null,
        updating: false,
      });
    } catch (error) {
      // A malformed payload must surface as an error, never as a crash.
      this.markUnavailable(
        new Error(
          `The engine returned an unreadable result: ${describeError(error)}`,
        ),
      );
      this.setState({ updating: false });
      return true;
    }

    await this.openFrameStream(result.runId);
    return true;
  }

  private stopFrameStream(): void {
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

    if (
      isSnapshotCurrent(
        this.state.run,
        this.state.snapshot,
        snapshot,
        this.engineSchematic ?? undefined,
      )
    ) {
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
    let resync = false;

    try {
      while (!this.disposed) {
        const next = this.pending.entries().next();

        if (next.done) {
          break;
        }

        const [key, update] = next.value;
        this.pending.delete(key);

        if (!(await this.applyUpdate(update)) && this.documentBackedUpdates) {
          // The engine does not have this edit, so a later update would only
          // produce a result that leaves it out. Drop the rest and resync.
          this.pending.clear();
          resync = true;
          break;
        }
      }
    } finally {
      this.drain = null;
      this.documentBackedUpdates = false;
    }

    // A compile already waiting for this drain picks the document up itself.
    if (resync && !this.disposed && this.compiling === null) {
      await this.requestCompile();
    }
  }

  /**
   * Sends one update. Returns false when the engine certainly or possibly does
   * not have it afterwards: it was rejected, failed, or could not be confirmed.
   */
  private async applyUpdate(update: ParameterUpdate): Promise<boolean> {
    const { run } = this.state;

    if (this.state.status !== "ready" || !run || this.compiling) {
      return true;
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
      this.engineMayDifferFromDocument();
      this.setState({
        updating: false,
        run,
        error: describeError(error),
      });
      return false;
    }

    if (this.disposed) {
      return true;
    }

    try {
      if (result.status === "failure") {
        // A rejected update leaves the previous valid state and result in place.
        this.engineMayDifferFromDocument();
        this.setState({
          updating: false,
          run,
          diagnostics: result.diagnostics,
          error: null,
        });
        return false;
      }

      const schematic = this.engineSchematic ?? undefined;
      if (
        isSnapshotSuperseded(
          sentRun,
          this.state.snapshot,
          result.snapshot,
          schematic,
        )
      ) {
        // A newer frame of this revision is already on screen, so the update is
        // confirmed; only the document has to follow.
        this.setState({
          updating: false,
          run: sentRun,
          diagnostics: result.diagnostics,
          error: null,
        });
        return true;
      }

      if (
        !isSnapshotCurrent(
          sentRun,
          this.state.snapshot,
          result.snapshot,
          schematic,
        )
      ) {
        // The engine may have applied the change, but its answer cannot be
        // trusted for this document, so the update is reported as unconfirmed.
        this.engineMayDifferFromDocument();
        this.setState({
          updating: false,
          run,
          error: UNCONFIRMED_UPDATE,
          discardedFrames: this.state.discardedFrames + 1,
        });
        return false;
      }

      this.setState({
        updating: false,
        run: sentRun,
        snapshot: result.snapshot,
        diagnostics: result.diagnostics,
        error: null,
      });
      return true;
    } catch (error) {
      // A malformed payload must surface as an error, never as a crash.
      this.engineMayDifferFromDocument();
      this.setState({
        updating: false,
        run,
        error: `The engine returned an unreadable result: ${describeError(error)}`,
      });
      return false;
    }
  }

  /**
   * An update did not certainly reach the engine, so the engine is resynced
   * with a full compile, and a later document sync never sends a lightweight
   * update on top of it.
   */
  private engineMayDifferFromDocument(): void {
    this.engineSchematic = null;
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
