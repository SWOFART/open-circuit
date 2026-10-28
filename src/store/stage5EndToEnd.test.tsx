import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Assignment } from "../generated/Assignment";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CheckResult } from "../generated/CheckResult";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { DiagnosisEntry } from "../generated/DiagnosisEntry";
import type { DiagnosisResult } from "../generated/DiagnosisResult";
import type { Schematic } from "../generated/Schematic";
import { symbols } from "../symbols";
import { CheckResultView } from "../ui/assignment/CheckResultView";
import { DiagnosisResultView } from "../ui/assignment/DiagnosisResultView";
import { ResultPanel } from "../ui/ResultPanel";
import { assignmentToDraft, draftToAssignment } from "./assignmentDraft";
import type { AssignmentPort } from "./assignmentBridge";
import { TauriEngineAdapter } from "./engineAdapter";
import { componentLabels } from "./fixtures";
import { readSlot, SimulationController } from "./simulation";
import {
  answerForKey,
  checkStudentCircuit,
  checkStudentDiagnosis,
  studentCopyOf,
} from "./studentAssignment";

// Issue #85: the Stage 5 workflows on the real Engine, replayed through the real TypeScript
// stack. `crates/app/tests/stage5_session.rs` runs the canonical assignments through the real
// Engine and the bridge code and records the requests and the replies; this file feeds the
// recording to the adapter, the controller, the student-side request builders, and the panels.
// Only the Tauri API itself is replaced, by a replay of the recording. A request the interface
// builds that differs from the recorded one is not answered, so the two sides cannot drift.
//
// `pnpm test` runs the Rust tests first, which write the recording. Running Vitest alone needs
// the recording to exist already; it is skipped locally when absent and required in CI.

interface FrameRecord {
  step: number;
  hex: string;
}

interface Attempt {
  name: string;
  request: CheckRequest;
  result: CheckResult;
}

interface Session {
  catalog: ComponentCatalog;
  divider: {
    teacherFile: Assignment;
    studentCopyText: string;
    attempts: Attempt[];
  };
  fault: {
    teacherFile: Assignment;
    studentCopyText: string;
    request: CompileRequest;
    faultedReply: CompileResult & { status: "success" };
    faultedFrames: FrameRecord[];
    healthyReply: CompileResult & { status: "success" };
    diagnoses: {
      name: string;
      submitted: DiagnosisEntry[];
      result: DiagnosisResult;
    }[];
  };
}

const sessionPath =
  process.env.OPEN_CIRCUIT_STAGE5_SESSION ??
  fileURLToPath(
    new URL("../../target/tmp/stage5-session.json", import.meta.url),
  );
const recorded = existsSync(sessionPath);

if (!recorded && process.env.CI) {
  throw new Error(
    `The recorded Engine session ${sessionPath} is missing. \`pnpm test\` writes it by running the Rust tests first.`,
  );
}

const session = (
  recorded ? JSON.parse(readFileSync(sessionPath, "utf8")) : {}
) as Session;

const tauri = vi.hoisted(() => {
  class FakeChannel {
    onmessage: (message: unknown) => void = () => {};
  }

  return {
    FakeChannel,
    state: {
      frameChannel: null as InstanceType<typeof FakeChannel> | null,
      calls: [] as {
        command: string;
        args: Record<string, unknown> | undefined;
      }[],
      answer: (() => undefined) as (
        command: string,
        args?: Record<string, unknown>,
      ) => unknown,
    },
  };
});

vi.mock("@tauri-apps/api/core", () => ({
  Channel: tauri.FakeChannel,
  isTauri: () => true,
  invoke: (command: string, args?: Record<string, unknown>) => {
    tauri.state.calls.push({ command, args });

    if (command === "start_simulation") {
      tauri.state.frameChannel = args?.onFrame as InstanceType<
        typeof tauri.FakeChannel
      >;
    }

    try {
      return Promise.resolve(tauri.state.answer(command, args));
    } catch (error) {
      return Promise.reject(error);
    }
  },
}));

const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const METER = "00000000-0000-4000-8000-000000000004";

function fromHex(hex: string): ArrayBuffer {
  const bytes = new Uint8Array(hex.length / 2);

  for (let index = 0; index < bytes.length; index += 1) {
    bytes[index] = parseInt(hex.slice(2 * index, 2 * index + 2), 16);
  }

  return bytes.buffer;
}

function revisions(): () => string {
  let next = 0;
  return () => `rev-${(next += 1)}`;
}

/** The compile reply for a later compile: the same circuit answered with the new revision. */
function withRevision(
  reply: CompileResult & { status: "success" },
  documentRevision: string,
): CompileResult {
  return {
    ...reply,
    initialSnapshot: {
      ...reply.initialSnapshot,
      metadata: { ...reply.initialSnapshot.metadata, documentRevision },
    },
  };
}

/** The same JSON whatever the order of an object's keys (the Engine's writer orders them its own way). */
function canonical(value: unknown): string {
  return JSON.stringify(value, (_key, item: unknown) =>
    item !== null && typeof item === "object" && !Array.isArray(item)
      ? Object.fromEntries(
          Object.entries(item as Record<string, unknown>).sort(([a], [b]) =>
            a < b ? -1 : 1,
          ),
        )
      : item,
  );
}

/** A bridge that answers only the requests the Engine was recorded answering. */
class ReplayPort implements AssignmentPort {
  constructor(
    private readonly checks: readonly Attempt[],
    private readonly diagnoses: Session["fault"]["diagnoses"],
  ) {}

  async checkAssignment(request: CheckRequest): Promise<CheckResult> {
    const text = canonical(request);
    const attempt = this.checks.find(
      (candidate) => canonical(candidate.request) === text,
    );

    if (!attempt) {
      throw new Error("No recorded answer for this check request.");
    }

    return attempt.result;
  }

  async checkDiagnosis(_assignment: Assignment, diagnosis: DiagnosisEntry[]) {
    const text = canonical(diagnosis);
    const entry = this.diagnoses.find(
      (candidate) => canonical(candidate.submitted) === text,
    );

    if (!entry) {
      throw new Error("No recorded answer for this diagnosis.");
    }

    return { status: "checked" as const, result: entry.result };
  }

  open(): never {
    throw new Error("not used");
  }
  validate(): never {
    throw new Error("not used");
  }
  save(): never {
    throw new Error("not used");
  }
  saveAs(): never {
    throw new Error("not used");
  }
  exportStudentCopy(): never {
    throw new Error("not used");
  }
  clearPath(): never {
    throw new Error("not used");
  }
}

function checkView(
  result: CheckResult,
  schematic: Schematic,
  assignment: Assignment,
): string {
  return renderToStaticMarkup(
    <CheckResultView
      result={result}
      labels={componentLabels(schematic)}
      names={
        new Map(
          (assignment.checks ?? []).flatMap((check) =>
            check.label ? [[check.id, check.label] as const] : [],
          ),
        )
      }
    />,
  );
}

beforeEach(() => {
  vi.useFakeTimers();
  tauri.state.calls = [];
  tauri.state.frameChannel = null;
});

afterEach(() => {
  vi.useRealTimers();
});

describe.skipIf(!recorded)(
  "the divider assignment, from teacher to student",
  () => {
    const teacherFile = () => session.divider.teacherFile;
    const studentCopy = () =>
      JSON.parse(session.divider.studentCopyText) as Assignment;

    it("is the file the application's own authoring writes, and the student's copy is that file without the reference", () => {
      // The draft the panel builds from the file converts back to the same document.
      expect(draftToAssignment(assignmentToDraft(teacherFile()))).toEqual(
        teacherFile(),
      );
      // What the Engine's writer wrote for the student is what the student side keeps.
      expect(studentCopy()).toEqual(studentCopyOf(teacherFile()));
      expect(session.divider.studentCopyText).not.toContain('"reference"');
      expect(studentCopy().checks).toHaveLength(3);
    });

    it("builds, for every attempt, the check request the Engine was recorded answering", async () => {
      const port = new ReplayPort(session.divider.attempts, []);

      for (const attempt of session.divider.attempts) {
        const outcome = await checkStudentCircuit(
          port,
          studentCopy(),
          attempt.request.schematic,
          symbols,
          () => attempt.request.documentRevision,
        );

        expect(outcome, attempt.name).toEqual({
          status: "result",
          result: attempt.result,
        });
      }
    });

    const textOf = (name: string): string => {
      const attempt = session.divider.attempts.find(
        (candidate) => candidate.name === name,
      )!;
      return checkView(
        attempt.result,
        attempt.request.schematic,
        studentCopy(),
      );
    };

    it("shows the correct attempt as passed, with the Engine's reading and units", () => {
      const html = textOf("correct");

      expect(html).toContain("Every check passed.");
      expect(html).toContain("3 passed, 0 failed, 0 not checked.");
      expect(html).toContain("Check 1 (Reading): Passed.");
      expect(html).toContain("expected 2.5 V, ±25 mV");
      expect(html).toContain("Measured 2.498126 V.");
      expect(html).toContain("Expected exactly 2.");
      expect(html).toContain("Found 2.");
      expect(html).toContain("The locked parts are unchanged.");
    });

    it("shows the partial attempts: right reading with three resistors, right count with the wrong ratio", () => {
      const three = textOf("three-resistors");
      expect(three).toContain("Some checks passed.");
      expect(three).toContain("Check 1 (Reading): Passed.");
      expect(three).toContain("Measured 2.498126 V.");
      expect(three).toContain("Check 3: Failed.");
      expect(three).toContain("Found 3.");

      const equal = textOf("equal-resistors");
      expect(equal).toContain("Some checks passed.");
      expect(equal).toContain("Check 1 (Reading): Failed.");
      expect(equal).toContain("Measured 4.997501 V.");
      expect(equal).toContain("The value is outside the tolerance.");
      expect(equal).toContain("Check 3: Passed.");
    });

    it("shows the incorrect attempts: no resistors reads 0 V, and a changed locked part fails everything", () => {
      const none = textOf("no-resistors");
      expect(none).toContain("The circuit does not meet the task.");
      expect(none).toContain("Measured 0 V.");
      expect(none).toContain("Found 0.");

      const locked = textOf("locked-part-changed");
      expect(locked).toContain("The circuit does not meet the task.");
      expect(locked).toContain("Locked parts: Failed.");
      expect(locked).toContain("A locked part was changed or removed.");
      expect(locked).toContain("(V1)");
    });

    it("says why a circuit that does not compile could not be checked, and still counts its parts", () => {
      const shorted = textOf("source-shorted");

      expect(shorted).toContain("The circuit could not be checked.");
      expect(shorted).toContain("An ideal source is short-circuited.");
      expect(shorted).toContain("Check 1 (Reading): Not checked.");
      expect(shorted).toContain("The value could not be checked.");
      expect(shorted).toContain("Check 3: Passed.");
      expect(shorted).not.toMatch(/NaN|Infinity|undefined/);
    });

    it("never shows a result for a circuit it does not belong to", async () => {
      const [correct, equal] = session.divider.attempts;
      const port = new ReplayPort(session.divider.attempts, []);
      const outcome = await checkStudentCircuit(
        port,
        studentCopy(),
        correct!.request.schematic,
        symbols,
        () => correct!.request.documentRevision,
      );
      if (outcome.status !== "result") throw new Error("a result");

      // Shown for the circuit it was asked for; not for another circuit, not for an edited copy.
      const answer = { key: correct!.request.schematic, value: outcome.result };
      expect(answerForKey(answer, correct!.request.schematic)).toBe(
        outcome.result,
      );
      expect(answerForKey(answer, equal!.request.schematic)).toBeNull();
      expect(
        answerForKey(answer, structuredClone(correct!.request.schematic)),
      ).toBeNull();

      // A result whose revision is not the request's is dropped: a bridge that answers with the
      // recorded result (made for revision "check-1") for a request made under another revision.
      class LateAnswerPort extends ReplayPort {
        override async checkAssignment(): Promise<CheckResult> {
          return correct!.result;
        }
      }
      const lateAnswer = new LateAnswerPort([], []);
      expect(correct!.result.documentRevision).toBe("check-1");
      expect(
        await checkStudentCircuit(
          lateAnswer,
          studentCopy(),
          correct!.request.schematic,
          symbols,
          () => "another-revision",
        ),
      ).toEqual({ status: "stale" });
      // And a request the recording has no answer for is an unavailable Engine, not a result.
      expect(
        await checkStudentCircuit(
          port,
          studentCopy(),
          correct!.request.schematic,
          symbols,
          () => "another-revision",
        ),
      ).toMatchObject({ status: "unavailable" });
    });
  },
);

describe.skipIf(!recorded)(
  "the fault-finding assignment, from the student's side",
  () => {
    const fault = () => session.fault;
    const schematic = () => fault().request.schematic;

    function replayEngine(): void {
      tauri.state.answer = (command, args) => {
        switch (command) {
          case "get_component_catalog":
            return session.catalog;
          case "compile_circuit": {
            const request = args?.request as CompileRequest;
            return withRevision(
              request.faults ? fault().faultedReply : fault().healthyReply,
              request.documentRevision,
            );
          }
          default:
            return undefined;
        }
      };
    }

    function deliver(frame: FrameRecord): void {
      tauri.state.frameChannel?.onmessage(fromHex(frame.hex));
    }

    const meter = (simulation: SimulationController): number | undefined =>
      readSlot(
        simulation.getState().run,
        simulation.getState().snapshot,
        METER,
        "voltage",
      );

    it("sends the faults with the circuit the student sees, and shows the faulted reading the Engine gave", async () => {
      replayEngine();
      const simulation = new SimulationController(
        new TauriEngineAdapter(),
        symbols,
        revisions(),
      );

      await simulation.setFaults(fault().request.faults ?? null);
      await simulation.start(schematic());
      await vi.advanceTimersByTimeAsync(0);

      const sent = tauri.state.calls.find(
        (call) => call.command === "compile_circuit",
      );
      expect(sent?.args?.request).toEqual({
        ...fault().request,
        documentRevision: "rev-1",
      });
      expect(simulation.getState().status).toBe("ready");
      expect(meter(simulation)).toBeCloseTo(9.99900009999, 8);

      const html = renderToStaticMarkup(
        <ResultPanel
          state={simulation.getState()}
          labels={componentLabels(schematic())}
        />,
      );
      expect(html).toContain("VM1 voltage");
      expect(html).toContain("9.999 V");
      expect(html).not.toMatch(/NaN|Infinity/);

      // The frames that follow carry the same reading, in order.
      for (const frame of fault().faultedFrames) deliver(frame);
      expect(simulation.getState().snapshot?.metadata.simulationStep).toBe(3n);
      expect(meter(simulation)).toBeCloseTo(9.99900009999, 8);
      expect(simulation.getState().discardedFrames).toBe(0);
    });

    it("starts a healthy run when the faults go, and refuses a frame of the faulted run afterwards", async () => {
      replayEngine();
      const simulation = new SimulationController(
        new TauriEngineAdapter(),
        symbols,
        revisions(),
      );

      await simulation.setFaults(fault().request.faults ?? null);
      await simulation.start(schematic());
      await vi.advanceTimersByTimeAsync(0);
      const faultedRun = simulation.getState().run?.runId;

      await simulation.setFaults(null);
      await vi.advanceTimersByTimeAsync(0);
      const compiles = tauri.state.calls.filter(
        (call) => call.command === "compile_circuit",
      );

      expect(compiles).toHaveLength(2);
      expect(compiles[1]?.args?.request).not.toHaveProperty("faults");
      expect(compiles[1]?.args?.request).toMatchObject({
        schematic: schematic(),
        documentRevision: "rev-2",
      });
      expect(meter(simulation)).toBeCloseTo(4.999750012499375, 8);

      const healthyRun = simulation.getState().run?.runId;
      expect(healthyRun).toBeDefined();
      expect(healthyRun).not.toBe(faultedRun);

      // A late frame of the faulted run is discarded, and the healthy reading stays.
      deliver(fault().faultedFrames[0]!);
      expect(simulation.getState().discardedFrames).toBe(1);
      expect(meter(simulation)).toBeCloseTo(4.999750012499375, 8);
    });

    it("keeps the fault out of the circuit the student edits and saves", () => {
      const text = JSON.stringify(schematic());

      expect(text).not.toContain("faults");
      expect(text).not.toContain(fault().request.faults![0]!.id);
      expect(fault().teacherFile.startingCircuit).toEqual(schematic());
    });

    it("asks the Engine to compare each diagnosis and shows as much of the answer as the task allows", async () => {
      const student = JSON.parse(fault().studentCopyText) as Assignment;
      const port = new ReplayPort([], fault().diagnoses);
      const labels = componentLabels(schematic());
      const render = (
        result: DiagnosisResult,
        reveal: "never" | "afterCheck",
      ) =>
        renderToStaticMarkup(
          <DiagnosisResultView
            result={result}
            reveal={reveal}
            labels={labels}
            faults={student.faults ?? []}
          />,
        );
      const outcomes: Record<string, string> = {
        correct: "You found every fault and named no wrong one.",
        "right-component-wrong-kind": "You found part of the problem.",
        "right-fault-and-a-false-one": "You found part of the problem.",
        "wrong-component": "None of your findings were right.",
        "nothing-submitted": "None of your findings were right.",
      };

      expect(fault().diagnoses).toHaveLength(5);
      for (const diagnosis of fault().diagnoses) {
        const outcome = await checkStudentDiagnosis(
          port,
          student,
          diagnosis.submitted,
        );
        if (outcome.status !== "checked") throw new Error(diagnosis.name);

        // With the policy `never`: the outcome, and nothing of a finding or a fault.
        const hidden = render(outcome.result, "never");
        expect(hidden, diagnosis.name).toContain(outcomes[diagnosis.name]);
        expect(hidden).not.toMatch(/Finding|Fault \d|R1|R2/);

        // After a check: each finding's verdict and each fault, with the part.
        const shown = render(outcome.result, "afterCheck");
        expect(shown, diagnosis.name).toContain(outcomes[diagnosis.name]);
        expect(shown).toContain("Fault 1:");
        expect(shown).toContain("R2");
        for (const [index] of diagnosis.submitted.entries()) {
          expect(shown).toContain(`Finding ${index + 1}:`);
        }
      }

      const verdictOf = (name: string) =>
        render(
          fault().diagnoses.find((diagnosis) => diagnosis.name === name)!
            .result,
          "afterCheck",
        );
      expect(verdictOf("correct")).toContain("You found this fault.");
      expect(verdictOf("correct")).toContain("That part has that fault.");
      expect(verdictOf("right-component-wrong-kind")).toContain(
        "That part is faulty, but not in that way.",
      );
      expect(verdictOf("right-fault-and-a-false-one")).toContain(
        "That part has no fault.",
      );
      expect(verdictOf("wrong-component")).toContain(
        "You did not find this fault.",
      );
      expect(verdictOf("nothing-submitted")).not.toContain("Finding");
      // The two parts the fixture names are the ones the drawn circuit has.
      expect([R1, R2].every((id) => labels.has(id))).toBe(true);
    });
  },
);
