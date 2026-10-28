import type { AssignmentError } from "../../generated/AssignmentError";
import type { CheckCriterion } from "../../generated/CheckCriterion";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { ComponentTypeId } from "../../generated/ComponentTypeId";
import type { DeviceStateName } from "../../generated/DeviceStateName";
import type { FaultKind } from "../../generated/FaultKind";
import type { ParameterDefinition } from "../../generated/ParameterDefinition";
import type { ResultLayout } from "../../generated/ResultLayout";
import type { ResultSlot } from "../../generated/ResultSlot";
import type { Schematic } from "../../generated/Schematic";
import type { StableId } from "../../generated/StableId";
import { message } from "../messages";
import { slotLabel } from "../slotLabels";
import { formatWithSymbol } from "../units";

// Presentation helpers for the authoring panel. None of them judges a circuit: what a valid
// check, fault, or restriction is belongs to the Engine, which validates the document and
// answers with errors and a result. These helpers only offer the choices the Engine's own
// data lists (the catalog, the result layout) and put its answers into words.

/** A signal a check can read: a result slot of a part of the starting circuit. */
export interface SignalOption {
  readonly key: string;
  readonly unit: ResultSlot["unit"];
  readonly componentId: StableId;
  readonly label: string;
}

/**
 * The signals of the parts that are in the starting circuit, from the layout the Engine
 * reported for the circuit in the editor. A part the circuit in the editor no longer has
 * offers no signal, and neither does one the starting circuit does not have.
 */
export function signalOptions(
  layout: ResultLayout | null,
  startingCircuit: Schematic | null,
  labels: ReadonlyMap<StableId, string>,
): SignalOption[] {
  if (layout === null || startingCircuit === null) return [];

  const inStartingCircuit = new Set(
    startingCircuit.components.map((component) => component.id),
  );

  return layout.slots
    .filter((slot) => inStartingCircuit.has(slot.componentId))
    .map((slot) => ({
      key: slot.key,
      unit: slot.unit,
      componentId: slot.componentId,
      label: slotLabel(slot, labels),
    }))
    .sort((left, right) => left.label.localeCompare(right.label, "en"));
}

/** Device states a part can be checked for, by the part's type (contract section 4.4). */
export function deviceStatesFor(typeId: ComponentTypeId): DeviceStateName[] {
  switch (typeId) {
    case "relay_spdt":
      return ["released", "picked"];
    case "fuse":
      return ["intact", "tripped"];
    default:
      return [];
  }
}

export const deviceStateWords: Readonly<Record<DeviceStateName, string>> = {
  released: "released",
  picked: "picked up",
  intact: "intact",
  tripped: "tripped",
};

/** The fault kinds the Engine's catalog lists for a type. */
export function faultKindsFor(
  catalog: ComponentCatalog | null,
  typeId: ComponentTypeId,
): FaultKind[] {
  const entry = catalog?.componentTypes.find((type) => type.typeId === typeId);
  return entry ? [...entry.faultKinds] : [];
}

/** The parameters a `parameter` fault may name: the catalog's, without initial conditions. */
export function faultParametersFor(
  catalog: ComponentCatalog | null,
  typeId: ComponentTypeId,
): ParameterDefinition[] {
  const entry = catalog?.componentTypes.find((type) => type.typeId === typeId);
  return entry
    ? entry.parameters.filter((parameter) => !parameter.initialCondition)
    : [];
}

export const faultKindWords: Readonly<Record<FaultKind, string>> = {
  open: "Open (no current flows)",
  short: "Short (1 mΩ)",
  parameter: "Wrong parameter value",
};

/** A time in seconds typed as `2`, `5 ms`, `250us`, or `1.5e-3 s`. */
export type SecondsResult =
  { readonly ok: true; readonly value: number } | { readonly ok: false };

const SECONDS_PATTERN =
  /^([+-]?(?:\d+(?:\.\d*)?|\.\d+))(?:[eE]([+-]?\d+))?\s*([mµμun]?)\s*(?:s|sec|second|seconds)?$/i;
const SECONDS_EXPONENT: Readonly<Record<string, number>> = {
  "": 0,
  m: -3,
  µ: -6,
  μ: -6,
  u: -6,
  n: -9,
};

export function parseSeconds(text: string): SecondsResult {
  const match = SECONDS_PATTERN.exec(text.trim());
  if (!match) return { ok: false };

  const [, mantissa = "", exponentText = "0", prefix = ""] = match;
  // `M` and `m` are both milli here: a time on a circuit is never mega-seconds.
  const power = SECONDS_EXPONENT[prefix.toLowerCase()];
  if (power === undefined) return { ok: false };

  const value = Number(`${mantissa}e${Number(exponentText) + power}`);
  return Number.isFinite(value) ? { ok: true, value } : { ok: false };
}

export function formatSeconds(value: number): string {
  return formatWithSymbol(value, "s");
}

/**
 * An answer from the Engine (errors, a check result) belongs to the draft it was asked for. Once
 * the draft has changed, in any field, the answer is not shown: its pointers and numbers
 * would describe something that is no longer there.
 */
export function answerFor<T>(
  answer: { readonly version: number; readonly value: T } | null,
  version: number,
): T | null {
  return answer !== null && answer.version === version ? answer.value : null;
}

// --- the Engine's errors in words ------------------------------------------------------

export type ErrorSection =
  "assignment" | "details" | "circuits" | "restrictions" | "checks" | "faults";

export interface ErrorLocation {
  readonly section: ErrorSection;
  /** The 0-based position in the checks or faults list. */
  readonly index?: number;
  /** The field inside the entry, such as `atSeconds`. */
  readonly field?: string;
  readonly where: string;
}

const CHECK_FIELDS: Readonly<Record<string, string>> = {
  id: "ID",
  kind: "kind",
  label: "label",
  signal: "signal",
  "signal/key": "signal",
  "signal/unit": "signal unit",
  atSeconds: "time",
  fromSeconds: "start time",
  toSeconds: "end time",
  expected: "expected value",
  toleranceAbs: "absolute tolerance",
  toleranceRel: "relative tolerance",
  min: "minimum",
  max: "maximum",
  componentId: "part",
  state: "state",
  typeId: "part type",
};

const FAULT_FIELDS: Readonly<Record<string, string>> = {
  id: "ID",
  kind: "kind",
  componentId: "part",
  parameterId: "parameter",
  value: "value",
};

/** Where a JSON pointer of an Engine error points, in words and as a section and field. */
export function locateError(path: string): ErrorLocation {
  const parts = path.split("/").slice(1);
  const [head, ...rest] = parts;

  switch (head) {
    case undefined:
      return { section: "assignment", where: "The assignment" };
    case "title":
      return { section: "details", field: "title", where: "Title" };
    case "instructions":
      return {
        section: "details",
        field: "instructions",
        where: "Instructions",
      };
    case "task":
      return { section: "details", field: "task", where: "Task" };
    case "startingCircuit":
      return { section: "circuits", field: head, where: "Starting circuit" };
    case "reference":
      return { section: "circuits", field: head, where: "Reference solution" };
    case "restrictions":
      return {
        section: "restrictions",
        field: rest[0],
        where:
          rest[0] === "allowedComponentTypes"
            ? "Allowed part types"
            : "Locked parts",
      };
    case "checks":
    case "faults": {
      const faults = head === "faults";
      const index = Number(rest[0]);
      if (!Number.isInteger(index)) {
        return {
          section: head,
          where: faults ? "Faults" : "Checks",
        };
      }
      const field = rest.slice(1).join("/");
      const names = faults ? FAULT_FIELDS : CHECK_FIELDS;
      const fieldName = names[field];
      return {
        section: head,
        index,
        field: field === "" ? undefined : field,
        where: `${faults ? "Fault" : "Check"} ${index + 1}${fieldName ? `, ${fieldName}` : ""}`,
      };
    }
    default:
      return { section: "assignment", where: "The assignment" };
  }
}

/** The sentence for an error. A fault on a part that is not locked is told apart. */
export function describeAssignmentError(error: AssignmentError): string {
  const location = locateError(error.path);

  if (
    error.code === "TaskMismatch" &&
    location.section === "faults" &&
    location.field === "componentId"
  ) {
    return "A part with a fault must be locked, so a student cannot change or delete it.";
  }

  return message(error.messageKey);
}

/** The errors that belong to one entry of a list, or to one field of the details. */
export function errorsAt(
  errors: readonly AssignmentError[],
  section: ErrorSection,
  index?: number,
): AssignmentError[] {
  return errors.filter((error) => {
    const location = locateError(error.path);
    return (
      location.section === section &&
      (index === undefined || location.index === index)
    );
  });
}

export function hasErrorAt(
  errors: readonly AssignmentError[],
  section: ErrorSection,
  field: string,
  index?: number,
): boolean {
  return errorsAt(errors, section, index).some((error) => {
    const located = locateError(error.path).field;
    return located === field || located?.startsWith(`${field}/`) === true;
  });
}

/** A one-line name for a check in the list. */
export function checkTitle(criterion: CheckCriterion, index: number): string {
  const names: Readonly<Record<CheckCriterion["kind"], string>> = {
    value: "Value at a time",
    range: "Value within a range",
    deviceState: "Device state",
    componentCount: "Number of parts",
  };
  const label = criterion.label?.trim();

  return `Check ${index + 1}: ${label ? label : names[criterion.kind]}`;
}

// --- the starting values of a new check or fault ------------------------------------------

/** A value check at time zero on `signal`, with every number zero until the teacher sets it. */
export function newValueCheck(
  id: StableId,
  signal: Pick<SignalOption, "key" | "unit">,
): CheckCriterion {
  return {
    kind: "value",
    id,
    signal: { key: signal.key, unit: signal.unit },
    atSeconds: 0,
    expected: 0,
    toleranceAbs: 0,
    toleranceRel: 0,
  };
}

export function newRangeCheck(
  id: StableId,
  signal: Pick<SignalOption, "key" | "unit">,
): CheckCriterion {
  return {
    kind: "range",
    id,
    signal: { key: signal.key, unit: signal.unit },
    fromSeconds: 0,
    toSeconds: 0.01,
    min: 0,
    max: 0,
  };
}

export function newDeviceStateCheck(
  id: StableId,
  componentId: StableId,
  typeId: ComponentTypeId,
): CheckCriterion {
  return {
    kind: "deviceState",
    id,
    componentId,
    state: deviceStatesFor(typeId)[0] ?? "released",
    atSeconds: 0.01,
  };
}

export function newCountCheck(
  id: StableId,
  typeId: ComponentTypeId,
): CheckCriterion {
  return { kind: "componentCount", id, typeId, min: 1 };
}
