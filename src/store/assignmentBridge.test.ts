import { beforeEach, describe, expect, it, vi } from "vitest";
import assignmentText from "../../tests/golden/stage-5-fault-finding.ocassignment?raw";
import type { Assignment } from "../generated/Assignment";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CheckResult } from "../generated/CheckResult";

// A stand-in for the Tauri JavaScript API: commands are recorded and answered by the test.
const tauri = vi.hoisted(() => {
  type Call = { command: string; args: Record<string, unknown> | undefined };

  const state = {
    inTauri: true,
    calls: [] as Call[],
    answer: (() => undefined) as (
      command: string,
      args?: Record<string, unknown>,
    ) => unknown,
  };

  return { state };
});

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: () => tauri.state.inTauri,
  invoke: (command: string, args?: Record<string, unknown>) => {
    tauri.state.calls.push({ command, args });

    try {
      return Promise.resolve(tauri.state.answer(command, args));
    } catch (error) {
      return Promise.reject(error);
    }
  },
}));

import { checkResultBelongsTo, tauriAssignments } from "./assignmentBridge";

const assignment = JSON.parse(assignmentText) as Assignment;

beforeEach(() => {
  tauri.state.inTauri = true;
  tauri.state.calls = [];
  tauri.state.answer = () => undefined;
});

describe("the assignment bridge", () => {
  it("sends each operation to its native command with the arguments Rust reads", async () => {
    tauri.state.answer = () => ({ status: "saved" });
    const request = { checks: [] } as unknown as CheckRequest;
    const diagnosis = [
      { componentId: "00000000-0000-4000-8000-000000000003", kind: "open" },
    ] as const;

    await tauriAssignments.open();
    await tauriAssignments.validate(assignment);
    await tauriAssignments.save(assignment);
    await tauriAssignments.saveAs(assignment);
    await tauriAssignments.exportStudentCopy(assignment);
    await tauriAssignments.clearPath();
    await tauriAssignments.checkAssignment(request);
    await tauriAssignments.checkDiagnosis(assignment, [...diagnosis]);

    expect(tauri.state.calls).toEqual([
      { command: "open_assignment", args: undefined },
      { command: "validate_assignment", args: { assignment } },
      { command: "save_assignment", args: { assignment } },
      { command: "save_assignment_as", args: { assignment } },
      { command: "export_student_copy", args: { assignment } },
      { command: "clear_assignment_path", args: undefined },
      { command: "check_assignment", args: { request } },
      {
        command: "check_fault_diagnosis",
        args: { assignment, diagnosis },
      },
    ]);
  });

  it("returns the Engine's structured answers as they are", async () => {
    const invalid = {
      status: "invalid",
      errors: [
        {
          code: "UnknownField",
          path: "/grade",
          messageKey: "assignment.error.unknownField",
        },
      ],
    };
    tauri.state.answer = () => invalid;

    await expect(tauriAssignments.validate(assignment)).resolves.toEqual(
      invalid,
    );
    tauri.state.answer = () => null;
    await expect(tauriAssignments.open()).resolves.toBeNull();
  });

  it("throws a plain error when the backend cannot be reached or a file cannot be read", async () => {
    tauri.state.answer = () => {
      throw "Could not read assignment file: access denied";
    };

    await expect(tauriAssignments.open()).rejects.toThrow(
      "Could not read assignment file: access denied",
    );

    tauri.state.inTauri = false;
    tauri.state.calls = [];
    await expect(tauriAssignments.save(assignment)).rejects.toThrow(
      "Assignment files are available in the desktop app only.",
    );
    expect(tauri.state.calls).toEqual([]);
  });

  it("shows a check result only for the document revision it was made for", () => {
    const result = { documentRevision: "rev-9" } as CheckResult;

    expect(checkResultBelongsTo(result, "rev-9")).toBe(true);
    expect(checkResultBelongsTo(result, "rev-10")).toBe(false);
    expect(checkResultBelongsTo(result, "")).toBe(false);
  });
});
