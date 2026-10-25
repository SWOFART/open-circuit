import type { StableId } from "../generated/StableId";
import type { TraceBatch } from "../generated/TraceBatch";
import type { TraceRequest } from "../generated/TraceRequest";
import type { Unit } from "../generated/Unit";
import type { EngineAdapter } from "./engineAdapter";
import type { ActiveRun, SimulationState } from "./simulation";

/** The most signals one request may name (docs/contracts/stage-3-dynamics.md section 7.4). */
export const MAX_SCOPE_SIGNALS = 8;
/** The application's cap on points per signal in one response (stage-1.md section 8.3). */
export const MAX_SCOPE_POINTS = 2048;
/** The public step, used only before the first response names the sample interval. */
const DEFAULT_SAMPLE_INTERVAL_SECONDS = 0.001;
/** The visible time windows offered. The longest is the most history the Engine keeps. */
export const SCOPE_WINDOWS_SECONDS: readonly number[] = [
  0.01, 0.05, 0.1, 0.5, 1, 5, 10,
];
export const DEFAULT_SCOPE_WINDOW_SECONDS = 0.1;
/** Where a request starts relative to the newest step known, so a response is not short. */
const WINDOW_SLACK_STEPS = 64;

export interface ScopeSignalData {
  /** The layout slot key, the same one the selection uses. */
  readonly key: string;
  readonly unit: Unit;
  /** Seconds from the start of the run, strictly increasing. */
  readonly times: readonly number[];
  /** Per point: the smallest value of the samples it stands for. */
  readonly minimums: readonly number[];
  /** Per point: the largest value of the samples it stands for. */
  readonly maximums: readonly number[];
}

/** One response, kept only for the run and layout it was produced for. */
export interface ScopeData {
  readonly runId: StableId;
  readonly layoutVersion: number;
  readonly documentRevision: string;
  readonly sampleIntervalSeconds: number;
  /** Time of the newest stored sample, or null while the run has stored none. */
  readonly latestTimeSeconds: number | null;
  /** Samples each point stands for: 1 means every point is one exact sample. */
  readonly samplesPerPoint: number;
  readonly signals: readonly ScopeSignalData[];
}

export interface ScopeState {
  /** Slot keys of the traced signals, in selection order, all present in the layout. */
  readonly selectedKeys: readonly string[];
  readonly windowSeconds: number;
  /** The newest trace of the current run, or null: never one from another run. */
  readonly data: ScopeData | null;
  /** The last trace read failed for the current run. */
  readonly error: string | null;
  /** A trace read is waiting for the engine. */
  readonly loading: boolean;
}

/** What the controller needs of the simulation store: the run and a way to follow it. */
export interface SimulationSource {
  getState(): SimulationState;
  subscribe(listener: () => void): () => void;
}

export interface ScopeOptions {
  /** Time between reads while the run advances. */
  readonly pollIntervalMs?: number;
  /** Time between reads while nothing new arrives, or after a failure. */
  readonly idleIntervalMs?: number;
}

export function initialScopeState(): ScopeState {
  return {
    selectedKeys: [],
    windowSeconds: DEFAULT_SCOPE_WINDOW_SECONDS,
    data: null,
    error: null,
    loading: false,
  };
}

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function toNumber(step: bigint): number {
  if (step > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error("The engine sent a simulation step that is too large.");
  }

  return Number(step);
}

function sameRun(
  run: ActiveRun | null,
  runId: StableId,
  layoutVersion: number,
): boolean {
  return (
    run !== null &&
    run.runId === runId &&
    run.layout.layoutVersion === layoutVersion
  );
}

/**
 * Turns one Engine response into display data. It checks that the response answers
 * the request that was sent, and calculates nothing physical: a point's time is its
 * step times the sample interval the Engine reported. Returns null for a response
 * that belongs to another run, layout, or selection.
 */
export function scopeDataFromBatch(
  request: TraceRequest,
  batch: TraceBatch,
): ScopeData | null {
  const { metadata, signals, points } = batch;

  if (
    metadata.runId !== request.runId ||
    metadata.layoutVersion !== request.layoutVersion ||
    signals.length !== request.signalKeys.length ||
    signals.some((signal, index) => signal.key !== request.signalKeys[index])
  ) {
    return null;
  }

  const interval = metadata.sampleIntervalSeconds;
  const steps = points.map((point) => toNumber(point.simulationStep));
  const times = steps.map((step) => step * interval);
  const first = steps[0];
  const second = steps[1];

  return {
    runId: metadata.runId,
    layoutVersion: metadata.layoutVersion,
    documentRevision: metadata.documentRevision,
    sampleIntervalSeconds: interval,
    latestTimeSeconds:
      metadata.latestSampleStep === null
        ? null
        : toNumber(metadata.latestSampleStep) * interval,
    samplesPerPoint:
      first === undefined || second === undefined
        ? 1
        : Math.max(1, second - first),
    signals: signals.map((signal, index) => ({
      key: signal.key,
      unit: signal.unit,
      times,
      minimums: points.map((point) => point.minimumValues[index] ?? Number.NaN),
      maximums: points.map((point) => point.maximumValues[index] ?? Number.NaN),
    })),
  };
}

/**
 * The oscilloscope's state and its reads of the Engine's trace history. Plain
 * TypeScript with no React, Pixi or editor: React subscribes through
 * `subscribe`/`getState`, and the editor never sees it.
 *
 * The Engine samples at every public step whether or not anything is watching
 * (stage-3-dynamics.md section 7.2), so this only reads: it asks for the visible
 * window of the selected signals, decimated to a bounded number of points, and
 * replaces what it holds with the answer. It therefore keeps at most one response,
 * never the whole history. Every response is checked against the run and selection
 * it was asked for, and all data is dropped the moment the active run changes.
 */
export class OscilloscopeController {
  private state: ScopeState = initialScopeState();
  private readonly listeners = new Set<() => void>();
  /** The user's choice. The visible selection is this, limited to the current layout. */
  private preferredKeys: readonly string[] = [];
  private unsubscribe: (() => void) | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private reading = false;
  /** A change arrived during a read, so the answer is outdated and another read is due. */
  private dirty = false;
  private disposed = false;
  private lastLatestStep: bigint | null = null;
  /** What the last simulation notification looked like, to react to changes only. */
  private seenRun: { runId: StableId; layoutVersion: number } | null = null;
  private seenStatus: SimulationState["status"] | null = null;

  constructor(
    private readonly adapter: Pick<EngineAdapter, "readTraces">,
    private readonly simulation: SimulationSource,
    private readonly options: ScopeOptions = {},
  ) {}

  readonly getState = (): ScopeState => this.state;

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** Starts following the simulation. Safe to call once. */
  start(): void {
    if (this.unsubscribe || this.disposed) {
      return;
    }

    this.unsubscribe = this.simulation.subscribe(this.handleSimulationChange);
    this.handleSimulationChange();
  }

  dispose(): void {
    this.disposed = true;
    this.clearTimer();
    this.unsubscribe?.();
    this.unsubscribe = null;
    this.listeners.clear();
  }

  /** Adds the signal to the traces, or removes it. Refused beyond eight signals. */
  toggleSignal(key: string): void {
    const visible = this.state.selectedKeys;

    if (visible.includes(key)) {
      this.preferredKeys = visible.filter((candidate) => candidate !== key);
    } else if (
      visible.length < MAX_SCOPE_SIGNALS &&
      this.layoutKeys().has(key)
    ) {
      this.preferredKeys = [...visible, key];
    } else {
      return;
    }

    this.applySelection();
  }

  setWindowSeconds(seconds: number): void {
    if (
      !SCOPE_WINDOWS_SECONDS.includes(seconds) ||
      seconds === this.state.windowSeconds
    ) {
      return;
    }

    this.setState({ windowSeconds: seconds });
    this.readSoon();
  }

  private layoutKeys(): ReadonlySet<string> {
    const run = this.simulation.getState().run;

    return new Set(run ? run.layout.slots.map((slot) => slot.key) : []);
  }

  /**
   * Makes the selection the user's choice limited to the current layout (a signal whose
   * component is gone leaves the display, section 7.5), and drops the data of signals
   * that are no longer shown. A change of selection triggers a new read.
   */
  private applySelection(): void {
    const layout = this.layoutKeys();
    const selectedKeys = this.preferredKeys
      .filter((key) => layout.has(key))
      .slice(0, MAX_SCOPE_SIGNALS);
    const unchanged =
      selectedKeys.length === this.state.selectedKeys.length &&
      selectedKeys.every(
        (key, index) => key === this.state.selectedKeys[index],
      );

    if (unchanged) {
      return;
    }

    const data = this.state.data;

    this.setState({
      selectedKeys,
      data: data && {
        ...data,
        signals: data.signals.filter((signal) =>
          selectedKeys.includes(signal.key),
        ),
      },
    });
    this.readSoon();
  }

  /**
   * Frames arrive many times a second and do not concern the scope, so only a change of
   * run, layout or status is acted on.
   */
  private readonly handleSimulationChange = (): void => {
    if (this.disposed) {
      return;
    }

    const { run, status } = this.simulation.getState();
    const seen = this.seenRun;
    const runChanged =
      run === null
        ? seen !== null
        : seen === null ||
          seen.runId !== run.runId ||
          seen.layoutVersion !== run.layout.layoutVersion;

    if (!runChanged && status === this.seenStatus) {
      return;
    }

    this.seenRun = run
      ? { runId: run.runId, layoutVersion: run.layout.layoutVersion }
      : null;
    this.seenStatus = status;

    if (runChanged) {
      // A new run, a new layout, or no run: nothing shown may belong to the old one.
      this.lastLatestStep = null;
      this.setState({ data: null, error: null, loading: false });
    }

    this.applySelection();
    this.readSoon();
  };

  private clearTimer(): void {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  /** Reads as soon as no read is running. */
  private readSoon(): void {
    if (this.reading) {
      this.dirty = true;
      return;
    }

    this.scheduleNext(0);
  }

  private scheduleNext(delayMs: number): void {
    this.clearTimer();

    if (!this.disposed) {
      this.timer = setTimeout(() => void this.read(), delayMs);
    }
  }

  /**
   * The visible window of the selected signals. It starts a little before the newest
   * step known, so the response is not short when the Engine is ahead of the last frame;
   * the Engine clamps the end to its newest sample.
   */
  private buildRequest(run: ActiveRun, keys: readonly string[]): TraceRequest {
    const { data, windowSeconds } = this.state;
    const interval =
      data?.sampleIntervalSeconds ?? DEFAULT_SAMPLE_INTERVAL_SECONDS;
    const windowSteps = Math.ceil(windowSeconds / interval);
    const frameStep =
      this.simulation.getState().snapshot?.metadata.simulationStep;
    const traceStep =
      data?.latestTimeSeconds == null
        ? 0
        : Math.round(data.latestTimeSeconds / interval);
    const knownLatest = Math.max(
      frameStep === undefined ? 0 : toNumber(frameStep),
      traceStep,
    );

    return {
      runId: run.runId,
      layoutVersion: run.layout.layoutVersion,
      signalKeys: [...keys],
      query: {
        kind: "history",
        startStep: Math.max(
          0,
          knownLatest - windowSteps + 1 - WINDOW_SLACK_STEPS,
        ),
        endStep: Number.MAX_SAFE_INTEGER,
      },
      maxPoints: MAX_SCOPE_POINTS,
    };
  }

  private async read(): Promise<void> {
    this.timer = null;

    const simulation = this.simulation.getState();
    const { run } = simulation;
    const keys = this.state.selectedKeys;

    if (
      this.disposed ||
      this.reading ||
      simulation.status !== "ready" ||
      run === null ||
      keys.length === 0
    ) {
      // Nothing to read; a change of run, status or selection starts reading again.
      return;
    }

    this.reading = true;
    this.dirty = false;

    let request: TraceRequest | null = null;
    let data: ScopeData | null = null;
    let latest: bigint | null = null;
    let failure: string | null;

    try {
      request = this.buildRequest(run, keys);
      this.setState({ loading: true });

      const batch = await this.adapter.readTraces(request);

      data = scopeDataFromBatch(request, batch);
      latest = batch.metadata.latestSampleStep;
      failure = data
        ? null
        : "The engine returned a trace for a different run or selection.";
    } catch (error) {
      failure = describeError(error);
    }

    this.reading = false;

    if (this.disposed) {
      return;
    }

    const selected = this.state.selectedKeys;
    const stale =
      request !== null &&
      (this.dirty ||
        !sameRun(
          this.simulation.getState().run,
          request.runId,
          request.layoutVersion,
        ) ||
        request.signalKeys.length !== selected.length ||
        request.signalKeys.some((key, index) => key !== selected[index]));

    if (stale) {
      // The answer belongs to a run or selection that is gone. It is dropped without an
      // error, as a stale frame is, and the current one is read next.
      this.setState({ loading: false });
      this.readSoon();
      return;
    }

    const idle = this.options.idleIntervalMs ?? 500;
    let delay = this.options.pollIntervalMs ?? 100;

    if (failure !== null || data === null) {
      this.setState({
        loading: false,
        error: failure ?? "The engine returned no trace.",
      });
      delay = idle;
    } else {
      if (latest !== null && latest === this.lastLatestStep) {
        // The run is not advancing, so nothing new is expected soon.
        delay = idle;
      }

      this.lastLatestStep = latest;
      this.setState({ data, error: null, loading: false });
    }

    this.scheduleNext(delay);
  }

  private setState(patch: Partial<ScopeState>): void {
    this.state = { ...this.state, ...patch };

    for (const listener of [...this.listeners]) {
      listener();
    }
  }
}
