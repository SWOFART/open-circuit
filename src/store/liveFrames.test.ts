import { describe, expect, it, vi } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { SetParameterResult } from "../generated/SetParameterResult";
import { dividerSchematic } from "./fixtures";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { SimulationController, isSnapshotSuperseded } from "./simulation";
import { symbols } from "../symbols";

const R1 = "00000000-0000-4000-8000-000000000002";
const RUN_ID = "00000000-0000-4000-8000-0000000000a0";
const OTHER_RUN = "00000000-0000-4000-8000-0000000000b1";

const layout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 1,
  slots: [
    {
      key: "r1.voltage",
      componentId: R1,
      quantity: "voltage",
      unit: "volt",
      signConvention: "voltagePin1MinusPin2",
      offset: 0,
    },
  ],
};

function revisions(): () => string {
  let next = 0;
  return () => {
    next += 1;
    return `rev-${next}`;
  };
}

function frame(
  documentRevision: string,
  sequence: bigint,
  value = 1,
  overrides: { runId?: string; layoutVersion?: number; values?: number[] } = {},
): ResultSnapshot {
  return {
    metadata: {
      runId: overrides.runId ?? RUN_ID,
      documentRevision,
      layoutVersion: overrides.layoutVersion ?? 1,
      simulationStep: sequence,
      simulationTimeSeconds: Number(sequence) * 0.001,
      frameSequence: sequence,
    },
    values: overrides.values ?? [value],
    states: [],
  };
}

function compiled(documentRevision: string): CompileResult {
  return {
    status: "success",
    internalSubsteps: 1,
    runId: RUN_ID,
    layout,
    initialSnapshot: frame(documentRevision, 0n, 1),
    diagnostics: [],
  };
}

/** An adapter whose parameter updates stay pending until the test answers them. */
function adapterWithPendingUpdate() {
  let answer: ((result: SetParameterResult) => void) | undefined;
  const adapter = new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request) => compiled(request.documentRevision),
    setParameter: () =>
      new Promise<SetParameterResult>((resolve) => {
        answer = resolve;
      }),
  });

  return {
    adapter,
    answer: (result: SetParameterResult) => answer?.(result),
  };
}

function controllerFor(adapter: PredeterminedEngineAdapter) {
  return new SimulationController(adapter, symbols, revisions());
}

async function started(adapter = adapterWithPendingUpdate().adapter) {
  const controller = controllerFor(adapter);
  await controller.start(dividerSchematic);
  return { controller, adapter };
}

describe("opening the frame stream", () => {
  it("requests frames for the compiled run once it is ready", async () => {
    const { controller, adapter } = await started();

    expect(controller.getState().status).toBe("ready");
    expect(adapter.startedRuns).toEqual([RUN_ID]);
  });

  it("does not request frames when the circuit was rejected", async () => {
    const adapter = new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: () => ({ status: "failure", diagnostics: [] }),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });

    await controllerFor(adapter).start(dividerSchematic);

    expect(adapter.startedRuns).toEqual([]);
  });

  it("stops the stream on dispose and ignores frames after it", async () => {
    const { controller, adapter } = await started();

    controller.dispose();
    const before = controller.getState().snapshot;
    adapter.emitFrame(frame("rev-1", 9n, 5));

    expect(adapter.stopCount).toBe(1);
    expect(controller.getState().snapshot).toBe(before);
  });

  it("stops a stream that finished opening after dispose", async () => {
    let finishOpening: ((stop: () => void) => void) | undefined;
    const stop = vi.fn();
    const adapter = new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: (request) => compiled(request.documentRevision),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    adapter.startFrames = () =>
      new Promise((resolve) => {
        finishOpening = resolve;
      });
    const controller = controllerFor(adapter);

    const starting = controller.start(dividerSchematic);
    await vi.waitFor(() => expect(finishOpening).toBeDefined());
    controller.dispose();
    finishOpening?.(stop);
    await starting;

    expect(stop).toHaveBeenCalledTimes(1);
  });

  it("keeps the results and says so when live updates cannot start", async () => {
    const adapter = new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: (request) => compiled(request.documentRevision),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    adapter.startFrames = () => Promise.reject(new Error("channel failed"));
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    const state = controller.getState();

    expect(state.status).toBe("ready");
    expect(state.snapshot).not.toBeNull();
    expect(state.error).toBe("Live updates are unavailable: channel failed");
  });
});

describe("applying live frames", () => {
  it("shows a newer frame of the active revision", async () => {
    const { controller, adapter } = await started();

    adapter.emitFrame(frame("rev-1", 5n, 42));

    expect(controller.getState().snapshot?.values).toEqual([42]);
    expect(controller.getState().discardedFrames).toBe(0);
  });

  it.each([
    ["another revision", frame("elsewhere", 5n, 42)],
    ["the sequence of the frame already shown", frame("rev-1", 0n, 42)],
    ["another run", frame("rev-1", 5n, 42, { runId: OTHER_RUN })],
    ["another layout version", frame("rev-1", 5n, 42, { layoutVersion: 2 })],
    ["the wrong number of values", frame("rev-1", 5n, 42, { values: [1, 2] })],
    ["a non-finite value", frame("rev-1", 5n, 42, { values: [Number.NaN] })],
  ])("drops a frame with %s without raising an error", async (_name, stale) => {
    const { controller, adapter } = await started();
    const before = controller.getState().snapshot;

    adapter.emitFrame(stale);
    const state = controller.getState();

    expect(state.snapshot).toBe(before);
    expect(state.discardedFrames).toBe(1);
    expect(state.error).toBeNull();
  });

  it("drops a frame that is not newer than the last one applied", async () => {
    const { controller, adapter } = await started();

    adapter.emitFrame(frame("rev-1", 6n, 7));
    adapter.emitFrame(frame("rev-1", 5n, 99));
    adapter.emitFrame(frame("rev-1", 6n, 99));

    expect(controller.getState().snapshot?.values).toEqual([7]);
    expect(controller.getState().discardedFrames).toBe(2);
  });

  it("reports an unreadable frame as an error and counts it as discarded", async () => {
    const { controller, adapter } = await started();
    const before = controller.getState().snapshot;

    adapter.emitFrameError(
      new Error("The frame does not start with the OCF2 magic."),
    );
    const state = controller.getState();

    expect(state.snapshot).toBe(before);
    expect(state.discardedFrames).toBe(1);
    expect(state.error).toMatch(/unusable: The frame does not start/);
  });
});

describe("frames during a parameter update", () => {
  it("accepts frames of the new revision and drops frames of the old one", async () => {
    const { adapter } = adapterWithPendingUpdate();
    const { controller } = await started(adapter);

    void controller.setParameter(R1, "resistanceOhms", 2000);

    adapter.emitFrame(frame("rev-1", 4n, 11)); // produced before the update
    expect(controller.getState().snapshot?.values).toEqual([1]);

    adapter.emitFrame(frame("rev-2", 8n, 22)); // produced after it
    expect(controller.getState().snapshot?.values).toEqual([22]);
    expect(controller.getState().discardedFrames).toBe(1);
  });

  it("confirms an update whose newer frame overtook its reply", async () => {
    const { adapter, answer } = adapterWithPendingUpdate();
    const { controller } = await started(adapter);

    const done = controller.setParameter(R1, "resistanceOhms", 2000);
    adapter.emitFrame(frame("rev-2", 8n, 22));

    // The reply for the same revision carries an older sequence than that frame.
    answer({
      status: "success",
      internalSubsteps: 1,
      snapshot: frame("rev-2", 7n, 21),
      diagnostics: [],
    });
    await done;

    const state = controller.getState();
    expect(state.updating).toBe(false);
    expect(state.error).toBeNull();
    expect(state.snapshot?.values).toEqual([22]);
    expect(state).not.toHaveProperty("schematic");
    expect(state.run?.documentRevision).toBe("rev-2");
  });

  it("applies the reply normally when no frame overtook it", async () => {
    const { adapter, answer } = adapterWithPendingUpdate();
    const { controller } = await started(adapter);

    const done = controller.setParameter(R1, "resistanceOhms", 2000);
    answer({
      status: "success",
      internalSubsteps: 1,
      snapshot: frame("rev-2", 7n, 21),
      diagnostics: [],
    });
    await done;

    expect(controller.getState().snapshot?.values).toEqual([21]);
    expect(controller.getState().error).toBeNull();
  });

  it("still treats a reply for a different revision as unconfirmed", async () => {
    const { adapter, answer } = adapterWithPendingUpdate();
    const { controller } = await started(adapter);

    const done = controller.setParameter(R1, "resistanceOhms", 2000);
    adapter.emitFrame(frame("rev-2", 8n, 22));
    answer({
      status: "success",
      internalSubsteps: 1,
      snapshot: frame("not-this-revision", 9n, 5),
      diagnostics: [],
    });
    await done;

    const state = controller.getState();
    expect(state.error).toMatch(/unconfirmed/);
    expect(state.run?.documentRevision).toBe("rev-1");
    expect(state).not.toHaveProperty("schematic");
  });
});

describe("isSnapshotSuperseded", () => {
  const run = { runId: RUN_ID, layout, documentRevision: "rev-2" };

  it("needs a frame of the same revision that is not older than the reply", () => {
    const reply = frame("rev-2", 7n);

    expect(isSnapshotSuperseded(run, frame("rev-2", 8n), reply)).toBe(true);
    expect(isSnapshotSuperseded(run, frame("rev-2", 7n), reply)).toBe(true);
    expect(isSnapshotSuperseded(run, frame("rev-2", 6n), reply)).toBe(false);
  });

  it("is false without an applied frame or for a frame of another revision", () => {
    const reply = frame("rev-2", 7n);

    expect(isSnapshotSuperseded(run, null, reply)).toBe(false);
    expect(isSnapshotSuperseded(run, frame("rev-1", 9n), reply)).toBe(false);
    expect(isSnapshotSuperseded(null, frame("rev-2", 9n), reply)).toBe(false);
  });
});
