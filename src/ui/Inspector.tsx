import type { ParameterId } from "../generated/ParameterId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SimulationState } from "../store/simulation";
import { ComponentParameterFields } from "./ComponentParameterFields";
import { DesignatorField } from "./DesignatorField";

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
  /** Stores a designator and returns why it was refused, or null; left out in an assignment. */
  readonly onSetDesignator?: (
    componentId: StableId,
    text: string,
  ) => string | null;
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
  onSetDesignator,
}: InspectorProps) {
  const { catalog } = state;
  const editable = !disabled && catalog !== null && state.status !== "loading";
  const designators = new Map(
    schematic.documentation.designators.map((entry) => [
      entry.componentId,
      entry.designator,
    ]),
  );

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
        schematic.components.map((component) => (
          <div key={component.id}>
            {onSetDesignator !== undefined && (
              <DesignatorField
                label={labels.get(component.id) ?? component.typeId}
                committed={designators.get(component.id)}
                others={[...designators]
                  .filter(([id]) => id !== component.id)
                  .map(([, name]) => name)}
                disabled={!editable}
                onCommit={(text) => onSetDesignator(component.id, text)}
              />
            )}
            <ComponentParameterFields
              catalog={catalog}
              component={component}
              label={labels.get(component.id) ?? component.typeId}
              editable={
                editable && !(lockedComponents?.has(component.id) ?? false)
              }
              onCommit={onCommit}
              onClear={onClear}
            />
          </div>
        ))
      )}
    </section>
  );
}
