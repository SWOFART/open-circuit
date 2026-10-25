import type { ScopeSignalData } from "../store/traces";

// Presentation helpers for the oscilloscope. They choose where the axes and ticks go and
// which points fall inside the window; they never calculate a physical quantity.

export interface TimeWindow {
  /** Seconds from the start of the run at the left edge. */
  readonly start: number;
  /** Seconds from the start of the run at the right edge. */
  readonly end: number;
}

/**
 * The visible window: it always spans `windowSeconds`. While the run is younger than the
 * window the axis starts at time zero and the trace grows from the left; afterwards the
 * right edge follows the newest sample.
 */
export function timeWindow(
  latestSeconds: number | null,
  windowSeconds: number,
): TimeWindow {
  const end =
    latestSeconds === null
      ? windowSeconds
      : Math.max(windowSeconds, latestSeconds);

  return { start: end - windowSeconds, end };
}

/** A "nice" step (1, 2, or 5 times a power of ten) close to `rough`. */
function niceStep(rough: number): number {
  const exponent = Math.floor(Math.log10(rough));
  const base = 10 ** exponent;
  const fraction = rough / base;
  // A small tolerance keeps 0.0002 / 1e-4 = 2.0000000000000004 from jumping to 5.
  const nice =
    fraction <= 1 + 1e-9
      ? 1
      : fraction <= 2 + 1e-9
        ? 2
        : fraction <= 5 + 1e-9
          ? 5
          : 10;

  return nice * base;
}

/** Round-off from multiplying a step is removed, so labels do not show 0.30000000000000004. */
function clean(value: number): number {
  return Number(value.toPrecision(12));
}

/** Tick positions inside `[min, max]`, about `target` of them, on round numbers. */
export function niceTicks(min: number, max: number, target = 5): number[] {
  if (!(max > min) || !Number.isFinite(min) || !Number.isFinite(max)) {
    return [];
  }

  const step = niceStep((max - min) / Math.max(1, target));
  const ticks: number[] = [];
  const first = Math.ceil(min / step - 1e-9);

  for (
    let index = first;
    clean(index * step) <= max + step * 1e-9;
    index += 1
  ) {
    ticks.push(clean(index * step));

    if (ticks.length > 50) {
      break;
    }
  }

  return ticks;
}

/**
 * The vertical range that holds every drawn value, with a margin so a trace never
 * touches the border. A flat trace gets a range around its value, never a zero-height one.
 */
export function valueRange(
  signals: readonly ScopeSignalData[],
  window: TimeWindow,
): { readonly min: number; readonly max: number } {
  let min = Number.POSITIVE_INFINITY;
  let max = Number.NEGATIVE_INFINITY;

  for (const signal of signals) {
    for (let index = 0; index < signal.times.length; index += 1) {
      const time = signal.times[index]!;

      if (time < window.start || time > window.end) {
        continue;
      }

      min = Math.min(min, signal.minimums[index]!);
      max = Math.max(max, signal.maximums[index]!);
    }
  }

  if (min > max) {
    // Nothing visible yet: an arbitrary but finite range, labelled by its ticks.
    return { min: -1, max: 1 };
  }

  if (min === max) {
    const spread = Math.abs(min) > 0 ? Math.abs(min) * 0.1 : 1;

    return { min: min - spread, max: max + spread };
  }

  const margin = (max - min) * 0.08;

  return { min: min - margin, max: max + margin };
}

export interface DrawnPoint {
  readonly time: number;
  readonly minimum: number;
  readonly maximum: number;
}

/** The points of one signal that lie inside the window, in time order. */
export function pointsInWindow(
  signal: ScopeSignalData,
  window: TimeWindow,
): DrawnPoint[] {
  const points: DrawnPoint[] = [];

  for (let index = 0; index < signal.times.length; index += 1) {
    const time = signal.times[index]!;

    if (time >= window.start && time <= window.end) {
      points.push({
        time,
        minimum: signal.minimums[index]!,
        maximum: signal.maximums[index]!,
      });
    }
  }

  return points;
}

/**
 * Groups signals by unit, in order of first appearance, so each unit gets one vertical
 * axis: volts and amperes are never drawn against the same scale.
 */
export function groupByUnit(signals: readonly ScopeSignalData[]): {
  readonly unit: ScopeSignalData["unit"];
  readonly signals: ScopeSignalData[];
}[] {
  const groups: {
    unit: ScopeSignalData["unit"];
    signals: ScopeSignalData[];
  }[] = [];

  for (const signal of signals) {
    const group = groups.find((candidate) => candidate.unit === signal.unit);

    if (group) {
      group.signals.push(signal);
    } else {
      groups.push({ unit: signal.unit, signals: [signal] });
    }
  }

  return groups;
}
