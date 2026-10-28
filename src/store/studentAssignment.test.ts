import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import divider from "../../tests/golden/stage-5-divider-build.ocassignment?raw";
import faultFinding from "../../tests/golden/stage-5-fault-finding.ocassignment?raw";
import type { EditorCommand } from "../editor/commands";
import type { Assignment } from "../generated/Assignment";
import type { AssignmentDecoded } from "../generated/AssignmentDecoded";
import type { AssignmentError } from "../generated/AssignmentError";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CheckResult } from "../generated/CheckResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { DiagnosisChecked } from "../generated/DiagnosisChecked";
import type { DiagnosisEntry } from "../generated/DiagnosisEntry";
import type { DiagnosisResult } from "../generated/DiagnosisResult";
import type { AssignmentPort } from "./assignmentBridge";
import {
  allowedCatalog,
  answerForKey,
  checkStudentCircuit,
  checkStudentDiagnosis,
  commandVerdict,
  faultsOf,
  openStudentAssignment,
  parameterVerdict,
  restrictionsOf,
  revealDiagnosis,
  studentCopyOf,
  StudentSessionStore,
} from "./studentAssignment";

const teacherFile = JSON.parse(divider) as Assignment;
const faultFile = JSON.parse(faultFinding) as Assignment;
const catalog = catalogFixture as ComponentCatalog;

const V1 = "00000000-0000-4000-8000-000000000001";
const VM1 = "00000000-0000-4000-8000-000000000002";
const R_NEW = "00000000-0000-4000-8000-0000000000aa";
const R2 = "00000000-0000-4000-8000-000000000003";

const refusal: AssignmentError = {
  code: "UnknownField",
  path: "/grade",
  messageKey: "assignment.error.unknownField",
};

class FakePort implements AssignmentPort {
  opened: AssignmentDecoded | null = null;
  checkRequests: CheckRequest[] = [];
  diagnosisCalls: { assignment: Assignment; diagnosis: DiagnosisEntry[] }[] =
    [];
  checkResult: (request: CheckRequest) => CheckResult = (request) =>
    resultFor(request.documentRevision);
  diagnosisResult: DiagnosisChecked = {
    status: "checked",
    result: diagnosisFor("passed"),
  };
  open() {
    return Promise.resolve(this.opened);
  }
  async checkAssignment(request: CheckRequest) {
    this.checkRequests.push(request);
    return this.checkResult(request);
  }
  async checkDiagnosis(assignment: Assignment, diagnosis: DiagnosisEntry[]) {
    this.diagnosisCalls.push({ assignment, diagnosis });
    return this.diagnosisResult;
  }
  validate(): never {
    throw new Error("not used by the student");
  }
  save(): never {
    throw new Error("a student does not save an assignment");
  }
  saveAs(): never {
    throw new Error("a student does not save an assignment");
  }
  exportStudentCopy(): never {
    throw new Error("a student does not export an assignment");
  }
  clearedPath = 0;
  clearPath() {
    this.clearedPath += 1;
    return Promise.resolve();
  }
}

function resultFor(documentRevision: string): CheckResult {
  return {
    documentRevision,
    outcome: "passed",
    counts: { pass: 3, fail: 0, notEvaluated: 0 },
    criteria: [],
    locked: {
      verdict: "pass",
      messageKey: "check.locked.pass",
      references: [],
    },
    outcomeKey: "check.outcome.passed",
    diagnostics: [],
  };
}

function diagnosisFor(outcome: DiagnosisResult["outcome"]): DiagnosisResult {
  return {
    outcome,
    entries: [
      {
        entry: { componentId: R2, kind: "open" },
        verdict: "correct",
        messageKey: "diagnosis.entry.correct",
      },
    ],
    faults: [
      {
        faultId: faultFile.faults![0]!.id,
        verdict: "found",
        messageKey: "diagnosis.fault.found",
      },
    ],
    outcomeKey: `diagnosis.outcome.${outcome}`,
  };
}

describe("the editor restrictions of an assignment", () => {
  const restrictions = restrictionsOf(teacherFile);

  const place = (typeId: "resistor" | "fuse"): EditorCommand => ({
    kind: "placeComponent",
    typeId,
    position: { x: 0, y: 0 },
    rotationDeg: 0,
  });

  it("reads the locked parts and the allowed part types from the file", () => {
    expect([...restrictions.locked]).toEqual([V1, VM1]);
    expect(restrictions.allowedTypes).toEqual(["resistor"]);
    expect(restrictionsOf(faultFile).allowedTypes).toEqual([
      "voltmeter",
      "ammeter",
    ]);
    // Without a `restrictions` field: every type, nothing locked.
    const bare = { ...teacherFile };
    delete bare.restrictions;
    expect(restrictionsOf(bare)).toEqual({
      allowedTypes: null,
      locked: new Set(),
    });
  });

  it("allows placing a part of an allowed type and refuses any other", () => {
    expect(commandVerdict(place("resistor"), restrictions)).toEqual({
      allowed: true,
    });
    expect(commandVerdict(place("fuse"), restrictions)).toEqual({
      allowed: false,
      reason: "The assignment does not allow adding that kind of part.",
    });
  });

  it("refuses to delete a locked part and allows deleting any other", () => {
    const remove = (componentId: string): EditorCommand => ({
      kind: "deleteComponent",
      componentId,
    });

    expect(commandVerdict(remove(V1), restrictions)).toEqual({
      allowed: false,
      reason: "That part is locked by the assignment and cannot be deleted.",
    });
    expect(commandVerdict(remove(R_NEW), restrictions).allowed).toBe(true);
  });

  it("lets a locked part be moved and rotated, and wires be drawn to it", () => {
    const commands: EditorCommand[] = [
      {
        kind: "transformComponent",
        componentId: V1,
        position: { x: 9, y: 9 },
        rotationDeg: 90,
        wires: [],
      },
      {
        kind: "addWire",
        points: [
          { x: 0, y: 0 },
          { x: 4, y: 0 },
        ],
      },
      { kind: "deleteWire", wireId: R_NEW },
      { kind: "addJunction", position: { x: 1, y: 1 } },
      { kind: "addReferenceMarker", position: { x: 1, y: 1 } },
      { kind: "undo" },
      { kind: "redo" },
    ];

    for (const command of commands) {
      expect(commandVerdict(command, restrictions).allowed, command.kind).toBe(
        true,
      );
    }
  });

  it("refuses to edit a locked part's parameters and allows the others", () => {
    expect(parameterVerdict(VM1, restrictions)).toEqual({
      allowed: false,
      reason: "That part is locked by the assignment and cannot be edited.",
    });
    expect(parameterVerdict(R_NEW, restrictions).allowed).toBe(true);
  });

  it("changes nothing when no assignment is open", () => {
    expect(commandVerdict(place("fuse"), null).allowed).toBe(true);
    expect(parameterVerdict(V1, null).allowed).toBe(true);
    expect(allowedCatalog(catalog, null)).toBe(catalog);
  });

  it("offers the palette only the allowed part types, and every type for null", () => {
    const only = allowedCatalog(catalog, restrictions);

    expect(only?.componentTypes.map((entry) => entry.typeId)).toEqual([
      "resistor",
    ]);
    expect(
      allowedCatalog(catalog, { allowedTypes: null, locked: new Set() }),
    ).toBe(catalog);
    expect(allowedCatalog(null, restrictions)).toBeNull();
  });
});

describe("opening an assignment as a student", () => {
  it("drops the teacher's reference solution and starts from a copy of the starting circuit", async () => {
    const port = new FakePort();
    port.opened = { status: "valid", assignment: teacherFile };

    const outcome = await openStudentAssignment(port);

    expect(outcome.status).toBe("opened");
    if (outcome.status !== "opened") return;
    expect(outcome.assignment).not.toHaveProperty("reference");
    expect(outcome.assignment.title).toBe(teacherFile.title);
    expect(outcome.circuit).toEqual(teacherFile.startingCircuit);
    expect(outcome.circuit).not.toBe(teacherFile.startingCircuit);
    // The file value itself is not changed.
    expect(teacherFile.reference).toBeDefined();
  });

  it("starts a build task without a starting circuit from an empty circuit", async () => {
    const port = new FakePort();
    const bare = { ...teacherFile };
    delete bare.startingCircuit;
    port.opened = { status: "valid", assignment: bare };

    const outcome = await openStudentAssignment(port);

    expect(outcome).toMatchObject({
      status: "opened",
      circuit: { schemaVersion: 1, components: [], wires: [] },
    });
  });

  it("returns the Engine's errors for a file it refused, and no circuit", async () => {
    const port = new FakePort();
    port.opened = { status: "invalid", errors: [refusal] };

    expect(await openStudentAssignment(port)).toEqual({
      status: "invalid",
      errors: [refusal],
    });
    port.opened = null;
    expect(await openStudentAssignment(port)).toEqual({ status: "cancelled" });
  });

  it("forgets the file's path, so the teacher's Save cannot overwrite a student's file", async () => {
    const port = new FakePort();
    port.opened = { status: "valid", assignment: teacherFile };

    await openStudentAssignment(port);
    expect(port.clearedPath).toBe(1);

    // A refused or cancelled open did not move the path in Rust, so it is left alone.
    port.opened = { status: "invalid", errors: [refusal] };
    await openStudentAssignment(port);
    port.opened = null;
    await openStudentAssignment(port);
    expect(port.clearedPath).toBe(1);
  });

  it("starts a fault-finding task from the circuit without any fault in it", async () => {
    const port = new FakePort();
    port.opened = { status: "valid", assignment: faultFile };

    const outcome = await openStudentAssignment(port);

    if (outcome.status !== "opened") throw new Error("opened expected");
    // The circuit the student edits and saves holds nothing of the answer key.
    const text = JSON.stringify(outcome.circuit);
    expect(text).not.toContain(faultFile.faults![0]!.id);
    expect(text).not.toContain("faults");
    expect(text).not.toContain("instructions");
    expect(text).not.toContain("checks");
    expect(faultsOf(outcome.assignment)).toEqual(faultFile.faults);
  });
});

describe("the faults a student's runs carry", () => {
  it("are those of a fault-finding task, and none for a build task or no task", () => {
    expect(faultsOf(faultFile)).toEqual(faultFile.faults);
    expect(faultsOf(teacherFile)).toBeNull();
    expect(faultsOf(null)).toBeNull();
  });
});

describe("checking the student's circuit", () => {
  const student = teacherFile.reference!;

  it("asks the Engine to check the circuit as it is now against the assignment's checks", async () => {
    const port = new FakePort();

    const outcome = await checkStudentCircuit(
      port,
      studentCopyOf(teacherFile),
      student,
      undefined,
      () => "check-1",
    );

    expect(outcome.status).toBe("result");
    const request = port.checkRequests[0]!;
    expect(request.checks).toEqual(teacherFile.checks);
    expect(request.lockedComponents).toEqual([V1, VM1]);
    expect(request.startingCircuit).toEqual(teacherFile.startingCircuit);
    expect(request.schematic).toBe(student);
    expect(request.documentRevision).toBe("check-1");
    expect(
      request.geometry.components.map((component) => component.componentId),
    ).toEqual(student.components.map((component) => component.id));
  });

  it("does not use a result that is not for this request", async () => {
    const port = new FakePort();
    port.checkResult = () => resultFor("check-from-before");

    expect(
      await checkStudentCircuit(
        port,
        teacherFile,
        student,
        undefined,
        () => "check-2",
      ),
    ).toEqual({ status: "stale" });
  });

  it("reports an Engine that cannot be reached, and leaves nothing to show", async () => {
    const port = new FakePort();
    port.checkResult = () => {
      throw new Error("The Rust engine is only available in the desktop app.");
    };

    expect(await checkStudentCircuit(port, teacherFile, student)).toEqual({
      status: "unavailable",
      reason: "The Rust engine is only available in the desktop app.",
    });
  });

  it("has no check to run for a fault-finding task", async () => {
    const port = new FakePort();

    expect(await checkStudentCircuit(port, faultFile, student)).toEqual({
      status: "notBuildTask",
    });
    expect(port.checkRequests).toEqual([]);
  });
});

describe("checking the diagnosis", () => {
  const entries: DiagnosisEntry[] = [{ componentId: R2, kind: "open" }];

  it("gives the Engine the assignment and the findings, and returns its comparison", async () => {
    const port = new FakePort();

    const outcome = await checkStudentDiagnosis(port, faultFile, entries);

    expect(outcome).toEqual({
      status: "checked",
      result: diagnosisFor("passed"),
    });
    expect(port.diagnosisCalls).toEqual([
      { assignment: faultFile, diagnosis: entries },
    ]);
  });

  it("returns the Engine's errors for findings it refused", async () => {
    const port = new FakePort();
    port.diagnosisResult = { status: "invalid", errors: [refusal] };

    expect(await checkStudentDiagnosis(port, faultFile, entries)).toEqual({
      status: "invalid",
      errors: [refusal],
    });
  });

  it("reports a bridge that failed", async () => {
    const port = new FakePort();
    port.checkDiagnosis = () =>
      Promise.reject(new Error("The bridge is down."));

    expect(await checkStudentDiagnosis(port, faultFile, entries)).toEqual({
      status: "unavailable",
      reason: "The bridge is down.",
    });
  });
});

describe("what the student sees of a diagnosis result", () => {
  const result = diagnosisFor("partial");

  it("shows only the outcome with the policy never", () => {
    const shown = revealDiagnosis(result, "never");

    expect(shown).toEqual({
      outcomeKey: "diagnosis.outcome.partial",
      entries: null,
      faults: null,
    });
    // Nothing of the detail is in what the interface is handed.
    expect(JSON.stringify(shown)).not.toContain(R2);
    expect(JSON.stringify(shown)).not.toContain(faultFile.faults![0]!.id);
  });

  it("shows the verdicts of the findings and of the faults after a check", () => {
    const shown = revealDiagnosis(result, "afterCheck");

    expect(shown.entries).toEqual(result.entries);
    expect(shown.faults).toEqual(result.faults);
  });
});

describe("the student session", () => {
  const first: DiagnosisEntry = { componentId: R2, kind: "open" };
  const second: DiagnosisEntry = { componentId: V1, kind: "open" };

  it("starts without the reference and with an empty diagnosis, and forgets all on close", () => {
    const store = new StudentSessionStore();
    let notified = 0;
    store.subscribe(() => (notified += 1));

    store.start(teacherFile);
    store.addEntry(first);
    expect(store.getState().assignment).not.toHaveProperty("reference");
    expect(store.getState().entries).toEqual([first]);

    store.start(faultFile);
    expect(store.getState().entries).toEqual([]);

    store.close();
    expect(store.getState().assignment).toBeNull();
    expect(notified).toBe(4);
  });

  it("adds, replaces, and removes findings, each a new version", () => {
    const store = new StudentSessionStore();
    store.start(faultFile);
    const versions = [store.getState().entriesVersion];

    store.addEntry(first);
    versions.push(store.getState().entriesVersion);
    store.addEntry(second);
    versions.push(store.getState().entriesVersion);
    store.replaceEntry(0, { ...first, kind: "short" });
    versions.push(store.getState().entriesVersion);
    store.removeEntry(1);
    versions.push(store.getState().entriesVersion);

    expect(store.getState().entries).toEqual([{ ...first, kind: "short" }]);
    expect(new Set(versions).size).toBe(versions.length);
  });
});

describe("answers are shown only for what they were asked for", () => {
  it("matches the very same object, not an equal one", () => {
    const circuit = { schemaVersion: 1 };
    const answer = { key: circuit, value: "result" };

    expect(answerForKey(answer, circuit)).toBe("result");
    expect(answerForKey(answer, { schemaVersion: 1 })).toBeNull();
    expect(answerForKey(null, circuit)).toBeNull();
    expect(answerForKey({ key: 3, value: "x" }, 3)).toBe("x");
    expect(answerForKey({ key: 3, value: "x" }, 4)).toBeNull();
  });
});
