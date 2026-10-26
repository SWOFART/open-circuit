import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { DeviceState } from "../generated/DeviceState";

// The frame envelope is specified in docs/contracts/stage-4-components.md section 5 and encoded by
// crates/app/src/frame.rs. This decoder must accept and reject the same bytes; the shared
// golden fixture tests/golden/stage-1-frame.json is what keeps the two in agreement.

export const FIXED_HEADER_LENGTH = 60;
/** Revision tokens are opaque; the bridge bounds them so a frame stays small. */
export const MAX_REVISION_BYTES = 256;
/** Far above any Stage 1 layout; guards the length arithmetic and allocation. */
export const MAX_VALUE_COUNT = 65_536;
export const MAX_STATE_COUNT = 32_768;

const MAGIC = [0x4f, 0x43, 0x46, 0x32]; // "OCF2"

export class FrameDecodeError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "FrameDecodeError";
  }
}

/** Where the numeric payload starts: after the header and revision, 8-byte aligned. */
export function payloadOffset(revisionLength: number): number {
  return Math.ceil((FIXED_HEADER_LENGTH + revisionLength) / 8) * 8;
}

function formatRunId(bytes: Uint8Array): string {
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));

  return [
    hex.slice(0, 4).join(""),
    hex.slice(4, 6).join(""),
    hex.slice(6, 8).join(""),
    hex.slice(8, 10).join(""),
    hex.slice(10, 16).join(""),
  ].join("-");
}

/**
 * Decodes one binary frame. Throws `FrameDecodeError` for anything malformed, so a
 * corrupt frame can be discarded instead of being shown.
 */
export function decodeFrame(buffer: ArrayBuffer): ResultSnapshot {
  if (buffer.byteLength < FIXED_HEADER_LENGTH) {
    throw new FrameDecodeError(
      `A frame needs at least ${FIXED_HEADER_LENGTH} bytes, found ${buffer.byteLength}.`,
    );
  }

  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);

  if (!MAGIC.every((byte, index) => bytes[index] === byte)) {
    throw new FrameDecodeError("The frame does not start with the OCF2 magic.");
  }

  const valueCount = view.getUint32(48, true);
  const stateCount = view.getUint32(52, true);
  const revisionLength = view.getUint32(56, true);

  if (revisionLength > MAX_REVISION_BYTES) {
    throw new FrameDecodeError(
      `The document revision is ${revisionLength} bytes; the limit is ${MAX_REVISION_BYTES}.`,
    );
  }

  if (valueCount > MAX_VALUE_COUNT) {
    throw new FrameDecodeError(
      `The frame has ${valueCount} values; the limit is ${MAX_VALUE_COUNT}.`,
    );
  }
  if (stateCount > MAX_STATE_COUNT) {
    throw new FrameDecodeError(
      `The frame has ${stateCount} state records; the limit is ${MAX_STATE_COUNT}.`,
    );
  }

  const payloadStart = payloadOffset(revisionLength);
  const expectedLength = payloadStart + 8 * valueCount + 32 * stateCount;

  if (buffer.byteLength !== expectedLength) {
    throw new FrameDecodeError(
      `The frame is ${buffer.byteLength} bytes but its header describes ${expectedLength}.`,
    );
  }

  const revisionEnd = FIXED_HEADER_LENGTH + revisionLength;
  let documentRevision: string;

  try {
    // `ignoreBOM: true` keeps a leading U+FEFF as part of the string, as Rust does, so
    // both decoders return the same revision for the same bytes.
    documentRevision = new TextDecoder("utf-8", {
      fatal: true,
      ignoreBOM: true,
    }).decode(bytes.subarray(FIXED_HEADER_LENGTH, revisionEnd));
  } catch {
    throw new FrameDecodeError("The document revision is not valid UTF-8.");
  }

  for (let index = revisionEnd; index < payloadStart; index += 1) {
    if (bytes[index] !== 0) {
      throw new FrameDecodeError("The header padding is not zero.");
    }
  }

  const simulationTimeSeconds = view.getFloat64(24, true);

  if (!Number.isFinite(simulationTimeSeconds)) {
    throw new FrameDecodeError("The simulation time is not finite.");
  }

  // DataView reads little-endian on every platform; the 8-byte alignment of the payload
  // also lets a Float64Array view it in place where that is worth doing.
  const values: number[] = [];

  for (let index = 0; index < valueCount; index += 1) {
    const value = view.getFloat64(payloadStart + 8 * index, true);

    if (!Number.isFinite(value)) {
      throw new FrameDecodeError("A frame value is not finite.");
    }

    values.push(value);
  }

  const states: DeviceState[] = [];
  const stateStart = payloadStart + 8 * valueCount;
  let previousId: Uint8Array | null = null;
  for (let index = 0; index < stateCount; index += 1) {
    const offset = stateStart + 32 * index;
    const idBytes = bytes.slice(offset, offset + 16);
    if (previousId && compareBytes(previousId, idBytes) >= 0) {
      throw new FrameDecodeError(
        "Device-state records are duplicated or not ordered by UUID bytes.",
      );
    }
    previousId = idBytes;
    const kind = view.getUint8(offset + 16);
    const value = view.getUint8(offset + 17);
    const flags = view.getUint16(offset + 18, true);
    if ((flags & ~1) !== 0) {
      throw new FrameDecodeError(
        "A device-state record contains unknown flag bits.",
      );
    }
    if (bytes.subarray(offset + 20, offset + 24).some((byte) => byte !== 0)) {
      throw new FrameDecodeError("A device-state reserved field is not zero.");
    }
    const transitionTime = view.getFloat64(offset + 24, true);
    const hasTransitionTime = (flags & 1) !== 0;
    if (
      (hasTransitionTime &&
        (!Number.isFinite(transitionTime) ||
          transitionTime < 0 ||
          transitionTime > simulationTimeSeconds)) ||
      (!hasTransitionTime &&
        bytes.subarray(offset + 24, offset + 32).some((byte) => byte !== 0))
    ) {
      throw new FrameDecodeError(
        "A device-state transition time is invalid or in the future.",
      );
    }
    const common = {
      componentId: formatRunId(idBytes),
      ...(hasTransitionTime
        ? { lastTransitionTimeSeconds: transitionTime }
        : {}),
    };
    if (kind === 1 && (value === 0 || value === 1)) {
      states.push({
        ...common,
        kind: "relay",
        state: value === 0 ? "released" : "picked",
      });
    } else if (kind === 2 && (value === 0 || value === 1)) {
      states.push({
        ...common,
        kind: "fuse",
        state: value === 0 ? "intact" : "tripped",
      });
    } else {
      throw new FrameDecodeError(
        "A device-state record has an unknown kind or value.",
      );
    }
  }

  return {
    metadata: {
      runId: formatRunId(bytes.subarray(32, 48)),
      documentRevision,
      layoutVersion: view.getUint32(4, true),
      simulationStep: view.getBigUint64(16, true),
      simulationTimeSeconds,
      frameSequence: view.getBigUint64(8, true),
    },
    values,
    states,
  };
}

function compareBytes(left: Uint8Array, right: Uint8Array): number {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index]! - right[index]!;
  }
  return 0;
}
