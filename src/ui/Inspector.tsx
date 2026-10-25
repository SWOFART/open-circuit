import type { ParameterId } from "../generated/ParameterId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SimulationState } from "../store/simulation";
import { ParameterField } from "./ParameterField";

export interface InspectorProps {
  readonly state: SimulationState;
  readonly schematic: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly onCommit: (
    componentId: StableId,
    parameterId: ParameterId,
    value: number,
  ) => void;
}

/**
 * Edits every parameter of every component. Names, units, defaults, and
 * domains come from the Rust catalog.
 */
export function Inspector({
  state,
  schematic,
  labels,
  onCommit,
}: InspectorProps) {
  const { catalog } = state;
  const editable = catalog !== null && state.status !== "loading";

  return (
    <section className="side-panel" aria-labelledby="inspector-heading">
      <h2 id="inspector-heading">Inspector</h2>
      {catalog === null ? (
        <p className="panel-note">
          {state.status === "unavailable" || state.status === "failed"
            ? "The component catalog is unavailable."
            : "Waiting for the component catalog."}
        </p>
      ) : schematic.components.length === 0 ? (
        <p className="panel-note">
          No components yet. Choose one from the palette.
        </p>
      ) : (
        schematic.components.flatMap((component) => {
          const entry = catalog.componentTypes.find(
            (candidate) => candidate.typeId === component.typeId,
          );
          const values = component.parameters as Record<string, number>;

          return (entry?.parameters ?? []).map((definition) => {
            const value = values[definition.id];

            return value === undefined ? null : (
              <ParameterField
                key={`${component.id}:${definition.id}`}
                label={labels.get(component.id) ?? component.typeId}
                definition={definition}
                committedValue={value}
                disabled={!editable}
                onCommit={(next) => onCommit(component.id, definition.id, next)}
              />
            );
          });
        })
      )}
    </section>
  );
}
