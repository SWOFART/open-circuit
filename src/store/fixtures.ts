import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";

// The canonical Stage 1 fixture is the single source for the demo circuit.
// Its shape is verified by the Rust serialization tests, which round-trip the
// same file, so the cast only narrows JSON string types to the generated ones.
export const dividerSchematic = schematicFixture as Schematic;

/** Display names such as `V1`, `R1`, `R2`, numbered per type in file order. */
export function componentLabels(schematic: Schematic): Map<StableId, string> {
  const counters = new Map<string, number>();
  const prefixes: Record<Schematic["components"][number]["typeId"], string> = {
    resistor: "R",
    dc_voltage_source: "V",
  };
  const labels = new Map<StableId, string>();

  for (const component of schematic.components) {
    const prefix = prefixes[component.typeId];
    const count = (counters.get(prefix) ?? 0) + 1;
    counters.set(prefix, count);
    labels.set(component.id, `${prefix}${count}`);
  }

  return labels;
}
