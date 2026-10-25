import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { Schematic } from "../generated/Schematic";
import type { SetParameterRequest } from "../generated/SetParameterRequest";
import type { SetParameterResult } from "../generated/SetParameterResult";
import { symbols } from "../symbols";
import { dividerSchematic } from "./fixtures";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { SimulationController } from "./simulation";

const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";

const floating: Diagnostic = {
  code: "FloatingNode",
  severity: "error",
  localizationKey: "diagnostic.floatingNode",
  references: [{ kind: "component", componentId: R1 }],
};

function runId(index: number): string {
  return `00000000-0000-4000-8000-${index.toString(16).padStart(12, "0")}`;
}

function success(request: CompileRequest, index: number): CompileResult {
  return {
    status: "success",
    internalSubsteps: 1,
    runId: runId(index),
    layout: { layoutVersion: 1, valueCount: 0, slots: [] },
    initialSnapshot: {
      metadata: {
        runId: runId(index),
        documentRevision: request.documentRevision,
        layoutVersion: 1,
        simulationStep: 0n,
        simulationTimeSeconds: 0,
        frameSequence: 0n,
      },
      values: [],
    },
    diagnostics: [],
  };
}

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

/**
 * Compiles succeed with a fresh run ID unless a test swaps in another
 * behaviour. Nothing here calculates physics.
 */
function scriptedAdapter() {
  let compiles = 0;
  const script = {
    compile: (
      request: CompileRequest,
    ): CompileResult | Promise<CompileResult> => success(request, ++compiles),
    setParameter: (
      request: SetParameterRequest,
    ): SetParameterResult | Promise<SetParameterResult> => ({
      status: "success",
      internalSubsteps: 1,
      snapshot: {
        metadata: {
          runId: request.runId,
          documentRevision: request.documentRevision,
          layoutVersion: 1,
          simulationStep: 1n,
          simulationTimeSeconds: 0.001,
          frameSequence: 1n,
        },
        values: [],
      },
      diagnostics: [],
    }),
    nextRun: () => ++compiles,
  };
  const adapter = new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request) => script.compile(request),
    setParameter: (request) => script.setParameter(request),
  });
  return { adapter, script };
}

function controllerFor(adapter: PredeterminedEngineAdapter) {
  let revision = 0;
  return new SimulationController(adapter, symbols, () => `rev-${++revision}`);
}

function edited(base: Schematic, x: number): Schematic {
  return {
    ...base,
    components: base.components.map((component) =>
      component.id === R1
        ? { ...component, position: { x, y: component.position.y } }
        : component,
    ),
  };
}

function withVoltage(base: Schematic, volts: number): Schematic {
  return {
    ...base,
    components: base.components.map((component) =>
      component.id === V1 && component.typeId === "dc_voltage_source"
        ? {
            ...component,
            parameters: { ...component.parameters, voltageVolts: volts },
          }
        : component,
    ),
  };
}

function withResistance(base: Schematic, ohms: number): Schematic {
  return {
    ...base,
    components: base.components.map((component) =>
      component.id === R1 && component.typeId === "resistor"
        ? { ...component, parameters: { resistanceOhms: ohms } }
        : component,
    ),
  };
}

describe("keeping the Engine in step with the document", () => {
  it("recompiles the whole document after a component edit", async () => {
    const { adapter } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);
    const moved = edited(dividerSchematic, 6);

    await controller.syncDocument(moved);

    expect(adapter.compileRequests.map((request) => request.schematic)).toEqual(
      [dividerSchematic, moved],
    );
    expect(controller.getState()).toMatchObject({
      status: "ready",
      updating: false,
      run: { runId: runId(2) },
    });
    // The new run streams; the old stream was stopped.
    expect(adapter.startedRuns).toEqual([runId(1), runId(2)]);
    expect(adapter.stopCount).toBe(1);
  });

  it("does nothing for the document it already has", async () => {
    const { adapter } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);

    await controller.syncDocument(dividerSchematic);

    expect(adapter.compileRequests).toHaveLength(1);
  });

  it("sends a lone parameter edit as a lightweight update", async () => {
    const { adapter } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);

    await controller.syncDocument(withResistance(dividerSchematic, 3000), {
      componentId: R1,
      parameterId: "resistanceOhms",
      value: 3000,
    });

    expect(adapter.compileRequests).toHaveLength(1);
    expect(adapter.parameterRequests).toHaveLength(1);
    expect(adapter.parameterRequests[0]?.value).toBe(3000);
  });

  it("recompiles a parameter edit that follows an unsent change", async () => {
    const { adapter, script } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);
    const gate = deferred<CompileResult>();
    script.compile = () => gate.promise;

    const moved = edited(dividerSchematic, 6);
    const first = controller.syncDocument(moved);
    const second = controller.syncDocument(withResistance(moved, 3000), {
      componentId: R1,
      parameterId: "resistanceOhms",
      value: 3000,
    });
    script.compile = (request) => success(request, script.nextRun());
    gate.resolve(success(adapter.compileRequests[1] as CompileRequest, 90));
    await Promise.all([first, second]);

    expect(adapter.parameterRequests).toEqual([]);
    expect(adapter.compileRequests.at(-1)?.schematic).toEqual(
      withResistance(moved, 3000),
    );
    expect(controller.getState().status).toBe("ready");
  });

  it("compiles only the newest document when edits pile up", async () => {
    const { adapter, script } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);
    const gate = deferred<CompileResult>();
    script.compile = () => gate.promise;

    const one = edited(dividerSchematic, 5);
    const two = edited(dividerSchematic, 6);
    const three = edited(dividerSchematic, 7);
    const pending = controller.syncDocument(one);
    void controller.syncDocument(two);
    void controller.syncDocument(three);
    script.compile = (request) => success(request, 50);
    gate.resolve(success(adapter.compileRequests[1] as CompileRequest, 40));
    await pending;

    expect(adapter.compileRequests.map((request) => request.schematic)).toEqual(
      [dividerSchematic, one, three],
    );
    // The answer for `one` was superseded and never shown.
    expect(controller.getState().run?.runId).toBe(runId(50));
    expect(adapter.startedRuns).toEqual([runId(1), runId(50)]);
  });

  it("withdraws the result but keeps diagnostics when the Engine rejects an edit", async () => {
    const { adapter, script } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);
    script.compile = () => ({ status: "failure", diagnostics: [floating] });

    await controller.syncDocument(edited(dividerSchematic, 6));

    expect(controller.getState()).toMatchObject({
      status: "failed",
      run: null,
      snapshot: null,
      diagnostics: [floating],
      updating: false,
    });
    expect(adapter.stopCount).toBe(1);

    script.compile = (request) => success(request, 7);
    await controller.syncDocument(edited(dividerSchematic, 4));

    expect(controller.getState()).toMatchObject({
      status: "ready",
      diagnostics: [],
      run: { runId: runId(7) },
    });
  });

  it("recompiles after a rejected edit even for a parameter change", async () => {
    const { adapter, script } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);
    script.compile = () => ({ status: "failure", diagnostics: [floating] });
    const broken = edited(dividerSchematic, 6);
    await controller.syncDocument(broken);
    script.compile = (request) => success(request, 8);

    await controller.syncDocument(withResistance(broken, 3000), {
      componentId: R1,
      parameterId: "resistanceOhms",
      value: 3000,
    });

    expect(adapter.parameterRequests).toEqual([]);
    expect(adapter.compileRequests).toHaveLength(3);
  });

  describe("after a parameter update did not reach the Engine", () => {
    const edit = (
      componentId: string,
      parameterId: "resistanceOhms" | "voltageVolts",
      value: number,
    ) => ({ componentId, parameterId, value });

    const rejected = (): SetParameterResult => ({
      status: "failure",
      diagnostics: [floating],
    });

    it("compiles the whole document at once, without waiting for another edit", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      script.setParameter = rejected;
      const first = withResistance(dividerSchematic, 3000);

      await controller.syncDocument(first, edit(R1, "resistanceOhms", 3000));

      expect(adapter.parameterRequests).toHaveLength(1);
      expect(
        adapter.compileRequests.map((request) => request.schematic),
      ).toEqual([dividerSchematic, first]);
      expect(controller.getState()).toMatchObject({
        status: "ready",
        updating: false,
        run: { runId: runId(2) },
        diagnostics: [],
        error: null,
      });
    });

    it("sends lightweight updates again once the Engine has the document", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      const original = script.setParameter;
      script.setParameter = rejected;
      await controller.syncDocument(
        withResistance(dividerSchematic, 3000),
        edit(R1, "resistanceOhms", 3000),
      );
      script.setParameter = original;

      await controller.syncDocument(
        withResistance(dividerSchematic, 4700),
        edit(R1, "resistanceOhms", 4700),
      );

      expect(adapter.parameterRequests).toHaveLength(2);
      expect(adapter.compileRequests).toHaveLength(2);
    });

    it("does not send a queued update after a failed one, and compiles both edits", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      const answer = deferred<SetParameterResult>();
      script.setParameter = () => answer.promise;

      const resized = withResistance(dividerSchematic, 3000);
      const both = withVoltage(resized, 9);
      const first = controller.syncDocument(
        resized,
        edit(R1, "resistanceOhms", 3000),
      );
      const second = controller.syncDocument(both, edit(V1, "voltageVolts", 9));
      answer.resolve(rejected());
      await Promise.all([first, second]);

      // The 9 V edit alone would have produced a clean result without the
      // 3 kΩ edit that the document contains.
      expect(adapter.parameterRequests).toHaveLength(1);
      expect(adapter.compileRequests.at(-1)?.schematic).toEqual(both);
      expect(controller.getState()).toMatchObject({
        status: "ready",
        diagnostics: [],
        run: { runId: runId(2) },
      });
    });

    it("keeps the rejection visible when the Engine rejects the whole document too", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      script.setParameter = rejected;
      script.compile = () => ({ status: "failure", diagnostics: [floating] });

      await controller.syncDocument(
        withResistance(dividerSchematic, 3000),
        edit(R1, "resistanceOhms", 3000),
      );

      // No result is shown as current for a document the Engine refused.
      expect(controller.getState()).toMatchObject({
        status: "failed",
        run: null,
        snapshot: null,
        diagnostics: [floating],
        updating: false,
      });
    });

    it("reports an unreachable Engine instead of an old result", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      script.setParameter = () =>
        Promise.reject(new Error("The bridge is down."));
      script.compile = () => Promise.reject(new Error("The bridge is down."));

      await controller.syncDocument(
        withResistance(dividerSchematic, 3000),
        edit(R1, "resistanceOhms", 3000),
      );

      expect(controller.getState()).toMatchObject({
        status: "unavailable",
        error: "The bridge is down.",
        updating: false,
      });
      // One resync, not a retry loop.
      expect(adapter.compileRequests).toHaveLength(2);
    });

    it("resyncs after an answer that cannot be confirmed", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      const real = script.setParameter;
      script.setParameter = (request) => {
        const answer = real(request) as Extract<
          SetParameterResult,
          { status: "success" }
        >;

        return {
          ...answer,
          snapshot: {
            ...answer.snapshot,
            metadata: { ...answer.snapshot.metadata, runId: runId(77) },
          },
        };
      };

      await controller.syncDocument(
        withResistance(dividerSchematic, 3000),
        edit(R1, "resistanceOhms", 3000),
      );

      expect(adapter.compileRequests).toHaveLength(2);
      expect(controller.getState()).toMatchObject({
        status: "ready",
        error: null,
        run: { runId: runId(2) },
      });
    });

    it("does not deadlock with a document edit that was waiting for the update", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      const answer = deferred<SetParameterResult>();
      script.setParameter = () => answer.promise;

      const resized = withResistance(dividerSchematic, 3000);
      const update = controller.syncDocument(
        resized,
        edit(R1, "resistanceOhms", 3000),
      );
      const moved = edited(resized, 6);
      const recompile = controller.syncDocument(moved);
      answer.resolve(rejected());
      await Promise.all([update, recompile]);

      expect(adapter.compileRequests.at(-1)?.schematic).toEqual(moved);
      expect(adapter.compileRequests).toHaveLength(2);
      expect(controller.getState().status).toBe("ready");
    });

    it("leaves a direct setParameter call alone, as it is not tied to a document", async () => {
      const { adapter, script } = scriptedAdapter();
      const controller = controllerFor(adapter);
      await controller.start(dividerSchematic);
      script.setParameter = rejected;

      await controller.setParameter(R1, "resistanceOhms", 3000);

      expect(adapter.compileRequests).toHaveLength(1);
      expect(controller.getState().diagnostics).toEqual([floating]);
    });
  });

  it("compiles the newest document synced before the catalog arrived", async () => {
    const { adapter } = scriptedAdapter();
    const controller = controllerFor(adapter);
    const moved = edited(dividerSchematic, 6);

    const started = controller.start(dividerSchematic);
    await controller.syncDocument(moved);
    await started;

    expect(adapter.compileRequests.map((request) => request.schematic)).toEqual(
      [moved],
    );
  });

  it("reports an unreachable backend and tries again on the next edit", async () => {
    const { adapter, script } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);
    script.compile = () => Promise.reject(new Error("The bridge is down."));

    await controller.syncDocument(edited(dividerSchematic, 6));

    expect(controller.getState()).toMatchObject({
      status: "unavailable",
      error: "The bridge is down.",
      updating: false,
    });

    script.compile = (request) => success(request, 9);
    await controller.syncDocument(edited(dividerSchematic, 7));

    expect(controller.getState()).toMatchObject({
      status: "ready",
      run: { runId: runId(9) },
    });
  });

  it("lets an in-flight parameter answer settle before replacing the run", async () => {
    const { adapter, script } = scriptedAdapter();
    const controller = controllerFor(adapter);
    await controller.start(dividerSchematic);
    const answer = deferred<SetParameterResult>();
    const defaultAnswer = script.setParameter;
    script.setParameter = () => answer.promise;

    const resized = withResistance(dividerSchematic, 3000);
    const update = controller.syncDocument(resized, {
      componentId: R1,
      parameterId: "resistanceOhms",
      value: 3000,
    });
    const moved = edited(resized, 6);
    const recompile = controller.syncDocument(moved);

    // The compile waits for the parameter answer.
    await Promise.resolve();
    expect(adapter.compileRequests).toHaveLength(1);

    answer.resolve(
      await defaultAnswer(adapter.parameterRequests[0] as SetParameterRequest),
    );
    await Promise.all([update, recompile]);

    expect(adapter.compileRequests.at(-1)?.schematic).toEqual(moved);
    expect(controller.getState()).toMatchObject({
      status: "ready",
      run: { runId: runId(2) },
    });
  });
});
