import type { ResultSlot } from "../generated/ResultSlot";
import type { StableId } from "../generated/StableId";
import type { Unit } from "../generated/Unit";

// A result slot says what it measures through its quantity and unit, and the Stage 4
// relay has three voltages and three currents, told apart only by the stable key
// (docs/contracts/stage-4-components.md section 5): `<id>/coil/voltage`,
// `<id>/normally_open/current`, and so on. Other keys are `<id>:voltage`,
// `<id>/accumulated_i2t`, and `<id>/angular_velocity`. This reads the words after the
// component ID for display only; nothing is calculated from them.

const DISPLAY_WORDS: Readonly<Record<string, string>> = {
  accumulated_i2t: "accumulated I²t",
  angular_velocity: "angular velocity",
  normally_open: "normally open",
  normally_closed: "normally closed",
};

/** What the slot measures, such as `voltage`, `coil current`, or `angular velocity`. */
export function slotDescription(slot: ResultSlot): string {
  const rest = slot.key.startsWith(slot.componentId)
    ? slot.key.slice(slot.componentId.length + 1)
    : "";
  const words = rest
    .split("/")
    .filter((part) => part !== "")
    .map((part) => DISPLAY_WORDS[part] ?? part.replace(/_/g, " "));

  return words.length > 0 ? words.join(" ") : slot.quantity;
}

/** The signal's name for a list or a legend: the component's name and what it measures. */
export function slotLabel(
  slot: ResultSlot,
  labels: ReadonlyMap<StableId, string>,
): string {
  return `${labels.get(slot.componentId) ?? "Component"} ${slotDescription(slot)}`;
}

const AXIS_NAMES: Readonly<Partial<Record<Unit, string>>> = {
  volt: "Voltage",
  amp: "Current",
  radian_per_second: "Angular velocity",
  ampere_squared_second: "Accumulated I²t",
};

/** The name of the quantity a unit's axis shows, such as `Voltage` for volts. */
export function axisName(unit: Unit): string {
  return AXIS_NAMES[unit] ?? unit;
}
