import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import type { PinId } from "../generated/PinId";

/** A point in a symbol's local grid space. Pin points must be integers. */
export interface LocalPoint {
  readonly x: number;
  readonly y: number;
}

export type SymbolPrimitive =
  | { readonly kind: "polyline"; readonly points: readonly LocalPoint[] }
  | {
      readonly kind: "circle";
      readonly center: LocalPoint;
      readonly radius: number;
    };

export interface SymbolPin {
  readonly id: PinId;
  readonly local: LocalPoint;
}

/**
 * Where a press operates a switch or a push button (docs/contracts/stage-6-control-circuits.md
 * section 6.3): the lever or actuator, in the symbol's local grid. The rest of the symbol
 * selects and moves the part as before.
 */
export interface OperatingArea {
  readonly center: LocalPoint;
  readonly radius: number;
}

/**
 * The TypeScript side of the Stage 1 device contract: local pin geometry and
 * drawing. Pin IDs and counts must match the Rust catalog; a contract test
 * checks that.
 */
export interface SymbolDefinition {
  readonly typeId: ComponentTypeId;
  readonly pins: readonly SymbolPin[];
  /** The drawing at rest: before the first result, and when a compile fails. */
  readonly primitives: readonly SymbolPrimitive[];
  /**
   * The drawing for the state the Engine reported, so that a closed contact, a pressed
   * button, and a picked contactor can be told apart. The symbol never decides what
   * conducts: it draws what the state says.
   */
  readonly drawn?: (
    state: DeviceState | undefined,
  ) => readonly SymbolPrimitive[];
  /** Present on a symbol that the user operates while a simulation runs. */
  readonly operatingArea?: OperatingArea;
}
