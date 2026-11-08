import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { Schematic } from "../generated/Schematic";
import type { SetParameterResult } from "../generated/SetParameterResult";
import { symbols } from "../symbols";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { SimulationController } from "./simulation";
import { layoutFor, valuesFor } from "./stage4TestSupport";

// Issue #114: the semiconductor drawings go through the same run lifecycle as every other
// drawing. The adapter answers with scripted results; nothing here solves a circuit.
const DIODE = "00000000-0000-4000-8000-0000000000f1";
const NPN = "00000000-0000-4000-8000-0000000000f2";

function drawing(saturation: number): Schematic {
  return {
    schemaVersion: 1,
    components: [
      {
        typeId: "diode",
        id: DIODE,
        position: { x: 10, y: 10 },
        rotationDeg: 0,
        parameters: {
          saturationCurrentAmps: saturation,
          emissionCoefficient: 1,
        },
      },
      {
        typeId: "bjt_npn",
        id: NPN,
        position: { x: 30, y: 10 },
        rotationDeg: 0,
        parameters: {
          saturationCurrentAmps: 1e-14,
          forwardBeta: 100,
          reverseBeta: 1,
        },
      },
    ],
    wires: [],
    junctions: [],
    referenceMarkers: [],
  };
}

const layout: ResultLayout = layoutFor(drawing(1e-14));

function runId(index: number): string {
  return `00000000-0000-4000-8000-${index.toString(16).padStart(12, "0")}`;
}

function snapshot(
  id: string,
  revision: string,
  sequence: bigint,
  value: number,
): ResultSnapshot {
  return {
    metadata: {
      runId: id,
      documentRevision: revision,
      layoutVersion: 1,
      simulationStep: sequence,
      simulationTimeSeconds: Number(sequence) * 0.001,
      frameSequence: sequence,
    },
    values: valuesFor(layout, { [layout.slots[0]!.key]: value }),
    states: [],
  };
}

function success(
  request: CompileRequest,
  index: number,
  value: number,
): CompileResult {
  return {
    status: "success",
    internalSubsteps: 1,
    runId: runId(index),
    layout,
    initialSnapshot: snapshot(
      runId(index),
      request.documentRevision,
      0n,
      value,
    ),
    diagnostics: [],
  };
}

const nonConvergence: Diagnostic = {
  code: "NonConvergence",
  severity: "error",
  localizationKey: "diagnostic.nonConvergence",
  references: [{ kind: "component", componentId: NPN }],
};

const outOfRange: Diagnostic = {
  code: "ModelRangeExceeded",
  severity: "error",
  localizationKey: "diagnostic.modelRangeExceeded",
  references: [{ kind: "component", componentId: DIODE }],
};

interface Deferred<T> {
  readonly promise: Promise<T>;
  resolve(value: T): void;
}

function deferred<T>(): Deferred<T> {
  let resolve: (value: T) => void = () => {};
  const promise = new Promise<T>((done) => {
    resolve = done;
  });

  return { promise, resolve };
}

function setUp() {
  let compiles = 0;
  const script = {
    compile: (
      request: CompileRequest,
    ): CompileResult | Promise<CompileResult> =>
      success(request, ++compiles, 0.5),
    setParameter: (): SetParameterResult | Promise<SetParameterResult> => ({
      status: "failure",
      diagnostics: [],
    }),
  };
  const adapter = new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request) => script.compile(request),
    setParameter: () => script.setParameter(),
  });
  let revision = 0;
  const controller = new SimulationController(
    adapter,
    symbols,
    () => `rev-${++revision}`,
  );

  return { adapter, script, controller };
}

describe("a semiconductor drawing recompiling", () => {
  it("shows no value of the previous document while the new compile is delayed", async () => {
    const { script, controller } = setUp();
    await controller.start(drawing(1e-14));
    expect(controller.getState().snapshot?.values[0]).toBe(0.5);

    const answer = deferred<CompileResult>();
    script.compile = () => answer.promise;
    const pending = controller.syncDocument({
      ...drawing(1e-14),
      components: drawing(1e-14).components.map((item) => ({
        ...item,
        position: { x: item.position.x + 2, y: item.position.y },
      })),
    });
    await Promise.resolve();

    const waiting = controller.getState();
    expect(waiting.status).toBe("loading");
    expect(waiting.run).toBeNull();
    expect(waiting.snapshot).toBeNull();
    expect(waiting.stopped).toBeNull();

    answer.resolve(
      success(
        {
          schematic: drawing(1e-14),
          geometry: {} as never,
          documentRevision: "rev-2",
        },
        2,
        0.9,
      ),
    );
    await pending;

    expect(controller.getState().status).toBe("ready");
    expect(controller.getState().snapshot?.values[0]).toBe(0.9);
  });

  it("rejects a compile result that carries another revision", async () => {
    const { script, controller } = setUp();
    script.compile = (request) => ({
      ...success(request, 1, 0.5),
      initialSnapshot: snapshot(runId(1), "rev-other", 0n, 0.5),
    });

    await controller.start(drawing(1e-14));

    const state = controller.getState();
    expect(state.status).toBe("unavailable");
    expect(state.snapshot).toBeNull();
    expect(state.run).toBeNull();
  });

  it("drops frames of the replaced run and revision after a recompile", async () => {
    const { adapter, controller } = setUp();
    await controller.start(drawing(1e-14));
    const oldRun = controller.getState().run!.runId;

    await controller.syncDocument({
      ...drawing(1e-14),
      components: drawing(1e-14).components.slice(0, 1),
    });
    const before = controller.getState().snapshot;
    adapter.emitFrame(snapshot(oldRun, "rev-1", 9n, 7));

    const state = controller.getState();
    expect(state.snapshot).toBe(before);
    expect(state.snapshot?.values[0]).toBe(0.5);
    expect(state.discardedFrames).toBeGreaterThan(0);
  });

  it("drops the reply of a parameter edit that finished after the run was replaced", async () => {
    const { script, adapter, controller } = setUp();
    await controller.start(drawing(1e-14));
    const answer = deferred<SetParameterResult>();
    script.setParameter = () => answer.promise;

    const edit = controller.syncDocument(drawing(2e-14), {
      componentId: DIODE,
      parameterId: "saturationCurrentAmps",
      value: 2e-14,
    });
    const replace = controller.syncDocument({
      ...drawing(2e-14),
      components: drawing(2e-14).components.slice(0, 1),
    });
    await Promise.resolve();

    answer.resolve({
      status: "success",
      internalSubsteps: 1,
      snapshot: snapshot(runId(1), "rev-2", 3n, 123),
      diagnostics: [],
    });
    await Promise.all([edit, replace]);

    const state = controller.getState();
    expect(adapter.compileRequests).toHaveLength(2);
    expect(state.run?.runId).toBe(runId(2));
    expect(state.snapshot?.values[0]).toBe(0.5);
    expect(state.snapshot?.metadata.runId).toBe(runId(2));
  });
});

describe("the three kinds of nonlinear failure", () => {
  it("keeps an initialization failure apart from a refused edit and from a stop", async () => {
    const { script, adapter, controller } = setUp();
    script.compile = () => ({
      status: "failure",
      diagnostics: [nonConvergence],
    });
    await controller.start(drawing(1e-14));

    const failed = controller.getState();
    expect(failed.status).toBe("failed");
    expect(failed.run).toBeNull();
    expect(failed.snapshot).toBeNull();
    expect(failed.refusedUpdate).toBe(false);
    expect(failed.stopped).toBeNull();
    expect(failed.diagnostics).toEqual([nonConvergence]);

    // The document stays editable: a changed drawing that does compile gives a run again.
    script.compile = (request) => success(request, 5, 0.25);
    await controller.syncDocument(drawing(2e-14));
    expect(controller.getState().status).toBe("ready");
    expect(controller.getState().diagnostics).toEqual([]);

    // A live edit the Engine refuses leaves the run and says so.
    script.setParameter = () => ({
      status: "failure",
      diagnostics: [outOfRange],
    });
    await controller.setParameter(DIODE, "saturationCurrentAmps", 3e-14);
    const refused = controller.getState();
    expect(refused.status).toBe("ready");
    expect(refused.refusedUpdate).toBe(true);
    expect(refused.stopped).toBeNull();
    expect(refused.diagnostics).toEqual([outOfRange]);
    expect(refused.snapshot?.values[0]).toBe(0.25);

    // A stop is a third state, with its own fields.
    adapter.emitNotice({
      kind: "runStopped",
      runId: refused.run!.runId,
      documentRevision: refused.run!.documentRevision,
      simulationStep: 0,
      diagnostics: [nonConvergence],
    } as never);
    const stopped = controller.getState();
    expect(stopped.stopped).not.toBeNull();
    expect(stopped.refusedUpdate).toBe(true);
  });

  it("clears the refused mark when a later edit is accepted", async () => {
    const { script, controller } = setUp();
    await controller.start(drawing(1e-14));
    script.setParameter = () => ({
      status: "failure",
      diagnostics: [outOfRange],
    });
    await controller.setParameter(DIODE, "saturationCurrentAmps", 3e-14);
    expect(controller.getState().refusedUpdate).toBe(true);

    script.setParameter = () => ({
      status: "success",
      internalSubsteps: 1,
      snapshot: snapshot(controller.getState().run!.runId, "rev-3", 1n, 0.4),
      diagnostics: [],
    });
    await controller.setParameter(DIODE, "saturationCurrentAmps", 2e-14);

    expect(controller.getState().refusedUpdate).toBe(false);
    expect(controller.getState().diagnostics).toEqual([]);
  });
});
