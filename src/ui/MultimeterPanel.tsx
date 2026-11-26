import {
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type Ref,
} from "react";
import { resolvePinPositions } from "../editor/geometry";
import { symbols } from "../symbols";
import type { ConductorSpan } from "../generated/ConductorSpan";
import type { FrameMetadata } from "../generated/FrameMetadata";
import type { GridPoint } from "../generated/GridPoint";
import type { ProbeResult } from "../generated/ProbeResult";
import type { ProbeTarget } from "../generated/ProbeTarget";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { ProbePort } from "../store/engineAdapter";
import type { SimulationState } from "../store/simulation";
import { describeDiagnostic } from "./messages";
import { formatSi } from "./units";

type Mode = "voltage" | "current" | "resistance";
type Points = readonly [GridPoint | null, GridPoint | null];
const EMPTY_POINTS: Points = [null, null];
const PROBE_INTERVAL_MS = 50;

export interface MultimeterHandle {
  placeAt(position: GridPoint): void;
}

interface Props {
  readonly ref: Ref<MultimeterHandle>;
  readonly port: ProbePort;
  readonly state: SimulationState;
  readonly schematic: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly canvasActive: boolean;
  readonly onUseCanvas: () => void;
  readonly onMarkers: (positions: readonly (GridPoint | null)[]) => void;
}

export function acceptsProbeResult(
  result: ProbeResult,
  requested: FrameMetadata,
  latest: FrameMetadata | undefined,
): boolean {
  const frame = result.metadata;
  return (
    frame === undefined ||
    (frame.runId === requested.runId &&
      frame.documentRevision === requested.documentRevision &&
      frame.frameSequence >= requested.frameSequence &&
      (latest === undefined || frame.frameSequence >= latest.frameSequence))
  );
}

export function formatProbeResult(result: ProbeResult): string {
  switch (result.status) {
    case "reading":
      return formatSi(result.value, result.unit);
    case "openCircuit":
      return "Open circuit";
    case "unavailable":
      return describeDiagnostic(result.diagnostic);
  }
}

export function isProbeOverRange(result: ProbeResult): boolean {
  if (result.status !== "reading") return false;
  const limit =
    result.unit === "volt" ? 30 : result.unit === "amp" ? 1 : Infinity;
  return Math.abs(result.value) > limit;
}

function pointName(point: GridPoint): string {
  return "(" + point.x + ", " + point.y + ")";
}

export function onSpan(point: GridPoint, span: ConductorSpan): boolean {
  return (
    point.x >= Math.min(span.start.x, span.end.x) &&
    point.x <= Math.max(span.start.x, span.end.x) &&
    point.y >= Math.min(span.start.y, span.end.y) &&
    point.y <= Math.max(span.start.y, span.end.y)
  );
}

function PointInput({
  point,
  name,
  onPlace,
}: {
  readonly point: GridPoint | null;
  readonly name: string;
  readonly onPlace: (point: GridPoint | null) => void;
}) {
  // Keyed by the placed position so a canvas click updates the coordinate inputs.
  return (
    <form
      key={point ? pointName(point) : "empty"}
      className="probe-coordinates"
      aria-label={name}
      onSubmit={(event) => {
        event.preventDefault();
        const data = new FormData(event.currentTarget);
        const x = Number(data.get("x"));
        const y = Number(data.get("y"));
        if (
          [x, y].every(
            (n) => Number.isInteger(n) && n >= -2147483648 && n <= 2147483647,
          )
        ) {
          onPlace({ x, y });
        }
      }}
    >
      <label>
        {name} X{" "}
        <input
          name="x"
          type="number"
          required
          step={1}
          defaultValue={point?.x}
        />
      </label>
      <label>
        {name} Y{" "}
        <input
          name="y"
          type="number"
          required
          step={1}
          defaultValue={point?.y}
        />
      </label>
      <button type="submit">Place {name}</button>
      <button type="button" disabled={!point} onClick={() => onPlace(null)}>
        Remove {name}
      </button>
    </form>
  );
}

export function MultimeterPanel({
  ref,
  port,
  state,
  schematic,
  labels,
  canvasActive,
  onUseCanvas,
  onMarkers,
}: Props) {
  const [mode, setMode] = useState<Mode>("voltage");
  const [points, setPoints] = useState<Points>(EMPTY_POINTS);
  const [activePoint, setActivePoint] = useState<0 | 1>(0);
  const [clampKind, setClampKind] = useState<"terminal" | "conductor">(
    "terminal",
  );
  const [terminal, setTerminal] = useState("");
  const [conductor, setConductor] = useState("");
  const [reverse, setReverse] = useState(false);
  const [pickNotice, setPickNotice] = useState("");
  const [page, setPage] = useState<{
    readonly key: string;
    readonly spans: readonly ConductorSpan[];
    readonly cursor: string | null;
  } | null>(null);
  const [pagePending, setPagePending] = useState(false);
  const [pageError, setPageError] = useState("");
  const [sample, setSample] = useState<{
    key: string;
    result: ProbeResult;
  } | null>(null);
  const [readError, setReadError] = useState("");
  const inFlight = useRef(false);
  const pageGeneration = useRef(0);
  const active =
    state.status === "ready" &&
    state.run &&
    state.snapshot &&
    state.run.runId === state.snapshot.metadata.runId &&
    state.run.documentRevision === state.snapshot.metadata.documentRevision
      ? state.snapshot.metadata
      : null;
  const activeRef = useRef(active);
  activeRef.current = active;
  const runId = active?.runId;
  const revision = active?.documentRevision;
  const runKey = JSON.stringify([runId, revision]);
  const spans = page?.key === runKey ? page.spans : [];
  const terminals = useMemo(
    () =>
      schematic.components.flatMap((component) =>
        resolvePinPositions(component, symbols).map((pin) => ({
          key: component.id + "/" + pin.pinId,
          position: pin.position,
          componentId: component.id,
          pinId: pin.pinId,
          label:
            (labels.get(component.id) ?? component.typeId) +
            " " +
            pin.pinId +
            " " +
            pointName(pin.position),
        })),
      ),
    [schematic, labels],
  );
  const localTerminal = terminals.find((pin) => pin.key === terminal);
  const localSpan = spans.find((span) => JSON.stringify(span) === conductor);
  const selected = useMemo<ProbeTarget | null>(() => {
    if (mode === "current") {
      if (clampKind === "terminal" && localTerminal) {
        return {
          mode: "terminalCurrent",
          componentId: localTerminal.componentId,
          pinId: localTerminal.pinId,
        };
      }
      return clampKind === "conductor" && localSpan
        ? {
            mode: "conductorCurrent",
            span: reverse
              ? { ...localSpan, start: localSpan.end, end: localSpan.start }
              : localSpan,
          }
        : null;
    }
    const [first, second] = points;
    if (!first || !second) return null;
    return mode === "voltage"
      ? { mode, positive: first, negative: second }
      : { mode, first, second };
  }, [mode, clampKind, localTerminal, localSpan, points, reverse]);
  const selectionKey = JSON.stringify([runId, revision, selected]);

  useEffect(() => {
    if (!runId || !revision || !selected) return;
    let cancelled = false;
    let latest: FrameMetadata | undefined;
    const tick = () => {
      const requested = activeRef.current;
      if (
        inFlight.current ||
        requested?.runId !== runId ||
        requested.documentRevision !== revision
      )
        return;
      inFlight.current = true;
      void port
        .read({
          runId,
          documentRevision: revision,
          frameSequence: requested.frameSequence,
          target: selected,
        })
        .then(
          (result) => {
            if (cancelled || !acceptsProbeResult(result, requested, latest))
              return;
            if (result.metadata) latest = result.metadata;
            setSample({ key: selectionKey, result });
            setReadError("");
          },
          () => {
            if (!cancelled) {
              setSample(null);
              setReadError(selectionKey);
            }
          },
        )
        .finally(() => {
          inFlight.current = false;
        });
    };
    const interval = setInterval(tick, PROBE_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [port, runId, revision, selected, selectionKey]);

  useEffect(() => {
    if (!runId || !revision || mode !== "current" || clampKind !== "conductor")
      return;
    const generation = ++pageGeneration.current;
    setPagePending(true);
    setPageError("");
    setPage(null);
    void port
      .spans({ runId, documentRevision: revision, cursor: null })
      .then(
        (result) => {
          if (generation !== pageGeneration.current) return;
          if ("Ok" in result)
            setPage({
              key: runKey,
              spans: result.Ok.spans,
              cursor: result.Ok.nextCursor,
            });
          else setPageError(describeDiagnostic(result.Err));
        },
        () => {
          if (generation === pageGeneration.current)
            setPageError("Conductor targets are unavailable.");
        },
      )
      .finally(() => {
        if (generation === pageGeneration.current) setPagePending(false);
      });
    return () => {
      pageGeneration.current += 1;
    };
  }, [port, runId, revision, runKey, mode, clampKind]);

  const loadNext = () => {
    if (
      !runId ||
      !revision ||
      page?.key !== runKey ||
      !page.cursor ||
      pagePending
    )
      return;
    const generation = pageGeneration.current;
    setPagePending(true);
    setPageError("");
    void port
      .spans({ runId, documentRevision: revision, cursor: page.cursor })
      .then(
        (result) => {
          if (generation !== pageGeneration.current) return;
          if ("Ok" in result) {
            setPage({
              key: runKey,
              spans: [...page.spans, ...result.Ok.spans],
              cursor: result.Ok.nextCursor,
            });
          } else setPageError(describeDiagnostic(result.Err));
        },
        () => {
          if (generation === pageGeneration.current)
            setPageError("Conductor targets are unavailable.");
        },
      )
      .finally(() => {
        if (generation === pageGeneration.current) setPagePending(false);
      });
  };

  useImperativeHandle(ref, () => ({
    placeAt(position: GridPoint) {
      setPickNotice("");
      setSample(null);
      if (mode !== "current") {
        setPoints((current) =>
          activePoint === 0 ? [position, current[1]] : [current[0], position],
        );
        if (activePoint === 0) setActivePoint(1);
      } else if (clampKind === "terminal") {
        const hits = terminals.filter(
          (pin) =>
            pin.position.x === position.x && pin.position.y === position.y,
        );
        if (hits.length === 1) setTerminal(hits[0]!.key);
        else
          setPickNotice(
            hits.length
              ? "Choose a terminal by name."
              : "Choose a point on a terminal.",
          );
      } else {
        const hits = spans.filter((span) => onSpan(position, span));
        if (hits.length === 1) {
          setConductor(JSON.stringify(hits[0]));
          setReverse(false);
        } else
          setPickNotice(
            hits.length
              ? "Choose the conductor span by name."
              : "Choose a listed conductor span.",
          );
      }
    },
  }));

  useEffect(() => {
    if (mode !== "current") {
      onMarkers(points);
    } else if (clampKind === "terminal" && localTerminal) {
      onMarkers([localTerminal.position]);
    } else if (clampKind === "conductor" && localSpan) {
      onMarkers(
        reverse
          ? [localSpan.end, localSpan.start]
          : [localSpan.start, localSpan.end],
      );
    } else onMarkers([]);
  }, [mode, clampKind, points, localTerminal, localSpan, reverse, onMarkers]);

  const shown = active && sample?.key === selectionKey ? sample.result : null;
  const [firstName, secondName] =
    mode === "voltage"
      ? ["Positive probe", "Negative probe"]
      : ["Probe A", "Probe B"];
  return (
    <section className="side-panel" aria-labelledby="multimeter-title">
      <h2 id="multimeter-title">Multimeter</h2>
      <div className="probe-controls">
        <label>
          Mode{" "}
          <select
            value={mode}
            onChange={(event) => {
              setMode(event.target.value as Mode);
              setPickNotice("");
              setSample(null);
            }}
          >
            <option value="voltage">Voltage</option>
            <option value="current">Current clamp</option>
            <option value="resistance">Resistance</option>
          </select>
        </label>
        <button type="button" aria-pressed={canvasActive} onClick={onUseCanvas}>
          Place on canvas
        </button>
      </div>
      {mode === "current" ? (
        <div className="probe-controls">
          <label>
            Clamp target{" "}
            <select
              value={clampKind}
              onChange={(event) => {
                setClampKind(event.target.value as typeof clampKind);
                setSample(null);
                setPickNotice("");
              }}
            >
              <option value="terminal">Component terminal</option>
              <option value="conductor">Wire or rail span</option>
            </select>
          </label>
          {clampKind === "terminal" && (
            <p className="panel-note">
              Positive current flows into the selected terminal.
            </p>
          )}
          {clampKind === "terminal" ? (
            <label>
              Terminal{" "}
              <select
                value={terminal}
                onChange={(event) => {
                  setTerminal(event.target.value);
                  setSample(null);
                }}
              >
                <option value="">Choose a terminal</option>
                {terminals.map((pin) => (
                  <option key={pin.key} value={pin.key}>
                    {pin.label}
                  </option>
                ))}
              </select>
            </label>
          ) : (
            <>
              <label>
                Conductor span{" "}
                <select
                  value={conductor}
                  onChange={(event) => {
                    setConductor(event.target.value);
                    setReverse(false);
                    setSample(null);
                  }}
                >
                  <option value="">Choose a span</option>
                  {spans.map((span, index) => (
                    <option key={index} value={JSON.stringify(span)}>
                      {span.conductorId.slice(-6)} {pointName(span.start)} to{" "}
                      {pointName(span.end)}
                    </option>
                  ))}
                </select>
              </label>
              {page?.cursor && page.key === runKey && (
                <button type="button" disabled={pagePending} onClick={loadNext}>
                  More conductors
                </button>
              )}
              {localSpan && (
                <button
                  type="button"
                  aria-pressed={reverse}
                  onClick={() => {
                    setReverse(!reverse);
                    setSample(null);
                  }}
                >
                  Reverse clamp direction
                </button>
              )}
              {localSpan && (
                <p className="panel-note">
                  Positive current flows from{" "}
                  {pointName(reverse ? localSpan.end : localSpan.start)} to{" "}
                  {pointName(reverse ? localSpan.start : localSpan.end)}.
                </p>
              )}
              {pageError && <span role="status">{pageError}</span>}
            </>
          )}
        </div>
      ) : (
        <>
          <label>
            Canvas probe{" "}
            <select
              value={activePoint}
              onChange={(event) =>
                setActivePoint(Number(event.target.value) as 0 | 1)
              }
            >
              <option value={0}>{firstName}</option>
              <option value={1}>{secondName}</option>
            </select>
          </label>
          <PointInput
            name={firstName}
            point={points[0]}
            onPlace={(point) => {
              setPoints([point, points[1]]);
              setSample(null);
            }}
          />
          <PointInput
            name={secondName}
            point={points[1]}
            onPlace={(point) => {
              setPoints([points[0], point]);
              setSample(null);
            }}
          />
        </>
      )}
      {pickNotice && (
        <p className="panel-note" role="status">
          {pickNotice}
        </p>
      )}
      <output aria-label="Probe reading" aria-live="off">
        {!active
          ? "Waiting for the engine…"
          : !selected
            ? "Place the required probes."
            : shown
              ? formatProbeResult(shown)
              : readError === selectionKey
                ? "Probe reading unavailable."
                : "Measuring…"}
      </output>
      {shown?.metadata && (
        <p className="panel-note">
          Model time {formatSi(shown.metadata.simulationTimeSeconds, "second")}
        </p>
      )}
      {shown && isProbeOverRange(shown) && (
        <p className="warning" role="status">
          Above the educational meter range.
        </p>
      )}
    </section>
  );
}
