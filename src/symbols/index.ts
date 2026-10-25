import type { ComponentTypeId } from "../generated/ComponentTypeId";
import { capacitorSymbol } from "./capacitor";
import { dcVoltageSourceSymbol } from "./dcVoltageSource";
import { inductorSymbol } from "./inductor";
import { resistorSymbol } from "./resistor";
import type { SymbolDefinition } from "./types";

export type { LocalPoint, SymbolDefinition, SymbolPrimitive } from "./types";

export type SymbolRegistry = Readonly<
  Partial<Record<ComponentTypeId, SymbolDefinition>>
>;

/**
 * Visual symbols are added when the editor supports a device. The Engine catalog
 * may include devices that are not yet placeable in the UI.
 */
export const symbols: SymbolRegistry = {
  resistor: resistorSymbol,
  dc_voltage_source: dcVoltageSourceSymbol,
  capacitor: capacitorSymbol,
  inductor: inductorSymbol,
};

/** Returns a drawing symbol or explains why a component cannot be placed yet. */
export function symbolFor(
  typeId: ComponentTypeId,
  registry: SymbolRegistry = symbols,
): SymbolDefinition {
  const symbol = registry[typeId];
  if (!symbol) {
    throw new Error(`No UI symbol is registered for component type ${typeId}.`);
  }
  return symbol;
}
