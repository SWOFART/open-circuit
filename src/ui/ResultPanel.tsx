import type { Diagnostic } from "../generated/Diagnostic";
import type { DiagnosticReference } from "../generated/DiagnosticReference";
import type { StableId } from "../generated/StableId";
import type { SimulationState } from "../store/simulation";
import { describeDiagnostic } from "./messages";
import { formatSi } from "./units";

export interface ResultPanelProps {
  readonly state: SimulationState;
  readonly labels: ReadonlyMap<StableId, string>;
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

/** Shows what the backend reported. It never calculates anything itself. */
export function ResultPanel({ state, labels }: ResultPanelProps) {
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
