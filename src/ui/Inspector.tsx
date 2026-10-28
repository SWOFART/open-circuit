import type { ParameterId } from "../generated/ParameterId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SimulationState } from "../store/simulation";
import { OptionalParameterField } from "./OptionalParameterField";
import { ParameterField } from "./ParameterField";
import { parameterNote } from "./parameterHints";

export interface InspectorProps {
  readonly state: SimulationState;
  readonly schematic: Schematic;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly disabled?: boolean;
  /** Parts an assignment locks: their parameters are shown but cannot be edited. */
  readonly lockedComponents?: ReadonlySet<StableId>;
  readonly onCommit: (
    componentId: StableId,
    parameterId: ParameterId,
    value: number,
  ) => void;
  /** Stops storing an optional parameter, such as an initial condition. */
  readonly onClear?: (componentId: StableId, parameterId: ParameterId) => void;
}

/**
 * Edits every parameter of every component. Names, units, defaults, and
 * domains come from the Rust catalog.
 */
export function Inspector({
  state,
  schematic,
  labels,
  disabled = false,
  lockedComponents,
  onCommit,
  onClear = () => {},
}: InspectorProps) {
  const { catalog } = state;
  const editable = !disabled && catalog !== null && state.status !== "loading";

  return (
    <section className="side-panel" aria-labelledby="inspector-heading">
      <h2 id="inspector-heading">Inspector</h2>
      {lockedComponents !== undefined && lockedComponents.size > 0 && (
        <p className="parameter-hint">
          Parts locked by the assignment cannot be edited.
        </p>
      )}
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
            const label = labels.get(component.id) ?? component.typeId;
            const key = `${component.id}:${definition.id}`;
            const locked = lockedComponents?.has(component.id) ?? false;

            if (definition.optional) {
              return (
                <OptionalParameterField
                  key={key}
                  label={label}
                  definition={definition}
                  committedValue={value}
                  disabled={!editable || locked}
                  onCommit={(next) =>
                    onCommit(component.id, definition.id, next)
                  }
                  onClear={() => onClear(component.id, definition.id)}
                />
              );
            }

            return value === undefined ? null : (
              <ParameterField
                key={key}
                label={label}
                definition={definition}
                committedValue={value}
                disabled={!editable || locked}
                note={parameterNote(definition)}
                onCommit={(next) => onCommit(component.id, definition.id, next)}
              />
            );
          });
        })
      )}
    </section>
  );
}
