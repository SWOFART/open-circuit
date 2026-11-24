import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import earthFault from "../../../tests/golden/stage-13-earth-fault.ocassignment?raw";
import { emptyDocumentation } from "../../editor/documentation";
import type { Assignment } from "../../generated/Assignment";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { DiagnosisResult } from "../../generated/DiagnosisResult";
import type { Schematic } from "../../generated/Schematic";
import {
  allowedCatalog,
  commandVerdict,
  restrictionsOf,
} from "../../store/studentAssignment";
import { DiagnosisEditor } from "./DiagnosisEditor";
import { DiagnosisResultView } from "./DiagnosisResultView";
import { FaultEditor } from "./FaultEditor";
import {
  describeAssignmentError,
  exposedPartsOf,
  faultKindsFor,
  faultPinsFor,
  locateError,
  pinWords,
} from "./assignmentView";

const catalog = catalogFixture as ComponentCatalog;

const SOURCE = "00000000-0000-4000-8000-000000000001";
const LOAD = "00000000-0000-4000-8000-000000000002";
const FRAME = "00000000-0000-4000-8000-000000000003";
const ELECTRODE = "00000000-0000-4000-8000-000000000004";

function parsed(text: string): Assignment {
  const assignment = JSON.parse(text) as Assignment;
  const starting = assignment.startingCircuit as Schematic;
  return {
    ...assignment,
    startingCircuit: { ...starting, documentation: emptyDocumentation() },
  };
}

const assignment = parsed(earthFault);
const startingCircuit = assignment.startingCircuit!;
const leak = assignment.faults![0]!;
const labels = new Map([
  [SOURCE, "G1"],
  [LOAD, "R1"],
  [FRAME, "EP1"],
  [ELECTRODE, "PE1"],
]);

function error(code: AssignmentError["code"], path: string): AssignmentError {
  return {
    code,
    path,
    messageKey: `assignment.error.${code.charAt(0).toLowerCase()}${code.slice(1)}`,
  };
}

describe("what the catalog says about the Stage 13 faults", () => {
  it("lists the leak for the parts of section 3.2 and not for the earth devices", () => {
    expect(faultKindsFor(catalog, "three_phase_load_star")).toEqual(["leak"]);
    expect(faultKindsFor(catalog, "resistor")).toContain("leak");
    expect(faultKindsFor(catalog, "earth_electrode")).toEqual([]);
    expect(faultKindsFor(catalog, "exposed_part")).toEqual([]);
  });

  it("offers the pins of the part in catalog order, in words", () => {
    expect(faultPinsFor(catalog, "three_phase_load_star")).toEqual([
      "l1",
      "l2",
      "l3",
      "n",
    ]);
    expect(faultPinsFor(catalog, "resistor")).toEqual(["pin1", "pin2"]);
    expect(faultPinsFor(null, "resistor")).toEqual([]);
    expect(pinWords("pin2")).toBe("Pin 2");
    expect(pinWords("l1")).toBe("L1");
  });

  it("finds the exposed parts of the starting circuit", () => {
    expect(exposedPartsOf(startingCircuit)).toEqual([FRAME]);
    expect(exposedPartsOf(null)).toEqual([]);
  });

  it("names the new fields of a fault entry in the Engine's errors", () => {
    expect(locateError("/faults/0/pinId").where).toContain("pin");
    expect(locateError("/faults/0/frameId").where).toContain("frame");
    expect(
      describeAssignmentError(error("TaskMismatch", "/faults/0/frameId")),
    ).toContain("locked");
  });
});

describe("a fault editor for a leak", () => {
  const render = (
    overrides: Partial<Parameters<typeof FaultEditor>[0]> = {},
  ): string =>
    renderToStaticMarkup(
      <FaultEditor
        fault={leak}
        index={0}
        errors={[]}
        allErrors={[]}
        startingCircuit={startingCircuit}
        labels={labels}
        catalog={catalog}
        disabled={false}
        onChange={() => {}}
        onRemove={() => {}}
        {...overrides}
      />,
    );

  it("asks for the pin, the frame and the resistance of the leak", () => {
    const markup = render();

    expect(markup).toContain("Insulation leak to a frame");
    expect(markup).toContain("Pin that leaks");
    expect(markup).toContain(">L1<");
    expect(markup).toContain("Leaks to the frame");
    expect(markup).toContain(">EP1<");
    expect(markup).toContain("Leakage resistance (Ω)");
    expect(markup).toContain('value="500 Ω"');
  });

  it("shows no leak fields for another kind", () => {
    const markup = render({
      fault: { id: leak.id, kind: "open", componentId: LOAD },
    });

    expect(markup).not.toContain("Pin that leaks");
    expect(markup).not.toContain("Leaks to the frame");
  });

  it("says what is missing when the circuit has no frame", () => {
    const without = {
      ...startingCircuit,
      components: startingCircuit.components.filter(
        (component) => component.id !== FRAME,
      ),
    };
    const markup = render({
      startingCircuit: without,
      fault: { ...leak, frameId: FRAME },
    });

    expect(markup).toContain("add an exposed part to the starting circuit");
  });

  it("marks the pin, the frame and the value the Engine refused", () => {
    const errors = [
      error("UnknownReference", "/faults/0/pinId"),
      error("UnknownReference", "/faults/0/frameId"),
      error("InvalidField", "/faults/0/value"),
    ];
    const markup = render({ errors, allErrors: errors });

    expect(markup.match(/aria-invalid="true"/g)?.length).toBeGreaterThanOrEqual(
      3,
    );
  });

  it("labels every control", () => {
    const markup = render();
    for (const match of markup.matchAll(
      /<(input|select)\b[^>]*\bid="([^"]+)"/g,
    )) {
      expect(markup).toContain(`for="${match[2]}"`);
    }
  });
});

describe("a finding for a leak", () => {
  const render = (entry: Parameters<typeof DiagnosisEditor>[0]["entry"]) =>
    renderToStaticMarkup(
      <DiagnosisEditor
        entry={entry}
        index={0}
        startingCircuit={startingCircuit}
        labels={labels}
        catalog={catalog}
        errors={[]}
        disabled={false}
        onChange={() => {}}
        onRemove={() => {}}
      />,
    );

  it("asks which pin leaks and never names the frame or the resistance", () => {
    const markup = render({ componentId: LOAD, kind: "leak", pinId: "l2" });

    expect(markup).toContain("Which pin leaks");
    expect(markup).toContain(">L1<");
    expect(markup).toContain(">N<");
    expect(markup).not.toContain("Leaks to the frame");
    expect(markup).not.toContain("Leakage resistance");
  });

  it("asks nothing more for an open finding", () => {
    expect(render({ componentId: LOAD, kind: "leak" })).toContain(
      "(choose a pin)",
    );
    expect(
      render({ componentId: LOAD, kind: "leak", pinId: "l1" }),
    ).not.toContain("(choose a pin)");
  });
});

describe("the student's editor in a fault-finding task with the Stage 13 devices", () => {
  const place = (typeId: "exposed_part" | "rcd_two_pole") =>
    ({
      kind: "placeComponent",
      typeId,
      position: { x: 0, y: 0 },
      rotationDeg: 0,
    }) as const;

  it("offers the allowed new devices to the student", () => {
    const restrictions = restrictionsOf({
      ...assignment,
      restrictions: {
        allowedComponentTypes: ["exposed_part", "ac_voltmeter"],
        lockedComponents: [],
      },
    });

    expect(restrictions.findFault).toBe(true);
    expect(commandVerdict(place("exposed_part"), restrictions)).toEqual({
      allowed: true,
    });
    // A device the teacher did not list is refused as before.
    expect(commandVerdict(place("rcd_two_pole"), restrictions).allowed).toBe(
      false,
    );
    expect(
      allowedCatalog(catalog, restrictions)?.componentTypes.map(
        (entry) => entry.typeId,
      ),
    ).toEqual(["ac_voltmeter", "exposed_part"]);
  });

  it("keeps the new devices out of a build task", () => {
    const restrictions = restrictionsOf({
      ...assignment,
      task: { kind: "build" },
    });

    expect(restrictions.findFault).toBe(false);
    expect(commandVerdict(place("exposed_part"), restrictions).allowed).toBe(
      false,
    );
    expect(
      allowedCatalog(catalog, restrictions)?.componentTypes.map(
        (entry) => entry.typeId,
      ),
    ).not.toContain("exposed_part");
  });
});

describe("the result of a diagnosis with a leak", () => {
  const result: DiagnosisResult = {
    outcome: "passed",
    entries: [
      {
        entry: { componentId: LOAD, kind: "leak", pinId: "l1" },
        verdict: "correct",
        messageKey: "diagnosis.entry.correct",
      },
    ],
    faults: [
      {
        faultId: leak.id,
        verdict: "found",
        messageKey: "diagnosis.fault.found",
      },
    ],
    outcomeKey: "diagnosis.outcome.passed",
  };

  it("names the pin of a finding and of a fault the policy lets the student see", () => {
    const shown = renderToStaticMarkup(
      <DiagnosisResultView
        result={result}
        reveal="afterCheck"
        labels={labels}
        faults={assignment.faults!}
      />,
    );

    expect(shown.match(/, L1/g)?.length).toBe(2);
  });

  it("names nothing when the task never reveals", () => {
    const hidden = renderToStaticMarkup(
      <DiagnosisResultView
        result={result}
        reveal="never"
        labels={labels}
        faults={assignment.faults!}
      />,
    );

    expect(hidden).not.toContain("L1");
  });
});
