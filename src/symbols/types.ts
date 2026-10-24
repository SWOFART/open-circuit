import type { ComponentTypeId } from "../generated/ComponentTypeId";
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
 * The TypeScript side of the Stage 1 device contract: local pin geometry and
 * drawing. Pin IDs and counts must match the Rust catalog; a contract test
 * checks that.
 */
export interface SymbolDefinition {
  readonly typeId: ComponentTypeId;
  readonly pins: readonly SymbolPin[];
  readonly primitives: readonly SymbolPrimitive[];
}
