import type { TraceBatch } from "../generated/TraceBatch";
import type { TraceBatchMetadata } from "../generated/TraceBatchMetadata";
import type { TracePoint } from "../generated/TracePoint";
import type { TraceSignal } from "../generated/TraceSignal";
import type { Unit } from "../generated/Unit";

// The trace envelope is specified in docs/contracts/stage-1.md section 8.3 and encoded by
// crates/app/src/trace.rs. This decoder must accept and reject the same bytes; the shared
// golden fixture tests/golden/stage-3-trace.json is what keeps the two in agreement.

export const FIXED_HEADER_LENGTH = 8;
export const MAX_HEADER_BYTES = 8192;
export const MAX_TRACE_SIGNALS = 8;
export const MAX_TRACE_POINTS = 2048;
export const MAX_RESPONSE_BYTES = 512 * 1024;
const MAX_KEY_LENGTH = 256;
/** Revision tokens are opaque; the bridge bounds them so a response stays small. */
const MAX_REVISION_BYTES = 256;

const MAGIC = [0x4f, 0x43, 0x54, 0x31]; // "OCT1"
// Every unit the Engine can name. Typed as a record so that a unit added to the generated
// `Unit` type fails to compile here until the decoder accepts it.
const UNITS: Readonly<Record<Unit, true>> = {
  ohm: true,
  volt: true,
  amp: true,
  farad: true,
  henry: true,
  ampere_squared_second: true,
  kilogram_meter_squared: true,
  newton_meter: true,
  newton_meter_per_amp: true,
  newton_meter_second_per_radian: true,
  radian_per_second: true,
  dimensionless: true,
};
const STABLE_ID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const DECIMAL = /^(0|[1-9][0-9]*)$/;
const MAX_U64 = 2n ** 64n - 1n;

export class TraceDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "TraceDecodeError";
  }
}

/** One decoded response: the bridge's sequence number and the Engine's batch. */
export interface DecodedTrace {
  /** Identifies the response for `acknowledge_trace`. */
  readonly traceSequence: bigint;
  readonly batch: TraceBatch;
}

type Json = Record<string, unknown>;

function isObject(value: unknown): value is Json {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fail(message: string): never {
  throw new TraceDecodeError(message);
}

function requireObject(value: unknown, name: string): Json {
  return isObject(value) ? value : fail(`The trace ${name} is not an object.`);
}

function requireString(value: unknown, name: string): string {
  return typeof value === "string"
    ? value
    : fail(`The trace ${name} is not a string.`);
}

/** A `u64` the header carries as a base-10 string, so no precision is lost. */
function requireU64(value: unknown, name: string): bigint {
  if (typeof value !== "string" || !DECIMAL.test(value)) {
    return fail(`The trace ${name} is not a base-10 unsigned integer string.`);
  }

  const parsed = BigInt(value);

  return parsed <= MAX_U64
    ? parsed
    : fail(`The trace ${name} does not fit in 64 bits.`);
}

function optionalU64(value: unknown, name: string): bigint | null {
  return value === null ? null : requireU64(value, name);
}

function requireCount(value: unknown, name: string, limit: number): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < 0 ||
    value > limit
  ) {
    return fail(`The trace ${name} must be an integer from 0 to ${limit}.`);
  }

  return value;
}

function requireFiniteNumber(value: unknown, name: string): number {
  return typeof value === "number" && Number.isFinite(value)
    ? value
    : fail(`The trace ${name} is not a finite number.`);
}

function decodeMetadata(value: unknown): TraceBatchMetadata {
  const metadata = requireObject(value, "metadata");
  const runId = requireString(metadata.runId, "run ID");
  const documentRevision = requireString(
    metadata.documentRevision,
    "document revision",
  );

  if (!STABLE_ID.test(runId)) {
    fail("The trace run ID is not a lowercase UUID.");
  }

  if (new TextEncoder().encode(documentRevision).length > MAX_REVISION_BYTES) {
    fail(`The trace document revision is over ${MAX_REVISION_BYTES} bytes.`);
  }

  const sampleIntervalSeconds = requireFiniteNumber(
    metadata.sampleIntervalSeconds,
    "sample interval",
  );

  if (sampleIntervalSeconds <= 0) {
    fail("The trace sample interval must be positive.");
  }

  if (typeof metadata.replaced !== "boolean") {
    fail("The trace `replaced` flag is not a boolean.");
  }

  return {
    runId,
    documentRevision,
    layoutVersion: requireCount(
      metadata.layoutVersion,
      "layout version",
      0xffff_ffff,
    ),
    sampleIntervalSeconds,
    historySamples: requireCount(
      metadata.historySamples,
      "history size",
      0xffff_ffff,
    ),
    oldestSampleStep: optionalU64(metadata.oldestSampleStep, "oldest step"),
    latestSampleStep: optionalU64(metadata.latestSampleStep, "latest step"),
    firstSampleStep: optionalU64(metadata.firstSampleStep, "first step"),
    lastSampleStep: optionalU64(metadata.lastSampleStep, "last step"),
    sampleCount: requireCount(
      metadata.sampleCount,
      "sample count",
      0xffff_ffff,
    ),
    missedSamples: requireU64(metadata.missedSamples, "missed samples"),
    replaced: metadata.replaced,
  };
}

function decodeSignals(value: unknown): TraceSignal[] {
  if (
    !Array.isArray(value) ||
    value.length < 1 ||
    value.length > MAX_TRACE_SIGNALS
  ) {
    return fail(`A trace carries 1 to ${MAX_TRACE_SIGNALS} signals.`);
  }

  const keys = new Set<string>();

  return value.map((candidate: unknown) => {
    const signal = requireObject(candidate, "signal");
    const key = requireString(signal.key, "signal key");
    const unit = requireString(signal.unit, "signal unit");

    if (key.length === 0 || key.length > MAX_KEY_LENGTH || keys.has(key)) {
      fail("A trace signal key is empty, too long, or repeated.");
    }

    if (!Object.hasOwn(UNITS, unit)) {
      fail(`The trace signal unit "${unit}" is not known.`);
    }

    keys.add(key);

    return { key, unit: unit as Unit };
  });
}

/**
 * Decodes one binary trace response. Throws `TraceDecodeError` for anything malformed,
 * so a corrupt response can be discarded instead of being drawn. Every length is checked
 * against the contract's limits before it is used in arithmetic.
 */
export function decodeTrace(buffer: ArrayBuffer): DecodedTrace {
  if (buffer.byteLength < FIXED_HEADER_LENGTH) {
    fail(
      `A trace needs at least ${FIXED_HEADER_LENGTH} bytes, found ${buffer.byteLength}.`,
    );
  }

  if (buffer.byteLength > MAX_RESPONSE_BYTES) {
    fail(
      `The trace is ${buffer.byteLength} bytes; the limit is ${MAX_RESPONSE_BYTES}.`,
    );
  }

  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);

  if (!MAGIC.every((byte, index) => bytes[index] === byte)) {
    fail("The trace does not start with the OCT1 magic.");
  }

  const headerLength = view.getUint32(4, true);

  if (headerLength > MAX_HEADER_BYTES) {
    fail(
      `The trace header is ${headerLength} bytes; the limit is ${MAX_HEADER_BYTES}.`,
    );
  }

  const headerEnd = FIXED_HEADER_LENGTH + headerLength;
  const payloadStart = Math.ceil(headerEnd / 8) * 8;

  if (payloadStart > buffer.byteLength) {
    fail("The trace is shorter than its header.");
  }

  for (let index = headerEnd; index < payloadStart; index += 1) {
    if (bytes[index] !== 0) {
      fail("The trace header padding is not zero.");
    }
  }

  let header: unknown;

  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(
      bytes.subarray(FIXED_HEADER_LENGTH, headerEnd),
    );
    header = JSON.parse(text);
  } catch {
    return fail("The trace header is not valid UTF-8 JSON.");
  }

  const fields = requireObject(header, "header");
  const traceSequence = requireU64(fields.traceSequence, "sequence");
  const metadata = decodeMetadata(fields.metadata);
  const signals = decodeSignals(fields.signals);
  const pointCount = requireCount(
    fields.pointCount,
    "point count",
    MAX_TRACE_POINTS,
  );

  const recordLength = 8 + 16 * signals.length;
  const expectedLength = payloadStart + pointCount * recordLength;

  if (buffer.byteLength !== expectedLength) {
    fail(
      `The trace is ${buffer.byteLength} bytes but its header describes ${expectedLength}.`,
    );
  }

  const points: TracePoint[] = [];
  let previousStep = -1n;

  for (let index = 0; index < pointCount; index += 1) {
    let offset = payloadStart + index * recordLength;
    const simulationStep = view.getBigUint64(offset, true);
    offset += 8;

    if (simulationStep <= previousStep) {
      fail("The trace points are not in strictly increasing step order.");
    }

    previousStep = simulationStep;

    const minimumValues: number[] = [];
    const maximumValues: number[] = [];

    for (let signal = 0; signal < signals.length; signal += 1) {
      const minimum = view.getFloat64(offset, true);
      const maximum = view.getFloat64(offset + 8, true);
      offset += 16;

      if (!Number.isFinite(minimum) || !Number.isFinite(maximum)) {
        fail("A trace value is not finite.");
      }

      if (minimum > maximum) {
        fail("A trace point's minimum is above its maximum.");
      }

      minimumValues.push(minimum);
      maximumValues.push(maximum);
    }

    points.push({ simulationStep, minimumValues, maximumValues });
  }

  // The first and last sample steps describe what the points represent, so they have to
  // agree with the points themselves: a response is empty exactly when it has no range.
  const empty = pointCount === 0;

  if (
    empty !==
    (metadata.firstSampleStep === null || metadata.lastSampleStep === null)
  ) {
    fail("The trace range does not match its points.");
  }

  const firstPoint = points[0];

  if (
    firstPoint &&
    (metadata.firstSampleStep !== firstPoint.simulationStep ||
      metadata.lastSampleStep === null ||
      metadata.lastSampleStep <
        (points[points.length - 1]?.simulationStep ?? 0n))
  ) {
    fail("The trace range does not match its points.");
  }

  return { traceSequence, batch: { metadata, signals, points } };
}
