import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";

// Keyed by the generated type ID, so adding a device type forces a prefix.
const PREFIXES: Readonly<Record<ComponentTypeId, string>> = {
  diode: "D",
  bjt_npn: "T",
  bjt_pnp: "T",
  resistor: "R",
  dc_voltage_source: "V",
  ac_voltage_source: "V",
  three_phase_source_star: "G",
  three_phase_source_delta: "G",
  ac_voltmeter: "PV",
  ac_ammeter: "PA",
  wattmeter: "PW",
  capacitor: "C",
  inductor: "L",
  relay_spdt: "K",
  limit_switch: "S",
  emergency_stop: "S",
  selector_switch: "S",
  key_switch: "S",
  relay_on_delay: "K",
  relay_off_delay: "K",
  lamp: "H",
  indicator: "H",
  solenoid: "M",
  valve: "M",
  buzzer: "H",
  frequency_converter: "T",
  fuse: "F",
  pmdc_motor: "M",
  voltmeter: "VM",
  ammeter: "AM",
  switch_spst: "S",
  push_button_no: "S",
  push_button_nc: "S",
  contactor: "Q",
  contactor_three_pole: "Q",
  breaker_three_pole: "QF",
  thermal_overload_three_pole: "FR",
  induction_motor_three_phase: "M",
  transformer_three_phase: "T",
  three_phase_load_star: "Z",
  three_phase_load_delta: "Z",
  earth_electrode: "PE",
  exposed_part: "EP",
  rcd_two_pole: "FI",
  rcd_four_pole: "FI",
};

/**
 * Display names such as `G1`, `R1`, `R2`. A name belongs to a component's stable ID.
 *
 * - A component with a stored designator (documentation, saved in the project) is shown under
 *   it.
 * - Any other component has an automatic name, given once, the first time the component is seen,
 *   numbered per type in file order. Numbers are never reused, so deleting R1 leaves R2 as R2
 *   and undoing the deletion brings R1 back under its own name. Automatic names are editor
 *   state, not part of the saved schematic, and skip any name equal to a stored designator
 *   (compared as the full name), so no name is shown twice.
 * - A component that is gone keeps the name it was last seen under, so a result that still
 *   refers to it reads the same.
 */
export class ComponentLabels {
  private readonly names = new Map<StableId, string>();
  private readonly designated = new Map<StableId, string>();
  private readonly counters = new Map<string, number>();
  // The map last handed out; the same instance until a name changes.
  private snapshot: ReadonlyMap<StableId, string> = new Map();

  /**
   * Names every component of `schematic` that has no name yet and returns all names given so
   * far. The returned map is the same instance as long as no name changed.
   */
  labelsFor(schematic: Schematic): ReadonlyMap<StableId, string> {
    const stored = new Map(
      schematic.documentation.designators.map((entry) => [
        entry.componentId,
        entry.designator,
      ]),
    );
    const taken = new Set(stored.values());
    const living = new Set<StableId>();

    for (const component of schematic.components) {
      living.add(component.id);
      const designator = stored.get(component.id);
      if (designator !== undefined) {
        this.designated.set(component.id, designator);
      } else {
        this.designated.delete(component.id);
      }
      const current = this.names.get(component.id);
      if (
        current === undefined ||
        (designator === undefined && taken.has(current))
      ) {
        this.names.set(
          component.id,
          this.automaticName(component.typeId, taken),
        );
      }
    }

    const next = new Map<StableId, string>();
    for (const [id, name] of this.names) {
      next.set(
        id,
        (living.has(id) ? stored.get(id) : this.designated.get(id)) ?? name,
      );
    }
    // A part that lost its designator is shown under its automatic name again.
    for (const [id, name] of stored) {
      if (living.has(id)) next.set(id, name);
    }

    if (!sameNames(this.snapshot, next)) {
      this.snapshot = next;
    }

    return this.snapshot;
  }

  /** Forgets every name; the next document is numbered from scratch. */
  reset(): void {
    this.names.clear();
    this.designated.clear();
    this.counters.clear();
    this.snapshot = new Map();
  }

  private automaticName(
    typeId: ComponentTypeId,
    taken: ReadonlySet<string>,
  ): string {
    const prefix = PREFIXES[typeId];
    let name: string;
    do {
      const number = (this.counters.get(prefix) ?? 0) + 1;
      this.counters.set(prefix, number);
      name = `${prefix}${number}`;
    } while (taken.has(name));
    return name;
  }
}

function sameNames(
  a: ReadonlyMap<StableId, string>,
  b: ReadonlyMap<StableId, string>,
): boolean {
  if (a.size !== b.size) return false;
  for (const [id, name] of b) {
    if (a.get(id) !== name) return false;
  }
  return true;
}
