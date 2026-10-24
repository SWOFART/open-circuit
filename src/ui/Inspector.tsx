import type { ParameterId } from "../generated/ParameterId";
import type { StableId } from "../generated/StableId";
import type { SimulationState } from "../store/simulation";
import { ParameterField } from "./ParameterField";

export interface InspectorProps {
  readonly state: SimulationState;
  readonly labels: ReadonlyMap<StableId, string>;
  readonly onCommit: (
    componentId: StableId,
    parameterId: ParameterId,
    value: number,
  ) => void;
}

/** Edits the resistances of the schematic's resistors. */
export function Inspector({ state, labels, onCommit }: InspectorProps) {
  const resistorDefinition = state.catalog?.componentTypes
    .find((entry) => entry.typeId === "resistor")
    ?.parameters.find((parameter) => parameter.id === "resistanceOhms");
  const editable = state.status === "ready" && resistorDefinition !== undefined;

  return (
    <section className="side-panel" aria-labelledby="inspector-heading">
      <h2 id="inspector-heading">Inspector</h2>
      {resistorDefinition === undefined ? (
        <p className="panel-note">
          {state.status === "unavailable" || state.status === "failed"
            ? "The component catalog is unavailable."
            : "Waiting for the component catalog."}
        </p>
      ) : (
        state.schematic.components.map((component) =>
          component.typeId === "resistor" ? (
            <ParameterField
              key={component.id}
              label={labels.get(component.id) ?? "Resistor"}
              definition={resistorDefinition}
              committedValue={component.parameters.resistanceOhms}
              disabled={!editable}
              onCommit={(value) =>
                onCommit(component.id, resistorDefinition.id, value)
              }
            />
          ) : null,
        )
      )}
    </section>
  );
}
