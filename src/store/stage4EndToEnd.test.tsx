import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import motorExpected from "../../tests/golden/stage-4-pmdc-motor.expected.json";
import meterExpected from "../../tests/golden/stage-4-meters-loading.expected.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import type { SetParameterResult } from "../generated/SetParameterResult";
import type { TraceRequest } from "../generated/TraceRequest";
import { symbols } from "../symbols";
import { DevicePanel } from "../ui/DevicePanel";
import { Oscilloscope } from "../ui/Oscilloscope";
import { ResultPanel } from "../ui/ResultPanel";
import { TauriEngineAdapter } from "./engineAdapter";
import { componentLabels } from "./fixtures";
import {
  readSlot,
  SimulationController,
  type SimulationState,
} from "./simulation";
import { OscilloscopeController, type ScopeState } from "./traces";

// The Stage 4 acceptance (issue #69), as the user sees it. `crates/app/tests/
// stage4_session.rs` compiles the circuits that `stage4Drawing.test.ts` draws from a blank
// document with the real Rust Engine, runs them, and records the exchange: the JSON
// results, the binary OCF2 frames with the device states, and the OCT1 trace responses.
// This file replays that recording through the real TypeScript adapter, decoders,
// controllers, and panels; only the Tauri API itself is a replay. The states and numbers on
// screen are the ones the Engine produced on this machine.
//
// `pnpm test` runs the Rust tests first, which write the recording. Running Vitest alone
// needs the recording to exist already; it is skipped locally when absent and required in CI.

interface FrameRecord {
  step: number;
  revision?: string;
  hex: string;
}

interface Circuit {
  request: CompileRequest;
  compileReply: CompileResult & { status: "success" };
}

interface Session {
  catalog: ComponentCatalog;
  relay: Circuit & {
    frames: FrameRecord[];
    updates: {
      afterStep: number;
      request: unknown;
      reply: SetParameterResult;
    }[];
  };
  fuse: Circuit & {
    frames: FrameRecord[];
    traceRequest: TraceRequest;
    traceResponseHex: string;
    secondCompileReply: CompileResult & { status: "success" };
    secondFrames: FrameRecord[];
  };
  motor: Circuit & {
    frames: FrameRecord[];
    traceRequest: TraceRequest;
    traceResponseHex: string;
  };
  voltmeter: Circuit;
  ammeter: Circuit;
}

const sessionPath =
  process.env.OPEN_CIRCUIT_STAGE4_SESSION ??
  fileURLToPath(
    new URL("../../target/tmp/stage4-session.json", import.meta.url),
  );
const recorded = existsSync(sessionPath);

if (!recorded && process.env.CI) {
  throw new Error(
    `The recorded Engine session ${sessionPath} is missing. \`pnpm test\` writes it by running the Rust tests first.`,
  );
}

const session = (
  recorded ? JSON.parse(readFileSync(sessionPath, "utf8")) : {}
) as Session;

const tauri = vi.hoisted(() => {
  class FakeChannel {
    onmessage: (message: unknown) => void = () => {};
  }

  return {
    FakeChannel,
    state: {
      frameChannel: null as InstanceType<typeof FakeChannel> | null,
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

    if (command === "start_simulation") {
      tauri.state.frameChannel = args?.onFrame as InstanceType<
        typeof tauri.FakeChannel
      >;
    }

    try {
      return Promise.resolve(tauri.state.answer(command, args));
    } catch (error) {
      return Promise.reject(error);
    }
  },
}));

const SOURCE = "00000000-0000-4000-8000-000000000001";
const DEVICE = "00000000-0000-4000-8000-000000000002";
const OTHER = "00000000-0000-4000-8000-000000000003";

function fromHex(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = parseInt(hex.slice(2 * index, 2 * index + 2), 16);
  }

  return bytes.buffer;
}

function revisions(): () => string {
  let next = 0;
  return () => `rev-${(next += 1)}`;
}

/** The compile reply for a later compile: the same circuit answered with the new revision. */
function withRevision(
  reply: CompileResult & { status: "success" },
  documentRevision: string,
): CompileResult {
  return {
    ...reply,
    initialSnapshot: {
      ...reply.initialSnapshot,
      metadata: { ...reply.initialSnapshot.metadata, documentRevision },
    },
  };
}

interface Script {
  /** The circuit a compile request is answered for, by the device it contains. */
  compile: (request: CompileRequest, count: number) => CompileResult;
  updates?: SetParameterResult[];
  trace?: string;
}

function replay(script: Script): void {
  let compiles = 0;
  const updates = [...(script.updates ?? [])];

  tauri.state.answer = (command, args) => {
    switch (command) {
      case "get_component_catalog":
        return session.catalog;
      case "compile_circuit":
        compiles += 1;
        return script.compile(args?.request as CompileRequest, compiles);
      case "set_parameter": {
        const reply = updates.shift();

        if (!reply) {
          throw new Error("No recorded answer for this update.");
        }

        return reply;
      }
      case "read_traces":
        (args?.onTrace as InstanceType<typeof tauri.FakeChannel>).onmessage(
          fromHex(script.trace ?? ""),
        );
        return undefined;
      default:
        return undefined;
    }
  };
}

function deliver(frame: FrameRecord): void {
  tauri.state.frameChannel?.onmessage(fromHex(frame.hex));
}

async function boot(schematic: Schematic) {
  const adapter = new TauriEngineAdapter();
  const simulation = new SimulationController(adapter, symbols, revisions());
  const scope = new OscilloscopeController(adapter, simulation);

  await simulation.start(schematic);
  scope.start();
  await vi.advanceTimersByTimeAsync(0);

  return { simulation, scope };
}

function devices(state: SimulationState, schematic: Schematic): string {
  return renderToStaticMarkup(
    <DevicePanel
      state={state}
      schematic={schematic}
      labels={componentLabels(schematic)}
    />,
  );
}

function results(state: SimulationState, schematic: Schematic): string {
  return renderToStaticMarkup(
    <ResultPanel state={state} labels={componentLabels(schematic)} />,
  );
}

function oscilloscope(
  state: SimulationState,
  scope: ScopeState,
  schematic: Schematic,
): string {
  return renderToStaticMarkup(
    <Oscilloscope
      simulation={state}
      scope={scope}
      labels={componentLabels(schematic)}
      onToggleSignal={() => {}}
      onWindowChange={() => {}}
    />,
  );
}

/** The same circuit with one part moved: any other edit recompiles, so a new run starts. */
function moved(schematic: Schematic): Schematic {
  const [first, ...rest] = schematic.components;

  return {
    ...schematic,
    components: [
      {
        ...first!,
        position: { x: first!.position.x + 1, y: first!.position.y },
      },
      ...rest,
    ],
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  tauri.state.calls = [];
  tauri.state.frameChannel = null;
});

afterEach(() => {
  vi.useRealTimers();
});

describe.skipIf(!recorded)("the self-latching relay on the real Engine", () => {
  const relay = () => session.relay;
  const frame = (step: number) =>
    relay().frames.find((candidate) => candidate.step === step)!;
  const reply = (index: number) => relay().updates[index]!.reply;

  it("shows the relay picked at time zero, as the Engine's initialization decided", async () => {
    replay({ compile: () => relay().compileReply });
    const schematic = relay().request.schematic;
    const { simulation } = await boot(schematic);
    const html = devices(simulation.getState(), schematic);

    expect(simulation.getState().status).toBe("ready");
    expect(html).toContain("K1 SPDT relay");
    expect(html).toContain('device-badge-on">Picked');
    expect(html).toContain("Last change</dt><dd>since 0 s");
    expect(html).not.toMatch(/NaN|Infinity/);
  });

  it("follows the fixture: picked, held after the start path opens, released after the supply goes", async () => {
    replay({
      compile: () => relay().compileReply,
      updates: [reply(0), reply(1)],
    });
    const schematic = relay().request.schematic;
    const { simulation } = await boot(schematic);
    const coil = () =>
      readSlot(
        simulation.getState().run,
        simulation.getState().snapshot,
        DEVICE,
        "current",
      );

    for (const step of [1, 2]) deliver(frame(step));
    expect(simulation.getState().snapshot?.metadata.simulationStep).toBe(2n);

    // 2 ms: the start path is opened with a live update, as the inspector would send it.
    await simulation.setParameter(OTHER, "resistanceOhms", 1e12);
    for (const step of [3, 4, 5]) deliver(frame(step));
    const held = devices(simulation.getState(), schematic);

    expect(held).toContain('device-badge-on">Picked');
    expect(held).toContain("Coil current</dt><dd>49.95");
    expect(coil()).toBeCloseTo(0.04995004995, 3);

    // The supply is removed: the relay lets go, and the panel says so.
    deliver(frame(6));
    await simulation.setParameter(SOURCE, "voltageVolts", 0);
    for (const step of [7, 8, 9, 10]) deliver(frame(step));
    const released = devices(simulation.getState(), schematic);

    expect(released).toContain('device-badge-off">Released');
    expect(released).toMatch(/Last change<\/dt><dd>since 5\.\d+ ms/);
    expect(released).toMatch(/Coil current<\/dt><dd>\d+(\.\d+)? [pnµ]?A/);
    expect(Math.abs(coil() ?? 1)).toBeLessThan(1e-4);
    expect(simulation.getState().snapshot?.metadata.simulationStep).toBe(10n);
  });

  it("does not let a frame from before the update overwrite the state after it", async () => {
    replay({
      compile: () => relay().compileReply,
      updates: [reply(0), reply(1)],
    });
    const schematic = relay().request.schematic;
    const { simulation } = await boot(schematic);

    for (const step of [1, 2]) deliver(frame(step));
    await simulation.setParameter(OTHER, "resistanceOhms", 1e12);
    for (const step of [3, 4, 5, 6]) deliver(frame(step));
    await simulation.setParameter(SOURCE, "voltageVolts", 0);
    const before = simulation.getState().discardedFrames;

    // Step 6 as it was before the update arrives late: it still shows a picked relay.
    deliver(frame(6));

    expect(simulation.getState().discardedFrames).toBe(before + 1);
    expect(devices(simulation.getState(), schematic)).toContain(
      'device-badge-off">Released',
    );
  });

  it("starts a new run from the Engine's initial state when the document changes", async () => {
    replay({
      compile: (_request, count) =>
        count === 1
          ? relay().compileReply
          : withRevision(relay().compileReply, "rev-4"),
      updates: [reply(0), reply(1)],
    });
    const schematic = relay().request.schematic;
    const { simulation } = await boot(schematic);

    for (const step of [1, 2]) deliver(frame(step));
    await simulation.setParameter(OTHER, "resistanceOhms", 1e12);
    for (const step of [3, 4, 5, 6]) deliver(frame(step));
    await simulation.setParameter(SOURCE, "voltageVolts", 0);
    for (const step of [7, 8, 9, 10]) deliver(frame(step));
    expect(devices(simulation.getState(), schematic)).toContain("Released");

    // Any structural edit recompiles. The panel shows the new run's own initial state, not
    // the released relay of the run it replaces, and late frames of the old run are refused.
    const edited = moved(schematic);
    await simulation.syncDocument(edited);
    const fresh = devices(simulation.getState(), edited);

    expect(simulation.getState().snapshot?.metadata.simulationStep).toBe(0n);
    expect(fresh).toContain('device-badge-on">Picked');
    expect(fresh).toContain("since 0 s");
    expect(fresh).not.toContain("Released");

    const discarded = simulation.getState().discardedFrames;
    deliver(frame(10));

    expect(simulation.getState().discardedFrames).toBe(discarded + 1);
    expect(devices(simulation.getState(), edited)).toContain("Picked");
  });
});

describe.skipIf(!recorded)("the fuse on the real Engine", () => {
  const fuse = () => session.fuse;

  it("shows the fuse intact, trips it at 1 ms, and shows the open path", async () => {
    replay({ compile: () => fuse().compileReply });
    const schematic = fuse().request.schematic;
    const { simulation } = await boot(schematic);
    const start = devices(simulation.getState(), schematic);

    expect(start).toContain("F1 Fuse");
    expect(start).toContain('device-badge-on">Intact');
    expect(start).toContain("0 A²·s of 0.004 A²·s");
    expect(start).toContain("Current</dt><dd>2 A");

    deliver(fuse().frames[0]!);
    const tripped = devices(simulation.getState(), schematic);

    expect(tripped).toContain('device-badge-fault">Tripped');
    expect(tripped).toContain("Tripped</dt><dd>since 1 ms");
    expect(tripped).toContain("0.004 A²·s of 0.004 A²·s");

    deliver(fuse().frames[1]!);
    deliver(fuse().frames[2]!);
    const open = devices(simulation.getState(), schematic);
    const readings = results(simulation.getState(), schematic);

    expect(open).toContain("Current</dt><dd>0 A");
    // The whole source voltage is across the open fuse, and the load sees none.
    expect(readings).toContain("F1 voltage</dt><dd>2.02 V");
    expect(readings).toContain("R1 current</dt><dd>0 A");
    expect(readings).toContain("F1 accumulated I²t</dt><dd>0.004 A²·s");
  });

  it("starts intact again after a new compile, and trips again", async () => {
    replay({
      compile: (_request, count) =>
        count === 1 ? fuse().compileReply : fuse().secondCompileReply,
    });
    const schematic = fuse().request.schematic;
    const { simulation } = await boot(schematic);

    for (const recordedFrame of fuse().frames) deliver(recordedFrame);
    expect(devices(simulation.getState(), schematic)).toContain("Tripped");

    const edited = moved(schematic);
    await simulation.syncDocument(edited);
    const reset = devices(simulation.getState(), edited);

    expect(simulation.getState().run?.runId).not.toBe(
      fuse().compileReply.runId,
    );
    expect(reset).toContain('device-badge-on">Intact');
    expect(reset).toContain("0 A²·s of 0.004 A²·s");
    expect(reset).not.toContain("Tripped");

    // The new run's own first frame trips it again, at the same time.
    tauri.state.frameChannel?.onmessage(fromHex(fuse().secondFrames[0]!.hex));
    expect(devices(simulation.getState(), edited)).toContain(
      "Tripped</dt><dd>since 1 ms",
    );
  });

  it("traces the accumulated I²t and the current, in the units the Engine names", async () => {
    replay({
      compile: () => fuse().compileReply,
      trace: fuse().traceResponseHex,
    });
    const schematic = fuse().request.schematic;
    const { simulation, scope } = await boot(schematic);

    for (const key of fuse().traceRequest.signalKeys) scope.toggleSignal(key);
    await vi.advanceTimersByTimeAsync(0);
    const data = scope.getState().data;

    expect(scope.getState().error).toBeNull();
    expect(data?.signals.map((signal) => signal.unit)).toEqual([
      "ampere_squared_second",
      "amp",
    ]);
    // 0, 1, 2, and 3 ms: the accumulator reaches its threshold and stays there.
    expect(
      data?.signals[0]?.maximums.map((value) => Number(value.toFixed(9))),
    ).toEqual([0, 0.004, 0.004, 0.004]);
    expect(data?.signals[1]?.maximums.at(-1)).toBeCloseTo(0, 9);

    const html = oscilloscope(
      simulation.getState(),
      scope.getState(),
      schematic,
    );
    expect(html).toContain("Accumulated I²t (A²·s)");
    expect(html).toContain("F1 accumulated I²t");
    expect(html).not.toMatch(/NaN|Infinity/);
  });
});

describe.skipIf(!recorded)("the motor on the real Engine", () => {
  const motor = () => session.motor;
  const reference = motorExpected.reference;

  it("shows the motor at rest, then at the known-answer operating point at 8 s", async () => {
    replay({ compile: () => motor().compileReply });
    const schematic = motor().request.schematic;
    const { simulation } = await boot(schematic);

    expect(devices(simulation.getState(), schematic)).toContain(
      "Angular velocity</dt><dd>0 rad/s",
    );

    deliver(motor().frames.find((candidate) => candidate.step === 8000)!);
    const { run, snapshot } = simulation.getState();
    const speed = readSlot(run, snapshot, DEVICE, "angularVelocity")!;
    const current = readSlot(run, snapshot, DEVICE, "current")!;

    expect(snapshot?.metadata.simulationStep).toBe(8000n);
    expect(
      Math.abs(speed - reference.angularVelocityRadiansPerSecond),
    ).toBeLessThanOrEqual(reference.angularVelocityToleranceRadiansPerSecond);
    expect(
      Math.abs(current - reference.armatureCurrentAmps),
    ).toBeLessThanOrEqual(reference.armatureCurrentToleranceAmps);

    const html = devices(simulation.getState(), schematic);
    expect(html).toMatch(/Angular velocity<\/dt><dd>33\.33\d+ rad\/s/);
    expect(html).toMatch(/Armature current<\/dt><dd>3\.333\d+ A/);
    expect(html).toContain("Terminal voltage</dt><dd>10 V");
  });

  it("traces the motor's speed in rad/s up to the same value the frame shows", async () => {
    replay({
      compile: () => motor().compileReply,
      trace: motor().traceResponseHex,
    });
    const schematic = motor().request.schematic;
    const { simulation, scope } = await boot(schematic);

    deliver(motor().frames.find((candidate) => candidate.step === 8000)!);
    for (const key of motor().traceRequest.signalKeys) scope.toggleSignal(key);
    await vi.advanceTimersByTimeAsync(0);
    const data = scope.getState().data;
    const speed = data?.signals[0];
    const { run, snapshot } = simulation.getState();

    expect(scope.getState().error).toBeNull();
    expect(speed?.unit).toBe("radian_per_second");
    expect(speed?.times.at(-1)).toBeCloseTo(8, 9);
    expect(speed?.maximums.at(-1)).toBe(
      readSlot(run, snapshot, DEVICE, "angularVelocity"),
    );

    const html = oscilloscope(
      simulation.getState(),
      scope.getState(),
      schematic,
    );
    expect(html).toContain("Angular velocity (rad/s)");
    expect(html).toContain("M1 angular velocity");
  });
});

describe.skipIf(!recorded)("the meters on the real Engine", () => {
  it("shows the voltmeter's reading and the current it draws", async () => {
    replay({ compile: () => session.voltmeter.compileReply });
    const schematic = session.voltmeter.request.schematic;
    const { simulation } = await boot(schematic);
    const expected = meterExpected.voltmeterCircuit;
    const { run, snapshot } = simulation.getState();

    expect(
      Math.abs(
        readSlot(run, snapshot, DEVICE, "voltage")! -
          expected.expectedReadingVolts,
      ),
    ).toBeLessThanOrEqual(expected.readingToleranceVolts);
    expect(
      Math.abs(
        readSlot(run, snapshot, DEVICE, "current")! -
          expected.expectedInputCurrentAmps,
      ),
    ).toBeLessThanOrEqual(expected.currentToleranceAmps);

    const html = devices(simulation.getState(), schematic);
    expect(html).toContain("VM1 Voltmeter");
    expect(html).toContain("Reading</dt><dd>9.090909 V");
    expect(html).toContain("Input current</dt><dd>909.0909 nA");
  });

  it("shows the ammeter's reading and the burden voltage it adds", async () => {
    replay({ compile: () => session.ammeter.compileReply });
    const schematic = session.ammeter.request.schematic;
    const { simulation } = await boot(schematic);
    const expected = meterExpected.ammeterCircuit;
    const { run, snapshot } = simulation.getState();

    expect(
      Math.abs(
        readSlot(run, snapshot, DEVICE, "current")! -
          expected.expectedReadingAmps,
      ),
    ).toBeLessThanOrEqual(expected.readingToleranceAmps);
    expect(
      Math.abs(
        readSlot(run, snapshot, DEVICE, "voltage")! -
          expected.expectedBurdenVoltageVolts,
      ),
    ).toBeLessThanOrEqual(expected.voltageToleranceVolts);

    const html = devices(simulation.getState(), schematic);
    expect(html).toContain("AM1 Ammeter");
    expect(html).toContain("Reading</dt><dd>909.0909 mA");
    expect(html).toContain("Burden voltage</dt><dd>909.0909 mV");
  });

  it("never shows a meter before the Engine has answered", () => {
    const schematic = session.voltmeter.request.schematic;
    const html = renderToStaticMarkup(
      <DevicePanel
        state={{
          status: "loading",
          catalog: null,
          run: null,
          snapshot: null,
          diagnostics: [],
          error: null,
          updating: false,
          discardedFrames: 0,
        }}
        schematic={schematic}
        labels={componentLabels(schematic)}
      />,
    );

    expect(html).toContain("Waiting for the engine");
    expect(html).not.toContain("Reading");
  });
});
