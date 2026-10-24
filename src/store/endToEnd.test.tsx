import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";
import expected from "../../tests/golden/stage-1-divider.expected.json";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import { symbols } from "../symbols";
import { Inspector } from "../ui/Inspector";
import { ResultPanel } from "../ui/ResultPanel";
import { TauriEngineAdapter } from "./engineAdapter";
import {
  DIVIDER_OUTPUT_COMPONENT_ID,
  componentLabels,
  dividerSchematic,
} from "./fixtures";
import {
  SimulationController,
  readSlot,
  type SimulationState,
} from "./simulation";

// The end-to-end path for issue #10. `crates/app/tests/bridge_session.rs` runs the real
// Rust Engine through the real bridge code and records what crossed the boundary: the
// requests, the JSON replies, and the binary frames. This file replays that recording
// through the real TypeScript adapter, decoder, controller, and UI components, with only
// the Tauri API itself replaced by a replay of the recording. Nothing in between is
// mocked, and the numbers are the ones the Engine produced on this machine.
//
// `pnpm test` runs the Rust tests first, which write the recording. Running Vitest alone
// needs the recording to exist already; it is skipped locally when absent and required
// in CI.

interface Step {
  name: string;
  command: string;
  request: Record<string, unknown>;
  reply: Record<string, unknown>;
  frameHex: string;
}

interface Session {
  catalog: ComponentCatalog;
  steps: Step[];
}

const sessionPath =
  process.env.OPEN_CIRCUIT_BRIDGE_SESSION ??
  fileURLToPath(
    new URL("../../target/tmp/bridge-session.json", import.meta.url),
  );
const recorded = existsSync(sessionPath);

if (!recorded && process.env.CI) {
  throw new Error(
    `The recorded Engine session ${sessionPath} is missing. \`pnpm test\` writes it by running the Rust tests first.`,
  );
}

const session: Session = recorded
  ? (JSON.parse(readFileSync(sessionPath, "utf8")) as Session)
  : { catalog: { componentTypes: [] }, steps: [] };

// A stand-in for the Tauri JavaScript API that answers from the recording.
const tauri = vi.hoisted(() => {
  class FakeChannel {
    onmessage: (message: unknown) => void = () => {};
  }

  return {
    FakeChannel,
    state: {
      channel: null as InstanceType<typeof FakeChannel> | null,
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
      tauri.state.channel = args?.onFrame as InstanceType<
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

const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const SOURCE = "00000000-0000-4000-8000-000000000001";

function step(name: string): Step {
  const found = session.steps.find((candidate) => candidate.name === name);

  if (!found) {
    throw new Error(`The recorded session has no step ${name}.`);
  }

  return found;
}

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

/** Answers each command from the recording, as the real backend answered it. */
function replay(overrides: Partial<Record<string, unknown>> = {}) {
  tauri.state.answer = (command, args) => {
    if (command in overrides) {
      return overrides[command];
    }

    switch (command) {
      case "get_component_catalog":
        return session.catalog;
      case "compile_circuit":
        return step("compile").reply;
      case "set_parameter": {
        const request = args?.request as { documentRevision: string };
        const match = session.steps.find(
          (candidate) =>
            candidate.command === "set_parameter" &&
            candidate.request.documentRevision === request.documentRevision,
        );

        if (!match) {
          throw new Error(
            `No recorded answer for revision ${request.documentRevision}.`,
          );
        }

        return match.reply;
      }
      default:
        return undefined;
    }
  };
}

function sentRequests(command: string): unknown[] {
  return tauri.state.calls
    .filter((call) => call.command === command)
    .map((call) => call.args?.request);
}

async function boot(overrides: Partial<Record<string, unknown>> = {}) {
  replay(overrides);
  const controller = new SimulationController(
    new TauriEngineAdapter(),
    dividerSchematic,
    symbols,
    revisions(),
  );
  await controller.start();

  return controller;
}

function deliver(frameHex: string): void {
  tauri.state.channel?.onmessage(fromHex(frameHex));
}

function screen(state: SimulationState): string {
  const labels = componentLabels(state.schematic);

  return renderToStaticMarkup(
    <>
      <Inspector state={state} labels={labels} onCommit={() => {}} />
      <ResultPanel
        state={state}
        labels={labels}
        outputComponentId={DIVIDER_OUTPUT_COMPONENT_ID}
      />
    </>,
  );
}

beforeEach(() => {
  tauri.state.channel = null;
  tauri.state.calls = [];
});

describe.skipIf(!recorded)("the real Engine session, end to end", () => {
  it("shows the engine's divider result after the compile", async () => {
    const controller = await boot();
    const markup = screen(controller.getState());

    expect(controller.getState().status).toBe("ready");
    expect(markup).toContain("Vout (across R2)");
    expect(markup).toContain("<strong>2.499875 V</strong>");
    expect(markup).toContain("R1 voltage");
    expect(markup).toContain("V1 current");
    expect(markup).toContain("-2.499875 mA");
    expect(markup.match(/value="1 kΩ"/g)).toHaveLength(2);
    expect(markup).not.toContain("NaN");
  });

  it("sends the engine exactly the request it accepted when it was recorded", async () => {
    await boot();

    expect(sentRequests("compile_circuit")).toEqual([step("compile").request]);
  });

  it("opens the frame stream for the run the engine created", async () => {
    await boot();
    const start = tauri.state.calls.find(
      (call) => call.command === "start_simulation",
    );
    const reply = step("compile").reply as Extract<
      CompileResult,
      { status: "success" }
    >;

    expect(start?.args?.runId).toBe(reply.runId);
  });

  it("applies the engine's binary frames and acknowledges them", async () => {
    const controller = await boot();

    deliver(step("compile").frameHex);
    const state = controller.getState();

    expect(state.snapshot?.metadata.frameSequence).toBeGreaterThan(0n);
    expect(state.discardedFrames).toBe(0);
    expect(screen(state)).toContain("Simulation time 0.001 s");
    expect(
      tauri.state.calls.filter((call) => call.command === "acknowledge_frame"),
    ).toHaveLength(1);
  });

  it("keeps every raw value within 1 µV and 1 nA of the golden result", async () => {
    const controller = await boot();
    deliver(step("compile").frameHex);
    const { run, snapshot } = controller.getState();
    const golden = expected.equalResistors;

    const near = (
      actual: number | undefined,
      wanted: number,
      tolerance: number,
    ) => {
      expect(actual).toBeDefined();
      expect(Math.abs((actual ?? Number.NaN) - wanted)).toBeLessThanOrEqual(
        tolerance,
      );
    };

    near(
      readSlot(run, snapshot, R1, "voltage"),
      golden.resistor1VoltageVolts,
      1e-6,
    );
    near(
      readSlot(run, snapshot, R2, "voltage"),
      golden.resistor2VoltageVolts,
      1e-6,
    );
    near(
      readSlot(run, snapshot, R1, "current"),
      golden.sourceCurrentAmps,
      1e-9,
    );
    near(
      readSlot(run, snapshot, R2, "current"),
      golden.sourceCurrentAmps,
      1e-9,
    );
    // The source discharges, so its current leaves pin1: negative by the contract.
    near(
      readSlot(run, snapshot, SOURCE, "current"),
      -golden.sourceCurrentAmps,
      1e-9,
    );
  });

  it("agrees with the engine on units, directions, and IDs", async () => {
    const controller = await boot();
    const layout = controller.getState().run?.layout;
    const componentIds = new Set(dividerSchematic.components.map((c) => c.id));

    expect(layout?.slots.length).toBe(6);

    for (const slot of layout?.slots ?? []) {
      expect(componentIds.has(slot.componentId)).toBe(true);
      expect(slot.unit).toBe(slot.quantity === "voltage" ? "volt" : "amp");
      expect(slot.signConvention).toBe(
        slot.quantity === "voltage"
          ? "voltagePin1MinusPin2"
          : "currentIntoPin1",
      );
    }
  });

  it("recalculates through the engine when R1 changes to 3000 and again when restored", async () => {
    const controller = await boot();
    deliver(step("compile").frameHex);

    await controller.setParameter(R1, "resistanceOhms", 3000);
    deliver(step("r1-to-3000").frameHex);
    let markup = screen(controller.getState());

    expect(markup).toContain("<strong>1.249969 V</strong>");
    expect(markup).toContain("3.749906 V");
    expect(controller.getState().error).toBeNull();
    expect(sentRequests("set_parameter")).toEqual([step("r1-to-3000").request]);

    await controller.setParameter(R1, "resistanceOhms", 1000);
    deliver(step("r1-restored").frameHex);
    markup = screen(controller.getState());

    expect(markup).toContain("<strong>2.499875 V</strong>");
    expect(sentRequests("set_parameter")).toEqual([
      step("r1-to-3000").request,
      step("r1-restored").request,
    ]);
  });

  it("drops a real frame of the old revision after an edit", async () => {
    const controller = await boot();
    deliver(step("compile").frameHex);
    await controller.setParameter(R1, "resistanceOhms", 3000);
    deliver(step("r1-to-3000").frameHex);

    const before = controller.getState();
    deliver(step("compile").frameHex); // a late frame of revision rev-1

    const after = controller.getState();
    expect(after.snapshot).toBe(before.snapshot);
    expect(after.discardedFrames).toBe(before.discardedFrames + 1);
    expect(screen(after)).toContain("<strong>1.249969 V</strong>");
  });

  it("shows the engine's refusal of an invalid value as a diagnostic, never as a number", async () => {
    const controller = await boot();
    await controller.setParameter(R1, "resistanceOhms", 3000);
    await controller.setParameter(R1, "resistanceOhms", 1000);
    const before = controller.getState();

    await controller.setParameter(R1, "resistanceOhms", 0);
    const after = controller.getState();
    const markup = screen(after);

    expect(sentRequests("set_parameter").at(-1)).toEqual(
      step("r1-zero-rejected").request,
    );
    expect(markup).toContain("A component parameter is out of range.");
    expect(markup).toContain("(R1)");
    expect(markup).toContain("<strong>2.499875 V</strong>");
    expect(markup).not.toContain("NaN");
    expect(after.snapshot).toBe(before.snapshot);
    expect(after.status).toBe("ready");
  });

  it("refuses a corrupted binary frame instead of showing it", async () => {
    const controller = await boot();
    const before = controller.getState().snapshot;
    const corrupted = new Uint8Array(fromHex(step("compile").frameHex));
    corrupted[0] = 0x58;

    tauri.state.channel?.onmessage(corrupted.buffer);

    const state = controller.getState();
    expect(state.snapshot).toBe(before);
    expect(state.error).toMatch(/unusable/);
    expect(screen(state)).not.toContain("NaN");
    expect(
      tauri.state.calls.filter((call) => call.command === "acknowledge_frame"),
    ).toHaveLength(0);
  });

  it("never turns a non-finite engine value into a reading", async () => {
    // JSON cannot carry NaN; a serializer turns it into null.
    const reply = structuredClone(step("compile").reply) as {
      initialSnapshot: { values: (number | null)[] };
    };
    reply.initialSnapshot.values[1] = null;

    const controller = await boot({ compile_circuit: reply });
    const state = controller.getState();
    const markup = screen(state);

    expect(state.status).toBe("unavailable");
    expect(state.snapshot).toBeNull();
    expect(markup).toContain("Simulation unavailable");
    expect(markup).not.toContain("Vout");
    expect(markup).not.toContain("NaN");
  });

  it("stops the stream when the screen is closed", async () => {
    const controller = await boot();
    const reply = step("compile").reply as Extract<
      CompileResult,
      { status: "success" }
    >;

    controller.dispose();

    const stop = tauri.state.calls.find(
      (call) => call.command === "stop_simulation",
    );
    expect(stop?.args).toEqual({ runId: reply.runId });
  });
});
