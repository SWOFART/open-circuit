import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { SetParameterRequest } from "../generated/SetParameterRequest";
import type { SetParameterResult } from "../generated/SetParameterResult";
import type { StableId } from "../generated/StableId";
import type { TraceBatch } from "../generated/TraceBatch";
import type { TraceRequest } from "../generated/TraceRequest";
import { decodeFrame } from "./frameCodec";
import { decodeTrace, type DecodedTrace } from "./traceCodec";

/** How long a trace response may take to arrive after the request was accepted. */
export const TRACE_DELIVERY_TIMEOUT_MS = 5000;

/** What the adapter reports about live frames. Neither callback may throw. */
export interface FrameHandlers {
  /** A decoded frame. The caller decides, by the contract rules, whether to apply it. */
  onFrame(snapshot: ResultSnapshot): void;
  /** A frame that could not be read, or any failure of the frame stream. */
  onError(error: Error): void;
}

/**
 * The UI's only view of the Rust Engine. Requests and results are the
 * generated contract types; the UI never calculates physics itself.
 */
export interface EngineAdapter {
  getCatalog(): Promise<ComponentCatalog>;
  compile(request: CompileRequest): Promise<CompileResult>;
  setParameter(request: SetParameterRequest): Promise<SetParameterResult>;
  /**
   * Starts live frames of the compiled run. Resolves once the stream is open and
   * returns a function that stops it. Frames are an optimisation over the results
   * already returned by `compile` and `setParameter`, never the only source of them.
   */
  startFrames(runId: StableId, handlers: FrameHandlers): Promise<() => void>;
  /**
   * Reads one bounded trace response for the active run: the Engine's stored samples
   * of the requested signals, decimated to `maxPoints` with their minimum and maximum
   * kept. The adapter decodes and acknowledges the response; deciding whether it still
   * belongs to the current run is the caller's job, because a recompile can overtake
   * a request.
   */
  readTraces(request: TraceRequest): Promise<TraceBatch>;
}

/** The backend cannot be reached, as opposed to rejecting the circuit. */
export class EngineUnavailableError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "EngineUnavailableError";
  }
}

function describeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return typeof error === "string" ? error : JSON.stringify(error);
}

/**
 * The Rust engine serializes `u64` counters as JSON numbers, while the generated type
 * says `bigint` (the binary frames carry real 64-bit integers). Normalising at the
 * boundary keeps one representation in the UI and refuses anything that is not an
 * exact non-negative integer.
 */
function toCounter(value: unknown, field: string): bigint {
  if (typeof value === "bigint" && value >= 0n) {
    return value;
  }

  if (typeof value === "number" && Number.isSafeInteger(value) && value >= 0) {
    return BigInt(value);
  }

  throw new EngineUnavailableError(`The engine sent an invalid ${field}.`);
}

function normalizeSnapshot(snapshot: ResultSnapshot): ResultSnapshot {
  return {
    ...snapshot,
    metadata: {
      ...snapshot.metadata,
      simulationStep: toCounter(
        snapshot.metadata.simulationStep,
        "simulation step",
      ),
      frameSequence: toCounter(
        snapshot.metadata.frameSequence,
        "frame sequence",
      ),
    },
  };
}

function normalizeCompileResult(result: CompileResult): CompileResult {
  return result.status === "success"
    ? { ...result, initialSnapshot: normalizeSnapshot(result.initialSnapshot) }
    : result;
}

function normalizeSetParameterResult(
  result: SetParameterResult,
): SetParameterResult {
  return result.status === "success"
    ? { ...result, snapshot: normalizeSnapshot(result.snapshot) }
    : result;
}

function requireTauri(): void {
  if (!isTauri()) {
    throw new EngineUnavailableError(
      "The Rust engine is only available in the desktop app.",
    );
  }
}

/** Calls a Tauri command, turning any failure into an `EngineUnavailableError`. */
async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  requireTauri();

  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new EngineUnavailableError(describeError(error));
  }
}

/**
 * Production adapter: every call goes to the Rust engine through Tauri commands, and
 * live frames arrive as binary messages over a Tauri channel.
 */
export class TauriEngineAdapter implements EngineAdapter {
  getCatalog(): Promise<ComponentCatalog> {
    return call<ComponentCatalog>("get_component_catalog");
  }

  async compile(request: CompileRequest): Promise<CompileResult> {
    return normalizeCompileResult(
      await call<CompileResult>("compile_circuit", { request }),
    );
  }

  async setParameter(
    request: SetParameterRequest,
  ): Promise<SetParameterResult> {
    return normalizeSetParameterResult(
      await call<SetParameterResult>("set_parameter", { request }),
    );
  }

  async startFrames(
    runId: StableId,
    handlers: FrameHandlers,
  ): Promise<() => void> {
    requireTauri();

    let open = true;
    const channel = new Channel<ArrayBuffer>();

    channel.onmessage = (message) => {
      if (!open) {
        return;
      }

      if (!(message instanceof ArrayBuffer)) {
        handlers.onError(
          new Error("The engine sent a frame that is not binary."),
        );
        return;
      }

      let snapshot: ResultSnapshot;

      try {
        snapshot = decodeFrame(message);
      } catch (error) {
        handlers.onError(
          error instanceof Error ? error : new Error(String(error)),
        );
        return;
      }

      handlers.onFrame(snapshot);

      // Telling the backend a frame was processed lets it send the next one. Without
      // acknowledgements a slow webview would be sent frames faster than it reads them.
      invoke("acknowledge_frame", {
        frameSequence: Number(snapshot.metadata.frameSequence),
      }).catch((error: unknown) => {
        handlers.onError(new Error(describeError(error)));
      });
    };

    await call("start_simulation", { runId, onFrame: channel });

    return () => {
      if (!open) {
        return;
      }

      open = false;
      channel.onmessage = () => {};
      // A stop for a run that has since been replaced is ignored by the backend.
      invoke("stop_simulation", { runId }).catch(() => {});
    };
  }

  async readTraces(request: TraceRequest): Promise<TraceBatch> {
    requireTauri();

    const channel = new Channel<ArrayBuffer>();
    // The response arrives on the channel, separately from the command's own reply and
    // in either order, so both are awaited.
    const received = new Promise<DecodedTrace>((resolve, reject) => {
      channel.onmessage = (message) => {
        if (!(message instanceof ArrayBuffer)) {
          reject(new Error("The engine sent a trace that is not binary."));
          return;
        }

        try {
          resolve(decodeTrace(message));
        } catch (error) {
          reject(error instanceof Error ? error : new Error(String(error)));
        }
      };
    });
    // A failing command is reported below; this keeps the unused branch quiet.
    received.catch(() => {});

    try {
      await call("read_traces", { request, onTrace: channel });

      const decoded = await withTimeout(received, TRACE_DELIVERY_TIMEOUT_MS);
      const sequence = Number(decoded.traceSequence);

      // Acknowledging releases the one outstanding response of this run, so the backend
      // may serve the next request. Without it a request waits for the backend's timeout.
      if (Number.isSafeInteger(sequence)) {
        invoke("acknowledge_trace", {
          runId: request.runId,
          traceSequence: sequence,
        }).catch(() => {});
      }

      return decoded.batch;
    } finally {
      channel.onmessage = () => {};
    }
  }
}

/** Waits for `promise`, failing as an unavailable engine after `milliseconds`. */
function withTimeout<T>(promise: Promise<T>, milliseconds: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(
        new EngineUnavailableError("The engine did not deliver the trace."),
      );
    }, milliseconds);

    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error: unknown) => {
        clearTimeout(timer);
        reject(error instanceof Error ? error : new Error(String(error)));
      },
    );
  });
}

/**
 * Picks the adapter. The fixture-backed mock is reachable only in development
 * builds and only when explicitly requested, so a production bundle can never
 * show mock readings to a user.
 */
export async function createEngineAdapter(): Promise<EngineAdapter> {
  if (import.meta.env.DEV && import.meta.env.VITE_ENGINE_ADAPTER === "mock") {
    const { createDividerFixtureAdapter } = await import("./mockEngineAdapter");
    return createDividerFixtureAdapter();
  }

  return new TauriEngineAdapter();
}
