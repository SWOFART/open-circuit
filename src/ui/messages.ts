import type { Diagnostic } from "../generated/Diagnostic";
import type { DraftEvaluation } from "./units";

// User-facing prose lives in the UI. The Engine sends only stable keys.
const messages: Readonly<Record<string, string>> = {
  "component.resistor.name": "Resistor",
  "component.resistor.resistance": "Resistance",
  "component.dcVoltageSource.name": "DC voltage source",
  "component.dcVoltageSource.voltage": "Voltage",
  "component.dcVoltageSource.internalResistance": "Internal resistance",
  "component.capacitor.name": "Capacitor",
  "component.capacitor.capacitance": "Capacitance",
  "component.capacitor.initialVoltage": "Initial voltage",
  "component.inductor.name": "Inductor",
  "component.inductor.inductance": "Inductance",
  "component.inductor.initialCurrent": "Initial current",
  "component.relaySpdt.name": "SPDT relay",
  "component.relaySpdt.coilResistance": "Coil resistance",
  "component.relaySpdt.coilInductance": "Coil inductance",
  "component.relaySpdt.pickupCurrent": "Pickup current",
  "component.relaySpdt.dropoutCurrent": "Dropout current",
  "component.relaySpdt.closedContactResistance": "Closed contact resistance",
  "component.fuse.name": "Fuse",
  "component.fuse.intactResistance": "Intact resistance",
  "component.fuse.tripI2t": "I²t trip threshold",
  "component.pmdcMotor.name": "DC motor",
  "component.pmdcMotor.armatureResistance": "Armature resistance",
  "component.pmdcMotor.armatureInductance": "Armature inductance",
  "component.pmdcMotor.motorConstant": "Motor constant",
  "component.pmdcMotor.rotorInertia": "Rotor inertia",
  "component.pmdcMotor.viscousFriction": "Viscous friction",
  "component.pmdcMotor.loadTorque": "Load torque",
  "component.pmdcMotor.initialAngularVelocity": "Initial angular velocity",
  "component.pmdcMotor.initialArmatureCurrent": "Initial armature current",
  "component.voltmeter.name": "Voltmeter",
  "component.voltmeter.inputResistance": "Input resistance",
  "component.ammeter.name": "Ammeter",
  "component.ammeter.burdenResistance": "Burden resistance",
  "diagnostic.invalidSchemaVersion":
    "The circuit file version is not supported.",
  "diagnostic.invalidGeometry": "The circuit geometry is invalid.",
  "diagnostic.invalidParameter": "A component parameter is out of range.",
  "diagnostic.duplicateId": "Two circuit objects share the same ID.",
  "diagnostic.unknownComponentType":
    "The circuit has an unknown component type.",
  "diagnostic.unknownPin": "A component has a missing or unexpected pin.",
  "diagnostic.detachedReferenceMarker":
    "A ground marker is not placed on a wire or pin.",
  "diagnostic.noGroundReference":
    "A connected part of the circuit has no ground marker.",
  "diagnostic.floatingNode": "A node is not connected to any reference.",
  "diagnostic.shortCircuit": "An ideal source is short-circuited.",
  "diagnostic.inconsistentIdealConstraint":
    "Ideal voltage sources in a loop conflict with each other.",
  "diagnostic.inconsistentState":
    "The specified initial capacitor voltages or inductor currents conflict with ideal circuit constraints.",
  "diagnostic.singularSystem": "The circuit cannot be solved as drawn.",
  "diagnostic.eventChatter":
    "A relay or fuse changes state again and again at the same instant, so the circuit cannot settle.",
  "diagnostic.nonFiniteState":
    "A device state, such as a fuse's accumulated I²t or a motor's speed, became too large to represent.",
  "diagnostic.stepAccuracyLimited":
    "The circuit is too fast for the available internal step limit; simulation accuracy may be limited.",
  "diagnostic.staleRun":
    "The result belongs to an earlier calculation and was ignored.",
  "diagnostic.staleDocumentRevision":
    "The circuit changed while this update was in progress, so it was ignored.",
};

/** Looks up a localization key, falling back to the key itself. */
export function message(key: string): string {
  return messages[key] ?? key;
}

export function describeDiagnostic(diagnostic: Diagnostic): string {
  return message(diagnostic.localizationKey);
}

export function describeInvalidDraft(
  reason: Extract<DraftEvaluation, { state: "invalid" }>["reason"],
  unitSymbol: string,
): string {
  switch (reason) {
    case "empty":
      return "Enter a value.";
    case "format":
      return `Enter a number, optionally with a prefix and ${unitSymbol}, for example 4.7k.`;
    case "not-finite":
      return "That number is too large.";
    case "domain":
      return "That value is outside the allowed range.";
  }
}
