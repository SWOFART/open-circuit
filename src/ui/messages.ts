import type { Diagnostic } from "../generated/Diagnostic";
import type { DraftEvaluation } from "./units";

// User-facing prose lives in the UI. The Engine sends only stable keys.
const messages: Readonly<Record<string, string>> = {
  "drawing.documentation": "Drawing documentation",
  "drawing.titleBlock": "Title block",
  "drawing.title": "Title",
  "drawing.number": "Drawing number",
  "drawing.revision": "Revision",
  "drawing.author": "Author",
  "drawing.company": "Company",
  "drawing.date": "Date",
  "drawing.useToday": "Use today's date",
  "drawing.export": "Export drawing",
  "drawing.pageSize": "Page size",
  "drawing.autoSize": "Automatic",
  "drawing.preview": "Drawing and title block preview",
  "drawing.materials": "Bill of materials",
  "drawing.emptyMaterials": "The drawing has no components.",
  "drawing.terminalStrips": "Terminal strips",
  "drawing.unset": "Unset",
  "drawing.sheets": "Sheets",
  "drawing.activeSheet": "Current sheet",
  "drawing.fitSheet": "Fit sheet",
  "drawing.moveSheetEarlier": "Move sheet earlier",
  "drawing.moveSheetLater": "Move sheet later",
  "drawing.removeSheet": "Remove sheet",
  "drawing.addSheet": "Add sheet",
  "drawing.applySheet": "Apply sheet",
  "drawing.cancelSheet": "Cancel",
  "drawing.sheetName": "Sheet name",
  "drawing.sheetX": "Origin X (grid units)",
  "drawing.sheetY": "Origin Y (grid units)",
  "drawing.exportFolder": "Sheets exported to",
  "diagnostic.objectOutsideSheets": "An object is outside all sheets.",
  "diagnostic.objectCrossesSheetEdge":
    "A component crosses the printable border of its sheet.",
  "component.resistor.keywords": "resistance ohm load",
  "component.dcVoltageSource.keywords":
    "battery supply power source direct current dc",
  "component.acVoltageSource.keywords":
    "alternating current ac mains generator sine",
  "component.threePhaseSourceStar.keywords":
    "three phase 3 phase mains supply wye",
  "component.threePhaseSourceDelta.keywords":
    "three phase 3 phase mains supply triangle",
  "component.acVoltmeter.keywords": "meter measure alternating rms volt",
  "component.acAmmeter.keywords": "meter measure alternating rms current amp",
  "component.wattmeter.keywords": "power meter measure watt active reactive",
  "component.capacitor.keywords": "condenser farad",
  "component.inductor.keywords": "coil choke henry",
  "component.relaySpdt.keywords": "relay changeover coil contact",
  "component.fuse.keywords": "protection overcurrent breaker",
  "component.pmdcMotor.keywords": "motor load drive machine permanent magnet",
  "component.voltmeter.keywords": "meter measure volt",
  "component.ammeter.keywords": "meter measure current amp",
  "component.switchSpst.keywords": "toggle on off lever",
  "component.pushButtonNo.keywords": "button start momentary normally open",
  "component.pushButtonNc.keywords": "button stop momentary normally closed",
  "component.contactor.keywords": "relay starter coil contact",
  "component.diode.keywords": "rectifier semiconductor",
  "component.bjtNpn.keywords": "transistor bipolar semiconductor",
  "component.bjtPnp.keywords": "transistor bipolar semiconductor",
  "category.sources": "Sources",
  "category.passive": "Passive",
  "category.protection": "Protection",
  "category.switching": "Switching and control",
  "category.measurement": "Measurement",
  "category.motors_and_loads": "Motors and loads",
  "category.semiconductors": "Semiconductors",
  "category.favorites": "Favorites",
  "category.recent": "Recent",
  "component.resistor.name": "Resistor",
  "component.resistor.resistance": "Resistance",
  "component.dcVoltageSource.name": "DC voltage source",
  "component.dcVoltageSource.voltage": "Voltage",
  "component.dcVoltageSource.internalResistance": "Internal resistance",
  "component.acVoltageSource.name": "AC voltage source",
  "component.acVoltageSource.voltageRms": "RMS voltage",
  "component.acVoltageSource.frequency": "Frequency",
  "component.acVoltageSource.phaseOffset": "Phase offset",
  "component.acVoltageSource.internalResistance": "Internal resistance",
  "component.threePhaseSourceStar.name": "Three-phase star source",
  "component.threePhaseSourceStar.phaseVoltageRms": "Phase RMS voltage",
  "component.threePhaseSourceStar.frequency": "Frequency",
  "component.threePhaseSourceStar.phaseOffset": "Phase offset",
  "component.threePhaseSourceStar.internalResistance": "Internal resistance",
  "component.threePhaseSourceStar.reversedSequence": "Reversed phase sequence",
  "component.threePhaseSourceDelta.name": "Three-phase delta source",
  "component.threePhaseSourceDelta.lineVoltageRms": "Line RMS voltage",
  "component.threePhaseSourceDelta.frequency": "Frequency",
  "component.threePhaseSourceDelta.phaseOffset": "Phase offset",
  "component.threePhaseSourceDelta.internalResistance": "Internal resistance",
  "component.threePhaseSourceDelta.reversedSequence": "Reversed phase sequence",
  "component.acVoltmeter.name": "AC voltmeter",
  "component.acVoltmeter.inputResistance": "Input resistance",
  "component.acVoltmeter.frequency": "Measurement frequency",
  "component.acAmmeter.name": "AC ammeter",
  "component.acAmmeter.burdenResistance": "Burden resistance",
  "component.acAmmeter.frequency": "Measurement frequency",
  "component.contactorThreePole.name": "Three-pole contactor",
  "component.contactorThreePole.keywords":
    "three phase power contact switching starter",
  "component.contactorThreePole.coilResistance": "Coil resistance",
  "component.contactorThreePole.coilInductance": "Coil inductance",
  "component.contactorThreePole.pickupCurrent": "Pickup RMS current",
  "component.contactorThreePole.dropoutCurrent": "Dropout RMS current",
  "component.contactorThreePole.coilSense": "Coil detector time constant",
  "component.contactorThreePole.closedContactResistance":
    "Closed contact resistance",
  "component.breakerThreePole.name": "Three-pole breaker",
  "component.breakerThreePole.keywords": "three phase switch overcurrent trip",
  "component.breakerThreePole.closedContactResistance":
    "Closed contact resistance",
  "component.breakerThreePole.instantaneousTrip": "Instantaneous trip current",
  "component.breakerThreePole.initialClosed": "Initial position",
  "component.breakerThreePole.curveClass":
    "Trip class (0 instantaneous only, 1 B, 2 C, 3 D)",
  "component.breakerThreePole.ratedCurrent": "Rated current (classes B, C, D)",
  "component.breakerThreePole.thermalSense":
    "Thermal detector time constant (classes B, C, D)",
  "component.breakerThreePole.tripScale":
    "Thermal trip scale (classes B, C, D)",
  "component.breakerThreePole.cool": "Cooling time constant (classes B, C, D)",
  "component.thermalOverloadThreePole.name":
    "Three-pole thermal overload relay",
  "component.thermalOverloadThreePole.keywords":
    "three phase protection motor starter thermal trip",
  "component.thermalOverloadThreePole.closedContactResistance":
    "Closed auxiliary contact resistance",
  "component.thermalOverloadThreePole.senseResistance":
    "Phase sensing resistance",
  "component.thermalOverloadThreePole.ratedCurrent": "Rated phase current",
  "component.thermalOverloadThreePole.thermalSense":
    "Thermal detector time constant",
  "component.thermalOverloadThreePole.tripScale": "Trip delay scale",
  "component.thermalOverloadThreePole.cool": "Cooling time constant",
  "component.thermalOverloadThreePole.resetDose":
    "Maximum thermal dose for reset",
  "component.inductionMotorThreePhase.name": "Three-phase induction motor",
  "component.inductionMotorThreePhase.keywords": "asynchronous squirrel cage",
  "component.inductionMotorThreePhase.statorResistance": "Stator resistance",
  "component.inductionMotorThreePhase.rotorResistance": "Rotor resistance",
  "component.inductionMotorThreePhase.statorLeakage":
    "Stator leakage inductance",
  "component.inductionMotorThreePhase.rotorLeakage": "Rotor leakage inductance",
  "component.inductionMotorThreePhase.magnetizing": "Magnetizing inductance",
  "component.inductionMotorThreePhase.polePairs": "Pole pairs",
  "component.inductionMotorThreePhase.inertia": "Rotor inertia",
  "component.inductionMotorThreePhase.friction": "Viscous friction",
  "component.inductionMotorThreePhase.loadTorque": "Load torque",
  "component.transformerThreePhase.name": "Three-phase transformer",
  "component.transformerThreePhase.keywords": "coupled winding turns ratio",
  "component.transformerThreePhase.turnsRatio":
    "Primary-to-secondary turns ratio",
  "component.transformerThreePhase.primaryResistance":
    "Primary winding resistance",
  "component.transformerThreePhase.secondaryResistance":
    "Secondary winding resistance",
  "component.transformerThreePhase.primaryLeakage":
    "Primary leakage inductance",
  "component.transformerThreePhase.secondaryLeakage":
    "Secondary leakage inductance",
  "component.transformerThreePhase.magnetizing": "Magnetizing inductance",
  "component.rcdTwoPole.name": "Residual-current device, 2 poles",
  "component.rcdTwoPole.keywords": "RCD RCCB GFCI FI fault current protection",
  "component.rcdTwoPole.ratedResidualCurrent": "Rated residual current",
  "component.rcdTwoPole.tripTimeAtRated": "Trip time at rated current",
  "component.rcdTwoPole.residualSense": "Residual detector time constant",
  "component.rcdTwoPole.closedContactResistance": "Closed contact resistance",
  "component.rcdTwoPole.initialClosed": "Initially closed",
  "component.rcdFourPole.name": "Residual-current device, 4 poles",
  "component.rcdFourPole.keywords":
    "RCD RCCB FI fault current protection three phase",
  "component.rcdFourPole.ratedResidualCurrent": "Rated residual current",
  "component.rcdFourPole.tripTimeAtRated": "Trip time at rated current",
  "component.rcdFourPole.residualSense": "Residual detector time constant",
  "component.rcdFourPole.closedContactResistance": "Closed contact resistance",
  "component.rcdFourPole.initialClosed": "Initially closed",
  "component.earthElectrode.name": "Earth electrode",
  "component.earthElectrode.keywords":
    "ground earthing protective earth PE rod",
  "component.earthElectrode.earthResistance": "Earth resistance",
  "component.exposedPart.name": "Exposed part",
  "component.exposedPart.keywords":
    "frame housing enclosure touch voltage hazard body",
  "component.exposedPart.frequency": "Measurement frequency",
  "component.exposedPart.touchVoltageLimit": "Touch voltage limit",
  "component.threePhaseLoadStar.name": "Three-phase star load",
  "component.threePhaseLoadStar.keywords": "wye load impedance",
  "component.threePhaseLoadStar.phase1Resistance": "Branch 1 resistance",
  "component.threePhaseLoadStar.phase1Inductance": "Branch 1 inductance",
  "component.threePhaseLoadStar.phase2Resistance": "Branch 2 resistance",
  "component.threePhaseLoadStar.phase2Inductance": "Branch 2 inductance",
  "component.threePhaseLoadStar.phase3Resistance": "Branch 3 resistance",
  "component.threePhaseLoadStar.phase3Inductance": "Branch 3 inductance",
  "component.threePhaseLoadDelta.name": "Three-phase delta load",
  "component.threePhaseLoadDelta.keywords": "delta load impedance",
  "component.threePhaseLoadDelta.phase1Resistance": "Branch 1 resistance",
  "component.threePhaseLoadDelta.phase1Inductance": "Branch 1 inductance",
  "component.threePhaseLoadDelta.phase2Resistance": "Branch 2 resistance",
  "component.threePhaseLoadDelta.phase2Inductance": "Branch 2 inductance",
  "component.threePhaseLoadDelta.phase3Resistance": "Branch 3 resistance",
  "component.threePhaseLoadDelta.phase3Inductance": "Branch 3 inductance",
  "component.breakerThreePole.initialClosedHint":
    "The breaker starts open by default.",
  "component.wattmeter.name": "Wattmeter",
  "component.wattmeter.inputResistance": "Input resistance",
  "component.wattmeter.burdenResistance": "Burden resistance",
  "component.wattmeter.frequency": "Measurement frequency",
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
  "component.switchSpst.name": "Switch",
  "component.switchSpst.closedResistance": "Closed resistance",
  "component.switchSpst.initialClosed": "Initial position",
  "component.switchSpst.initialClosedHint":
    "The position when a simulation starts. The switch can be moved while it runs; that never changes the circuit you draw.",
  "component.pushButtonNo.name": "Push button (normally open)",
  "component.pushButtonNo.closedResistance": "Closed resistance",
  "component.pushButtonNc.name": "Push button (normally closed)",
  "component.pushButtonNc.closedResistance": "Closed resistance",
  "component.contactor.name": "Contactor",
  "component.contactor.coilResistance": "Coil resistance",
  "component.contactor.coilInductance": "Coil inductance",
  "component.contactor.pickupCurrent": "Pickup current",
  "component.contactor.dropoutCurrent": "Dropout current",
  "component.contactor.closedContactResistance": "Closed contact resistance",
  "component.diode.name": "Diode",
  "component.diode.saturationCurrent": "Saturation current",
  "component.diode.emissionCoefficient": "Emission coefficient",
  "component.bjtNpn.name": "NPN transistor",
  "component.bjtNpn.saturationCurrent": "Saturation current",
  "component.bjtNpn.forwardBeta": "Forward beta",
  "component.bjtNpn.reverseBeta": "Reverse beta",
  "component.bjtPnp.name": "PNP transistor",
  "component.bjtPnp.saturationCurrent": "Saturation current",
  "component.bjtPnp.forwardBeta": "Forward beta",
  "component.bjtPnp.reverseBeta": "Reverse beta",
  "assignment.error.malformed": "The file is not a valid assignment.",
  "assignment.error.unsupportedVersion":
    "This assignment file version is not supported.",
  "assignment.error.unknownField":
    "This field is not part of the assignment format.",
  "assignment.error.invalidField": "This value is not valid.",
  "assignment.error.duplicateId": "This ID is already used.",
  "assignment.error.unknownReference":
    "This refers to a part, part type, or signal that the starting circuit does not have.",
  "assignment.error.taskMismatch":
    "This does not fit the kind of task, or a field the task needs is missing.",
  "assignment.error.tooLarge":
    "There are too many entries, or the file is too large.",
  "check.value.pass": "The value is within the tolerance.",
  "check.value.fail": "The value is outside the tolerance.",
  "check.value.notEvaluated": "The value could not be checked.",
  "check.range.pass": "The value stayed within the range.",
  "check.range.fail": "The value left the range.",
  "check.range.notEvaluated": "The range could not be checked.",
  "check.deviceState.pass": "The part is in the expected state.",
  "check.deviceState.fail": "The part is not in the expected state.",
  "check.deviceState.notEvaluated": "The state could not be checked.",
  "check.componentCount.pass": "The number of parts is right.",
  "check.componentCount.fail": "The number of parts is wrong.",
  "check.componentCount.notEvaluated": "The parts could not be counted.",
  "check.locked.pass": "The locked parts are unchanged.",
  "check.locked.fail": "A locked part was changed or removed.",
  "check.outcome.passed": "Every check passed.",
  "check.outcome.partial": "Some checks passed.",
  "check.outcome.failed": "The circuit does not meet the task.",
  "check.outcome.notEvaluated": "The circuit could not be checked.",
  "diagnosis.entry.correct": "That part has that fault.",
  "diagnosis.entry.wrongKind": "That part is faulty, but not in that way.",
  "diagnosis.entry.incorrect": "That part has no fault.",
  "diagnosis.fault.found": "You found this fault.",
  "diagnosis.fault.missed": "You did not find this fault.",
  "diagnosis.outcome.passed": "You found every fault and named no wrong one.",
  "diagnosis.outcome.partial": "You found part of the problem.",
  "diagnosis.outcome.failed": "None of your findings were right.",
  "diagnostic.invalidFault":
    "A fault names a missing part, a kind of fault the part cannot have, or a value outside its range.",
  "diagnostic.invalidSchemaVersion":
    "The circuit file version is not supported.",
  "diagnostic.invalidGeometry": "The circuit geometry is invalid.",
  "diagnostic.invalidRailName":
    "Use a DC rail name of 1–32 ASCII letters, digits, underscores, plus or minus signs; begin with a letter or digit.",
  "diagnostic.duplicateRailName":
    "DC rail names must be unique in this document.",
  "diagnostic.railLimitExceeded":
    "The document exceeds the DC rail count or connection-work limit.",
  "diagnostic.unsupportedAssignmentRails":
    "DC rails are not supported in assignments yet.",
  "diagnostic.unsupportedAssignmentDocumentation":
    "Assignments hold no documentation (designators, wire numbers, terminals, notes, sheets).",
  "diagnostic.invalidDesignator":
    "A designator must be one to three capital letters followed by a number, such as K1 or Q12.",
  "diagnostic.duplicateDesignator": "Two components share the same designator.",
  "diagnostic.invalidWireNumber":
    "A wire number must be 1–16 ASCII letters, digits or + . _ - characters.",
  "diagnostic.wireNumberOnDifferentNets":
    "The same wire number appears on separate conductors.",
  "diagnostic.netHasTwoWireNumbers":
    "One conductor carries more than one wire number.",
  "diagnostic.wireNumberOnUnconnectedWire":
    "A numbered conductor does not connect to any component pin.",
  "diagnostic.invalidTerminal":
    "A terminal needs a strip name such as X1 and a label of 1–8 ASCII letters, digits or + . _ - characters.",
  "diagnostic.duplicateTerminal":
    "Two terminals share a strip and label or the same point.",
  "diagnostic.invalidNote":
    "A note must be 1–500 characters on at most 10 lines, without control characters.",
  "diagnostic.invalidTitleBlock":
    "A title-block field is too long or contains control characters.",
  "diagnostic.invalidSheet":
    "A sheet name must be 1–32 characters without control characters.",
  "diagnostic.overlappingSheets": "Two drawing sheets overlap.",
  "diagnostic.unknownDocumentationTarget":
    "Documentation refers to a component or wire that does not exist.",
  "diagnostic.documentationLimitExceeded":
    "The document exceeds a documentation limit (designators, wire numbers, terminals, notes or sheets).",
  "diagnostic.invalidParameter": "A component parameter is out of range.",
  "diagnostic.duplicateId": "Two circuit objects share the same ID.",
  "diagnostic.unknownComponentType":
    "The circuit has an unknown component type.",
  "diagnostic.unknownPin": "A component has a missing or unexpected pin.",
  "diagnostic.detachedReferenceMarker":
    "A ground marker is not placed on a wire or pin.",
  "diagnostic.noEarthPath":
    "An exposed part has no path to earth, so its touch voltage cannot be read. Add an earth electrode or a ground marker.",
  "diagnostic.exposedPartNotBonded":
    "An exposed part is not connected to a protective earth conductor.",
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
  "diagnostic.nonConvergence": "The nonlinear circuit did not converge.",
  "diagnostic.modelRangeExceeded":
    "A junction exceeds the supported model range.",
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
  range?: string | null,
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
    case "range":
      return range
        ? `That value is outside the range the circuit model allows: ${range}.`
        : "That value is outside the allowed range.";
  }
}
