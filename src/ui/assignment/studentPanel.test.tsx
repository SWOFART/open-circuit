import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../../tests/golden/stage-1-catalog.json";
import divider from "../../../tests/golden/stage-5-divider-build.ocassignment?raw";
import faultFinding from "../../../tests/golden/stage-5-fault-finding.ocassignment?raw";
import type { Assignment } from "../../generated/Assignment";
import type { AssignmentError } from "../../generated/AssignmentError";
import type { ComponentCatalog } from "../../generated/ComponentCatalog";
import type { DiagnosisEntry } from "../../generated/DiagnosisEntry";
import type { DiagnosisResult } from "../../generated/DiagnosisResult";
import type { AssignmentPort } from "../../store/assignmentBridge";
import { StudentSessionStore } from "../../store/studentAssignment";
import { DiagnosisEditor } from "./DiagnosisEditor";
import { DiagnosisResultView } from "./DiagnosisResultView";
import { StudentPanel, type StudentPanelProps } from "./StudentPanel";

const catalog = catalogFixture as ComponentCatalog;
const buildFile = JSON.parse(divider) as Assignment;
const faultFile = JSON.parse(faultFinding) as Assignment;

const V1 = "00000000-0000-4000-8000-000000000001";
const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const METER = "00000000-0000-4000-8000-000000000004";
const labels = new Map([
  [V1, "V1"],
  [R1, "R1"],
  [R2, "R2"],
  [METER, "VM1"],
]);

const unusedPort = new Proxy({} as AssignmentPort, {
  get() {
    throw new Error("rendering must not call the bridge");
  },
});

function panel(
  assignment: Assignment | null,
  entries: DiagnosisEntry[] = [],
  overrides: Partial<StudentPanelProps> = {},
): string {
  const store = new StudentSessionStore();
  if (assignment) store.start(assignment);
  entries.forEach((entry) => store.addEntry(entry));

  return renderToStaticMarkup(
    <StudentPanel
      sessionStore={store}
      port={unusedPort}
      catalog={catalog}
      document={(assignment ?? buildFile).startingCircuit!}
      labels={labels}
      disabled={false}
      fileErrors={null}
      circuit={{ failed: false, diagnostics: [] }}
      onOpen={() => {}}
      onClose={() => {}}
      {...overrides}
    />,
  );
}

function unlabeledControls(markup: string): string[] {
  const missing: string[] = [];
  for (const match of markup.matchAll(/<(input|select|textarea)\b[^>]*>/g)) {
    const id = /\bid="([^"]+)"/.exec(match[0])?.[1];
    if (id !== undefined) {
      if (!markup.includes(`for="${id}"`)) missing.push(match[0]);
      continue;
    }
    const before = markup.slice(0, match.index);
    if (before.lastIndexOf("<label") <= before.lastIndexOf("</label>")) {
      missing.push(match[0]);
    }
  }
  return missing;
}

function error(code: AssignmentError["code"], path: string): AssignmentError {
  return {
    code,
    path,
    messageKey: `assignment.error.${code.charAt(0).toLowerCase()}${code.slice(1)}`,
  };
}

describe("the student panel before an assignment is open", () => {
  const markup = panel(null);

  it("offers to open one and says the file is never changed", () => {
    expect(markup).toContain('aria-label="Student assignment"');
    expect(markup).toContain(">Open an assignment</button>");
    expect(markup).toContain("the file is never changed");
    expect(markup).not.toContain("Close assignment");
    expect(markup).not.toContain("Check my circuit");
  });

  it("disables the open action while a project operation is under way", () => {
    expect(panel(null, [], { disabled: true })).toMatch(
      /disabled=""[^>]*>Open an assignment</,
    );
  });

  it("shows why a file was refused in words, and says the circuit is unchanged", () => {
    const refused = panel(null, [], {
      fileErrors: [
        error("UnsupportedVersion", "/schemaVersion"),
        error("UnknownField", "/grade"),
      ],
    });

    expect(refused).toContain(
      "That file was not opened. Your circuit is unchanged.",
    );
    expect(refused).toContain("This assignment file version is not supported.");
    expect(refused).toContain(
      "This field is not part of the assignment format.",
    );
    expect(refused).toContain('role="alert"');
  });
});

describe("the student panel for a build task", () => {
  // In this file the locked meter has the ID that is a resistor in the fault-finding file.
  const buildLabels = new Map([
    [V1, "V1"],
    [R1, "VM1"],
  ]);
  const markup = panel(buildFile, [], { labels: buildLabels });

  it("shows the title and the instructions, and a Check action", () => {
    expect(markup).toContain(">Divider to 2.5 V</h3>");
    expect(markup).toContain(
      "Add two resistors so that the voltmeter reads 2.5 V.",
    );
    expect(markup).toContain(">Check my circuit</button>");
    expect(markup).toContain(">Close assignment</button>");
    expect(markup).not.toContain("Add a finding");
  });

  it("names the locked parts and tells that the palette is limited", () => {
    expect(markup).toContain("Locked by the assignment: V1, VM1.");
    expect(markup).toContain(
      "move and wire them but not change or delete them",
    );
    expect(markup).toContain("only the part types this assignment allows");
  });

  it("says the progress file holds the circuit only", () => {
    expect(markup).toContain("It holds your circuit only.");
  });

  it("never shows the teacher's reference solution", () => {
    const session = new StudentSessionStore();
    session.start(buildFile);
    const text = JSON.stringify(session.getState());

    expect(text).not.toContain('"reference"');
    expect(markup).not.toContain("reference solution");
  });

  it("lists the Engine's problems when the circuit has no result, and stays editable", () => {
    const failing = panel(buildFile, [], {
      circuit: {
        failed: true,
        diagnostics: [
          {
            code: "ShortCircuit",
            severity: "error",
            localizationKey: "diagnostic.shortCircuit",
            references: [],
          },
        ],
      },
    });

    expect(failing).toContain("Your circuit has no result yet:");
    expect(failing).toContain("An ideal source is short-circuited.");
    // The Check action is still there: an uncompilable circuit is checked and not refused.
    expect(failing).not.toMatch(/disabled=""[^>]*>Check my circuit/);
  });

  it("labels every control and names every button", () => {
    expect(unlabeledControls(markup)).toEqual([]);
    expect(
      [...markup.matchAll(/<button\b[^>]*>(.*?)<\/button>/g)].filter(
        (match) => match[1]!.trim() === "",
      ),
    ).toEqual([]);
  });
});

describe("the student panel for a fault-finding task", () => {
  const markup = panel(faultFile);

  it("offers a diagnosis and no check, and shows the instructions", () => {
    expect(markup).toContain("Find the fault");
    expect(markup).toContain("Find the faulty part.");
    expect(markup).toContain(">Add a finding</button>");
    expect(markup).toContain(">Check my diagnosis</button>");
    expect(markup).not.toContain("Check my circuit");
    expect(markup).toContain("Measure it with the meters and the oscilloscope");
  });

  it("names no fault: not the part, not the kind, not the ID", () => {
    const fault = faultFile.faults![0]!;

    expect(markup).not.toContain(fault.id);
    expect(markup).not.toContain("Open (no current flows)");
    expect(markup).not.toContain("Short (1 mΩ)");
    expect(markup).not.toContain("faulty part is R2");
  });

  it("shows a finding with the parts of the circuit and the kinds the catalog lists", () => {
    const withFinding = panel(faultFile, [{ componentId: R2, kind: "open" }]);

    expect(withFinding).toContain("Finding 1");
    expect(withFinding).toContain(">R2</option>");
    expect(withFinding).toContain(">VM1</option>");
    expect(withFinding).toContain("Open (no current flows)");
    expect(withFinding).toContain("Short (1 mΩ)");
    expect(withFinding).toContain("Remove finding 1");
    expect(unlabeledControls(withFinding)).toEqual([]);
  });

  it("starts with no finding, so nothing is a guess until the student makes one", () => {
    expect(markup).not.toContain("Finding 1");
  });
});

describe("a finding", () => {
  const render = (
    entry: DiagnosisEntry,
    errors: AssignmentError[] = [],
    part = faultFile.startingCircuit!,
  ): string =>
    renderToStaticMarkup(
      <DiagnosisEditor
        entry={entry}
        index={0}
        startingCircuit={part}
        labels={labels}
        catalog={catalog}
        errors={errors}
        disabled={false}
        onChange={() => {}}
        onRemove={() => {}}
      />,
    );

  it("offers a source only the kinds the catalog lists for it", () => {
    const markup = render({ componentId: V1, kind: "open" });

    expect(markup).toContain("Open (no current flows)");
    expect(markup).not.toContain("Short (1 mΩ)");
    expect(markup).toContain("Wrong parameter value");
  });

  it("asks which parameter is wrong for a parameter finding, with the catalog's names", () => {
    const markup = render({
      componentId: R2,
      kind: "parameter",
      parameterId: "resistanceOhms",
    });

    expect(markup).toContain("Which parameter is wrong");
    expect(markup).toContain("Resistance");
    // A finding carries no value.
    expect(markup).not.toContain("Wrong value");
    expect(unlabeledControls(markup)).toEqual([]);
  });

  it("marks the field the Engine refused", () => {
    const markup = render({ componentId: R2, kind: "parameter" }, [
      error("InvalidField", "/0/parameterId"),
    ]);

    expect(markup).toMatch(/aria-invalid="true"/);
    expect(
      render({ componentId: R2, kind: "open" }).match(/aria-invalid="true"/g),
    ).toBeNull();
  });

  it("keeps a part that is no longer in the circuit visible", () => {
    expect(
      render({
        componentId: "00000000-0000-4000-8000-0000000000ee",
        kind: "open",
      }),
    ).toContain("(not in the circuit)");
  });
});

describe("the diagnosis result", () => {
  const result: DiagnosisResult = {
    outcome: "partial",
    entries: [
      {
        entry: { componentId: R2, kind: "short" },
        verdict: "wrongKind",
        messageKey: "diagnosis.entry.wrongKind",
      },
      {
        entry: { componentId: R1, kind: "open" },
        verdict: "incorrect",
        messageKey: "diagnosis.entry.incorrect",
      },
    ],
    faults: [
      {
        faultId: faultFile.faults![0]!.id,
        verdict: "missed",
        messageKey: "diagnosis.fault.missed",
      },
    ],
    outcomeKey: "diagnosis.outcome.partial",
  };
  const render = (reveal: "never" | "afterCheck"): string =>
    renderToStaticMarkup(
      <DiagnosisResultView
        result={result}
        reveal={reveal}
        labels={labels}
        faults={faultFile.faults!}
      />,
    );

  it("shows only the outcome with the policy never: no verdict, no part, no fault", () => {
    const markup = render("never");

    expect(markup).toContain("You found part of the problem.");
    expect(markup).not.toContain("Finding");
    expect(markup).not.toContain("Fault 1");
    expect(markup).not.toContain("R2");
    expect(markup).not.toContain("R1");
    expect(markup).not.toContain("wrongKind");
    expect(markup).not.toContain("Open (no current flows)");
  });

  it("shows each finding's verdict and each fault after a check, with the parts", () => {
    const markup = render("afterCheck");

    expect(markup).toContain("You found part of the problem.");
    expect(markup).toContain("Finding 1:");
    expect(markup).toContain("That part is faulty, but not in that way.");
    expect(markup).toContain("Finding 2:");
    expect(markup).toContain("That part has no fault.");
    expect(markup).toContain("Fault 1:");
    expect(markup).toContain("You did not find this fault.");
    expect(markup).toContain("R2");
    expect(markup).toContain("Open (no current flows)");
  });
});
