import { describe, expect, it } from "vitest";
import traceFixture from "../../tests/golden/stage-3-trace.json";
import {
  decodeTrace,
  MAX_HEADER_BYTES,
  MAX_RESPONSE_BYTES,
  MAX_TRACE_POINTS,
  TraceDecodeError,
} from "./traceCodec";

function fromHex(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = parseInt(hex.slice(2 * index, 2 * index + 2), 16);
  }

  return bytes.buffer;
}

interface TestPoint {
  readonly step: bigint;
  readonly values: readonly (readonly [number, number])[];
}

/** A second encoder, written from the spec, for building deliberately broken input. */
function encode(
  header: unknown,
  points: readonly TestPoint[],
  options: { paddingByte?: number; extraBytes?: number } = {},
): ArrayBuffer {
  const headerBytes = new TextEncoder().encode(JSON.stringify(header));
  const payloadOffset = Math.ceil((8 + headerBytes.length) / 8) * 8;
  const signalCount = points[0]?.values.length ?? 0;
  const length =
    payloadOffset +
    points.length * (8 + 16 * signalCount) +
    (options.extraBytes ?? 0);
  const buffer = new ArrayBuffer(length);
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);

  bytes.set([0x4f, 0x43, 0x54, 0x31], 0);
  view.setUint32(4, headerBytes.length, true);
  bytes.set(headerBytes, 8);

  if (
    options.paddingByte !== undefined &&
    payloadOffset > 8 + headerBytes.length
  ) {
    bytes[8 + headerBytes.length] = options.paddingByte;
  }

  let offset = payloadOffset;

  for (const point of points) {
    view.setBigUint64(offset, point.step, true);
    offset += 8;

    for (const [minimum, maximum] of point.values) {
      view.setFloat64(offset, minimum, true);
      view.setFloat64(offset + 8, maximum, true);
      offset += 16;
    }
  }

  return buffer;
}

const KEY = "00000000-0000-4000-8000-000000000011:voltage";

function header(overrides: Record<string, unknown> = {}, pointCount = 1) {
  return {
    metadata: {
      documentRevision: "rev-1",
      firstSampleStep: "0",
      historySamples: 10000,
      lastSampleStep: "0",
      latestSampleStep: "0",
      layoutVersion: 1,
      missedSamples: "0",
      oldestSampleStep: "0",
      replaced: false,
      runId: "00000000-0000-4000-8000-0000000000a0",
      sampleCount: 1,
      sampleIntervalSeconds: 0.001,
      ...overrides,
    },
    pointCount,
    signals: [{ key: KEY, unit: "volt" }],
    traceSequence: "1",
  };
}

const onePoint: readonly TestPoint[] = [{ step: 0n, values: [[1, 1]] }];

function expectRejected(buffer: ArrayBuffer, message?: RegExp): void {
  expect(() => decodeTrace(buffer)).toThrow(TraceDecodeError);

  if (message) {
    expect(() => decodeTrace(buffer)).toThrow(message);
  }
}

describe("the golden trace response", () => {
  const decoded = decodeTrace(fromHex(traceFixture.hex));

  it("reads the sequence and the run metadata", () => {
    expect(decoded.traceSequence).toBe(7n);
    expect(decoded.batch.metadata).toEqual({
      runId: "00000000-0000-4000-8000-0000000000a0",
      documentRevision: "rev-2-abc123",
      layoutVersion: 1,
      sampleIntervalSeconds: 0.001,
      historySamples: 10000,
      oldestSampleStep: 0n,
      latestSampleStep: 105n,
      firstSampleStep: 100n,
      lastSampleStep: 105n,
      sampleCount: 6,
      missedSamples: 0n,
      replaced: false,
    });
  });

  it("reads the signals with their units, in header order", () => {
    expect(decoded.batch.signals).toEqual(traceFixture.header.signals);
  });

  it("reads every point's step and per-signal minimum and maximum", () => {
    expect(decoded.batch.points).toEqual(
      traceFixture.points.map((point) => ({
        simulationStep: BigInt(point.simulationStep),
        minimumValues: point.minimumValues,
        maximumValues: point.maximumValues,
      })),
    );
  });
});

describe("a well-formed response", () => {
  it("decodes an empty response whose range is null", () => {
    const decoded = decodeTrace(
      encode(
        header(
          {
            firstSampleStep: null,
            lastSampleStep: null,
            latestSampleStep: null,
            oldestSampleStep: null,
            sampleCount: 0,
          },
          0,
        ),
        [],
      ),
    );

    expect(decoded.batch.points).toEqual([]);
    expect(decoded.batch.metadata.firstSampleStep).toBeNull();
  });

  it("keeps 64-bit steps exact", () => {
    const step = 2n ** 63n + 5n;
    const decoded = decodeTrace(
      encode(
        header({
          firstSampleStep: step.toString(),
          lastSampleStep: step.toString(),
          latestSampleStep: step.toString(),
        }),
        [{ step, values: [[1, 1]] }],
      ),
    );

    expect(decoded.batch.points[0]?.simulationStep).toBe(step);
    expect(decoded.batch.metadata.lastSampleStep).toBe(step);
  });

  it("accepts a response with the maximum number of points", () => {
    const points: TestPoint[] = Array.from(
      { length: MAX_TRACE_POINTS },
      (_, step) => ({
        step: BigInt(step),
        values: [[0, 1]] as const,
      }),
    );
    const buffer = encode(
      header(
        {
          lastSampleStep: String(MAX_TRACE_POINTS - 1),
          latestSampleStep: String(MAX_TRACE_POINTS - 1),
          sampleCount: MAX_TRACE_POINTS,
        },
        MAX_TRACE_POINTS,
      ),
      points,
    );

    expect(decodeTrace(buffer).batch.points).toHaveLength(MAX_TRACE_POINTS);
  });
});

describe("a malformed response is rejected", () => {
  it("shorter than the fixed header", () => {
    expectRejected(new ArrayBuffer(4), /at least 8 bytes/);
  });

  it("with the wrong magic", () => {
    const buffer = encode(header(), onePoint);
    new Uint8Array(buffer)[3] = 0x32;

    expectRejected(buffer, /OCT1 magic/);
  });

  it("larger than the response limit", () => {
    expectRejected(new ArrayBuffer(MAX_RESPONSE_BYTES + 8), /limit/);
  });

  it("whose header length is over the limit or past the end", () => {
    const tooLong = encode(header(), onePoint);
    new DataView(tooLong).setUint32(4, MAX_HEADER_BYTES + 1, true);
    expectRejected(tooLong, /header is/);

    const pastEnd = encode(header(), onePoint);
    new DataView(pastEnd).setUint32(4, 4000, true);
    expectRejected(pastEnd, /shorter than its header/);
  });

  it("with a non-zero padding byte", () => {
    // The default header is not a multiple of 8 bytes, so it is padded.
    const buffer = encode(header(), onePoint, { paddingByte: 1 });
    const headerLength = new DataView(buffer).getUint32(4, true);

    expect((8 + headerLength) % 8).not.toBe(0);
    expectRejected(buffer, /padding/);
  });

  it("with a header that is not JSON", () => {
    const buffer = encode(header(), onePoint);
    new Uint8Array(buffer)[8] = 0x7e;

    expectRejected(buffer, /not valid UTF-8 JSON/);
  });

  it("whose length differs from the one the header describes", () => {
    expectRejected(
      encode(header(), onePoint, { extraBytes: 8 }),
      /header describes/,
    );
    expectRejected(encode(header({}, 2), onePoint), /header describes/);
  });

  it("with more points than the limit", () => {
    expectRejected(
      encode(header({}, MAX_TRACE_POINTS + 1), onePoint),
      /point count/,
    );
  });

  it("with a step that is not a base-10 string", () => {
    for (const bad of ["01", "-1", "1.5", "0x10", 5, "18446744073709551616"]) {
      expectRejected(
        encode(header({ latestSampleStep: bad }), onePoint),
        /latest step/,
      );
    }
  });

  it("with a missing or invalid field", () => {
    expectRejected(encode(header({ runId: "not-a-uuid" }), onePoint), /run ID/);
    expectRejected(encode(header({ replaced: "no" }), onePoint), /replaced/);
    expectRejected(
      encode(header({ sampleIntervalSeconds: 0 }), onePoint),
      /sample interval/,
    );
    expectRejected(
      encode(header({ layoutVersion: -1 }), onePoint),
      /layout version/,
    );
  });

  it("with an unknown unit, a repeated key, or too many signals", () => {
    const unknownUnit = header();
    unknownUnit.signals = [{ key: KEY, unit: "newton" }];
    expectRejected(encode(unknownUnit, onePoint), /unit/);

    const repeated = header();
    repeated.signals = [
      { key: KEY, unit: "volt" },
      { key: KEY, unit: "volt" },
    ];
    expectRejected(
      encode(repeated, [
        {
          step: 0n,
          values: [
            [1, 1],
            [1, 1],
          ],
        },
      ]),
      /repeated/,
    );

    const many = header();
    many.signals = Array.from({ length: 9 }, (_, index) => ({
      key: `${KEY}-${index}`,
      unit: "volt",
    }));
    expectRejected(
      encode(many, [
        { step: 0n, values: Array.from({ length: 9 }, () => [1, 1] as const) },
      ]),
      /1 to 8 signals/,
    );
  });

  it("with a value that is not finite or a minimum above its maximum", () => {
    expectRejected(
      encode(header(), [{ step: 0n, values: [[Number.NaN, 1]] }]),
      /not finite/,
    );
    expectRejected(
      encode(header(), [{ step: 0n, values: [[0, Number.POSITIVE_INFINITY]] }]),
      /not finite/,
    );
    expectRejected(
      encode(header(), [{ step: 0n, values: [[2, 1]] }]),
      /above its maximum/,
    );
  });

  it("with steps that are not strictly increasing", () => {
    const points: TestPoint[] = [
      { step: 3n, values: [[1, 1]] },
      { step: 3n, values: [[1, 1]] },
    ];

    expectRejected(
      encode(
        header(
          { firstSampleStep: "3", lastSampleStep: "3", sampleCount: 2 },
          2,
        ),
        points,
      ),
      /strictly increasing/,
    );
  });

  it("whose range does not match its points", () => {
    expectRejected(
      encode(header({ firstSampleStep: "1" }), onePoint),
      /range does not match/,
    );
    expectRejected(
      encode(header({ firstSampleStep: null, lastSampleStep: null }), onePoint),
      /range does not match/,
    );
  });
});
