import type { Diagnostic } from "../generated/Diagnostic";
import type { DiagnosticReference } from "../generated/DiagnosticReference";
import type { StableId } from "../generated/StableId";
import { readSlot, type SimulationState } from "../store/simulation";
import { describeDiagnostic } from "./messages";
import { slotLabel } from "./slotLabels";
import { formatSi } from "./units";

export interface ResultPanelProps {
  readonly state: SimulationState;
  readonly labels: ReadonlyMap<StableId, string>;
  /**
   * The component whose terminal voltage is the circuit's output, shown first as
   * `Vout`. The Stage 1 divider defines it as the voltage across R2.
   */
  readonly outputComponentId?: StableId;
  /** Rail names by ID, so a diagnostic can say which rail it names. */
  readonly railNames?: ReadonlyMap<StableId, string>;
}

function referenceLabel(
  reference: DiagnosticReference,
  labels: ReadonlyMap<StableId, string>,
  railNames: ReadonlyMap<StableId, string>,
): string | undefined {
  switch (reference.kind) {
    case "component":
    case "pin":
      return labels.get(reference.componentId);
    // Markers have no names; say what kind of object is meant.
    case "junction":
      return "junction";
    case "referenceMarker":
      return "ground marker";
    case "rail": {
      const name = railNames.get(reference.railId);
      return name === undefined ? "DC rail" : `DC rail ${name}`;
    }
    default:
      return undefined;
  }
}

const NO_RAIL_NAMES: ReadonlyMap<StableId, string> = new Map();

function DiagnosticsList({
  diagnostics,
  labels,
  railNames,
}: {
  readonly diagnostics: readonly Diagnostic[];
  readonly labels: ReadonlyMap<StableId, string>;
  readonly railNames: ReadonlyMap<StableId, string>;
}) {
  if (diagnostics.length === 0) {
    return null;
  }

  return (
    <ul className="diagnostics">
      {diagnostics.map((diagnostic, index) => {
        const affected = diagnostic.references
          .map((reference) => referenceLabel(reference, labels, railNames))
          .filter((label): label is string => label !== undefined);

        return (
          <li
            // Diagnostics have no ID; the engine returns them in a stable order.
            key={`${diagnostic.code}-${index}`}
            className={`diagnostic diagnostic-${diagnostic.severity}`}
          >
            <span className="diagnostic-severity">{diagnostic.severity}</span>{" "}
            {describeDiagnostic(diagnostic)}
            {affected.length > 0 && (
              <span className="diagnostic-affected">
                {" "}
                ({[...new Set(affected)].join(", ")})
              </span>
            )}
          </li>
        );
      })}
    </ul>
  );
}

/** The circuit's output voltage, read through the layout like every other value. */
function OutputReading({
  state,
  componentId,
  labels,
}: {
  readonly state: SimulationState;
  readonly componentId: StableId;
  readonly labels: ReadonlyMap<StableId, string>;
}) {
  const voltage = readSlot(state.run, state.snapshot, componentId, "voltage");

  if (voltage === undefined) {
    return null;
  }

  return (
    <p className="output-reading">
      <span>Vout (across {labels.get(componentId) ?? "output"})</span>
      <strong>{formatSi(voltage, "volt")}</strong>
    </p>
  );
}

/** Shows what the backend reported. It never calculates anything itself. */
export function ResultPanel({
  state,
  labels,
  outputComponentId,
  railNames = NO_RAIL_NAMES,
}: ResultPanelProps) {
  const { run, snapshot } = state;

  return (
    <section
      className="side-panel"
      aria-labelledby="results-heading"
      aria-busy={state.status === "ready" && state.updating}
    >
      <h2 id="results-heading">Results</h2>

      {(state.status === "idle" || state.status === "loading") && (
        <p className="panel-note" role="status">
          Waiting for the simulation backend…
        </p>
      )}

      {state.status === "unavailable" && (
        <p className="panel-error" role="alert">
          Simulation unavailable: {state.error ?? "unknown error"}
        </p>
      )}

      {state.status === "failed" && (
        <p className="panel-error" role="alert">
          The engine could not calculate this circuit. There is no result, and
          nothing below is a reading of it.
        </p>
      )}

      {state.status === "ready" && state.refusedUpdate && (
        <p className="panel-error" role="alert">
          The engine refused the last change and did not apply it. The result on
          screen is the one from before it.
        </p>
      )}

      {state.status === "ready" && outputComponentId !== undefined && (
        <OutputReading
          state={state}
          componentId={outputComponentId}
          labels={labels}
        />
      )}

      {state.status === "ready" && run && snapshot && (
        <dl
          className={
            state.updating || state.stopped
              ? "readings readings-unsettled"
              : "readings"
          }
        >
          {run.layout.slots.map((slot) => {
            const value = snapshot.values[slot.offset];

            return (
              <div key={slot.key} className="reading">
                <dt>{slotLabel(slot, labels)}</dt>
                <dd>
                  {value === undefined ? "—" : formatSi(value, slot.unit)}
                </dd>
              </div>
            );
          })}
        </dl>
      )}

      {state.status === "ready" && snapshot && (
        <p className="panel-note simulation-time">
          Simulation time {snapshot.metadata.simulationTimeSeconds.toFixed(3)} s
        </p>
      )}

      {state.status === "ready" && state.updating && (
        <p className="panel-note" role="status">
          Updating…
        </p>
      )}

      {state.status === "ready" && state.stopped && (
        <p className="panel-error" role="alert">
          The simulation stopped at step {state.stopped.simulationStep}. The
          values shown are from the last step before the stop; nothing was
          calculated after it. Changing a value or editing the circuit lets it
          go on.
        </p>
      )}

      {state.status === "ready" && state.error && (
        <p className="panel-error" role="alert">
          Update failed: {state.error}
        </p>
      )}

      <DiagnosticsList
        diagnostics={state.diagnostics}
        labels={labels}
        railNames={railNames}
      />
    </section>
  );
}
