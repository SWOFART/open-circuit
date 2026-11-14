import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import circuits from "../../tests/golden/stage-8-circuits.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { Schematic } from "../generated/Schematic";
import { buildGeometryEnvelope } from "../editor/geometry";
import { symbols } from "../symbols";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { SimulationController } from "./simulation";
import { layoutFor, valuesFor } from "./stage4TestSupport";

// Issue #132: a rail is a document edit like any other. The controller compiles the whole
// document, keeps the drawing editable after a failed compile, withdraws the previous
// document's values while it waits, and rejects a reply of another revision. The adapter
// answers with scripted results; nothing here resolves a rail.
const divider = circuits.circuits.find((item) => item.id === "divider")!
  .railSchematic as unknown as Schematic;
const FIRST_RAIL = divider.rails[0]!.id;
const layout: ResultLayout = layoutFor(divider);

function runId(index: number): string {
  return `00000000-0000-4000-8000-${index.toString(16).padStart(12, "0")}`;
}

function snapshot(
  id: string,
  revision: string,
  step: bigint,
  value: number,
): ResultSnapshot {
  return {
    metadata: {
      runId: id,
      documentRevision: revision,
      layoutVersion: 1,
      simulationStep: step,
      simulationTimeSeconds: Number(step) * 0.001,
      frameSequence: step,
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

const invalidName: Diagnostic = {
  code: "InvalidRailName",
  severity: "error",
  localizationKey: "diagnostic.invalidRailName",
  references: [{ kind: "rail", railId: FIRST_RAIL }],
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
  };
  const adapter = new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request) => script.compile(request),
    setParameter: () => ({ status: "failure", diagnostics: [] }),
  });
  let revision = 0;
  const controller = new SimulationController(
    adapter,
    symbols,
    () => `rev-${++revision}`,
  );

  return { adapter, script, controller };
}

function renamed(name: string): Schematic {
  return {
    ...divider,
    rails: divider.rails.map((rail, index) =>
      index === 0 ? { ...rail, name } : rail,
    ),
  };
}

describe("rail documents in the simulation controller", () => {
  it("sends the whole rail document in a compile and no rail in the geometry envelope", async () => {
    const { adapter, controller } = setUp();
    await controller.start(divider);

    const request = adapter.compileRequests[0]!;
    expect(request.schematic.rails).toEqual(divider.rails);
    // The envelope holds component pins only: rail coordinates live in the schematic.
    expect(request.geometry).toEqual(buildGeometryEnvelope(divider, symbols));
    expect(JSON.stringify(request.geometry)).not.toContain(FIRST_RAIL);
  });

  it("compiles again for a rename that changes no connection, and never sends it as a parameter", async () => {
    const { adapter, controller } = setUp();
    await controller.start(divider);

    await controller.syncDocument(renamed("MAIN"));

    expect(adapter.compileRequests).toHaveLength(2);
    expect(adapter.parameterRequests).toHaveLength(0);
    expect(adapter.compileRequests[1]!.schematic.rails[0]!.name).toBe("MAIN");
    // A new run at step 0 replaces the old one.
    expect(controller.getState().run?.runId).toBe(runId(2));
    expect(controller.getState().snapshot?.metadata.simulationStep).toBe(0n);
  });

  it("does nothing for a document that did not change", async () => {
    const { adapter, controller } = setUp();
    await controller.start(divider);

    await controller.syncDocument(divider);

    expect(adapter.compileRequests).toHaveLength(1);
  });

  it("shows none of the previous document's values while the rail edit compiles", async () => {
    const { script, controller } = setUp();
    await controller.start(divider);
    expect(controller.getState().snapshot?.values[0]).toBe(0.5);

    const answer = deferred<CompileResult>();
    script.compile = () => answer.promise;
    const pending = controller.syncDocument(renamed("MAIN"));
    await Promise.resolve();

    const waiting = controller.getState();
    expect(waiting.status).toBe("loading");
    expect(waiting.run).toBeNull();
    expect(waiting.snapshot).toBeNull();

    answer.resolve(
      success(
        {
          schematic: divider,
          geometry: {} as never,
          documentRevision: "rev-2",
        },
        2,
        0.9,
      ),
    );
    await pending;

    expect(controller.getState().snapshot?.values[0]).toBe(0.9);
  });

  it("compiles only the newest of two rail edits made while a compile is in flight", async () => {
    const { adapter, script, controller } = setUp();
    await controller.start(divider);
    const first = deferred<CompileResult>();
    script.compile = () => first.promise;

    const one = controller.syncDocument(renamed("ONE"));
    await Promise.resolve();
    const two = controller.syncDocument(renamed("TWO"));
    script.compile = (request) => success(request, 7, 0.7);
    first.resolve(success(adapter.compileRequests[1]!, 5, 0.1));
    await Promise.all([one, two]);

    // The reply for the older name is not shown; the run on screen is that of the newest.
    const names = adapter.compileRequests.map(
      (request) => request.schematic.rails[0]!.name,
    );
    expect(names.at(-1)).toBe("TWO");
    expect(controller.getState().snapshot?.values[0]).not.toBe(0.1);
  });

  it("drops a frame of the replaced run after a rail edit", async () => {
    const { adapter, controller } = setUp();
    await controller.start(divider);
    const oldRun = controller.getState().run!.runId;

    await controller.syncDocument(renamed("MAIN"));
    const before = controller.getState().snapshot;
    adapter.emitFrame(snapshot(oldRun, "rev-1", 9n, 7));

    expect(controller.getState().snapshot).toBe(before);
    expect(controller.getState().discardedFrames).toBeGreaterThan(0);
  });

  it("keeps the drawing editable after a rail diagnostic and compiles the corrected one", async () => {
    const { script, controller } = setUp();
    await controller.start(divider);

    script.compile = () => ({
      status: "failure",
      diagnostics: [invalidName],
    });
    await controller.syncDocument(renamed("bad name"));

    const failed = controller.getState();
    expect(failed.status).toBe("failed");
    // Not a current run: no values of the previous document and no result.
    expect(failed.run).toBeNull();
    expect(failed.snapshot).toBeNull();
    expect(failed.diagnostics).toEqual([invalidName]);
    expect(failed.diagnostics[0]!.references).toEqual([
      { kind: "rail", railId: FIRST_RAIL },
    ]);

    let compiles = 10;
    script.compile = (request) => success(request, ++compiles, 0.25);
    await controller.syncDocument(renamed("MAIN"));

    expect(controller.getState().status).toBe("ready");
    expect(controller.getState().diagnostics).toEqual([]);
    expect(controller.getState().snapshot?.values[0]).toBe(0.25);
  });
});
