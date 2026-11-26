import { describe, expect, it } from "vitest";
import type { FrameMetadata } from "../generated/FrameMetadata";
import type { ProbeResult } from "../generated/ProbeResult";
import {
  acceptsProbeResult,
  formatProbeResult,
  isProbeOverRange,
  onSpan,
} from "./MultimeterPanel";

const frame: FrameMetadata = {
  runId: "00000000-0000-4000-8000-000000000001",
  documentRevision: "rev-1",
  layoutVersion: 1,
  frameSequence: 4n,
  simulationStep: 3n,
  simulationTimeSeconds: 0.003,
};

const reading = (
  value: number,
  unit: "volt" | "amp" | "ohm",
  metadata = frame,
): ProbeResult => ({ status: "reading", value, unit, metadata });

describe("Stage 15 multimeter result selection", () => {
  it("accepts a result from the requested run after the observed endpoint", () => {
    expect(
      acceptsProbeResult(
        reading(12, "volt", { ...frame, frameSequence: 5n }),
        frame,
        undefined,
      ),
    ).toBe(true);
  });

  it("rejects replies from an old run, revision or frame", () => {
    for (const metadata of [
      { ...frame, runId: "00000000-0000-4000-8000-000000000002" },
      { ...frame, documentRevision: "rev-0" },
      { ...frame, frameSequence: 3n },
    ])
      expect(
        acceptsProbeResult(reading(12, "volt", metadata), frame, undefined),
      ).toBe(false);
    expect(
      acceptsProbeResult(reading(12, "volt"), frame, {
        ...frame,
        frameSequence: 6n,
      }),
    ).toBe(false);
  });

  it("flags magnitudes above, but not equal to, the educational limits", () => {
    expect(isProbeOverRange(reading(-30, "volt"))).toBe(false);
    expect(isProbeOverRange(reading(-30.01, "volt"))).toBe(true);
    expect(isProbeOverRange(reading(1, "amp"))).toBe(false);
    expect(isProbeOverRange(reading(1.01, "amp"))).toBe(true);
    expect(isProbeOverRange(reading(1000000, "ohm"))).toBe(false);
  });

  it("picks the same vertical conductor in either clamp direction", () => {
    const span = {
      conductorId: frame.runId,
      start: { x: 2, y: 4 },
      end: { x: 2, y: 0 },
    };
    expect(onSpan({ x: 2, y: 3 }, span)).toBe(true);
    expect(
      onSpan({ x: 2, y: 3 }, { ...span, start: span.end, end: span.start }),
    ).toBe(true);
    expect(onSpan({ x: 2, y: 5 }, span)).toBe(false);
    expect(onSpan({ x: 3, y: 3 }, span)).toBe(false);
  });
});

it("formats signed readings and explains the non-numeric Engine statuses", () => {
  expect(formatProbeResult(reading(-0.25, "amp"))).toBe("-250 mA");
  expect(formatProbeResult({ status: "openCircuit", metadata: frame })).toBe(
    "Open circuit",
  );
  expect(
    formatProbeResult({
      status: "unavailable",
      metadata: frame,
      diagnostic: {
        code: "UnconnectedProbe",
        severity: "error",
        localizationKey: "diagnostic.unconnectedProbe",
        references: [],
      },
    }),
  ).toBe("This probe is not connected.");
});
