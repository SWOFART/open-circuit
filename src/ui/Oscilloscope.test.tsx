import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import rcCharge from "../../tests/golden/stage-3-rc-charge.schematic.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ResultLayout } from "../generated/ResultLayout";
import type { Schematic } from "../generated/Schematic";
import type { TraceBatch } from "../generated/TraceBatch";
import type { TraceRequest } from "../generated/TraceRequest";
import { componentLabels } from "../store/fixtures";
import {
  PredeterminedEngineAdapter,
  PredeterminedTraceMissingError,
} from "../store/mockEngineAdapter";
import {
  initialSimulationState,
  SimulationController,
  type SimulationState,
} from "../store/simulation";
import {
  MAX_SCOPE_SIGNALS,
  OscilloscopeController,
  SCOPE_WINDOWS_SECONDS,
  type ScopeData,
  type ScopeSignalData,
  type ScopeState,
} from "../store/traces";
import { Oscilloscope } from "./Oscilloscope";

const schematic = rcCharge as Schematic;
const labels = componentLabels(schematic);
const RUN_ID = "00000000-0000-4000-8000-0000000000a0";
const SOURCE = "00000000-0000-4000-8000-000000000001";
const RESISTOR = "00000000-0000-4000-8000-000000000002";
const CAPACITOR = "00000000-0000-4000-8000-000000000003";

function slot(
  componentId: string,
  quantity: "voltage" | "current",
  offset: number,
) {
  return {
    key: `${componentId}:${quantity}`,
    componentId,
    quantity,
    unit: quantity === "voltage" ? ("volt" as const) : ("amp" as const),
    signConvention: "voltagePin1MinusPin2" as const,
    offset,
  };
}

const layout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 6,
  slots: [
    slot(SOURCE, "voltage", 0),
    slot(SOURCE, "current", 1),
    slot(RESISTOR, "voltage", 2),
    slot(RESISTOR, "current", 3),
    slot(CAPACITOR, "voltage", 4),
    slot(CAPACITOR, "current", 5),
  ],
};

const VC = `${CAPACITOR}:voltage`;
const IC = `${CAPACITOR}:current`;
const VR = `${RESISTOR}:voltage`;

function readySimulation(runId = RUN_ID, revision = "rev-1"): SimulationState {
  return {
    ...initialSimulationState(),
    status: "ready",
    catalog: catalogFixture as ComponentCatalog,
    run: { runId, layout, documentRevision: revision },
  };
}

/** Charging curve of the RC fixture (tau = 10 ms), as test data, one sample per step. */
function signalData(
  key: string,
  unit: ScopeSignalData["unit"],
  count: number,
  value: (seconds: number) => number,
): ScopeSignalData {
  const times = Array.from({ length: count }, (_, step) => step * 0.001);
  const values = times.map(value);

  return { key, unit, times, minimums: values, maximums: values };
}

function dataFor(
  signals: ScopeSignalData[],
  overrides: Partial<ScopeData> = {},
): ScopeData {
  return {
    runId: RUN_ID,
    layoutVersion: 1,
    documentRevision: "rev-1",
    sampleIntervalSeconds: 0.001,
    latestTimeSeconds: 0.1,
    samplesPerPoint: 1,
    signals,
    ...overrides,
  };
}

const chargeVoltage = (seconds: number) => 5 * (1 - Math.exp(-seconds / 0.01));
const chargeCurrent = (seconds: number) => 0.005 * Math.exp(-seconds / 0.01);

function scopeState(overrides: Partial<ScopeState> = {}): ScopeState {
  return {
    selectedKeys: [],
    windowSeconds: 0.1,
    data: null,
    error: null,
    loading: false,
    ...overrides,
  };
}

function render(simulation: SimulationState, scope: ScopeState): string {
  return renderToStaticMarkup(
    <Oscilloscope
      simulation={simulation}
      scope={scope}
      labels={labels}
      onToggleSignal={() => {}}
      onWindowChange={() => {}}
    />,
  );
}

function tracing(): ScopeState {
  return scopeState({
    selectedKeys: [VC, IC],
    data: dataFor([
      signalData(VC, "volt", 101, chargeVoltage),
      signalData(IC, "amp", 101, chargeCurrent),
    ]),
  });
}

describe("signal selection", () => {
  it("offers every voltage and current of the run's layout by component name", () => {
    const html = render(readySimulation(), scopeState());

    for (const name of ["V1", "R1", "C1"]) {
      expect(html).toContain(`${name} voltage`);
      expect(html).toContain(`${name} current`);
    }
    expect(html.match(/type="checkbox"/g)).toHaveLength(layout.slots.length);
  });

  it("checks the selected signals only", () => {
    const html = render(readySimulation(), tracing());

    expect(html.match(/checked=""/g)).toHaveLength(2);
  });

  it("gives each selected signal its own colour, in the list and the legend", () => {
    const html = render(readySimulation(), tracing());

    expect(html).toContain("#60a5fa");
    expect(html).toContain("#f59e0b");
  });

  it("offers every window of the contract and shows the chosen one", () => {
    const html = render(readySimulation(), scopeState({ windowSeconds: 0.5 }));

    for (const seconds of SCOPE_WINDOWS_SECONDS) {
      expect(html).toContain(`value="${seconds}"`);
    }
    expect(html).toMatch(/<option value="0.5" selected="">500 ms<\/option>/);
    expect(html).toContain("10 s");
  });

  it("stops offering more signals at the limit", () => {
    const many: ResultLayout = {
      layoutVersion: 1,
      valueCount: 10,
      slots: Array.from({ length: 10 }, (_, index) =>
        slot(
          `00000000-0000-4000-8000-0000000000${index + 10}`,
          "voltage",
          index,
        ),
      ),
    };
    const html = renderToStaticMarkup(
      <Oscilloscope
        simulation={{
          ...readySimulation(),
          run: { runId: RUN_ID, layout: many, documentRevision: "rev-1" },
        }}
        scope={scopeState({
          selectedKeys: many.slots
            .slice(0, MAX_SCOPE_SIGNALS)
            .map((candidate) => candidate.key),
        })}
        labels={labels}
        onToggleSignal={() => {}}
        onWindowChange={() => {}}
      />,
    );

    // The two signals that are not selected can no longer be added.
    expect(html.match(/disabled=""/g)).toHaveLength(2);
    expect(html).toContain(`At most ${MAX_SCOPE_SIGNALS} signals`);
  });

  it("does not disable anything below the limit", () => {
    expect(render(readySimulation(), tracing())).not.toContain("disabled");
  });
});

describe("the plot", () => {
  it("draws one trace per signal, one lane per unit", () => {
    const html = render(readySimulation(), tracing());

    expect(html.match(/<svg/g)).toHaveLength(2);
    expect(html.match(/<polyline/g)).toHaveLength(2);
    expect(html).toContain("Voltage (V)");
    expect(html).toContain("Current (A)");
    expect(html).toContain("Voltage traces: C1 voltage");
    expect(html).toContain("Current traces: C1 current");
  });

  it("puts two signals of the same unit on one axis", () => {
    const html = render(
      readySimulation(),
      scopeState({
        selectedKeys: [VC, VR],
        data: dataFor([
          signalData(VC, "volt", 101, chargeVoltage),
          signalData(VR, "volt", 101, (t) => 5 - chargeVoltage(t)),
        ]),
      }),
    );

    expect(html.match(/<svg/g)).toHaveLength(1);
    expect(html.match(/<polyline/g)).toHaveLength(2);
  });

  it("labels the time axis in seconds with an SI prefix and the value axes with units", () => {
    const html = render(readySimulation(), tracing());

    expect(html).toMatch(/>0 s</);
    expect(html).toMatch(/>\d+ ms</);
    expect(html).toMatch(/>-?\d+(\.\d+)? V</);
    expect(html).toMatch(/>-?\d+(\.\d+)? mA</);
  });

  it("states the visible window and the sample interval", () => {
    const html = render(readySimulation(), tracing());

    expect(html).toContain("Time since the run started, 0 s to 100 ms.");
    expect(html).toContain("One sample every 1 ms.");
  });

  it("slides the window with the newest sample once the run is older than it", () => {
    const html = render(
      readySimulation(),
      scopeState({
        selectedKeys: [VC],
        windowSeconds: 0.01,
        data: dataFor(
          [
            {
              key: VC,
              unit: "volt",
              times: [0.995, 1],
              minimums: [4, 5],
              maximums: [4, 5],
            },
          ],
          { latestTimeSeconds: 1 },
        ),
      }),
    );

    expect(html).toContain("990 ms to 1 s.");
    expect(html).toContain("<polyline");
  });

  it("shows the minimum and maximum band when points stand for several samples", () => {
    const envelope = dataFor(
      [
        {
          key: VC,
          unit: "volt",
          times: [0, 0.05, 0.1],
          minimums: [0, 1, 2],
          maximums: [1, 3, 5],
        },
      ],
      { samplesPerPoint: 50 },
    );
    const html = render(
      readySimulation(),
      scopeState({ selectedKeys: [VC], data: envelope }),
    );

    expect(html).toContain("<polygon");
    expect(html).toContain("Each point stands for 50 samples");
    expect(html).toContain("minimum and maximum");
  });

  it("draws exact samples without a band", () => {
    const html = render(readySimulation(), tracing());

    expect(html).not.toContain("<polygon");
    expect(html).not.toContain("Each point stands for");
  });

  it("draws a single sample as a point", () => {
    const html = render(
      readySimulation(),
      scopeState({
        selectedKeys: [VC],
        data: dataFor(
          [
            {
              key: VC,
              unit: "volt",
              times: [0],
              minimums: [2.5],
              maximums: [2.5],
            },
          ],
          { latestTimeSeconds: 0 },
        ),
      }),
    );

    expect(html).toContain("<circle");
    expect(html).not.toContain("<polyline");
  });

  it("never writes NaN or infinity into the plot", () => {
    const flat = render(
      readySimulation(),
      scopeState({
        selectedKeys: [VC],
        data: dataFor([signalData(VC, "volt", 101, () => 0)]),
      }),
    );

    for (const html of [flat, render(readySimulation(), tracing())]) {
      expect(html).not.toMatch(/NaN|Infinity/);
    }
  });
});

describe("empty and error states", () => {
  it("waits while the backend starts", () => {
    for (const status of ["idle", "loading"] as const) {
      const html = render(
        { ...initialSimulationState(), status },
        scopeState(),
      );

      expect(html).toContain("Waiting for the simulation backend");
      expect(html).not.toContain("<svg");
    }
  });

  it("says why there are no traces when the engine is unavailable", () => {
    const html = render(
      {
        ...initialSimulationState(),
        status: "unavailable",
        error: "The Rust engine is only available in the desktop app.",
      },
      scopeState(),
    );

    expect(html).toContain('role="alert"');
    expect(html).toContain(
      "Traces unavailable: The Rust engine is only available",
    );
    expect(html).toContain("No signals available.");
  });

  it("says there is no run when the circuit does not compile", () => {
    const html = render(
      { ...initialSimulationState(), status: "failed", run: null },
      scopeState({ selectedKeys: [VC], data: tracing().data }),
    );

    expect(html).toContain("No active run");
    expect(html).not.toContain("<svg");
  });

  it("asks for a signal before tracing anything", () => {
    const html = render(readySimulation(), scopeState());

    expect(html).toContain("Select a signal to trace it.");
    expect(html).not.toContain("<svg");
  });

  it("waits for samples after a signal is chosen", () => {
    const html = render(readySimulation(), scopeState({ selectedKeys: [VC] }));

    expect(html).toContain("Waiting for samples");
    expect(html).not.toContain("<svg");
  });

  it("says so when the run has produced no samples yet", () => {
    const html = render(
      readySimulation(),
      scopeState({
        selectedKeys: [VC],
        data: dataFor(
          [{ key: VC, unit: "volt", times: [], minimums: [], maximums: [] }],
          { latestTimeSeconds: null },
        ),
      }),
    );

    expect(html).toContain("has not produced samples");
    expect(html).not.toContain("<svg");
  });

  it("reports a failed trace read as an alert", () => {
    const html = render(
      readySimulation(),
      scopeState({
        selectedKeys: [VC],
        error: "The engine did not deliver the trace.",
      }),
    );

    expect(html).toContain('role="alert"');
    expect(html).toContain(
      "Trace read failed: The engine did not deliver the trace.",
    );
  });

  it("keeps the earlier trace of the same run visible beside a read failure", () => {
    const html = render(readySimulation(), {
      ...tracing(),
      error: "The engine did not deliver the trace.",
    });

    expect(html).toContain("<svg");
    expect(html).toContain("Trace read failed");
  });
});

describe("a trace never shown for another circuit", () => {
  it("draws nothing for data of another run", () => {
    const state = tracing();
    const other = readySimulation("00000000-0000-4000-8000-0000000000b2");
    const html = render(other, state);

    expect(html).not.toContain("<svg");
    expect(html).toContain("Waiting for samples");
  });

  it("draws nothing for data of another layout version", () => {
    const state = tracing();
    const html = render(
      {
        ...readySimulation(),
        run: {
          runId: RUN_ID,
          layout: { ...layout, layoutVersion: 2 },
          documentRevision: "rev-1",
        },
      },
      state,
    );

    expect(html).not.toContain("<svg");
  });

  it("says the trace is from the last accepted circuit while an edit is waiting for the engine", () => {
    const waiting = render({ ...readySimulation(), updating: true }, tracing());
    const settled = render(readySimulation(), tracing());

    expect(waiting).toContain("has not accepted the latest change yet");
    expect(waiting).toContain("last circuit it accepted");
    expect(settled).not.toContain("has not accepted");
  });

  it("does not claim a pending edit when there is no trace to qualify", () => {
    const html = render(
      { ...readySimulation(), updating: true },
      scopeState({ selectedKeys: [VC] }),
    );

    expect(html).not.toContain("has not accepted");
  });

  it("still draws after a live parameter update, which keeps the run", () => {
    const html = render(readySimulation(RUN_ID, "rev-2"), tracing());

    expect(html).toContain("<svg");
  });
});

describe("the oscilloscope following a real simulation", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function dividerLikeResponses(runs: string[]) {
    let compiles = 0;

    return new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: (request) => {
        const runId = runs[Math.min(compiles, runs.length - 1)]!;
        compiles += 1;

        return {
          status: "success",
          internalSubsteps: 1,
          runId,
          layout,
          initialSnapshot: {
            metadata: {
              runId,
              documentRevision: request.documentRevision,
              layoutVersion: 1,
              simulationStep: 0n,
              simulationTimeSeconds: 0,
              frameSequence: 0n,
            },
            values: layout.slots.map(() => 0),
            states: [],
          },
          diagnostics: [],
        };
      },
      setParameter: () => ({ status: "failure", diagnostics: [] }),
      traces: (request: TraceRequest): TraceBatch => {
        if (request.query.kind !== "history") {
          throw new PredeterminedTraceMissingError();
        }

        const steps = [0, 1, 2, 3];

        return {
          metadata: {
            runId: request.runId,
            documentRevision: "rev",
            layoutVersion: request.layoutVersion,
            sampleIntervalSeconds: 0.001,
            historySamples: 10000,
            oldestSampleStep: 0n,
            latestSampleStep: 3n,
            firstSampleStep: 0n,
            lastSampleStep: 3n,
            sampleCount: 4,
            missedSamples: 0n,
            replaced: false,
          },
          signals: request.signalKeys.map((key) => ({
            key,
            unit: key.endsWith("current")
              ? ("amp" as const)
              : ("volt" as const),
          })),
          points: steps.map((step) => ({
            simulationStep: BigInt(step),
            minimumValues: request.signalKeys.map(() => step),
            maximumValues: request.signalKeys.map(() => step),
          })),
        };
      },
    });
  }

  it("clears the plot when the document changes and a new run replaces the old one", async () => {
    const RUN_B = "00000000-0000-4000-8000-0000000000b2";
    const adapter = dividerLikeResponses([RUN_ID, RUN_B]);
    const simulation = new SimulationController(adapter);
    const scope = new OscilloscopeController(adapter, simulation);

    await simulation.start(schematic);
    scope.start();
    scope.toggleSignal(VC);
    await vi.advanceTimersByTimeAsync(0);

    const before = renderToStaticMarkup(
      <Oscilloscope
        simulation={simulation.getState()}
        scope={scope.getState()}
        labels={labels}
        onToggleSignal={() => {}}
        onWindowChange={() => {}}
      />,
    );
    expect(before).toContain("<svg");

    // Any other document change recompiles and starts a new run at time zero.
    await simulation.syncDocument({
      ...schematic,
      components: [...schematic.components],
    });

    expect(scope.getState().data?.runId).not.toBe(RUN_ID);
    const afterReset = renderToStaticMarkup(
      <Oscilloscope
        simulation={simulation.getState()}
        scope={scope.getState()}
        labels={labels}
        onToggleSignal={() => {}}
        onWindowChange={() => {}}
      />,
    );
    // No trace of the first run is on screen, even before the new run's samples come.
    expect(afterReset).not.toContain("<svg");
    expect(afterReset).toContain("Waiting for samples");
    await vi.advanceTimersByTimeAsync(0);
    expect(scope.getState().data?.runId).toBe(RUN_B);

    scope.dispose();
    simulation.dispose();
  });
});
