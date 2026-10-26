import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import expected from "../../tests/golden/stage-3-rc-charge.expected.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import type { SetParameterResult } from "../generated/SetParameterResult";
import type { TraceRequest } from "../generated/TraceRequest";
import { symbols } from "../symbols";
import { Oscilloscope } from "../ui/Oscilloscope";
import { ResultPanel } from "../ui/ResultPanel";
import { SchematicDocumentStore } from "./document";
import { TauriEngineAdapter } from "./engineAdapter";
import { componentLabels } from "./fixtures";
import type { ProjectFilePort } from "./projectFiles";
import { blankSchematic, ProjectWorkflow } from "./projectWorkflow";
import { SimulationController, type SimulationState } from "./simulation";
import { OscilloscopeController, type ScopeState } from "./traces";

// The Stage 3 acceptance (issue #50), as the user sees it. `crates/app/tests/
// stage3_session.rs` compiles the circuit that `stage3Drawing.test.ts` draws from a blank
// document with the real Rust Engine, runs it, reads the traces through the bridge's
// encoder, and records the exchange. This file replays that recording through the real
// TypeScript adapter, decoder, controllers, and panels: only the Tauri API itself is a
// replay. The numbers on screen are the ones the Engine produced on this machine.
//
// `pnpm test` runs the Rust tests first, which write the recording. Running Vitest alone
// needs the recording to exist already; it is skipped locally when absent and required
// in CI.

interface Session {
  catalog: ComponentCatalog;
  rc: {
    request: CompileRequest;
    compileReply: CompileResult & { status: "success" };
    traceRequest: TraceRequest;
    traceResponseHex: string;
  };
  conflict: {
    request: CompileRequest;
    compileReply: CompileResult & { status: "failure" };
  };
}

const sessionPath =
  process.env.OPEN_CIRCUIT_STAGE3_SESSION ??
  fileURLToPath(
    new URL("../../target/tmp/stage3-session.json", import.meta.url),
  );
const recorded = existsSync(sessionPath);

if (!recorded && process.env.CI) {
  throw new Error(
    `The recorded Engine session ${sessionPath} is missing. \`pnpm test\` writes it by running the Rust tests first.`,
  );
}

const session = (
  recorded
    ? JSON.parse(readFileSync(sessionPath, "utf8"))
    : { rc: {}, conflict: {} }
) as Session;

const tauri = vi.hoisted(() => {
  class FakeChannel {
    onmessage: (message: unknown) => void = () => {};
  }

  return {
    FakeChannel,
    state: {
      calls: [] as {
        command: string;
        args: Record<string, unknown> | undefined;
      }[],
      answer: (() => undefined) as (
        command: string,
        args?: Record<string, unknown>,
      ) => unknown,
    },
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  Channel: tauri.FakeChannel,
  isTauri: () => true,
  invoke: (command: string, args?: Record<string, unknown>) => {
    tauri.state.calls.push({ command, args });

    try {
      return Promise.resolve(tauri.state.answer(command, args));
    } catch (error) {
      return Promise.reject(error);
    }
  },
}));

const RC_RUN = () => session.rc.compileReply.runId;
const NEW_RUN = "00000000-0000-4000-8000-0000000000e9";

function fromHex(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = parseInt(hex.slice(2 * index, 2 * index + 2), 16);
  }

  return bytes.buffer;
}

function revisions(): () => string {
  let next = 0;
  return () => {
    next += 1;
    return `rev-${next}`;
  };
}

/**
 * The recorded compile answer for the request: the Engine's own reply for the drawn
 * circuit, or for the conflicting one. Another run of the same circuit gets a new run
 * ID, as the Engine gives one to every compile.
 */
function compileReply(
  request: CompileRequest,
  runId = RC_RUN(),
): CompileResult {
  if (
    request.schematic.components.length !==
    session.rc.request.schematic.components.length
  ) {
    return session.conflict.compileReply;
  }

  return {
    ...session.rc.compileReply,
    runId,
    initialSnapshot: {
      ...session.rc.compileReply.initialSnapshot,
      metadata: {
        ...session.rc.compileReply.initialSnapshot.metadata,
        runId,
        documentRevision: request.documentRevision,
      },
    },
  };
}

interface Replay {
  /** The next `set_parameter` is held until the test answers it. */
  holdParameter?: boolean;
  parameterResolvers: ((result: SetParameterResult) => void)[];
  /** Run IDs for the compiles after the first, in order. */
  nextRuns: string[];
}

function replay(options: Partial<Replay> = {}): Replay {
  const state: Replay = {
    parameterResolvers: [],
    nextRuns: [],
    ...options,
  };
  let compiles = 0;

  tauri.state.answer = (command, args) => {
    switch (command) {
      case "get_component_catalog":
        return session.catalog;
      case "compile_circuit": {
        const request = args?.request as CompileRequest;
        compiles += 1;

        return compileReply(
          request,
          compiles === 1 ? RC_RUN() : (state.nextRuns.shift() ?? NEW_RUN),
        );
      }
      case "read_traces":
        (args?.onTrace as InstanceType<typeof tauri.FakeChannel>).onmessage(
          fromHex(session.rc.traceResponseHex),
        );
        return undefined;
      case "set_parameter":
        return state.holdParameter
          ? new Promise<SetParameterResult>((resolve) => {
              state.parameterResolvers.push(resolve);
            })
          : { status: "failure", diagnostics: [] };
      default:
        return undefined;
    }
  };

  return state;
}

const rcSchematic = () => session.rc.request.schematic;
const conflictSchematic = () => session.conflict.request.schematic;

async function boot(schematic: Schematic) {
  const adapter = new TauriEngineAdapter();
  const simulation = new SimulationController(adapter, symbols, revisions());
  const scope = new OscilloscopeController(adapter, simulation);

  await simulation.start(schematic);
  scope.start();
  await vi.advanceTimersByTimeAsync(0);

  return { simulation, scope };
}

function panels(
  state: SimulationState,
  scopeState: ScopeState,
  schematic: Schematic,
): string {
  const labels = componentLabels(schematic);

  return renderToStaticMarkup(
    <>
      <ResultPanel state={state} labels={labels} />
      <Oscilloscope
        simulation={state}
        scope={scopeState}
        labels={labels}
        onToggleSignal={() => {}}
        onWindowChange={() => {}}
      />
    </>,
  );
}

/** Selects C1's voltage and current, as the user ticks them in the panel. */
function traceCapacitor(scope: OscilloscopeController): void {
  for (const key of session.rc.traceRequest.signalKeys) {
    scope.toggleSignal(key);
  }
}

beforeEach(() => {
  vi.useFakeTimers();
  tauri.state.calls = [];
});

afterEach(() => {
  vi.useRealTimers();
});

describe.skipIf(!recorded)(
  "the drawn RC circuit on the real Engine, as the user sees it",
  () => {
    it("compiles, runs, and shows C1's charging curve on the oscilloscope", async () => {
      replay();
      const { simulation, scope } = await boot(rcSchematic());
      traceCapacitor(scope);
      await vi.advanceTimersByTimeAsync(0);

      expect(simulation.getState().status).toBe("ready");
      const data = scope.getState().data;

      expect(data?.runId).toBe(RC_RUN());
      expect(data?.signals.map((signal) => signal.key)).toEqual(
        session.rc.traceRequest.signalKeys,
      );
      expect(data?.signals.map((signal) => signal.unit)).toEqual([
        "volt",
        "amp",
      ]);
      expect(data?.sampleIntervalSeconds).toBe(0.001);
      expect(data?.samplesPerPoint).toBe(1);

      const voltage = data!.signals[0]!;
      const current = data!.signals[1]!;

      // One sample for every public step to 50 ms, at its simulation time.
      expect(voltage.times).toHaveLength(51);
      expect(voltage.times[0]).toBe(0);
      expect(voltage.times[50]).toBeCloseTo(0.05, 12);

      // The accepted values at 10, 20, and 30 ms, within 0.1% of the 10 V full scale.
      for (const point of expected.analytical) {
        const index = voltage.times.findIndex(
          (time) => Math.abs(time - point.timeSeconds) < 1e-9,
        );

        expect(index).toBeGreaterThanOrEqual(0);
        expect(
          Math.abs(voltage.maximums[index]! - point.voltageVolts),
        ).toBeLessThanOrEqual(expected.analyticalToleranceVolts);
      }

      // Every step follows 10 (1 - e^(-t/tau)) and i = (10 V / 1 kOhm) e^(-t/tau).
      voltage.times.forEach((time, index) => {
        expect(
          Math.abs(
            voltage.maximums[index]! - 10 * (1 - Math.exp(-time / 0.01)),
          ),
        ).toBeLessThanOrEqual(0.01);
        expect(
          Math.abs(current.maximums[index]! - 0.01 * Math.exp(-time / 0.01)),
        ).toBeLessThanOrEqual(1e-5);
      });
    });

    it("asks the Engine for the selected signals of the current run, bounded, and acknowledges the answer", async () => {
      replay();
      const { scope } = await boot(rcSchematic());
      traceCapacitor(scope);
      await vi.advanceTimersByTimeAsync(0);

      const read = tauri.state.calls.find(
        (call) => call.command === "read_traces",
      );
      const request = read?.args?.request as TraceRequest;

      expect(request.runId).toBe(RC_RUN());
      expect(request.layoutVersion).toBe(session.rc.traceRequest.layoutVersion);
      expect(request.signalKeys).toEqual(session.rc.traceRequest.signalKeys);
      expect(request.query.kind).toBe("history");
      expect(request.maxPoints).toBe(2048);

      const acknowledgement = tauri.state.calls.find(
        (call) => call.command === "acknowledge_trace",
      );

      expect(acknowledgement?.args).toMatchObject({ runId: RC_RUN() });
      expect(typeof acknowledgement?.args?.traceSequence).toBe("number");
    });

    it("draws the voltage and current lanes with SI axes and a rising voltage curve", async () => {
      replay();
      const { simulation, scope } = await boot(rcSchematic());
      traceCapacitor(scope);
      await vi.advanceTimersByTimeAsync(0);

      const markup = panels(
        simulation.getState(),
        scope.getState(),
        rcSchematic(),
      );

      expect(markup).toContain("Voltage (V)");
      expect(markup).toContain("Current (A)");
      expect(markup).toContain("C1 voltage");
      expect(markup).toContain("C1 current");
      expect(markup).toMatch(/>\d+ ms</);
      expect(markup).toMatch(/>-?\d+(\.\d+)? V</);
      expect(markup).toMatch(/>-?\d+(\.\d+)? mA</);
      expect(markup).not.toMatch(/NaN|Infinity/);

      // The first polyline is C1's voltage: higher on screen means a smaller y, and a
      // charging capacitor only goes up.
      const points =
        /<polyline points="([^"]+)"/.exec(markup)?.[1]?.split(" ") ?? [];
      const ys = points.map((point) => Number(point.split(",")[1]));

      expect(ys).toHaveLength(51);
      ys.slice(1).forEach((y, index) => {
        expect(y).toBeLessThanOrEqual(ys[index]!);
      });
      expect(ys[50]!).toBeLessThan(ys[0]!);
    });
  },
);

describe.skipIf(!recorded)("an incompatible combined initial state", () => {
  it("shows the Engine's structured diagnostic and keeps the drawing editable", async () => {
    replay();
    const store = new SchematicDocumentStore(conflictSchematic());
    const { simulation, scope } = await boot(store.getState().schematic);
    traceCapacitor(scope);
    await vi.advanceTimersByTimeAsync(0);
    const state = simulation.getState();

    expect(state.status).toBe("failed");
    expect(state.run).toBeNull();
    expect(state.diagnostics.map((diagnostic) => diagnostic.code)).toContain(
      "InconsistentState",
    );

    const markup = panels(state, scope.getState(), store.getState().schematic);

    expect(markup).toContain("The engine could not calculate this circuit.");
    expect(markup).toContain("C1");
    expect(markup).toContain("C2");
    expect(markup).toContain("No active run");
    expect(markup).not.toContain("<svg");

    // Nothing was done to the drawing: both capacitors and both explicit voltages remain.
    expect(store.getState().schematic).toEqual(conflictSchematic());
    expect(store.getState().isDirty).toBe(false);
  });

  it("clears the trace the moment an edit makes the circuit incompatible", async () => {
    replay();
    const { simulation, scope } = await boot(rcSchematic());
    traceCapacitor(scope);
    await vi.advanceTimersByTimeAsync(0);
    expect(scope.getState().data).not.toBeNull();

    await simulation.syncDocument(conflictSchematic());
    await vi.advanceTimersByTimeAsync(0);

    expect(simulation.getState().status).toBe("failed");
    expect(scope.getState().data).toBeNull();
    expect(
      panels(simulation.getState(), scope.getState(), conflictSchematic()),
    ).not.toContain("<svg");
  });
});

describe.skipIf(!recorded)("saving and reopening", () => {
  function memoryFiles() {
    let saved: string | null = null;
    const files: ProjectFilePort = {
      open: () =>
        Promise.resolve(
          saved === null ? null : (JSON.parse(saved) as Schematic),
        ),
      save: (schematic) => {
        saved = JSON.stringify(schematic);
        return Promise.resolve(true);
      },
      saveAs: (schematic) => {
        saved = JSON.stringify(schematic);
        return Promise.resolve(true);
      },
      clearPath: () => Promise.resolve(),
    };

    return { files, text: () => saved ?? "" };
  }

  it("keeps the schematic inputs, and nothing the Engine computed", async () => {
    replay();
    const { files, text } = memoryFiles();
    const store = new SchematicDocumentStore(rcSchematic());
    const { scope } = await boot(rcSchematic());
    traceCapacitor(scope);
    await vi.advanceTimersByTimeAsync(0);
    expect(scope.getState().data).not.toBeNull();

    await new ProjectWorkflow(store, files).save();

    const file = JSON.parse(text()) as Schematic;
    const capacitor = file.components.find(
      (item) => item.typeId === "capacitor",
    );

    expect(file).toEqual(rcSchematic());
    expect(capacitor?.typeId === "capacitor" && capacitor.parameters).toEqual({
      capacitanceFarads: 1e-5,
      initialVoltageVolts: 0,
    });
    // Neither the run, the readings, nor the oscilloscope's samples or choices are saved.
    for (const word of [
      "runId",
      "layout",
      "snapshot",
      "trace",
      "history",
      "samples",
      "window",
      "simulationStep",
      "values",
    ]) {
      expect(text()).not.toContain(word);
    }
  });

  it("reopens the same circuit, and the new run starts with an empty oscilloscope", async () => {
    replay();
    const { files } = memoryFiles();
    const store = new SchematicDocumentStore(rcSchematic());
    const workflow = new ProjectWorkflow(store, files);
    await workflow.save();

    const reopened = new SchematicDocumentStore(blankSchematic);
    await new ProjectWorkflow(reopened, files).perform("open");

    expect(reopened.getState().schematic).toEqual(rcSchematic());
    expect(reopened.getState().isDirty).toBe(false);

    // The reopened circuit is compiled afresh. Nothing carried over: no selection, no data.
    const { simulation, scope } = await boot(reopened.getState().schematic);

    expect(simulation.getState().status).toBe("ready");
    expect(simulation.getState().run?.documentRevision).toBe("rev-1");
    expect(scope.getState().selectedKeys).toEqual([]);
    expect(scope.getState().data).toBeNull();
  });
});

describe.skipIf(!recorded)(
  "a parameter edit the Engine does not accept",
  () => {
    it("never leaves a trace that looks current for the unaccepted document", async () => {
      const held = replay({ holdParameter: true });
      const store = new SchematicDocumentStore(rcSchematic());
      const { simulation, scope } = await boot(store.getState().schematic);
      traceCapacitor(scope);
      await vi.advanceTimersByTimeAsync(0);
      const firstRun = simulation.getState().run?.runId;
      expect(scope.getState().data?.runId).toBe(firstRun);

      // The user raises R1 to 2 kOhm: the document takes the edit at once, and the update
      // travels to the Engine as a lightweight parameter change that is still in flight.
      const resistor = store
        .getState()
        .schematic.components.find((item) => item.typeId === "resistor");
      if (!resistor) throw new Error("The circuit has no resistor.");
      store.setParameter(resistor.id, "resistanceOhms", 2000, session.catalog);
      const pending = simulation.syncDocument(store.getState().schematic, {
        componentId: resistor.id,
        parameterId: "resistanceOhms",
        value: 2000,
      });
      await vi.advanceTimersByTimeAsync(0);

      // While the Engine has not answered, the trace on screen is labelled as the last
      // accepted circuit's, not presented as the edited one's.
      expect(simulation.getState().updating).toBe(true);
      expect(
        panels(
          simulation.getState(),
          scope.getState(),
          store.getState().schematic,
        ),
      ).toContain("has not accepted the latest change yet");

      // The Engine refuses the update. The application resynchronizes with a full compile,
      // which starts a new run, and the old trace is gone before any sample of the new run.
      held.parameterResolvers[0]!({ status: "failure", diagnostics: [] });
      await pending;
      await vi.advanceTimersByTimeAsync(0);

      const state = simulation.getState();

      expect(state.run?.runId).toBe(NEW_RUN);
      expect(state.run?.runId).not.toBe(firstRun);
      expect(scope.getState().data).toBeNull();
      expect(store.getState().schematic.components).toContainEqual(
        expect.objectContaining({ parameters: { resistanceOhms: 2000 } }),
      );
    });

    it("does not show the first run's samples for the run that replaced it", async () => {
      replay();
      const { simulation, scope } = await boot(rcSchematic());
      traceCapacitor(scope);
      await vi.advanceTimersByTimeAsync(0);
      const first = scope.getState().data;
      expect(first?.runId).toBe(RC_RUN());

      // A structural edit recompiles: a new run, so the recorded samples of the first run
      // must not be shown for it even though the replay would answer with them again.
      const edited: Schematic = {
        ...rcSchematic(),
        referenceMarkers: [...rcSchematic().referenceMarkers],
      };
      await simulation.syncDocument(edited);
      await vi.advanceTimersByTimeAsync(0);

      expect(simulation.getState().run?.runId).toBe(NEW_RUN);
      expect(scope.getState().data).toBeNull();
      expect(scope.getState().error).toBe(
        "The engine returned a trace for a different run or selection.",
      );
    });
  },
);
