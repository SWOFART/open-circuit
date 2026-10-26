import { beforeEach, describe, expect, it, vi } from "vitest";
import frameFixture from "../../tests/golden/stage-1-frame.json";
import traceFixture from "../../tests/golden/stage-3-trace.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { TraceRequest } from "../generated/TraceRequest";
import { dividerSchematic } from "./fixtures";

// A stand-in for the Tauri JavaScript API: commands are recorded and answered by the
// test, and a channel is an object the test can push messages into.
const tauri = vi.hoisted(() => {
  type Call = { command: string; args: Record<string, unknown> | undefined };

  class FakeChannel {
    onmessage: (message: unknown) => void = () => {};
  }

  const state = {
    inTauri: true,
    calls: [] as Call[],
    answer: (() => undefined) as (
      command: string,
      args?: Record<string, unknown>,
    ) => unknown,
  };

  return { FakeChannel, state };
});

vi.mock("@tauri-apps/api/core", () => ({
  Channel: tauri.FakeChannel,
  isTauri: () => tauri.state.inTauri,
  invoke: (command: string, args?: Record<string, unknown>) => {
    tauri.state.calls.push({ command, args });

    try {
      return Promise.resolve(tauri.state.answer(command, args));
    } catch (error) {
      return Promise.reject(error);
    }
  },
}));

import {
  EngineUnavailableError,
  TauriEngineAdapter,
  TRACE_DELIVERY_TIMEOUT_MS,
} from "./engineAdapter";

const RUN_ID = "00000000-0000-4000-8000-0000000000a0";

function fromHex(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = parseInt(hex.slice(2 * index, 2 * index + 2), 16);
  }

  return bytes.buffer;
}

function snapshotJson(step: unknown, sequence: unknown) {
  return {
    metadata: {
      runId: RUN_ID,
      documentRevision: "rev-1",
      layoutVersion: 1,
      simulationStep: step,
      simulationTimeSeconds: 0,
      frameSequence: sequence,
    },
    values: [1],
  };
}

function commandsCalled(): string[] {
  return tauri.state.calls.map((call) => call.command);
}

const compileRequest: CompileRequest = {
  schematic: dividerSchematic,
  geometry: { components: [] },
  documentRevision: "rev-1",
};

beforeEach(() => {
  tauri.state.inTauri = true;
  tauri.state.calls = [];
  tauri.state.answer = () => undefined;
});

describe("commands", () => {
  it("compiles through the compile_circuit command", async () => {
    tauri.state.answer = () => ({
      status: "success",
      runId: RUN_ID,
      layout: { layoutVersion: 1, valueCount: 1, slots: [] },
      initialSnapshot: snapshotJson(0, 0),
      diagnostics: [],
    });

    await new TauriEngineAdapter().compile(compileRequest);

    expect(tauri.state.calls).toEqual([
      { command: "compile_circuit", args: { request: compileRequest } },
    ]);
  });

  it("turns the engine's JSON counters into bigint", async () => {
    tauri.state.answer = () => ({
      status: "success",
      runId: RUN_ID,
      layout: { layoutVersion: 1, valueCount: 1, slots: [] },
      initialSnapshot: snapshotJson(16, 3),
      diagnostics: [],
    });

    const result = await new TauriEngineAdapter().compile(compileRequest);

    if (result.status !== "success") {
      throw new Error("expected a successful compile");
    }
    expect(result.initialSnapshot.metadata.simulationStep).toBe(16n);
    expect(result.initialSnapshot.metadata.frameSequence).toBe(3n);
  });

  it("normalizes the counters of a parameter update too", async () => {
    tauri.state.answer = () => ({
      status: "success",
      snapshot: snapshotJson(20, 9),
      diagnostics: [],
    });

    const result = await new TauriEngineAdapter().setParameter({
      runId: RUN_ID,
      expectedDocumentRevision: "rev-1",
      documentRevision: "rev-2",
      componentId: "00000000-0000-4000-8000-000000000002",
      parameterId: "resistanceOhms",
      value: 3000,
    });

    expect(commandsCalled()).toEqual(["set_parameter"]);
    if (result.status !== "success") {
      throw new Error("expected a successful update");
    }
    expect(result.snapshot.metadata.frameSequence).toBe(9n);
  });

  it("passes engine failures through untouched", async () => {
    const failure = { status: "failure", diagnostics: [] };
    tauri.state.answer = () => failure;

    expect(await new TauriEngineAdapter().compile(compileRequest)).toEqual(
      failure,
    );
  });

  it.each([
    ["a negative counter", -1],
    ["a fractional counter", 1.5],
    ["a counter beyond the safe range", 2 ** 60],
    ["a string counter", "7"],
    ["a missing counter", undefined],
  ])("refuses %s from the engine", async (_name, bad) => {
    tauri.state.answer = () => ({
      status: "success",
      runId: RUN_ID,
      layout: { layoutVersion: 1, valueCount: 1, slots: [] },
      initialSnapshot: snapshotJson(0, bad),
      diagnostics: [],
    });

    await expect(
      new TauriEngineAdapter().compile(compileRequest),
    ).rejects.toThrow(/invalid frame sequence/);
  });

  it("reports a failing command as an unavailable engine", async () => {
    tauri.state.answer = () => {
      throw "the simulation thread is not running";
    };

    const error = await new TauriEngineAdapter()
      .compile(compileRequest)
      .catch((caught: unknown) => caught);

    expect(error).toBeInstanceOf(EngineUnavailableError);
    expect((error as Error).message).toBe(
      "the simulation thread is not running",
    );
  });

  it("refuses every call outside the desktop app", async () => {
    tauri.state.inTauri = false;
    const adapter = new TauriEngineAdapter();

    await expect(adapter.getCatalog()).rejects.toThrow(/desktop app/);
    await expect(adapter.compile(compileRequest)).rejects.toThrow(
      /desktop app/,
    );
    await expect(
      adapter.startFrames(RUN_ID, handlers().handlers),
    ).rejects.toThrow(/desktop app/);
    expect(tauri.state.calls).toEqual([]);
  });
});

function handlers() {
  const frames: ResultSnapshot[] = [];
  const errors: Error[] = [];

  return {
    frames,
    errors,
    handlers: {
      onFrame: (snapshot: ResultSnapshot) => frames.push(snapshot),
      onError: (error: Error) => errors.push(error),
    },
  };
}

/** Starts a stream and returns the channel the adapter handed to the backend. */
async function openStream(seen = handlers()) {
  const stop = await new TauriEngineAdapter().startFrames(
    RUN_ID,
    seen.handlers,
  );
  const start = tauri.state.calls.find(
    (call) => call.command === "start_simulation",
  );
  const channel = start?.args?.onFrame as InstanceType<
    typeof tauri.FakeChannel
  >;

  return { stop, channel, ...seen };
}

describe("the frame stream", () => {
  it("starts the stream for the run over a binary channel", async () => {
    const { channel } = await openStream();

    expect(tauri.state.calls[0]).toMatchObject({
      command: "start_simulation",
      args: { runId: RUN_ID },
    });
    expect(channel).toBeInstanceOf(tauri.FakeChannel);
  });

  it("decodes a binary frame, hands it on, then acknowledges its sequence", async () => {
    const { channel, frames, errors } = await openStream();

    channel.onmessage(fromHex(frameFixture.hex));

    expect(errors).toEqual([]);
    expect(frames).toHaveLength(1);
    expect(frames[0]?.metadata.frameSequence).toBe(3n);
    expect(frames[0]?.values).toEqual(frameFixture.snapshot.values);
    expect(tauri.state.calls.at(-1)).toEqual({
      command: "acknowledge_frame",
      args: { frameSequence: 3 },
    });
  });

  it("sends the acknowledgement as a plain number", async () => {
    const { channel } = await openStream();

    channel.onmessage(fromHex(frameFixture.hex));

    const ack = tauri.state.calls.at(-1);
    expect(typeof ack?.args?.frameSequence).toBe("number");
  });

  it("reports a corrupt frame and does not hand it on", async () => {
    const { channel, frames, errors } = await openStream();
    const corrupt = new Uint8Array(fromHex(frameFixture.hex));
    corrupt[0] = 0x58;

    channel.onmessage(corrupt.buffer);

    expect(frames).toEqual([]);
    expect(errors).toHaveLength(1);
    expect(errors[0]?.message).toMatch(/OCF2 magic/);
    expect(commandsCalled()).not.toContain("acknowledge_frame");
  });

  it("reports a message that is not binary", async () => {
    const { channel, frames, errors } = await openStream();

    channel.onmessage({ not: "binary" });

    expect(frames).toEqual([]);
    expect(errors[0]?.message).toMatch(/not binary/);
  });

  it("reports a failed acknowledgement", async () => {
    const seen = handlers();
    const { channel } = await openStream(seen);
    tauri.state.answer = (command) => {
      if (command === "acknowledge_frame") {
        throw "the simulation thread is not running";
      }
    };

    channel.onmessage(fromHex(frameFixture.hex));
    await vi.waitFor(() => expect(seen.errors).toHaveLength(1));

    expect(seen.errors[0]?.message).toBe(
      "the simulation thread is not running",
    );
    expect(seen.frames).toHaveLength(1);
  });

  it("stops the run's stream and ignores later frames", async () => {
    const { stop, channel, frames } = await openStream();

    stop();
    channel.onmessage(fromHex(frameFixture.hex));

    expect(tauri.state.calls.at(-1)).toEqual({
      command: "stop_simulation",
      args: { runId: RUN_ID },
    });
    expect(frames).toEqual([]);
  });

  it("stops only once however often the stop function is called", async () => {
    const { stop } = await openStream();

    stop();
    stop();

    expect(
      commandsCalled().filter((name) => name === "stop_simulation"),
    ).toHaveLength(1);
  });

  it("fails to start when the backend refuses the run", async () => {
    tauri.state.answer = () => {
      throw "the run cannot be started: Some(StaleRun)";
    };

    await expect(
      new TauriEngineAdapter().startFrames(RUN_ID, handlers().handlers),
    ).rejects.toThrow(/cannot be started/);
  });
});

const TRACE_RUN_ID = traceFixture.header.metadata.runId;

const traceRequest: TraceRequest = {
  runId: TRACE_RUN_ID,
  layoutVersion: 1,
  signalKeys: traceFixture.header.signals.map((signal) => signal.key),
  query: { kind: "history", startStep: 100, endStep: 200 },
  maxPoints: 2048,
};

type FakeChannelInstance = InstanceType<typeof tauri.FakeChannel>;

function traceChannel(): FakeChannelInstance {
  const call = tauri.state.calls.find(
    (candidate) => candidate.command === "read_traces",
  );

  return call?.args?.onTrace as FakeChannelInstance;
}

describe("trace reads", () => {
  it("asks for the traces over a binary channel and decodes the response", async () => {
    tauri.state.answer = (command, args) => {
      if (command === "read_traces") {
        // The backend sends the response before it replies to the command.
        (args?.onTrace as FakeChannelInstance).onmessage(
          fromHex(traceFixture.hex),
        );
      }
    };

    const batch = await new TauriEngineAdapter().readTraces(traceRequest);

    expect(tauri.state.calls[0]).toMatchObject({
      command: "read_traces",
      args: { request: traceRequest },
    });
    expect(traceChannel()).toBeInstanceOf(tauri.FakeChannel);
    expect(batch.metadata.runId).toBe(TRACE_RUN_ID);
    expect(batch.metadata.latestSampleStep).toBe(105n);
    expect(batch.points).toHaveLength(traceFixture.points.length);
    expect(batch.signals.map((signal) => signal.unit)).toEqual(["volt", "amp"]);
  });

  it("accepts a response that arrives after the command's own reply", async () => {
    const pending = new TauriEngineAdapter().readTraces(traceRequest);
    await vi.waitFor(() => expect(commandsCalled()).toContain("read_traces"));

    traceChannel().onmessage(fromHex(traceFixture.hex));

    expect((await pending).points).toHaveLength(3);
  });

  it("acknowledges the response with its run and sequence as plain numbers", async () => {
    tauri.state.answer = (command, args) => {
      if (command === "read_traces") {
        (args?.onTrace as FakeChannelInstance).onmessage(
          fromHex(traceFixture.hex),
        );
      }
    };

    await new TauriEngineAdapter().readTraces(traceRequest);

    expect(tauri.state.calls.at(-1)).toEqual({
      command: "acknowledge_trace",
      args: { runId: TRACE_RUN_ID, traceSequence: 7 },
    });
    expect(typeof tauri.state.calls.at(-1)?.args?.traceSequence).toBe("number");
  });

  it("rejects a corrupt response without acknowledging it", async () => {
    const corrupt = new Uint8Array(fromHex(traceFixture.hex));
    corrupt[0] = 0x58;
    tauri.state.answer = (command, args) => {
      if (command === "read_traces") {
        (args?.onTrace as FakeChannelInstance).onmessage(corrupt.buffer);
      }
    };

    await expect(
      new TauriEngineAdapter().readTraces(traceRequest),
    ).rejects.toThrow(/OCT1 magic/);
    expect(commandsCalled()).not.toContain("acknowledge_trace");
  });

  it("rejects a message that is not binary", async () => {
    tauri.state.answer = (command, args) => {
      if (command === "read_traces") {
        (args?.onTrace as FakeChannelInstance).onmessage({ not: "binary" });
      }
    };

    await expect(
      new TauriEngineAdapter().readTraces(traceRequest),
    ).rejects.toThrow(/not binary/);
  });

  it("reports a refused request as an unavailable engine", async () => {
    tauri.state.answer = () => {
      throw "the trace request cannot be served: Some(StaleRun)";
    };

    const error = await new TauriEngineAdapter()
      .readTraces(traceRequest)
      .catch((caught: unknown) => caught);

    expect(error).toBeInstanceOf(EngineUnavailableError);
    expect((error as Error).message).toMatch(/StaleRun/);
  });

  it("gives up when no response arrives", async () => {
    vi.useFakeTimers();

    try {
      const outcome = new TauriEngineAdapter()
        .readTraces(traceRequest)
        .catch((caught: unknown) => caught);

      await vi.advanceTimersByTimeAsync(TRACE_DELIVERY_TIMEOUT_MS + 1);

      expect(await outcome).toBeInstanceOf(EngineUnavailableError);
    } finally {
      vi.useRealTimers();
    }
  });

  it("ignores a response that arrives after the read was given up", async () => {
    vi.useFakeTimers();

    try {
      const outcome = new TauriEngineAdapter()
        .readTraces(traceRequest)
        .catch((caught: unknown) => caught);

      await vi.advanceTimersByTimeAsync(TRACE_DELIVERY_TIMEOUT_MS + 1);
      await outcome;
      traceChannel().onmessage(fromHex(traceFixture.hex));

      expect(commandsCalled()).not.toContain("acknowledge_trace");
    } finally {
      vi.useRealTimers();
    }
  });

  it("refuses to read outside the desktop app", async () => {
    tauri.state.inTauri = false;

    await expect(
      new TauriEngineAdapter().readTraces(traceRequest),
    ).rejects.toThrow(/desktop app/);
    expect(tauri.state.calls).toEqual([]);
  });
});
