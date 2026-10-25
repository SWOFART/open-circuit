import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";

// Keyed by the generated type ID, so adding a device type forces a prefix.
const PREFIXES: Readonly<Record<ComponentTypeId, string>> = {
  resistor: "R",
  dc_voltage_source: "V",
  capacitor: "C",
  inductor: "L",
};

/**
 * Display names such as `V1`, `R1`, `R2`. A name belongs to a component's
 * stable ID: it is given once, the first time the component is seen, and it
 * stays for as long as the document stays open.
 *
 * - A component opened with the document is numbered per type in file order.
 * - A component added later takes the next number of its type. Numbers are
 *   never reused, so deleting R1 leaves R2 as R2, a new resistor is R3, and
 *   undoing the deletion brings R1 back under its own name.
 * - Names are editor state, not part of the saved schematic. Reopening a saved
 *   file numbers its components in file order again.
 */
export class ComponentLabels {
  private readonly names = new Map<StableId, string>();
  private readonly counters = new Map<string, number>();
  // The map last handed out; the same instance until a name is added.
  private snapshot: ReadonlyMap<StableId, string> = new Map();

  /**
   * Names every component of `schematic` that has no name yet and returns all
   * names given so far. Components that are gone keep their entry, so a result
   * that still refers to one is shown under the name the user last saw. The
   * returned map is the same instance as long as no name was added.
   */
  labelsFor(schematic: Schematic): ReadonlyMap<StableId, string> {
    let added = false;

    for (const component of schematic.components) {
      if (!this.names.has(component.id)) {
        const prefix = PREFIXES[component.typeId];
        const number = (this.counters.get(prefix) ?? 0) + 1;
        this.counters.set(prefix, number);
        this.names.set(component.id, `${prefix}${number}`);
        added = true;
      }
    }

    if (added) {
      this.snapshot = new Map(this.names);
    }

    return this.snapshot;
  }

  /** Forgets every name; the next document is numbered from scratch. */
  reset(): void {
    this.names.clear();
    this.counters.clear();
    this.snapshot = new Map();
  }
}
