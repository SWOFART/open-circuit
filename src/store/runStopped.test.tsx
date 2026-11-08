import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { SetParameterResult } from "../generated/SetParameterResult";
import type { StreamNotice } from "../generated/StreamNotice";
import { ResultPanel } from "../ui/ResultPanel";
import { dividerSchematic } from "./fixtures";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { SimulationController } from "./simulation";

const RUN_ID = "00000000-0000-4000-8000-0000000000a0";
const DIODE = "00000000-0000-4000-8000-000000000002";
const R1 = "00000000-0000-4000-8000-000000000003";

const layout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 1,
  slots: [
    {
      key: `${R1}:current`,
      componentId: R1,
      quantity: "current",
      unit: "amp",
      signConvention: "currentIntoPin1",
      offset: 0,
    },
  ],
};

function snapshot(revision: string, sequence: bigint): ResultSnapshot {
  return {
    metadata: {
      runId: RUN_ID,
      documentRevision: revision,
      layoutVersion: 1,
      simulationStep: 40n,
      simulationTimeSeconds: 0.04,
      frameSequence: sequence,
    },
    values: [0.004],
    states: [],
  };
}

const nonConvergence: Diagnostic = {
  code: "NonConvergence",
  severity: "error",
  localizationKey: "diagnostic.nonConvergence",
  references: [{ kind: "component", componentId: DIODE }],
};

function notice(overrides: Partial<StreamNotice> = {}): StreamNotice {
  return {
    kind: "runStopped",
    runId: RUN_ID,
    documentRevision: "rev-1",
    simulationStep: 40,
    diagnostics: [nonConvergence],
    ...overrides,
  } as StreamNotice;
}

function setUp(
  answer: () => SetParameterResult = () => ({
    status: "failure",
    diagnostics: [],
  }),
) {
  const adapter = new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request) => ({
      status: "success",
      runId: RUN_ID,
      internalSubsteps: 1,
      layout,
      initialSnapshot: snapshot(request.documentRevision, 0n),
      diagnostics: [],
    }),
    setParameter: answer,
  });
  let next = 0;
  const controller = new SimulationController(adapter, undefined, () => {
    next += 1;
    return `rev-${next}`;
  });

  return { adapter, controller };
}

describe("a run that stops on an Engine error", () => {
  it("shows the stop of the run and revision on screen and keeps the last snapshot", async () => {
    const { adapter, controller } = setUp();
    await controller.start(dividerSchematic);
    adapter.emitFrame(snapshot("rev-1", 5n));
    const before = controller.getState().snapshot;

    adapter.emitNotice(notice());

    const state = controller.getState();
    expect(state.stopped).toEqual(notice());
    expect(state.snapshot).toBe(before);
    expect(state.status).toBe("ready");
    expect(state.diagnostics).toEqual([nonConvergence]);
    expect(state.discardedFrames).toBe(0);
  });

  it("drops a notice of another run or of another revision", async () => {
    const { adapter, controller } = setUp();
    await controller.start(dividerSchematic);

    adapter.emitNotice(
      notice({ runId: "00000000-0000-4000-8000-0000000000bb" }),
    );
    adapter.emitNotice(notice({ documentRevision: "rev-0" }));

    const state = controller.getState();
    expect(state.stopped).toBeNull();
    expect(state.diagnostics).toEqual([]);
    expect(state.discardedFrames).toBe(2);
  });

  it("ends the stop when the circuit is compiled again", async () => {
    const { adapter, controller } = setUp();
    await controller.start(dividerSchematic);
    adapter.emitNotice(notice());
    expect(controller.getState().stopped).not.toBeNull();

    // Any change that is not a lone parameter edit starts a new run.
    await controller.syncDocument({
      ...dividerSchematic,
      wires: [
        {
          id: "00000000-0000-4000-8000-0000000000c1",
          points: [
            { x: 0, y: 0 },
            { x: 2, y: 0 },
          ],
        },
      ],
    });

    expect(adapter.compileRequests).toHaveLength(2);
    expect(controller.getState().stopped).toBeNull();
  });

  it("opens the stream again when an update succeeds, and says nothing more", async () => {
    const { adapter, controller } = setUp(() => ({
      status: "success",
      internalSubsteps: 1,
      snapshot: snapshot("rev-2", 7n),
      diagnostics: [],
    }));
    await controller.start(dividerSchematic);
    adapter.emitNotice(notice());
    expect(adapter.startedRuns).toHaveLength(1);

    await controller.setParameter(R1, "resistanceOhms", 2000);

    const state = controller.getState();
    expect(state.stopped).toBeNull();
    expect(state.run?.documentRevision).toBe("rev-2");
    // The ended stream was closed and a new one opened for the same run.
    expect(adapter.stopCount).toBe(1);
    expect(adapter.startedRuns).toEqual([RUN_ID, RUN_ID]);
  });

  it("keeps the stop, and the stream closed, when an update is refused", async () => {
    const { adapter, controller } = setUp(() => ({
      status: "failure",
      diagnostics: [nonConvergence],
    }));
    await controller.start(dividerSchematic);
    adapter.emitNotice(notice());

    await controller.setParameter(R1, "resistanceOhms", 2000);

    expect(controller.getState().stopped).not.toBeNull();
    expect(adapter.startedRuns).toHaveLength(1);
    expect(adapter.stopCount).toBe(0);
  });

  it("is shown in words, with the diagnostic, and the last result stays", async () => {
    const { adapter, controller } = setUp();
    await controller.start(dividerSchematic);
    adapter.emitFrame(snapshot("rev-1", 5n));
    adapter.emitNotice(notice());

    const html = renderToStaticMarkup(
      <ResultPanel state={controller.getState()} labels={new Map()} />,
    );

    expect(html).toContain("The simulation stopped at step 40.");
    expect(html).toContain("The nonlinear circuit did not converge.");
    // The last committed result is still on screen.
    expect(html).toContain("Simulation time 0.040 s");
  });
});
