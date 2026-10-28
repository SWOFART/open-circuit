import { describe, expect, it } from "vitest";
import divider from "../../tests/golden/stage-5-divider-build.ocassignment?raw";
import dividerGeometry from "../../tests/golden/stage-5-divider-build.reference.geometry.json";
import faultFinding from "../../tests/golden/stage-5-fault-finding.ocassignment?raw";
import type { Assignment } from "../generated/Assignment";
import type { AssignmentDecoded } from "../generated/AssignmentDecoded";
import type { AssignmentError } from "../generated/AssignmentError";
import type { AssignmentSaveOutcome } from "../generated/AssignmentSaveOutcome";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CheckResult } from "../generated/CheckResult";
import { AssignmentAuthoring } from "./assignmentAuthoring";
import type { AssignmentPort } from "./assignmentBridge";
import {
  AssignmentDraftStore,
  assignmentToDraft,
  draftToAssignment,
  emptyDraft,
} from "./assignmentDraft";

const dividerAssignment = JSON.parse(divider) as Assignment;
const faultAssignment = JSON.parse(faultFinding) as Assignment;

const refusal: AssignmentError = {
  code: "UnknownField",
  path: "/grade",
  messageKey: "assignment.error.unknownField",
};

/** A bridge double that records what it was asked and answers as the test says. */
class FakePort implements AssignmentPort {
  readonly calls: string[] = [];
  readonly saved: Assignment[] = [];
  readonly checkRequests: CheckRequest[] = [];
  opened: AssignmentDecoded | null = null;
  saveOutcome: AssignmentSaveOutcome = { status: "saved" };
  validated: AssignmentDecoded = {
    status: "valid",
    assignment: dividerAssignment,
  };
  checkResult: (request: CheckRequest) => CheckResult | Promise<CheckResult> = (
    request,
  ) => resultFor(request.documentRevision);
  /** Runs once the Engine has been asked, before it answers. */
  duringCheck: () => void = () => {};
  duringValidate: () => void = () => {};

  open() {
    this.calls.push("open");
    return Promise.resolve(this.opened);
  }
  validate(document: unknown) {
    this.calls.push("validate");
    this.saved.push(document as Assignment);
    this.duringValidate();
    return Promise.resolve(this.validated);
  }
  save(assignment: Assignment) {
    this.calls.push("save");
    this.saved.push(assignment);
    return Promise.resolve(this.saveOutcome);
  }
  saveAs(assignment: Assignment) {
    this.calls.push("saveAs");
    this.saved.push(assignment);
    return Promise.resolve(this.saveOutcome);
  }
  exportStudentCopy(assignment: Assignment) {
    this.calls.push("export");
    this.saved.push(assignment);
    return Promise.resolve(this.saveOutcome);
  }
  clearPath() {
    this.calls.push("clearPath");
    return Promise.resolve();
  }
  async checkAssignment(request: CheckRequest) {
    this.calls.push("check");
    this.checkRequests.push(request);
    this.duringCheck();
    return this.checkResult(request);
  }
  checkDiagnosis(): never {
    throw new Error("not used by the teacher");
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

function setup(initial = emptyDraft()) {
  const store = new AssignmentDraftStore(initial);
  const port = new FakePort();
  let revision = 0;
  const authoring = new AssignmentAuthoring(
    store,
    port,
    undefined,
    () => `preview-${(revision += 1)}`,
  );
  return { store, port, authoring };
}

describe("opening an assignment", () => {
  it("replaces the draft only with a file the Engine accepted", async () => {
    const { store, port, authoring } = setup();
    port.opened = { status: "valid", assignment: dividerAssignment };

    expect(await authoring.open()).toEqual({ status: "opened" });

    expect(draftToAssignment(store.getState().draft)).toEqual(
      dividerAssignment,
    );
    expect(store.getState().isDirty).toBe(false);
  });

  it("keeps the teacher's work when the file is refused, and returns the errors", async () => {
    const { store, port, authoring } = setup(
      assignmentToDraft(faultAssignment),
    );
    store.setTitle("Work in progress");
    const before = store.getState();
    port.opened = { status: "invalid", errors: [refusal] };

    expect(await authoring.open()).toEqual({
      status: "invalid",
      errors: [refusal],
    });

    expect(store.getState()).toBe(before);
    expect(store.getState().draft.title).toBe("Work in progress");
  });

  it("changes nothing when the file dialog is cancelled", async () => {
    const { store, port, authoring } = setup();
    port.opened = null;
    const before = store.getState();

    expect(await authoring.open()).toEqual({ status: "cancelled" });
    expect(store.getState()).toBe(before);
  });
});

describe("saving an assignment", () => {
  it("sends the document built from the draft and counts it as saved", async () => {
    const { store, port, authoring } = setup(
      assignmentToDraft(dividerAssignment),
    );
    store.setTitle("Divider, second try");

    expect(store.getState().isDirty).toBe(true);
    expect(await authoring.save()).toEqual({ status: "saved" });

    expect(port.calls).toEqual(["save"]);
    expect(port.saved[0]?.title).toBe("Divider, second try");
    expect(store.getState().isDirty).toBe(false);
    await authoring.save(true);
    expect(port.calls).toEqual(["save", "saveAs"]);
  });

  it("stays dirty and returns the Engine's errors when the document is refused", async () => {
    const { store, port, authoring } = setup(
      assignmentToDraft(dividerAssignment),
    );
    store.setTitle("");
    port.saveOutcome = { status: "invalid", errors: [refusal] };

    expect(await authoring.save()).toEqual({
      status: "invalid",
      errors: [refusal],
    });

    expect(store.getState().isDirty).toBe(true);
    expect(store.getState().draft.title).toBe("");
  });

  it("stays dirty when the file dialog is cancelled", async () => {
    const { store, port, authoring } = setup(
      assignmentToDraft(dividerAssignment),
    );
    store.setTitle("Changed");
    port.saveOutcome = { status: "cancelled" };

    await authoring.save();

    expect(store.getState().isDirty).toBe(true);
  });

  it("exports the student's copy without marking the draft saved", async () => {
    const { store, port, authoring } = setup(
      assignmentToDraft(dividerAssignment),
    );
    store.setTitle("Changed");

    await authoring.exportStudentCopy();

    expect(port.calls).toEqual(["export"]);
    expect(store.getState().isDirty).toBe(true);
  });

  it("starts a new assignment by forgetting the path and emptying the draft", async () => {
    const { store, port, authoring } = setup(
      assignmentToDraft(dividerAssignment),
    );

    await authoring.newAssignment();

    expect(port.calls).toEqual(["clearPath"]);
    expect(store.getState().draft.title).toBe("");
    expect(store.getState().draft.checks).toEqual([]);
  });
});

describe("validating the draft", () => {
  it("returns the Engine's errors for the draft as it is", async () => {
    const { port, authoring } = setup(assignmentToDraft(dividerAssignment));
    port.validated = { status: "invalid", errors: [refusal] };

    expect(await authoring.validate()).toEqual({
      status: "invalid",
      errors: [refusal],
    });
    expect(port.saved[0]).toEqual(dividerAssignment);
  });

  it("drops errors for a draft that has changed while the Engine was working", async () => {
    const { store, port, authoring } = setup(
      assignmentToDraft(dividerAssignment),
    );
    port.validated = { status: "invalid", errors: [refusal] };
    port.duringValidate = () => store.setTitle("Edited meanwhile");

    expect(await authoring.validate()).toEqual({ status: "stale" });
  });
});

describe("previewing the checks on the reference solution", () => {
  it("asks the Engine to check the reference with the assignment's checks and locks", async () => {
    const { port, authoring } = setup(assignmentToDraft(dividerAssignment));

    const outcome = await authoring.previewChecks();

    expect(outcome.status).toBe("result");
    const request = port.checkRequests[0]!;
    expect(request.checks).toEqual(dividerAssignment.checks);
    expect(request.lockedComponents).toEqual(
      dividerAssignment.restrictions?.lockedComponents,
    );
    expect(request.startingCircuit).toEqual(dividerAssignment.startingCircuit);
    expect(request.schematic).toEqual(dividerAssignment.reference);
    expect(request.geometry.components).toHaveLength(
      dividerGeometry.components.length,
    );
    expect(request.documentRevision).toBe("preview-1");
    if (outcome.status === "result") {
      expect(outcome.result.documentRevision).toBe("preview-1");
    }
  });

  it("derives the pin geometry of the reference from the symbols, not from the file", async () => {
    const { port, authoring } = setup(assignmentToDraft(dividerAssignment));

    await authoring.previewChecks();

    const reference = dividerAssignment.reference!;
    expect(
      port.checkRequests[0]?.geometry.components.map((c) => c.componentId),
    ).toEqual(reference.components.map((component) => component.id));
    for (const component of port.checkRequests[0]!.geometry.components) {
      expect(component.pins.length).toBeGreaterThanOrEqual(2);
    }
  });

  it("needs a reference solution and a build task", async () => {
    const withoutReference = assignmentToDraft(dividerAssignment);
    const { port, authoring } = setup({ ...withoutReference, reference: null });

    expect(await authoring.previewChecks()).toEqual({
      status: "needsReference",
    });

    const fault = setup(assignmentToDraft(faultAssignment));
    expect(await fault.authoring.previewChecks()).toEqual({
      status: "notBuildTask",
    });
    expect(port.checkRequests).toEqual([]);
    expect(fault.port.checkRequests).toEqual([]);
  });

  it("drops an answer for a draft that has changed while the Engine was working", async () => {
    const { store, authoring, port } = setup(
      assignmentToDraft(dividerAssignment),
    );
    port.duringCheck = () =>
      store.removeCheck(store.getState().draft.checks[0]!.id);

    expect(await authoring.previewChecks()).toEqual({ status: "stale" });
  });

  it("drops an answer that belongs to another request", async () => {
    const { authoring, port } = setup(assignmentToDraft(dividerAssignment));
    port.checkResult = () => resultFor("preview-from-before");

    expect(await authoring.previewChecks()).toEqual({ status: "stale" });
  });

  it("reports an Engine that cannot be reached, with the reason", async () => {
    const { authoring, port } = setup(assignmentToDraft(dividerAssignment));
    port.checkResult = () => {
      throw new Error("A check can have at most 64 criteria.");
    };

    expect(await authoring.previewChecks()).toEqual({
      status: "unavailable",
      reason: "A check can have at most 64 criteria.",
    });
  });
});
