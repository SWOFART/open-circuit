import type { ResultSlot } from "../generated/ResultSlot";

/** The Engine publishes validity as a numeric companion to windowed measurements. */
export function measurementValidityKey(slot: ResultSlot): string | undefined {
  switch (slot.quantity) {
    case "frequency":
      return `${slot.componentId}/frequency_valid`;
    case "rmsVoltage":
    case "meanVoltage":
    case "peakVoltage":
    case "rmsCurrent":
    case "meanCurrent":
    case "peakCurrent":
    case "realPower":
    case "reactivePower":
    case "apparentPower":
    case "powerFactor":
    case "phaseAngle":
      return `${slot.componentId}/valid`;
    default:
      return undefined;
  }
}
