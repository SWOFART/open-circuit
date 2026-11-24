import type { ComponentTypeId } from "../generated/ComponentTypeId";
import {
  acAmmeterSymbol,
  acVoltmeterSymbol,
  acVoltageSourceSymbol,
  threePhaseDeltaSymbol,
  threePhaseStarSymbol,
  wattmeterSymbol,
} from "./ac";
import { ammeterSymbol } from "./ammeter";
import { capacitorSymbol } from "./capacitor";
import { bjtNpnSymbol, bjtPnpSymbol } from "./bjt";
import { contactorSymbol } from "./contactor";
import { dcVoltageSourceSymbol } from "./dcVoltageSource";
import { diodeSymbol } from "./diode";
import { fuseSymbol } from "./fuse";
import { inductorSymbol } from "./inductor";
import { pmdcMotorSymbol } from "./motor";
import {
  earthElectrodeSymbol,
  exposedPartSymbol,
  rcdFourPoleSymbol,
  rcdTwoPoleSymbol,
} from "./protection";
import { pushButtonNcSymbol, pushButtonNoSymbol } from "./pushButton";
import { relaySpdtSymbol } from "./relay";
import { resistorSymbol } from "./resistor";
import { switchSpstSymbol } from "./switch";
import {
  breakerThreePoleSymbol,
  contactorThreePoleSymbol,
  inductionMotorThreePhaseSymbol,
  thermalOverloadThreePoleSymbol,
  threePhaseLoadDeltaSymbol,
  threePhaseLoadStarSymbol,
  transformerThreePhaseSymbol,
} from "./threePhase";
import type { SymbolDefinition } from "./types";
import { voltmeterSymbol } from "./voltmeter";

export type {
  LocalPoint,
  OperatingArea,
  SymbolDefinition,
  SymbolPrimitive,
} from "./types";

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
  ac_voltage_source: acVoltageSourceSymbol,
  three_phase_source_star: threePhaseStarSymbol,
  three_phase_source_delta: threePhaseDeltaSymbol,
  ac_voltmeter: acVoltmeterSymbol,
  ac_ammeter: acAmmeterSymbol,
  wattmeter: wattmeterSymbol,
  capacitor: capacitorSymbol,
  inductor: inductorSymbol,
  relay_spdt: relaySpdtSymbol,
  fuse: fuseSymbol,
  pmdc_motor: pmdcMotorSymbol,
  contactor_three_pole: contactorThreePoleSymbol,
  breaker_three_pole: breakerThreePoleSymbol,
  earth_electrode: earthElectrodeSymbol,
  exposed_part: exposedPartSymbol,
  rcd_two_pole: rcdTwoPoleSymbol,
  rcd_four_pole: rcdFourPoleSymbol,
  thermal_overload_three_pole: thermalOverloadThreePoleSymbol,
  induction_motor_three_phase: inductionMotorThreePhaseSymbol,
  transformer_three_phase: transformerThreePhaseSymbol,
  three_phase_load_star: threePhaseLoadStarSymbol,
  three_phase_load_delta: threePhaseLoadDeltaSymbol,
  voltmeter: voltmeterSymbol,
  ammeter: ammeterSymbol,
  switch_spst: switchSpstSymbol,
  push_button_no: pushButtonNoSymbol,
  push_button_nc: pushButtonNcSymbol,
  contactor: contactorSymbol,
  diode: diodeSymbol,
  bjt_npn: bjtNpnSymbol,
  bjt_pnp: bjtPnpSymbol,
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
