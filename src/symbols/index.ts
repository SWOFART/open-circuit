import type { ComponentTypeId } from "../generated/ComponentTypeId";
import { dcVoltageSourceSymbol } from "./dcVoltageSource";
import { resistorSymbol } from "./resistor";
import type { SymbolDefinition } from "./types";

export type { LocalPoint, SymbolDefinition, SymbolPrimitive } from "./types";

export type SymbolRegistry = Readonly<
  Record<ComponentTypeId, SymbolDefinition>
>;

/** Keyed by the generated type ID, so adding a device type forces a symbol. */
export const symbols: SymbolRegistry = {
  resistor: resistorSymbol,
  dc_voltage_source: dcVoltageSourceSymbol,
};
