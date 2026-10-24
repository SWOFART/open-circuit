import type { Diagnostic } from "../generated/Diagnostic";
import type { DiagnosticReference } from "../generated/DiagnosticReference";
import type { StableId } from "../generated/StableId";
import { readSlot, type SimulationState } from "../store/simulation";
import { describeDiagnostic } from "./messages";
import { formatSi } from "./units";

export interface ResultPanelProps {
  readonly state: SimulationState;
  readonly labels: ReadonlyMap<StableId, string>;
  /**
   * The component whose terminal voltage is the circuit's output, shown first as
   * `Vout`. The Stage 1 divider defines it as the voltage across R2.
   */
  readonly outputComponentId?: StableId;
}

function referenceLabel(
  reference: DiagnosticReference,
  labels: ReadonlyMap<StableId, string>,
): string | undefined {
  return reference.kind === "component" || reference.kind === "pin"
    ? labels.get(reference.componentId)
    : undefined;
}

function DiagnosticsList({
  diagnostics,
  labels,
}: {
  readonly diagnostics: readonly Diagnostic[];
  readonly labels: ReadonlyMap<StableId, string>;
}) {
  if (diagnostics.length === 0) {
    return null;
  }

  return (
    <ul className="diagnostics">
      {diagnostics.map((diagnostic, index) => {
        const affected = diagnostic.references
          .map((reference) => referenceLabel(reference, labels))
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
}: ResultPanelProps) {
  const { run, snapshot } = state;

  return (
    <section className="side-panel" aria-labelledby="results-heading">
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
          The engine could not calculate this circuit.
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
        <dl className="readings">
          {run.layout.slots.map((slot) => {
            const value = snapshot.values[slot.offset];

            return (
              <div key={slot.key} className="reading">
                <dt>
                  {labels.get(slot.componentId) ?? "Component"} {slot.quantity}
                </dt>
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

      {state.status === "ready" && state.error && (
        <p className="panel-error" role="alert">
          Update failed: {state.error}
        </p>
      )}

      <DiagnosticsList diagnostics={state.diagnostics} labels={labels} />
    </section>
  );
}
