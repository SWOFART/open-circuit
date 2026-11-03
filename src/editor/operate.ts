import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import type { SymbolRegistry } from "../symbols";
import { toWorld } from "./geometry";
import type { ScenePoint } from "./scene";

/** A device the user operates while a simulation runs. */
export type OperableKind = "switch" | "pushButton";

/**
 * The lever or actuator of one switch or push button, in grid units (docs/contracts/
 * stage-6-control-circuits.md section 6.3). A press inside it operates the device; a press
 * anywhere else on the part selects and moves it as before.
 */
export interface OperatingTarget {
  readonly componentId: StableId;
  readonly kind: OperableKind;
  readonly center: ScenePoint;
  readonly radius: number;
}

/** The operating areas of every switch and push button of `schematic`. */
export function operatingTargets(
  schematic: Schematic,
  registry: SymbolRegistry,
): OperatingTarget[] {
  const targets: OperatingTarget[] = [];

  for (const component of schematic.components) {
    const area = registry[component.typeId]?.operatingArea;

    if (!area) {
      continue;
    }

    targets.push({
      componentId: component.id,
      kind: component.typeId === "switch_spst" ? "switch" : "pushButton",
      center: toWorld(area.center, component),
      radius: area.radius,
    });
  }

  return targets;
}

/** The target whose area contains `point` (within `tolerance`), nearest centre first. */
export function operatingTargetAt(
  targets: readonly OperatingTarget[],
  point: ScenePoint,
  tolerance: number,
): OperatingTarget | null {
  let best: OperatingTarget | null = null;
  let bestDistance = Infinity;

  for (const target of targets) {
    const distance = Math.hypot(
      point.x - target.center.x,
      point.y - target.center.y,
    );

    if (distance <= target.radius + tolerance && distance < bestDistance) {
      best = target;
      bestDistance = distance;
    }
  }

  return best;
}
