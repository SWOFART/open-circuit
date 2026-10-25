import { useId } from "react";
import type { ResultSlot } from "../generated/ResultSlot";
import type { StableId } from "../generated/StableId";
import type { Unit } from "../generated/Unit";
import type { SimulationState } from "../store/simulation";
import {
  MAX_SCOPE_SIGNALS,
  SCOPE_WINDOWS_SECONDS,
  type ScopeData,
  type ScopeSignalData,
  type ScopeState,
} from "../store/traces";
import {
  groupByUnit,
  niceTicks,
  pointsInWindow,
  timeWindow,
  valueRange,
  type TimeWindow,
} from "./scopeAxis";
import { formatSi, formatWithSymbol, unitSymbols } from "./units";

export interface OscilloscopeProps {
  readonly simulation: SimulationState;
  readonly scope: ScopeState;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly onToggleSignal: (key: string) => void;
  readonly onWindowChange: (seconds: number) => void;
}

/** One colour per selected signal, in selection order. */
const SIGNAL_COLORS = [
  "#60a5fa",
  "#f59e0b",
  "#34d399",
  "#f472b6",
  "#a78bfa",
  "#f87171",
  "#22d3ee",
  "#a3e635",
] as const;

const LANE_WIDTH = 640;
const LANE_HEIGHT = 150;
const MARGIN_LEFT = 72;
const MARGIN_RIGHT = 14;
const MARGIN_TOP = 10;

function colorAt(index: number): string {
  return SIGNAL_COLORS[index % SIGNAL_COLORS.length] ?? SIGNAL_COLORS[0];
}

function formatSeconds(seconds: number): string {
  return formatWithSymbol(seconds, "s");
}

/** What a signal is called: its component's name and what is measured on it. */
function signalLabel(
  slot: ResultSlot,
  labels: ReadonlyMap<StableId, string>,
): string {
  return `${labels.get(slot.componentId) ?? "Component"} ${slot.quantity}`;
}

interface LaneProps {
  readonly unit: Unit;
  readonly signals: readonly ScopeSignalData[];
  readonly colors: ReadonlyMap<string, string>;
  readonly window: TimeWindow;
  readonly envelope: boolean;
  /** The bottom lane carries the time labels. */
  readonly showTime: boolean;
  readonly description: string;
}

function ScopeLane({
  unit,
  signals,
  colors,
  window,
  envelope,
  showTime,
  description,
}: LaneProps) {
  const height = LANE_HEIGHT + (showTime ? 24 : 0);
  const plotWidth = LANE_WIDTH - MARGIN_LEFT - MARGIN_RIGHT;
  const range = valueRange(signals, window);
  const x = (time: number) =>
    MARGIN_LEFT +
    ((time - window.start) / (window.end - window.start)) * plotWidth;
  const y = (value: number) =>
    MARGIN_TOP +
    (1 - (value - range.min) / (range.max - range.min)) *
      (LANE_HEIGHT - MARGIN_TOP - 10);
  const valueTicks = niceTicks(range.min, range.max, 4);
  const timeTicks = niceTicks(window.start, window.end, 5);

  return (
    <svg
      className="scope-lane"
      viewBox={`0 0 ${LANE_WIDTH} ${height}`}
      role="img"
      aria-label={description}
    >
      <title>{description}</title>
      <rect
        x={MARGIN_LEFT}
        y={MARGIN_TOP}
        width={plotWidth}
        height={LANE_HEIGHT - MARGIN_TOP - 10}
        className="scope-plot-area"
      />
      {valueTicks.map((tick) => (
        <g key={`v${tick}`}>
          <line
            className="scope-grid"
            x1={MARGIN_LEFT}
            x2={LANE_WIDTH - MARGIN_RIGHT}
            y1={y(tick)}
            y2={y(tick)}
          />
          <text
            className="scope-tick"
            x={MARGIN_LEFT - 6}
            y={y(tick)}
            textAnchor="end"
            dominantBaseline="middle"
          >
            {formatSi(tick, unit)}
          </text>
        </g>
      ))}
      {timeTicks.map((tick) => (
        <g key={`t${tick}`}>
          <line
            className="scope-grid"
            x1={x(tick)}
            x2={x(tick)}
            y1={MARGIN_TOP}
            y2={LANE_HEIGHT - 10}
          />
          {showTime && (
            <text
              className="scope-tick"
              x={x(tick)}
              y={LANE_HEIGHT + 6}
              textAnchor="middle"
              dominantBaseline="hanging"
            >
              {formatSeconds(tick)}
            </text>
          )}
        </g>
      ))}
      {signals.map((signal) => {
        const points = pointsInWindow(signal, window);
        const color = colors.get(signal.key) ?? SIGNAL_COLORS[0];

        if (points.length === 1) {
          const [point] = points;

          return (
            <g key={signal.key}>
              <circle
                cx={x(point!.time)}
                cy={y((point!.minimum + point!.maximum) / 2)}
                r={3}
                fill={color}
              />
            </g>
          );
        }

        const upper = points
          .map((point) => `${x(point.time)},${y(point.maximum)}`)
          .join(" ");
        const lower = points
          .map((point) => `${x(point.time)},${y(point.minimum)}`)
          .join(" ");
        const band = [...points]
          .reverse()
          .map((point) => `${x(point.time)},${y(point.minimum)}`)
          .join(" ");

        return (
          <g key={signal.key}>
            {envelope && (
              <polygon
                points={`${upper} ${band}`}
                fill={color}
                fillOpacity={0.25}
              />
            )}
            <polyline
              points={upper}
              fill="none"
              stroke={color}
              strokeWidth={1.5}
              strokeLinejoin="round"
            />
            {envelope && (
              <polyline
                points={lower}
                fill="none"
                stroke={color}
                strokeWidth={1.5}
                strokeLinejoin="round"
              />
            )}
          </g>
        );
      })}
      <text className="scope-unit" x={4} y={MARGIN_TOP + 8}>
        {unit === "volt"
          ? "Voltage (V)"
          : unit === "amp"
            ? "Current (A)"
            : unit}
      </text>
    </svg>
  );
}

function ScopeDisplay({
  data,
  scope,
  labels,
  simulation,
}: {
  readonly data: ScopeData;
  readonly scope: ScopeState;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly simulation: SimulationState;
}) {
  const slots = simulation.run?.layout.slots ?? [];
  const window = timeWindow(data.latestTimeSeconds, scope.windowSeconds);
  const envelope = data.samplesPerPoint > 1;
  // Only signals still selected are drawn, in selection order, each with its own colour.
  const shown = scope.selectedKeys
    .map((key) => data.signals.find((signal) => signal.key === key))
    .filter((signal): signal is ScopeSignalData => signal !== undefined);
  const colors = new Map(
    scope.selectedKeys.map((key, index) => [key, colorAt(index)] as const),
  );
  const groups = groupByUnit(shown);
  const hasPoints = shown.some(
    (signal) => pointsInWindow(signal, window).length > 0,
  );

  if (data.latestTimeSeconds === null || !hasPoints) {
    return (
      <p className="panel-note" role="status">
        The run has not produced samples for these signals yet.
      </p>
    );
  }

  return (
    <>
      <ul className="scope-legend">
        {shown.map((signal) => {
          const slot = slots.find((candidate) => candidate.key === signal.key);

          return (
            <li key={signal.key}>
              <span
                className="scope-swatch"
                style={{ background: colors.get(signal.key) }}
                aria-hidden="true"
              />
              {slot ? signalLabel(slot, labels) : signal.key} (
              {unitSymbols[signal.unit]})
            </li>
          );
        })}
      </ul>
      {groups.map((group, index) => (
        <ScopeLane
          key={group.unit}
          unit={group.unit}
          signals={group.signals}
          colors={colors}
          window={window}
          envelope={envelope}
          showTime={index === groups.length - 1}
          description={`${group.unit === "volt" ? "Voltage" : "Current"} traces: ${group.signals
            .map((signal) => {
              const slot = slots.find(
                (candidate) => candidate.key === signal.key,
              );
              return slot ? signalLabel(slot, labels) : signal.key;
            })
            .join(", ")}`}
        />
      ))}
      <p className="panel-note scope-caption">
        Time since the run started, {formatSeconds(window.start)} to{" "}
        {formatSeconds(window.end)}. One sample every{" "}
        {formatSeconds(data.sampleIntervalSeconds)}.
        {envelope &&
          ` Each point stands for ${data.samplesPerPoint} samples; the shaded band shows their minimum and maximum.`}
      </p>
    </>
  );
}

/**
 * The oscilloscope panel. It draws what the Engine reported, against simulation time,
 * for the run the simulation store holds now; it calculates no physical quantity. It is
 * given the scope data and the simulation state and reads nothing itself.
 */
export function Oscilloscope({
  simulation,
  scope,
  labels,
  onToggleSignal,
  onWindowChange,
}: OscilloscopeProps) {
  const windowId = useId();
  const { run, status } = simulation;
  const slots = status === "ready" && run ? run.layout.slots : [];
  const atLimit = scope.selectedKeys.length >= MAX_SCOPE_SIGNALS;
  // A trace that does not belong to the run on screen is never drawn, whatever the
  // controller holds.
  const data =
    status === "ready" &&
    run &&
    scope.data &&
    scope.data.runId === run.runId &&
    scope.data.layoutVersion === run.layout.layoutVersion
      ? scope.data
      : null;

  let notice: { readonly text: string; readonly alert: boolean } | null = null;

  if (status === "idle" || status === "loading") {
    notice = { text: "Waiting for the simulation backend…", alert: false };
  } else if (status === "unavailable") {
    notice = {
      text: `Traces unavailable: ${simulation.error ?? "the engine cannot be reached"}.`,
      alert: true,
    };
  } else if (status === "failed" || !run) {
    notice = {
      text: "No active run: the engine could not calculate this circuit, so there are no traces.",
      alert: false,
    };
  } else if (scope.selectedKeys.length === 0) {
    notice = { text: "Select a signal to trace it.", alert: false };
  } else if (data === null && scope.error === null) {
    notice = { text: "Waiting for samples from the engine…", alert: false };
  }

  return (
    <section className="side-panel scope-panel" aria-labelledby="scope-heading">
      <h2 id="scope-heading">Oscilloscope</h2>
      <div className="scope-layout">
        <div className="scope-controls">
          <fieldset className="scope-signals">
            <legend>Signals</legend>
            {slots.length === 0 && (
              <p className="panel-note">No signals available.</p>
            )}
            {slots.map((slot) => {
              const selected = scope.selectedKeys.includes(slot.key);
              const index = scope.selectedKeys.indexOf(slot.key);

              return (
                <label key={slot.key} className="scope-signal">
                  <input
                    type="checkbox"
                    checked={selected}
                    disabled={!selected && atLimit}
                    onChange={() => onToggleSignal(slot.key)}
                  />
                  {selected && (
                    <span
                      className="scope-swatch"
                      style={{ background: colorAt(index) }}
                      aria-hidden="true"
                    />
                  )}
                  {signalLabel(slot, labels)}
                </label>
              );
            })}
            {atLimit && (
              <p className="panel-note">
                At most {MAX_SCOPE_SIGNALS} signals at a time.
              </p>
            )}
          </fieldset>
          <div className="scope-window">
            <label htmlFor={windowId}>Time window</label>
            <select
              id={windowId}
              value={scope.windowSeconds}
              onChange={(event) => onWindowChange(Number(event.target.value))}
            >
              {SCOPE_WINDOWS_SECONDS.map((seconds) => (
                <option key={seconds} value={seconds}>
                  {formatSeconds(seconds)}
                </option>
              ))}
            </select>
          </div>
        </div>
        <div className="scope-display">
          {notice && (
            <p
              className={notice.alert ? "panel-error" : "panel-note"}
              role={notice.alert ? "alert" : "status"}
            >
              {notice.text}
            </p>
          )}
          {data && status === "ready" && (
            <ScopeDisplay
              data={data}
              scope={scope}
              labels={labels}
              simulation={simulation}
            />
          )}
          {status === "ready" && scope.error !== null && (
            <p className="panel-error" role="alert">
              Trace read failed: {scope.error}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}
