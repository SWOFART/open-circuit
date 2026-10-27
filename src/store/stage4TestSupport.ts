import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultQuantity } from "../generated/ResultQuantity";
import type { ResultSignConvention } from "../generated/ResultSignConvention";
import type { ResultSlot } from "../generated/ResultSlot";
import type { Schematic } from "../generated/Schematic";
import type { Unit } from "../generated/Unit";

// Test support only; production code never imports this file. It lays out the result
// slots of a schematic the way the Engine does (crates/engine/src/engine.rs,
// `build_result_layout`, and docs/contracts/stage-4-components.md section 5), so the
// panels can be tested against the real key formats without the Engine.

interface Definition {
  readonly quantity: ResultQuantity;
  readonly unit: Unit;
  readonly suffix: string;
  readonly separator: ":" | "/";
  readonly sign: ResultSignConvention;
}

const VOLTAGE = (
  suffix = "voltage",
  separator: ":" | "/" = ":",
): Definition => ({
  quantity: "voltage",
  unit: "volt",
  suffix,
  separator,
  sign: "voltagePin1MinusPin2",
});

const CURRENT = (
  suffix = "current",
  separator: ":" | "/" = ":",
): Definition => ({
  quantity: "current",
  unit: "amp",
  suffix,
  separator,
  sign: "currentIntoPin1",
});

function definitionsOf(typeId: string): readonly Definition[] {
  switch (typeId) {
    case "relay_spdt":
      return [
        VOLTAGE("coil/voltage", "/"),
        CURRENT("coil/current", "/"),
        VOLTAGE("normally_open/voltage", "/"),
        CURRENT("normally_open/current", "/"),
        VOLTAGE("normally_closed/voltage", "/"),
        CURRENT("normally_closed/current", "/"),
      ];
    case "fuse":
      return [
        VOLTAGE(),
        CURRENT(),
        {
          quantity: "accumulatedI2t",
          unit: "ampere_squared_second",
          suffix: "accumulated_i2t",
          separator: "/",
          sign: "currentIntoPin1",
        },
      ];
    case "pmdc_motor":
      return [
        VOLTAGE(),
        CURRENT(),
        {
          quantity: "angularVelocity",
          unit: "radian_per_second",
          suffix: "angular_velocity",
          separator: "/",
          sign: "positiveRotationWithPositivePin1Current",
        },
      ];
    default:
      return [VOLTAGE(), CURRENT()];
  }
}

/** The layout the Engine would produce: components in ID order, slots in catalog order. */
export function layoutFor(schematic: Schematic): ResultLayout {
  const slots: ResultSlot[] = [];

  for (const component of [...schematic.components].sort((a, b) =>
    a.id < b.id ? -1 : 1,
  )) {
    for (const definition of definitionsOf(component.typeId)) {
      slots.push({
        key: `${component.id}${definition.separator}${definition.suffix}`,
        componentId: component.id,
        quantity: definition.quantity,
        unit: definition.unit,
        signConvention: definition.sign,
        offset: slots.length,
      });
    }
  }

  return { layoutVersion: 1, valueCount: slots.length, slots };
}

/** The key of one slot, for setting a value by what it means. */
export function keyOf(
  schematic: Schematic,
  componentId: string,
  suffix: string,
): string {
  const component = schematic.components.find(
    (item) => item.id === componentId,
  );
  const definition = definitionsOf(component?.typeId ?? "").find(
    (candidate) => candidate.suffix === suffix,
  );

  if (!component || !definition) {
    throw new Error(`No slot ${suffix} for ${componentId}.`);
  }

  return `${componentId}${definition.separator}${suffix}`;
}

/** Values for a layout: every slot zero except the ones named by key. */
export function valuesFor(
  layout: ResultLayout,
  byKey: Readonly<Record<string, number>>,
): number[] {
  return layout.slots.map((slot) => byKey[slot.key] ?? 0);
}
