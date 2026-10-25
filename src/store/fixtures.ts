import schematicFixture from "../../tests/golden/stage-1-divider.schematic.json";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { ComponentLabels } from "./labels";

// The canonical Stage 1 fixture is the single source for the demo circuit.
// Its shape is verified by the Rust serialization tests, which round-trip the
// same file, so the cast only narrows JSON string types to the generated ones.
export const dividerSchematic = schematicFixture as Schematic;

/**
 * The divider's output is the voltage across R2 (docs/contracts/stage-1.md section 6:
 * `Vout = Vs × R2 / (Rs + R1 + R2)`).
 */
export const DIVIDER_OUTPUT_COMPONENT_ID =
  "00000000-0000-4000-8000-000000000003";

/**
 * Display names such as `V1`, `R1`, `R2` for a schematic seen once: numbered per
 * type in file order. A document being edited uses the store's labels instead,
 * which keep each name through edits (see `ComponentLabels`).
 */
export function componentLabels(schematic: Schematic): Map<StableId, string> {
  return new Map(new ComponentLabels().labelsFor(schematic));
}
