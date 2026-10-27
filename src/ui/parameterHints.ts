import type { ParameterDefinition } from "../generated/ParameterDefinition";
import type { Unit } from "../generated/Unit";

// Guidance the catalog does not carry: how a parameter relates to the others, and what a
// sign or an unset value means. It restates the Stage 3 and Stage 4 contracts as text; the
// Engine alone checks the values.

const NOTES: Readonly<Record<string, string>> = {
  pickupCurrentAmps:
    "The coil current magnitude at which the contacts change over.",
  dropoutCurrentAmps:
    "Must be below the pickup current; the relay lets go at or below this.",
  tripI2tAmpereSquaredSeconds:
    "A one-shot threshold: a tripped fuse stays open until the circuit is compiled again.",
  loadTorqueNewtonMeters:
    "Signed. A negative torque opposes positive rotation.",
  motorConstantNewtonMetersPerAmp: "Also the back-EMF constant.",
};

/** A line of guidance shown under the parameter's field, if there is one. */
export function parameterNote(
  definition: ParameterDefinition,
): string | undefined {
  return NOTES[definition.id];
}

const UNSET: Readonly<Record<string, string>> = {
  initialAngularVelocityRadiansPerSecond: "Not set: starts at 0 rad/s.",
  initialArmatureCurrentAmps:
    "Not set: starts from the DC solution at the initial speed.",
};

/** What leaving an optional parameter unset means. */
export function unsetHint(definition: ParameterDefinition): string {
  return UNSET[definition.id] ?? "Not set: starts from the operating point.";
}

const SIGN: Readonly<Record<string, string>> = {
  initialAngularVelocityRadiansPerSecond:
    "Positive speed is the rotation that positive current into pin1 drives.",
};

/** What the sign of an initial value refers to. */
export function signHint(definition: ParameterDefinition): string {
  const own = SIGN[definition.id];

  if (own !== undefined) {
    return own;
  }

  return signReference(definition.unit);
}

function signReference(unit: Unit): string {
  switch (unit) {
    case "volt":
      return "Voltage V(pin1) − V(pin2) at time zero.";
    case "amp":
      return "Current entering pin1 at time zero.";
    default:
      return "Value at time zero.";
  }
}
