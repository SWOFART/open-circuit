import { describe, expect, it } from "vitest";
import type { ScopeSignalData } from "../store/traces";
import {
  groupByUnit,
  niceTicks,
  pointsInWindow,
  timeWindow,
  valueRange,
} from "./scopeAxis";

function signal(
  key: string,
  unit: ScopeSignalData["unit"],
  times: number[],
  minimums: number[],
  maximums = minimums,
): ScopeSignalData {
  return { key, unit, times, minimums, maximums };
}

describe("timeWindow", () => {
  it("starts at time zero while the run is younger than the window", () => {
    expect(timeWindow(0.02, 0.1)).toEqual({ start: 0, end: 0.1 });
    expect(timeWindow(null, 0.1)).toEqual({ start: 0, end: 0.1 });
  });

  it("follows the newest sample once the run is older than the window", () => {
    const window = timeWindow(1, 0.1);

    expect(window.end).toBe(1);
    expect(window.start).toBeCloseTo(0.9);
    expect(window.end - window.start).toBeCloseTo(0.1);
  });
});

describe("niceTicks", () => {
  it("puts ticks on round numbers inside the range", () => {
    expect(niceTicks(0, 100)).toEqual([0, 20, 40, 60, 80, 100]);
    expect(niceTicks(-1.2, 1.2, 4)).toEqual([-1, 0, 1]);
    expect(niceTicks(-1.2, 1.2, 8)).toEqual([-1, -0.5, 0, 0.5, 1]);
  });

  it("does not show floating-point round-off", () => {
    const ticks = niceTicks(0, 0.3, 3);

    expect(ticks).toEqual([0, 0.1, 0.2, 0.3]);
  });

  it("works for very small and very large ranges", () => {
    expect(niceTicks(0, 0.001, 5)).toEqual([
      0, 0.0002, 0.0004, 0.0006, 0.0008, 0.001,
    ]);
    expect(niceTicks(0, 5e6, 5)).toEqual([0, 1e6, 2e6, 3e6, 4e6, 5e6]);
  });

  it("gives no ticks for an empty or invalid range", () => {
    expect(niceTicks(1, 1)).toEqual([]);
    expect(niceTicks(2, 1)).toEqual([]);
    expect(niceTicks(0, Number.POSITIVE_INFINITY)).toEqual([]);
    expect(niceTicks(Number.NaN, 1)).toEqual([]);
  });

  it("never produces an unbounded number of ticks", () => {
    expect(niceTicks(0, 1, 100000).length).toBeLessThanOrEqual(51);
  });
});

describe("valueRange", () => {
  const window = { start: 0, end: 1 };

  it("holds every visible value with a margin", () => {
    const range = valueRange([signal("a", "volt", [0, 1], [2, 4])], window);

    expect(range.min).toBeLessThan(2);
    expect(range.max).toBeGreaterThan(4);
  });

  it("uses the maximums of an envelope", () => {
    const range = valueRange([signal("a", "volt", [0.5], [1], [9])], window);

    expect(range.max).toBeGreaterThan(9);
  });

  it("ignores values outside the window", () => {
    const range = valueRange(
      [signal("a", "volt", [-1, 0.5, 2], [100, 1, -100])],
      window,
    );

    expect(range.max).toBeLessThan(2);
    expect(range.min).toBeGreaterThan(0);
  });

  it("gives a flat trace a range of positive height around its value", () => {
    const flat = valueRange([signal("a", "volt", [0, 1], [5, 5])], window);
    const zero = valueRange([signal("a", "volt", [0, 1], [0, 0])], window);

    expect(flat.max - flat.min).toBeGreaterThan(0);
    expect(flat.min).toBeLessThan(5);
    expect(flat.max).toBeGreaterThan(5);
    expect(zero).toEqual({ min: -1, max: 1 });
  });

  it("gives a finite range when nothing is visible", () => {
    expect(valueRange([], window)).toEqual({ min: -1, max: 1 });
  });
});

describe("pointsInWindow", () => {
  it("keeps the points inside the window, including its edges, in order", () => {
    const points = pointsInWindow(
      signal("a", "volt", [0, 1, 2, 3], [10, 11, 12, 13], [20, 21, 22, 23]),
      { start: 1, end: 2 },
    );

    expect(points).toEqual([
      { time: 1, minimum: 11, maximum: 21 },
      { time: 2, minimum: 12, maximum: 22 },
    ]);
  });
});

describe("groupByUnit", () => {
  it("never mixes units on one axis and keeps first-seen order", () => {
    const a = signal("a", "volt", [], []);
    const b = signal("b", "amp", [], []);
    const c = signal("c", "volt", [], []);

    expect(groupByUnit([a, b, c])).toEqual([
      { unit: "volt", signals: [a, c] },
      { unit: "amp", signals: [b] },
    ]);
  });
});
