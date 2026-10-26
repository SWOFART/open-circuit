import { describe, expect, it } from "vitest";
import frameFixture from "../../tests/golden/stage-1-frame.json";
import stateFrameFixture from "../../tests/golden/stage-4-frame.json";
import {
  FIXED_HEADER_LENGTH,
  FrameDecodeError,
  MAX_REVISION_BYTES,
  MAX_STATE_COUNT,
  MAX_VALUE_COUNT,
  decodeFrame,
  payloadOffset,
} from "./frameCodec";

function fromHex(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = parseInt(hex.slice(2 * index, 2 * index + 2), 16);
  }

  return bytes.buffer;
}

function golden(): Uint8Array<ArrayBuffer> {
  return new Uint8Array(fromHex(frameFixture.hex));
}

/** Builds a frame from the specification, independently of the decoder. */
function buildFrame(revision: string, values: number[]): ArrayBuffer {
  const revisionBytes = new TextEncoder().encode(revision);
  const start = payloadOffset(revisionBytes.length);
  const buffer = new ArrayBuffer(start + 8 * values.length);
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);

  bytes.set([0x4f, 0x43, 0x46, 0x32], 0);
  view.setUint32(4, 1, true);
  view.setBigUint64(8, 2n, true);
  view.setBigUint64(16, 7n, true);
  view.setFloat64(24, 0.007, true);
  bytes.set([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 0xa0], 32);
  view.setUint32(48, values.length, true);
  view.setUint32(52, 0, true);
  view.setUint32(56, revisionBytes.length, true);
  bytes.set(revisionBytes, FIXED_HEADER_LENGTH);
  values.forEach((value, index) =>
    view.setFloat64(start + 8 * index, value, true),
  );

  return buffer;
}

function decodingFails(bytes: Uint8Array, message: RegExp): void {
  expect(() => decodeFrame(bytes.slice().buffer)).toThrow(FrameDecodeError);
  expect(() => decodeFrame(bytes.slice().buffer)).toThrow(message);
}

describe("decoding the golden frame", () => {
  it("reads exactly what the independent encoder wrote", () => {
    const frame = decodeFrame(fromHex(frameFixture.hex));
    const expected = frameFixture.snapshot;

    expect(frame.metadata.runId).toBe(expected.metadata.runId);
    expect(frame.metadata.documentRevision).toBe(
      expected.metadata.documentRevision,
    );
    expect(frame.metadata.layoutVersion).toBe(expected.metadata.layoutVersion);
    expect(frame.metadata.simulationStep).toBe(
      BigInt(expected.metadata.simulationStep),
    );
    expect(frame.metadata.frameSequence).toBe(
      BigInt(expected.metadata.frameSequence),
    );
    expect(frame.metadata.simulationTimeSeconds).toBe(
      expected.metadata.simulationTimeSeconds,
    );
    expect(frame.values).toEqual(expected.values);
    expect(frame.states).toEqual([]);
  });

  it("decodes the independently encoded OCF2 discrete-state record", () => {
    const frame = decodeFrame(fromHex(stateFrameFixture.hex));
    expect(frame.values).toEqual(stateFrameFixture.frame.values);
    expect(frame.states).toEqual(stateFrameFixture.frame.states);
  });

  it("returns 64-bit counters as bigint so they never lose precision", () => {
    const frame = decodeFrame(fromHex(frameFixture.hex));

    expect(typeof frame.metadata.simulationStep).toBe("bigint");
    expect(typeof frame.metadata.frameSequence).toBe("bigint");
  });

  it("reads 64-bit counters beyond the safe integer range exactly", () => {
    const bytes = golden();
    new DataView(bytes.buffer).setBigUint64(8, 2n ** 63n + 5n, true);

    expect(decodeFrame(bytes.buffer).metadata.frameSequence).toBe(
      2n ** 63n + 5n,
    );
  });
});

describe("frame layout", () => {
  it("starts the payload on an 8-byte boundary for every revision length", () => {
    for (let length = 0; length <= 40; length += 1) {
      const start = payloadOffset(length);

      expect(start % 8).toBe(0);
      expect(start).toBeGreaterThanOrEqual(FIXED_HEADER_LENGTH + length);
      expect(start).toBeLessThan(FIXED_HEADER_LENGTH + length + 8);
      expect(buildFrame("r".repeat(length), [1.5, -2.5]).byteLength).toBe(
        start + 16,
      );
    }
  });

  it("round-trips awkward values, unicode revisions, and an empty payload", () => {
    const awkward = [0, -0, Number.MIN_VALUE, Number.MAX_VALUE, -1e-300, 1 / 3];
    const frame = decodeFrame(buildFrame("révision-ö-✓", awkward));

    expect(frame.metadata.documentRevision).toBe("révision-ö-✓");

    // A leading byte order mark is part of the revision, exactly as the Rust decoder
    // reads it.
    expect(decodeFrame(buildFrame("﻿rev", [1])).metadata.documentRevision).toBe(
      "﻿rev",
    );
    frame.values.forEach((value, index) =>
      expect(Object.is(value, awkward[index])).toBe(true),
    );
    expect(decodeFrame(buildFrame("rev", [])).values).toEqual([]);
  });

  it("accepts the largest revision and refuses one byte more", () => {
    expect(() =>
      decodeFrame(buildFrame("x".repeat(MAX_REVISION_BYTES), [1])),
    ).not.toThrow();
    expect(() =>
      decodeFrame(buildFrame("x".repeat(MAX_REVISION_BYTES + 1), [1])),
    ).toThrow(/limit is 256/);
  });
});

describe("refusing corrupted frames", () => {
  it("refuses frames shorter than the header", () => {
    decodingFails(golden().subarray(0, FIXED_HEADER_LENGTH - 1), /at least 60/);
    expect(() => decodeFrame(new ArrayBuffer(0))).toThrow(FrameDecodeError);
  });

  it("refuses a wrong magic", () => {
    const bytes = golden();
    bytes[0] = 0x58;

    decodingFails(bytes, /OCF2 magic/);
  });

  it("refuses a frame whose length disagrees with its header", () => {
    const bytes = golden();

    decodingFails(bytes.subarray(0, bytes.length - 1), /header describes/);
    decodingFails(new Uint8Array([...bytes, 0]), /header describes/);
  });

  it("refuses non-zero padding", () => {
    const bytes = golden();
    bytes[60 + 11] = 1;

    decodingFails(bytes, /padding is not zero/);
  });

  it("refuses an invalid UTF-8 revision", () => {
    const bytes = golden();
    bytes[60] = 0xff;

    decodingFails(bytes, /not valid UTF-8/);
  });

  it("refuses non-finite values and time", () => {
    const payloadStart = payloadOffset(11);

    for (const bad of [Number.NaN, Number.POSITIVE_INFINITY]) {
      const bytes = golden();
      new DataView(bytes.buffer).setFloat64(payloadStart, bad, true);

      decodingFails(bytes, /value is not finite/);
    }

    const bytes = golden();
    new DataView(bytes.buffer).setFloat64(24, Number.NaN, true);

    decodingFails(bytes, /time is not finite/);
  });

  it("refuses headers that overstate their size before trusting them", () => {
    const hugeCount = golden();
    new DataView(hugeCount.buffer).setUint32(48, 0xffffffff, true);
    decodingFails(hugeCount, new RegExp(`limit is ${MAX_VALUE_COUNT}`));

    const hugeRevision = golden();
    new DataView(hugeRevision.buffer).setUint32(56, 0xffffffff, true);
    decodingFails(hugeRevision, /limit is 256/);

    const hugeStateCount = golden();
    new DataView(hugeStateCount.buffer).setUint32(52, 0xffffffff, true);
    decodingFails(hugeStateCount, new RegExp(`limit is ${MAX_STATE_COUNT}`));
  });

  it("refuses malformed state records", () => {
    const fixture = new Uint8Array(fromHex(stateFrameFixture.hex));
    const stateOffset = payloadOffset(11) + 16;
    for (const [relativeOffset, value] of [
      [16, 3],
      [17, 2],
    ] as const) {
      const bytes = fixture.slice();
      bytes[stateOffset + relativeOffset] = value;
      decodingFails(bytes, /unknown kind or value/);
    }
    const badFlags = fixture.slice();
    new DataView(badFlags.buffer).setUint16(stateOffset + 18, 2, true);
    decodingFails(badFlags, /unknown flag bits/);

    const reserved = fixture.slice();
    reserved[stateOffset + 20] = 1;
    decodingFails(reserved, /reserved field is not zero/);

    const futureTransition = fixture.slice();
    new DataView(futureTransition.buffer).setFloat64(
      stateOffset + 24,
      0.02,
      true,
    );
    decodingFails(futureTransition, /transition time is invalid/);
  });

  it("renders any 16 bytes as a lowercase hyphenated run ID", () => {
    const bytes = golden();
    bytes.fill(0xab, 32, 48);

    expect(decodeFrame(bytes.buffer).metadata.runId).toBe(
      "abababab-abab-abab-abab-abababababab",
    );
  });
});
