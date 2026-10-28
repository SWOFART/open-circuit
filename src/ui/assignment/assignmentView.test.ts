import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import divider from "../../../tests/golden/stage-5-divider-build.ocassignment?raw";
import type { Assignment } from "../../generated/Assignment";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { ResultLayout } from "../../generated/ResultLayout";
import {
  answerFor,
  checkTitle,
  describeAssignmentError,
  deviceStatesFor,
  errorsAt,
  faultKindsFor,
  faultParametersFor,
  formatSeconds,
  hasErrorAt,
  locateError,
  newCountCheck,
  newDeviceStateCheck,
  newRangeCheck,
  newValueCheck,
  parseSeconds,
  signalOptions,
} from "./assignmentView";

const catalog = catalogFixture as ComponentCatalog;
const assignment = JSON.parse(divider) as Assignment;
const V1 = "00000000-0000-4000-8000-000000000001";
const VM1 = "00000000-0000-4000-8000-000000000002";
const EXTRA = "00000000-0000-4000-8000-000000000011";
const labels = new Map([
  [V1, "V1"],
  [VM1, "VM1"],
  [EXTRA, "R1"],
]);

function error(code: AssignmentError["code"], path: string): AssignmentError {
  return {
    code,
    path,
    messageKey: `assignment.error.${code.charAt(0).toLowerCase()}${code.slice(1)}`,
  };
}

describe("times", () => {
  it.each([
    ["0", 0],
    ["2", 2],
    ["0.002", 0.002],
    ["5 ms", 0.005],
    ["5ms", 0.005],
    ["250us", 0.00025],
    ["250 µs", 0.00025],
    ["1.5e-3 s", 0.0015],
    ["60 s", 60],
    [" 10 m ", 0.01],
  ])("reads %s as %s seconds", (text, seconds) => {
    expect(parseSeconds(text)).toEqual({ ok: true, value: seconds });
  });

  it.each(["", "ms", "five", "5 volts", "5 kms", "1,5", "--1", "1e999"])(
    "refuses %j",
    (text) => {
      expect(parseSeconds(text).ok).toBe(false);
    },
  );

  it("shows a time with a prefix and a unit", () => {
    expect(formatSeconds(0.005)).toBe("5 ms");
    expect(formatSeconds(0)).toBe("0 s");
    expect(formatSeconds(2)).toBe("2 s");
  });

  it("leaves the decision whether a time is allowed to the Engine", () => {
    // Not a multiple of the 1 ms step: the field accepts it, the Engine's errors refuse it.
    expect(parseSeconds("0.0005")).toEqual({ ok: true, value: 0.0005 });
    expect(parseSeconds("-1")).toEqual({ ok: true, value: -1 });
  });
});

describe("answers of the Engine", () => {
  it("are shown only for the draft version they were asked for", () => {
    const answer = { version: 4, value: ["problem"] };

    expect(answerFor(answer, 4)).toEqual(["problem"]);
    expect(answerFor(answer, 5)).toBeNull();
    expect(answerFor(answer, 3)).toBeNull();
    expect(answerFor(null, 4)).toBeNull();
  });
});

describe("the signals a check can read", () => {
  const layout: ResultLayout = {
    layoutVersion: 1,
    valueCount: 4,
    slots: [
      slot(V1, `${V1}:voltage`, "volt", 0),
      slot(VM1, `${VM1}:voltage`, "volt", 1),
      slot(VM1, `${VM1}:current`, "amp", 2),
      // A part only the circuit in the editor has.
      slot(EXTRA, `${EXTRA}:voltage`, "volt", 3),
    ],
  };

  function slot(
    componentId: string,
    key: string,
    unit: "volt" | "amp",
    offset: number,
  ): ResultLayout["slots"][number] {
    return {
      key,
      componentId,
      quantity: unit === "volt" ? "voltage" : "current",
      unit,
      signConvention:
        unit === "volt" ? "voltagePin1MinusPin2" : "currentIntoPin1",
      offset,
    };
  }

  it("lists the Engine's slots of the parts the starting circuit has, with their units", () => {
    const options = signalOptions(layout, assignment.startingCircuit!, labels);

    expect(options.map((option) => option.key)).toEqual([
      `${V1}:voltage`,
      `${VM1}:current`,
      `${VM1}:voltage`,
    ]);
    expect(options.map((option) => option.label)).toEqual([
      "V1 voltage",
      "VM1 current",
      "VM1 voltage",
    ]);
    expect(options.map((option) => option.unit)).toEqual([
      "volt",
      "amp",
      "volt",
    ]);
  });

  it("offers none without a layout or a starting circuit", () => {
    expect(signalOptions(null, assignment.startingCircuit!, labels)).toEqual(
      [],
    );
    expect(signalOptions(layout, null, labels)).toEqual([]);
  });
});

describe("what the catalog says about a part", () => {
  it("lists the device states of a relay and a fuse only", () => {
    expect(deviceStatesFor("relay_spdt")).toEqual(["released", "picked"]);
    expect(deviceStatesFor("fuse")).toEqual(["intact", "tripped"]);
    expect(deviceStatesFor("resistor")).toEqual([]);
    expect(deviceStatesFor("pmdc_motor")).toEqual([]);
  });

  it("reads the fault kinds from the catalog entry", () => {
    expect(faultKindsFor(catalog, "resistor")).toEqual([
      "open",
      "short",
      "parameter",
    ]);
    expect(faultKindsFor(catalog, "dc_voltage_source")).toEqual([
      "open",
      "parameter",
    ]);
    expect(faultKindsFor(catalog, "fuse")).toEqual(["parameter"]);
    expect(faultKindsFor(null, "resistor")).toEqual([]);
  });

  it("offers a parameter fault only the parameters that are not initial conditions", () => {
    expect(faultParametersFor(catalog, "capacitor").map((p) => p.id)).toEqual([
      "capacitanceFarads",
    ]);
    expect(faultParametersFor(catalog, "resistor").map((p) => p.id)).toEqual([
      "resistanceOhms",
    ]);
    expect(
      faultParametersFor(catalog, "pmdc_motor").every(
        (p) => !p.initialCondition,
      ),
    ).toBe(true);
  });
});

describe("the Engine's errors in words", () => {
  it.each([
    ["", "assignment", "The assignment"],
    ["/title", "details", "Title"],
    ["/instructions", "details", "Instructions"],
    ["/task/reveal", "details", "Task"],
    ["/startingCircuit", "circuits", "Starting circuit"],
    ["/startingCircuit/components/2", "circuits", "Starting circuit"],
    ["/reference/components/0", "circuits", "Reference solution"],
    ["/restrictions/lockedComponents/1", "restrictions", "Locked parts"],
    [
      "/restrictions/allowedComponentTypes",
      "restrictions",
      "Allowed part types",
    ],
    ["/checks", "checks", "Checks"],
    ["/checks/2", "checks", "Check 3"],
    ["/checks/0/atSeconds", "checks", "Check 1, time"],
    ["/checks/1/signal/key", "checks", "Check 2, signal"],
    ["/checks/1/signal/unit", "checks", "Check 2, signal unit"],
    ["/checks/3/toleranceAbs", "checks", "Check 4, absolute tolerance"],
    ["/faults/0/componentId", "faults", "Fault 1, part"],
    ["/faults/1/parameterId", "faults", "Fault 2, parameter"],
    ["/grade", "assignment", "The assignment"],
  ])("places %j in %s as %j", (path, section, where) => {
    const located = locateError(path);

    expect(located.section).toBe(section);
    expect(located.where).toBe(where);
  });

  it("finds the entry and the field a pointer names", () => {
    expect(locateError("/checks/4/min")).toMatchObject({
      section: "checks",
      index: 4,
      field: "min",
    });
    expect(locateError("/faults/0/value")).toMatchObject({
      section: "faults",
      index: 0,
      field: "value",
    });
    expect(locateError("/checks/1/signal/key").field).toBe("signal/key");
  });

  it("uses the message of the error's key, and says why a faulted part must be locked", () => {
    expect(describeAssignmentError(error("InvalidField", "/title"))).toBe(
      "This value is not valid.",
    );
    expect(describeAssignmentError(error("DuplicateId", "/checks/1/id"))).toBe(
      "This ID is already used.",
    );
    expect(
      describeAssignmentError(error("TaskMismatch", "/faults/0/componentId")),
    ).toBe(
      "A part with a fault must be locked, so a student cannot change or delete it.",
    );
    expect(describeAssignmentError(error("TaskMismatch", "/faults"))).toBe(
      "This does not fit the kind of task, or a field the task needs is missing.",
    );
  });

  it("groups errors by entry and finds the field", () => {
    const errors = [
      error("InvalidField", "/title"),
      error("InvalidField", "/checks/0/atSeconds"),
      error("UnknownReference", "/checks/0/signal/key"),
      error("InvalidField", "/checks/2/min"),
      error("InvalidField", "/faults/0/kind"),
    ];

    expect(errorsAt(errors, "checks", 0)).toHaveLength(2);
    expect(errorsAt(errors, "checks")).toHaveLength(3);
    expect(errorsAt(errors, "faults", 1)).toHaveLength(0);
    expect(hasErrorAt(errors, "details", "title")).toBe(true);
    expect(hasErrorAt(errors, "checks", "atSeconds", 0)).toBe(true);
    expect(hasErrorAt(errors, "checks", "atSeconds", 2)).toBe(false);
    // A field with a path below it (the signal has a key and a unit).
    expect(hasErrorAt(errors, "checks", "signal", 0)).toBe(true);
    expect(hasErrorAt(errors, "faults", "kind", 0)).toBe(true);
  });
});

describe("new checks start from values the Engine accepts in a document", () => {
  const id = "00000000-0000-4000-8000-0000000000c9";
  const golden = assignment.checks!;
  const keysOf = (value: object): string[] => Object.keys(value).sort();

  it("have exactly the fields of the canonical file's checks, in the generated shape", () => {
    const value = newValueCheck(id, { key: `${VM1}:voltage`, unit: "volt" });
    const range = newRangeCheck(id, { key: `${VM1}:voltage`, unit: "volt" });
    const count = newCountCheck(id, "resistor");

    expect(keysOf(value)).toEqual(
      keysOf(golden[0]!).filter((key) => key !== "label"),
    );
    expect(keysOf(range)).toEqual(keysOf(golden[1]!));
    expect(keysOf(count)).toEqual(
      keysOf(golden[2]!).filter((key) => key !== "max"),
    );
  });

  it("start a device state check with the first state of the part's type", () => {
    expect(newDeviceStateCheck(id, VM1, "fuse")).toMatchObject({
      kind: "deviceState",
      state: "intact",
    });
    expect(newDeviceStateCheck(id, VM1, "relay_spdt")).toMatchObject({
      state: "released",
    });
  });

  it("are named by position and by their own label when they have one", () => {
    const check = newCountCheck(id, "resistor");

    expect(checkTitle(check, 0)).toBe("Check 1: Number of parts");
    expect(checkTitle({ ...check, label: " Two resistors " }, 1)).toBe(
      "Check 2: Two resistors",
    );
    expect(checkTitle(newValueCheck(id, { key: "k", unit: "volt" }), 2)).toBe(
      "Check 3: Value at a time",
    );
  });
});
