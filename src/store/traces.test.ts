import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { TraceBatch } from "../generated/TraceBatch";
import type { TraceRequest } from "../generated/TraceRequest";
import {
  initialSimulationState,
  type ActiveRun,
  type SimulationState,
} from "./simulation";
import {
  DEFAULT_SCOPE_WINDOW_SECONDS,
  MAX_SCOPE_POINTS,
  MAX_SCOPE_SIGNALS,
  OscilloscopeController,
  scopeDataFromBatch,
  type SimulationSource,
} from "./traces";

const RUN_A = "00000000-0000-4000-8000-0000000000a1";
const RUN_B = "00000000-0000-4000-8000-0000000000b2";
const V1 = "00000000-0000-4000-8000-000000000011:voltage";
const I1 = "00000000-0000-4000-8000-000000000011:current";
const V2 = "00000000-0000-4000-8000-000000000022:voltage";

function layout(keys: readonly string[], layoutVersion = 1): ResultLayout {
  return {
    layoutVersion,
    valueCount: keys.length,
    slots: keys.map((key, offset) => ({
      key,
      componentId: key.split(":")[0]!,
      quantity: key.endsWith("current") ? "current" : "voltage",
      unit: key.endsWith("current") ? "amp" : "volt",
      signConvention: "voltagePin1MinusPin2",
      offset,
    })),
  };
}

function run(
  runId: string,
  keys: readonly string[],
  revision = "rev-1",
  layoutVersion = 1,
): ActiveRun {
  return {
    runId,
    layout: layout(keys, layoutVersion),
    documentRevision: revision,
  };
}

function snapshotAt(step: bigint): ResultSnapshot {
  return {
    metadata: {
      runId: RUN_A,
      documentRevision: "rev-1",
      layoutVersion: 1,
      simulationStep: step,
      simulationTimeSeconds: Number(step) * 0.001,
      frameSequence: step,
    },
    values: [],
    states: [],
  };
}

/** A stand-in for the simulation store whose run and status the test controls. */
class FakeSimulation implements SimulationSource {
  state: SimulationState = initialSimulationState();
  private readonly listeners = new Set<() => void>();

  constructor(activeRun: ActiveRun | null) {
    this.state = {
      ...initialSimulationState(),
      status: activeRun ? "ready" : "idle",
      run: activeRun,
    };
  }

  getState = (): SimulationState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  set(patch: Partial<SimulationState>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of [...this.listeners]) {
      listener();
    }
  }
}

interface BatchOptions {
  readonly runId?: string;
  readonly layoutVersion?: number;
  readonly steps?: readonly number[];
  readonly latest?: number | null;
  readonly value?: (key: string, step: number) => number;
}

/** What the Engine would answer: one exact sample per requested step. */
function batchFor(
  request: TraceRequest,
  options: BatchOptions = {},
): TraceBatch {
  const steps = options.steps ?? [0, 1, 2];
  const latest =
    options.latest === undefined ? steps[steps.length - 1] : options.latest;
  const value = options.value ?? ((_key, step) => step * 0.5);

  return {
    metadata: {
      runId: options.runId ?? request.runId,
      documentRevision: "rev-1",
      layoutVersion: options.layoutVersion ?? request.layoutVersion,
      sampleIntervalSeconds: 0.001,
      historySamples: 10000,
      oldestSampleStep: 0n,
      latestSampleStep:
        latest === null || latest === undefined ? null : BigInt(latest),
      firstSampleStep: steps.length > 0 ? BigInt(steps[0]!) : null,
      lastSampleStep:
        steps.length > 0 ? BigInt(steps[steps.length - 1]!) : null,
      sampleCount: steps.length,
      missedSamples: 0n,
      replaced: false,
    },
    signals: request.signalKeys.map((key) => ({
      key,
      unit: key.endsWith("current") ? "amp" : "volt",
    })),
    points: steps.map((step) => ({
      simulationStep: BigInt(step),
      minimumValues: request.signalKeys.map((key) => value(key, step)),
      maximumValues: request.signalKeys.map((key) => value(key, step)),
    })),
  };
}

interface Deferred {
  readonly request: TraceRequest;
  readonly resolve: (batch: TraceBatch) => void;
  readonly reject: (error: Error) => void;
}

/** An adapter whose reads stay pending until the test answers them. */
function manualAdapter() {
  const reads: Deferred[] = [];

  return {
    reads,
    readTraces: (request: TraceRequest) =>
      new Promise<TraceBatch>((resolve, reject) => {
        reads.push({ request, resolve, reject });
      }),
  };
}

/** An adapter that answers every read at once with `answer`. */
function answeringAdapter(
  answer: (request: TraceRequest) => TraceBatch | Promise<TraceBatch>,
) {
  const requests: TraceRequest[] = [];

  return {
    requests,
    readTraces: async (request: TraceRequest) => {
      requests.push(request);
      return answer(request);
    },
  };
}

async function settle(milliseconds = 0): Promise<void> {
  await vi.advanceTimersByTimeAsync(milliseconds);
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

function scopeFor(
  simulation: FakeSimulation,
  adapter: { readTraces: (request: TraceRequest) => Promise<TraceBatch> },
) {
  const scope = new OscilloscopeController(adapter, simulation, {
    pollIntervalMs: 100,
    idleIntervalMs: 500,
  });
  scope.start();

  return scope;
}

describe("selecting signals", () => {
  it("reads nothing while no signal is selected", async () => {
    const adapter = answeringAdapter((request) => batchFor(request));
    scopeFor(new FakeSimulation(run(RUN_A, [V1, I1])), adapter);

    await settle(1000);

    expect(adapter.requests).toEqual([]);
  });

  it("requests the visible window of the selected signals, bounded", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1, I1]));
    simulation.set({ snapshot: snapshotAt(500n) });
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    scope.toggleSignal(I1);
    await settle();

    const request = adapter.requests.at(-1)!;

    expect(request.runId).toBe(RUN_A);
    expect(request.layoutVersion).toBe(1);
    expect(request.signalKeys).toEqual([V1, I1]);
    expect(request.maxPoints).toBe(MAX_SCOPE_POINTS);
    expect(request.query.kind).toBe("history");
    // 0.1 s at 1 ms is 100 steps, ending at step 500, with a little slack before.
    if (request.query.kind !== "history") throw new Error("expected history");
    expect(request.query.startStep).toBeLessThanOrEqual(401);
    expect(request.query.startStep).toBeGreaterThan(300);
    expect(request.query.endStep).toBe(Number.MAX_SAFE_INTEGER);
  });

  it("never asks for more than eight signals", async () => {
    const keys = Array.from(
      { length: 10 },
      (_, index) => `00000000-0000-4000-8000-0000000000${index + 10}:voltage`,
    );
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(new FakeSimulation(run(RUN_A, keys)), adapter);

    for (const key of keys) {
      scope.toggleSignal(key);
    }
    await settle();

    expect(scope.getState().selectedKeys).toEqual(
      keys.slice(0, MAX_SCOPE_SIGNALS),
    );
    expect(adapter.requests.at(-1)!.signalKeys).toHaveLength(MAX_SCOPE_SIGNALS);
  });

  it("ignores a key that is not in the layout", async () => {
    const scope = scopeFor(
      new FakeSimulation(run(RUN_A, [V1])),
      answeringAdapter((request) => batchFor(request)),
    );

    scope.toggleSignal(V2);

    expect(scope.getState().selectedKeys).toEqual([]);
  });

  it("removes a deselected signal from the display at once", async () => {
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1, I1])), adapter);

    scope.toggleSignal(V1);
    scope.toggleSignal(I1);
    await settle();
    expect(scope.getState().data?.signals.map((s) => s.key)).toEqual([V1, I1]);

    scope.toggleSignal(V1);

    expect(scope.getState().data?.signals.map((s) => s.key)).toEqual([I1]);
  });
});

describe("what is displayed", () => {
  it("shows the Engine's samples against simulation time in seconds", async () => {
    const adapter = answeringAdapter((request) =>
      batchFor(request, {
        steps: [10, 11, 12],
        value: (_key, step) => step * 0.1,
      }),
    );
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle();

    const data = scope.getState().data;

    expect(data?.signals[0]).toMatchObject({
      key: V1,
      unit: "volt",
      times: [0.01, 0.011, 0.012],
    });
    expect(data?.signals[0]?.minimums[1]).toBeCloseTo(1.1);
    expect(data?.latestTimeSeconds).toBeCloseTo(0.012);
    expect(data?.samplesPerPoint).toBe(1);
  });

  it("reports how many samples a decimated point stands for", () => {
    const request: TraceRequest = {
      runId: RUN_A,
      layoutVersion: 1,
      signalKeys: [V1],
      query: { kind: "history", startStep: 0, endStep: 99 },
      maxPoints: 2,
    };
    const batch = batchFor(request, { steps: [0, 50], latest: 99 });
    batch.points[0] = {
      simulationStep: 0n,
      minimumValues: [-1],
      maximumValues: [2],
    };

    const data = scopeDataFromBatch(request, batch);

    expect(data?.samplesPerPoint).toBe(50);
    expect(data?.signals[0]?.minimums[0]).toBe(-1);
    expect(data?.signals[0]?.maximums[0]).toBe(2);
  });

  it("shows an empty trace for a run that has stored nothing yet", async () => {
    const adapter = answeringAdapter((request) =>
      batchFor(request, { steps: [], latest: null }),
    );
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle();

    expect(scope.getState().data?.signals[0]?.times).toEqual([]);
    expect(scope.getState().data?.latestTimeSeconds).toBeNull();
    expect(scope.getState().error).toBeNull();
  });

  it("starts with the contract's default window and reads a new one when it changes", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    simulation.set({ snapshot: snapshotAt(20000n) });
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(simulation, adapter);

    expect(scope.getState().windowSeconds).toBe(DEFAULT_SCOPE_WINDOW_SECONDS);

    scope.toggleSignal(V1);
    await settle();
    const before = adapter.requests.at(-1)!.query;

    scope.setWindowSeconds(1);
    await settle();
    const after = adapter.requests.at(-1)!.query;

    if (before.kind !== "history" || after.kind !== "history") {
      throw new Error("expected history queries");
    }
    expect(after.startStep).toBeLessThan(before.startStep - 800);
    expect(scope.getState().windowSeconds).toBe(1);
  });

  it("refuses a window that is not one of the offered ones", () => {
    const scope = scopeFor(
      new FakeSimulation(run(RUN_A, [V1])),
      answeringAdapter((request) => batchFor(request)),
    );

    scope.setWindowSeconds(3);

    expect(scope.getState().windowSeconds).toBe(DEFAULT_SCOPE_WINDOW_SECONDS);
  });
});

describe("a run or document change", () => {
  it("clears the trace at once when a new run replaces the active one", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    const adapter = manualAdapter();
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle();
    adapter.reads[0]!.resolve(batchFor(adapter.reads[0]!.request));
    await settle();
    expect(scope.getState().data?.runId).toBe(RUN_A);

    simulation.set({ run: run(RUN_B, [V1], "rev-2") });

    // Before anything is read for the new run, nothing of the old one is left.
    expect(scope.getState().data).toBeNull();
    expect(scope.getState().selectedKeys).toEqual([V1]);
  });

  it("drops a response that arrives after its run was replaced", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    const adapter = manualAdapter();
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle();
    const stale = adapter.reads[0]!;

    simulation.set({ run: run(RUN_B, [V1], "rev-2") });
    stale.resolve(batchFor(stale.request, { steps: [5, 6, 7] }));
    await settle();

    expect(scope.getState().data).toBeNull();
    expect(scope.getState().error).toBeNull();

    // The new run is read, and only its answer is shown.
    const fresh = adapter.reads.at(-1)!;
    expect(fresh.request.runId).toBe(RUN_B);
    fresh.resolve(batchFor(fresh.request, { steps: [0] }));
    await settle();

    expect(scope.getState().data?.runId).toBe(RUN_B);
  });

  it("drops an error that arrives after its run was replaced", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    const adapter = manualAdapter();
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle();
    const stale = adapter.reads[0]!;

    simulation.set({ run: run(RUN_B, [V1], "rev-2") });
    stale.reject(new Error("the trace request cannot be served: StaleRun"));
    await settle();

    expect(scope.getState().error).toBeNull();
  });

  it("clears the trace when the layout version changes with the same run", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle();
    expect(scope.getState().data).not.toBeNull();

    simulation.set({ run: run(RUN_A, [V1], "rev-1", 2) });

    expect(scope.getState().data).toBeNull();
  });

  it("keeps the trace across a live parameter update, which keeps the run", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle();
    const before = scope.getState().data;

    // A parameter update replaces the run object with a new revision, same run ID.
    simulation.set({ run: run(RUN_A, [V1], "rev-2") });

    expect(scope.getState().data).toBe(before);
  });

  it("clears the trace when the circuit no longer compiles", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle();
    simulation.set({ status: "failed", run: null, snapshot: null });
    const requestsBefore = adapter.requests.length;
    await settle(2000);

    expect(scope.getState().data).toBeNull();
    expect(adapter.requests).toHaveLength(requestsBefore);
    // The choice is kept for when the circuit works again.
    simulation.set({ status: "ready", run: run(RUN_B, [V1]) });
    await settle();
    expect(scope.getState().selectedKeys).toEqual([V1]);
    expect(adapter.requests.at(-1)!.runId).toBe(RUN_B);
  });

  it("drops selected signals that left the layout and keeps the others", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1, V2]));
    const scope = scopeFor(
      simulation,
      answeringAdapter((request) => batchFor(request)),
    );

    scope.toggleSignal(V1);
    scope.toggleSignal(V2);
    await settle();

    simulation.set({ run: run(RUN_B, [V2]) });
    await settle();

    expect(scope.getState().selectedKeys).toEqual([V2]);

    // The component comes back (undo): its signal is traced again.
    simulation.set({ run: run(RUN_A, [V1, V2]) });
    await settle();

    expect(scope.getState().selectedKeys).toEqual([V1, V2]);
  });

  it("does not read again for every frame", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    const adapter = answeringAdapter((request) =>
      batchFor(request, { steps: [0, 1, 2], latest: 2 }),
    );
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle();
    const before = adapter.requests.length;

    for (let step = 1n; step <= 60n; step += 1n) {
      simulation.set({ snapshot: snapshotAt(step) });
    }
    await settle(50);

    expect(adapter.requests.length).toBe(before);
  });
});

describe("answers that do not belong to the request", () => {
  it("does not display a trace of another run", async () => {
    const adapter = answeringAdapter((request) =>
      batchFor(request, { runId: RUN_B }),
    );
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle();

    expect(scope.getState().data).toBeNull();
    expect(scope.getState().error).toMatch(/different run or selection/);
  });

  it("does not display a trace of another layout", async () => {
    const adapter = answeringAdapter((request) =>
      batchFor(request, { layoutVersion: 9 }),
    );
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle();

    expect(scope.getState().data).toBeNull();
    expect(scope.getState().error).not.toBeNull();
  });

  it("does not display signals in a different order than selected", async () => {
    const adapter = answeringAdapter((request) => {
      const batch = batchFor(request);
      return { ...batch, signals: [...batch.signals].reverse() };
    });
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1, I1])), adapter);

    scope.toggleSignal(V1);
    scope.toggleSignal(I1);
    await settle();

    expect(scope.getState().data).toBeNull();
  });
});

describe("failures", () => {
  it("reports a failed read and keeps the earlier trace of the same run", async () => {
    let fail = false;
    const adapter = answeringAdapter((request) => {
      if (fail) {
        throw new Error("The engine did not deliver the trace.");
      }
      return batchFor(request);
    });
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle();
    const before = scope.getState().data;

    fail = true;
    await settle(200);

    expect(scope.getState().error).toMatch(/did not deliver/);
    expect(scope.getState().data).toBe(before);

    fail = false;
    await settle(1000);

    expect(scope.getState().error).toBeNull();
  });
});

describe("polling", () => {
  it("polls while the run advances and slows down when it does not", async () => {
    let latest = 10;
    const adapter = answeringAdapter((request) =>
      batchFor(request, { steps: [latest], latest }),
    );
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle();

    for (let tick = 0; tick < 5; tick += 1) {
      latest += 16;
      await settle(100);
    }
    const advancing = adapter.requests.length;
    expect(advancing).toBeGreaterThanOrEqual(5);

    // The run stops: the same newest step comes back, so reads become rarer.
    await settle(100);
    const stopped = adapter.requests.length;
    await settle(2000);

    expect(adapter.requests.length - stopped).toBeLessThanOrEqual(5);
  });

  it("makes one read at a time", async () => {
    const adapter = manualAdapter();
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle(1000);

    expect(adapter.reads).toHaveLength(1);
  });

  it("stops reading and ignores an answer after dispose", async () => {
    const adapter = manualAdapter();
    const scope = scopeFor(new FakeSimulation(run(RUN_A, [V1])), adapter);

    scope.toggleSignal(V1);
    await settle();
    scope.dispose();
    adapter.reads[0]!.resolve(batchFor(adapter.reads[0]!.request));
    await settle(2000);

    expect(adapter.reads).toHaveLength(1);
    expect(scope.getState().data).toBeNull();
  });

  it("does not read while the simulation is not ready", async () => {
    const simulation = new FakeSimulation(run(RUN_A, [V1]));
    simulation.set({ status: "unavailable" });
    const adapter = answeringAdapter((request) => batchFor(request));
    const scope = scopeFor(simulation, adapter);

    scope.toggleSignal(V1);
    await settle(1000);

    expect(adapter.requests).toEqual([]);
  });
});
