import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import divider from "../../../tests/golden/stage-5-divider-build.ocassignment?raw";
import faultFinding from "../../../tests/golden/stage-5-fault-finding.ocassignment?raw";
import fuse from "../../../tests/golden/stage-5-fuse-state.ocassignment?raw";
import type { Assignment } from "../../generated/Assignment";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { CheckResult } from "../../generated/CheckResult";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { ResultLayout } from "../../generated/ResultLayout";
import { AssignmentAuthoring } from "../../store/assignmentAuthoring";
import type { AssignmentPort } from "../../store/assignmentBridge";
import {
  AssignmentDraftStore,
  assignmentToDraft,
  emptyDraft,
} from "../../store/assignmentDraft";
import { AssignmentPanel, type AssignmentPanelProps } from "./AssignmentPanel";
import { CheckEditor } from "./CheckEditor";
import { CheckResultView } from "./CheckResultView";
import { FaultEditor } from "./FaultEditor";

const catalog = catalogFixture as ComponentCatalog;
const dividerAssignment = JSON.parse(divider) as Assignment;
const faultAssignment = JSON.parse(faultFinding) as Assignment;
const fuseAssignment = JSON.parse(fuse) as Assignment;

const V1 = "00000000-0000-4000-8000-000000000001";
const VM1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const labels = new Map([
  [V1, "V1"],
  [VM1, "VM1"],
  [R2, "R2"],
]);

const layout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 2,
  slots: [V1, VM1].map((id, offset) => ({
    key: `${id}:voltage`,
    componentId: id,
    quantity: "voltage" as const,
    unit: "volt" as const,
    signConvention: "voltagePin1MinusPin2" as const,
    offset,
  })),
};

const unusedPort = new Proxy({} as AssignmentPort, {
  get() {
    throw new Error("rendering must not call the bridge");
  },
});

function panel(
  assignment: Assignment | null,
  overrides: Partial<AssignmentPanelProps> = {},
): string {
  const draftStore = new AssignmentDraftStore(
    assignment ? assignmentToDraft(assignment) : emptyDraft(),
  );
  return renderToStaticMarkup(
    <AssignmentPanel
      draftStore={draftStore}
      authoring={new AssignmentAuthoring(draftStore, unusedPort)}
      catalog={catalog}
      document={dividerAssignment.startingCircuit!}
      layout={layout}
      disabled={false}
      onShowCircuit={() => {}}
      faultView={{ shown: false, failed: false, diagnostics: [] }}
      onFaultViewChange={() => {}}
      onClose={() => {}}
      {...overrides}
    />,
  );
}

function error(code: AssignmentError["code"], path: string): AssignmentError {
  return {
    code,
    path,
    messageKey: `assignment.error.${code.charAt(0).toLowerCase()}${code.slice(1)}`,
  };
}

/** Every form control has a label a screen reader can use: `for`, or a label around it. */
function unlabeledControls(markup: string): string[] {
  const missing: string[] = [];
  for (const match of markup.matchAll(/<(input|select|textarea)\b[^>]*>/g)) {
    const tag = match[0];
    const id = /\bid="([^"]+)"/.exec(tag)?.[1];
    if (id !== undefined) {
      if (!markup.includes(`for="${id}"`)) missing.push(tag);
      continue;
    }
    const before = markup.slice(0, match.index);
    const insideLabel =
      before.lastIndexOf("<label") > before.lastIndexOf("</label>");
    if (!insideLabel) missing.push(tag);
  }
  return missing;
}

function unnamedButtons(markup: string): string[] {
  return [...markup.matchAll(/<button\b[^>]*>(.*?)<\/button>/g)]
    .filter((match) => match[1]!.replace(/<[^>]*>/g, "").trim() === "")
    .map((match) => match[0]);
}

describe("the authoring panel for a build task", () => {
  const markup = panel(dividerAssignment);

  it("offers the task, the circuits, the restrictions, and the checks", () => {
    expect(markup).toContain('aria-label="Assignment authoring"');
    expect(markup).toContain(">Task</h3>");
    expect(markup).toContain(">Circuits</h3>");
    expect(markup).toContain(">Restrictions</h3>");
    expect(markup).toContain(">Checks</h3>");
    expect(markup).not.toContain(">Faults</h3>");
    expect(markup).toContain('value="Divider to 2.5 V"');
    expect(markup).toContain(
      "Add two resistors so that the voltmeter reads 2.5 V.",
    );
    expect(markup).toContain("Starting circuit: 2 parts");
    expect(markup).toContain("Reference solution: 4 parts");
  });

  it("has file actions, a validation action, and a preview action with words", () => {
    for (const label of [
      "New assignment",
      "Open assignment",
      "Save assignment",
      "Save assignment as",
      "Export student copy",
      "Check assignment",
      "Preview the checks on the reference solution",
      "Add a value check",
      "Add a range check",
      "Add a device state check",
      "Add a part count check",
    ]) {
      expect(markup).toContain(`>${label}</button>`);
    }
    expect(markup).toContain(">Saved</p>");
  });

  it("locks the parts the file locks and lists the part types the file allows", () => {
    expect(markup).toMatch(/<input[^>]*checked=""[^>]*\/?>\s*Lock V1/);
    expect(markup).toMatch(/<input[^>]*checked=""[^>]*\/?>\s*Lock VM1/);
    // The file allows resistors only: "every part type" is off and only Resistor is on.
    expect(markup).toMatch(/<input type="checkbox"[^>]*\/?>\s*Every part type/);
    expect(markup).not.toMatch(/checked=""[^>]*\/?>\s*Every part type/);
    expect(markup).toMatch(/checked=""[^>]*\/?>\s*Resistor/);
    expect(markup).not.toMatch(/checked=""[^>]*\/?>\s*Fuse/);
  });

  it("shows each check with its signal, times, and values in SI units with prefixes", () => {
    expect(markup).toContain("Check 1: Reading");
    expect(markup).toContain("VM1 voltage");
    expect(markup).toContain("Expected value (V)");
    expect(markup).toContain('value="2.5 V"');
    expect(markup).toContain("Absolute tolerance (V)");
    expect(markup).toContain('value="25 mV"');
    expect(markup).toContain(
      "Relative tolerance (fraction of the expected value)",
    );
    expect(markup).toContain("Check 2: Value within a range");
    expect(markup).toContain('value="10 ms"');
    expect(markup).toContain('value="2.4 V"');
    expect(markup).toContain('value="2.6 V"');
    expect(markup).toContain("Check 3: Number of parts");
    expect(markup).toContain("At least");
    expect(markup).toContain("At most (leave empty for no limit)");
    expect(markup).toContain("Remove check 3");
  });

  it("labels every control and names every button", () => {
    expect(unlabeledControls(markup)).toEqual([]);
    expect(unnamedButtons(markup)).toEqual([]);
  });

  it("turns checks off for the signals the Engine did not report", () => {
    const noSignals = panel(dividerAssignment, { layout: null });

    expect(noSignals).toMatch(/disabled=""[^>]*>Add a value check/);
    expect(noSignals).toMatch(/disabled=""[^>]*>Add a range check/);
    expect(noSignals).toContain("does it compile?");
    // Parts need no layout.
    expect(noSignals).not.toMatch(/disabled=""[^>]*>Add a part count check/);
  });

  it("explains that a circuit must be set before a check can be added", () => {
    const empty = panel(null);

    expect(empty).toContain(
      "Set the starting circuit to choose signals to check.",
    );
    expect(empty).toMatch(/disabled=""[^>]*>Add a part count check/);
    expect(empty).toMatch(/disabled=""[^>]*>Preview the checks/);
    expect(empty).toContain("Starting circuit: not set");
  });

  it("disables every action while a project operation is under way", () => {
    const locked = panel(dividerAssignment, { disabled: true });

    expect(locked).toMatch(/disabled=""[^>]*>Save assignment</);
    expect(locked).toMatch(/disabled=""[^>]*>Open assignment</);
    expect(locked).toMatch(
      /<input[^>]*disabled=""[^>]*id=[^>]*>|<input[^>]*id=[^>]*disabled=""[^>]*>/,
    );
  });

  it("reminds the teacher that a local file protects nothing", () => {
    expect(markup).toContain("not secure examination software");
  });
});

describe("the authoring panel for a fault-finding task", () => {
  const markup = panel(faultAssignment);

  it("replaces the checks with the faults and the reveal choice", () => {
    expect(markup).toContain(">Faults</h3>");
    expect(markup).not.toContain(">Checks</h3>");
    expect(markup).not.toContain("Add a value check");
    expect(markup).toContain("What the student sees of the result");
    expect(markup).toContain(
      "After a check: the student sees each verdict and the faults",
    );
    expect(markup).toContain(">Add a fault</button>");
    expect(markup).toContain("Fault 1");
    expect(markup).toContain("Show the faulted circuit in the workspace");
    expect(markup).not.toContain("reference solution");
  });

  it("shows why the faulted circuit has no result, only while it is shown", () => {
    const failing = panel(faultAssignment, {
      faultView: {
        shown: true,
        failed: true,
        diagnostics: [
          {
            code: "FloatingNode",
            severity: "error",
            localizationKey: "diagnostic.floatingNode",
            references: [],
          },
        ],
      },
    });

    expect(failing).toContain("The faulted circuit has no result");
    expect(failing).toContain("A node is not connected to any reference.");
    expect(failing).toMatch(/checked=""[^>]*\/?>\s*Show the faulted circuit/);
    expect(
      panel(faultAssignment, {
        faultView: { shown: false, failed: true, diagnostics: [] },
      }),
    ).not.toContain("The faulted circuit has no result");
  });

  it("labels every control", () => {
    expect(unlabeledControls(markup)).toEqual([]);
    expect(unnamedButtons(markup)).toEqual([]);
  });
});

describe("a check editor", () => {
  const signals = [
    {
      key: `${VM1}:voltage`,
      unit: "volt" as const,
      componentId: VM1,
      label: "VM1 voltage",
    },
  ];
  const criterion = dividerAssignment.checks![0]!;
  const render = (
    overrides: Partial<Parameters<typeof CheckEditor>[0]> = {},
  ): string =>
    renderToStaticMarkup(
      <CheckEditor
        criterion={criterion}
        index={0}
        errors={[]}
        allErrors={[]}
        signals={signals}
        startingCircuit={dividerAssignment.startingCircuit!}
        labels={labels}
        catalog={catalog}
        disabled={false}
        onChange={() => {}}
        onRemove={() => {}}
        {...overrides}
      />,
    );

  it("marks a field the Engine refused and says why, in words", () => {
    const errors = [
      error("InvalidField", "/checks/0/atSeconds"),
      error("UnknownReference", "/checks/0/signal/key"),
    ];
    const markup = render({ errors, allErrors: errors });

    expect(markup).toMatch(
      /aria-invalid="true"[^>]*value="0 s"|value="0 s"[^>]*aria-invalid="true"/,
    );
    expect(markup).toContain("This value is not valid.");
    expect(markup).toContain(
      "This refers to a part, part type, or signal that the starting circuit does not have.",
    );
    expect(markup).toContain('role="alert"');
    // The fields without an error are not marked.
    expect(render().match(/aria-invalid="true"/g)).toBeNull();
  });

  it("keeps a signal the choices no longer offer visible instead of replacing it", () => {
    const markup = render({ signals: [] });

    expect(markup).toContain(`(not available) ${VM1}:voltage`);
  });

  it("offers only the states of the chosen part's type for a device state check", () => {
    const markup = renderToStaticMarkup(
      <CheckEditor
        criterion={fuseAssignment.checks![0]!}
        index={0}
        errors={[]}
        allErrors={[]}
        signals={[]}
        startingCircuit={fuseAssignment.startingCircuit!}
        labels={
          new Map([
            [V1, "V1"],
            ["00000000-0000-4000-8000-000000000003", "F1"],
          ])
        }
        catalog={catalog}
        disabled={false}
        onChange={() => {}}
        onRemove={() => {}}
      />,
    );

    expect(markup).toContain("intact");
    expect(markup).toContain("tripped");
    expect(markup).not.toContain("released");
    expect(markup).toContain(">F1</option>");
    // The source cannot have a device state, so it is not a choice.
    expect(markup).not.toContain(">V1</option>");
    expect(markup).toContain('value="2 ms"');
  });
});

describe("a fault editor", () => {
  const fault = faultAssignment.faults![0]!;
  const render = (
    overrides: Partial<Parameters<typeof FaultEditor>[0]> = {},
  ): string =>
    renderToStaticMarkup(
      <FaultEditor
        fault={fault}
        index={0}
        errors={[]}
        allErrors={[]}
        startingCircuit={faultAssignment.startingCircuit!}
        labels={labels}
        catalog={catalog}
        disabled={false}
        onChange={() => {}}
        onRemove={() => {}}
        {...overrides}
      />,
    );

  it("offers the kinds the catalog lists for the part", () => {
    const resistor = render();
    expect(resistor).toContain("Open (no current flows)");
    expect(resistor).toContain("Short (1 mΩ)");
    expect(resistor).toContain("Wrong parameter value");

    // A source cannot be shorted (contract section 5.2), and the Engine's catalog says so.
    const source = render({ fault: { ...fault, componentId: V1 } });
    expect(source).toContain("Open (no current flows)");
    expect(source).not.toContain("Short (1 mΩ)");
    expect(source).toContain("Wrong parameter value");
  });

  it("shows the parameter and the wrong value in SI units for a parameter fault", () => {
    const markup = render({
      fault: {
        id: fault.id,
        kind: "parameter",
        componentId: R2,
        parameterId: "resistanceOhms",
        value: 3000,
      },
    });

    expect(markup).toContain("Parameter");
    expect(markup).toContain("Wrong value (Ω)");
    expect(markup).toContain('value="3 kΩ"');
    expect(unlabeledControls(markup)).toEqual([]);
  });

  it("marks what the Engine refused", () => {
    const errors = [error("TaskMismatch", "/faults/0/componentId")];
    const markup = render({ errors, allErrors: errors });

    expect(markup).toContain("A part with a fault must be locked");
    expect(markup).toMatch(/aria-invalid="true"/);
    expect(render().match(/aria-invalid="true"/g)).toBeNull();
  });
});

describe("the check result", () => {
  const result: CheckResult = {
    documentRevision: "preview-1",
    outcome: "partial",
    counts: { pass: 2, fail: 1, notEvaluated: 1 },
    criteria: [
      {
        kind: "value",
        id: "00000000-0000-4000-8000-0000000000c1",
        verdict: "pass",
        measured: 2.498126405196103,
        expected: 2.5,
        toleranceAbs: 0.025,
        toleranceRel: 0,
        unit: "volt",
        atSeconds: 0,
        messageKey: "check.value.pass",
        references: [{ kind: "component", componentId: VM1 }],
      },
      {
        kind: "range",
        id: "00000000-0000-4000-8000-0000000000c2",
        verdict: "fail",
        measured: 2.7,
        atSeconds: 0.004,
        min: 2.4,
        max: 2.6,
        unit: "volt",
        fromSeconds: 0,
        toSeconds: 0.01,
        messageKey: "check.range.fail",
        references: [{ kind: "component", componentId: VM1 }],
      },
      {
        kind: "deviceState",
        id: "00000000-0000-4000-8000-0000000000c4",
        verdict: "notEvaluated",
        expected: "tripped",
        atSeconds: 0.002,
        messageKey: "check.deviceState.notEvaluated",
        references: [{ kind: "component", componentId: R2 }],
      },
      {
        kind: "componentCount",
        id: "00000000-0000-4000-8000-0000000000c3",
        verdict: "pass",
        measured: 2,
        min: 2,
        max: 2,
        messageKey: "check.componentCount.pass",
        references: [],
      },
    ],
    locked: {
      verdict: "pass",
      messageKey: "check.locked.pass",
      references: [],
    },
    outcomeKey: "check.outcome.partial",
    diagnostics: [],
  };
  const markup = renderToStaticMarkup(
    <CheckResultView
      result={result}
      labels={labels}
      names={new Map([["00000000-0000-4000-8000-0000000000c1", "Reading"]])}
    />,
  );

  it("shows the outcome, the counts, and the locked parts as the Engine said", () => {
    expect(markup).toContain("Some checks passed.");
    expect(markup).toContain("2 passed, 1 failed, 1 not checked.");
    expect(markup).toContain("Locked parts: Passed.");
    expect(markup).toContain("The locked parts are unchanged.");
  });

  it("shows each criterion's verdict, measured and expected values, and tolerance with units", () => {
    expect(markup).toContain("Check 1 (Reading): Passed.");
    expect(markup).toContain("The value is within the tolerance.");
    expect(markup).toContain("expected 2.5 V, ±25 mV");
    expect(markup).toContain("Measured 2.498126 V.");
    expect(markup).toContain("Part: VM1.");
    expect(markup).toContain("Check 2: Failed.");
    expect(markup).toContain("between 2.4 V and 2.6 V");
    expect(markup).toContain("Closest to a limit: 2.7 V at 4 ms.");
    expect(markup).toContain("Check 3: Not checked.");
    expect(markup).toContain("expected tripped");
    expect(markup).toContain("Check 4: Passed.");
    expect(markup).toContain("Expected exactly 2.");
    expect(markup).toContain("Found 2.");
  });

  it("shows no measured value for a criterion that was not checked", () => {
    const third = markup.slice(
      markup.indexOf("Check 3"),
      markup.indexOf("Check 4"),
    );

    expect(third).not.toContain("Found");
    expect(third).not.toContain("Measured");
  });

  it("lists the circuit's problems in words when the circuit could not be checked", () => {
    const failed = renderToStaticMarkup(
      <CheckResultView
        result={{
          ...result,
          outcome: "notEvaluated",
          outcomeKey: "check.outcome.notEvaluated",
          diagnostics: [
            {
              code: "ShortCircuit",
              severity: "error",
              localizationKey: "diagnostic.shortCircuit",
              references: [],
            },
          ],
        }}
        labels={labels}
      />,
    );

    expect(failed).toContain("The circuit could not be checked.");
    expect(failed).toContain("An ideal source is short-circuited.");
  });
});
