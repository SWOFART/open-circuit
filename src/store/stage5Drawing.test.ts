import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { EditorCommand } from "../editor/commands";
import { buildGeometryEnvelope } from "../editor/geometry";
import type { Assignment } from "../generated/Assignment";
import type { CheckRequest } from "../generated/CheckRequest";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { GridPoint } from "../generated/GridPoint";
import type { ParameterId } from "../generated/ParameterId";
import type { PinId } from "../generated/PinId";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols } from "../symbols";
import { applyEditorCommand } from "../ui/editorCommands";
import { signalOptions } from "../ui/assignment/assignmentView";
import {
  assignmentToDraft,
  AssignmentDraftStore,
  draftToAssignment,
  emptyDraft,
} from "./assignmentDraft";
import type { AssignmentPort } from "./assignmentBridge";
import { SchematicDocumentStore } from "./document";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { blankSchematic } from "./projectWorkflow";
import { SimulationController } from "./simulation";
import { checkStudentCircuit, studentCopyOf } from "./studentAssignment";
import { layoutFor } from "./stage4TestSupport";
import { ComponentLabels } from "./labels";

// Issue #85: the Stage 5 fixtures (docs/contracts/stage-5-assignments.md section 8) built the
// way a teacher and a student build them. The circuits are drawn from a blank document with the
// editor's own commands, so the pin geometry is the real one; the teacher's assignments are
// authored through the draft store and its conversion; and what the student side sends (the
// check requests and the faulted compile request) is built by the real functions. Everything is
// pinned in tests/golden/stage-5-drawn-*, and `crates/app/tests/stage5_session.rs` runs the files
// through the real Rust Engine and bridge code and checks them against the fixtures'
// `stage-5-*.expected.json`.
//
// To regenerate the files after a deliberate change to the symbols, the editor, or the draft,
// run `UPDATE_GOLDEN=1 pnpm exec vitest run src/store/stage5Drawing.test.ts`.

const catalog = catalogFixture as ComponentCatalog;

function goldenPath(name: string): string {
  return fileURLToPath(
    new URL(`../../tests/golden/stage-5-drawn-${name}`, import.meta.url),
  );
}

/** Pins `value` as the file's contents, or compares it with the file. */
function pinned(name: string, value: unknown): void {
  const text = `${JSON.stringify(value, null, 2)}\n`;
  const path = goldenPath(name);

  // A golden file that is missing fails in CI instead of being written there.
  if (process.env.UPDATE_GOLDEN || (!existsSync(path) && !process.env.CI)) {
    writeFileSync(path, text);
  }

  expect(readFileSync(path, "utf8"), name).toBe(text);
}

function ids(start = 1): () => StableId {
  let next = start;
  return () =>
    `00000000-0000-4000-8000-${(next++).toString(16).padStart(12, "0")}`;
}

type Rotation = 0 | 90 | 180 | 270;

/** A document being drawn: placing parts, editing parameters, and wiring pins. */
class Sketch {
  readonly store = new SchematicDocumentStore(blankSchematic, ids());

  private apply(command: EditorCommand): void {
    applyEditorCommand(this.store, command, catalog);
  }

  place(
    typeId: ComponentTypeId,
    position: GridPoint,
    rotationDeg: Rotation,
    edits: Readonly<Record<string, number>> = {},
  ): StableId {
    this.apply({ kind: "placeComponent", typeId, position, rotationDeg });
    const id = this.store.getState().schematic.components.at(-1)!.id;

    for (const [parameter, value] of Object.entries(edits)) {
      this.store.setParameter(id, parameter as ParameterId, value, catalog);
    }

    return id;
  }

  pin(componentId: StableId, pinId: PinId): GridPoint {
    const envelope = buildGeometryEnvelope(
      this.store.getState().schematic,
      symbols,
    );
    const pin = envelope.components
      .find((component) => component.componentId === componentId)
      ?.pins.find((candidate) => candidate.pinId === pinId);

    if (!pin) {
      throw new Error(`No pin ${pinId} on ${componentId}.`);
    }

    return pin.position;
  }

  wire(...points: GridPoint[]): void {
    this.apply({ kind: "addWire", points });
  }

  ground(position: GridPoint): void {
    this.apply({ kind: "addReferenceMarker", position });
  }

  get schematic(): Schematic {
    return this.store.getState().schematic;
  }
}

const p = (x: number, y: number): GridPoint => ({ x, y });

// --- the divider assignment (a `build` task) --------------------------------------------------

/** The assignment's starting circuit: a locked ideal 10 V source and a locked 10 MOhm voltmeter. */
function dividerStart() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 6), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  const meter = sketch.place("voltmeter", p(11, 6), 90, {
    inputResistanceOhms: 1e7,
  });
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  // The ground rail joins the source's and the meter's lower pins.
  sketch.wire(at(source, "pin2"), p(0, 9), p(11, 9), at(meter, "pin2"));
  sketch.ground(at(source, "pin2"));

  return { sketch, source, meter };
}

type Attempt =
  | "correct"
  | "three-resistors"
  | "equal-resistors"
  | "no-resistors"
  | "locked-part-changed"
  | "source-shorted";

const ATTEMPTS: readonly Attempt[] = [
  "correct",
  "three-resistors",
  "equal-resistors",
  "no-resistors",
  "locked-part-changed",
  "source-shorted",
];

/** What a student draws on top of the starting circuit, as the golden fixture describes it. */
function dividerAttempt(attempt: Attempt): Schematic {
  const { sketch, source, meter } = dividerStart();
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  if (attempt === "no-resistors") {
    return sketch.schematic;
  }

  if (attempt === "three-resistors") {
    // The top resistor is 20 kOhm in series with 10 kOhm, and the bottom one is 10 kOhm.
    const first = sketch.place("resistor", p(3, 5), 0, {
      resistanceOhms: 20_000,
    });
    const second = sketch.place("resistor", p(5, 5), 0, {
      resistanceOhms: 10_000,
    });
    const bottom = sketch.place("resistor", p(8, 6), 90, {
      resistanceOhms: 10_000,
    });
    sketch.wire(at(source, "pin1"), at(first, "pin1"));
    sketch.wire(at(second, "pin2"), at(bottom, "pin1"));
    sketch.wire(at(bottom, "pin1"), at(meter, "pin1"));
    sketch.wire(at(bottom, "pin2"), p(8, 9));
    return sketch.schematic;
  }

  const [top, bottomOhms] =
    attempt === "equal-resistors" ? [10_000, 10_000] : [30_000, 10_000];
  const upper = sketch.place("resistor", p(5, 5), 0, { resistanceOhms: top });
  const lower = sketch.place("resistor", p(8, 6), 90, {
    resistanceOhms: bottomOhms,
  });
  sketch.wire(at(source, "pin1"), at(upper, "pin1"));
  sketch.wire(at(upper, "pin2"), at(lower, "pin1"));
  sketch.wire(at(lower, "pin1"), at(meter, "pin1"));
  sketch.wire(at(lower, "pin2"), p(8, 9));

  if (attempt === "locked-part-changed") {
    // A modified file or an editor that was bypassed: the locked source is 20 V.
    sketch.store.setParameter(source, "voltageVolts", 20, catalog);
  }

  if (attempt === "source-shorted") {
    // A wire drawn across the ideal source.
    sketch.wire(at(source, "pin1"), p(-2, 5), p(-2, 7), at(source, "pin2"));
  }

  return sketch.schematic;
}

/** The teacher's assignment, authored through the draft store from the drawn circuits. */
function authoredDivider(): Assignment {
  const start = dividerStart();
  const store = new AssignmentDraftStore(emptyDraft(ids(0x500)), ids(0x501));

  store.setTitle("Divider to 2.5 V");
  store.setInstructions(
    "Add two resistors so that the voltmeter reads 2.5 V.\nThe source and the voltmeter are locked.",
  );
  store.setStartingCircuit(start.sketch.schematic);
  store.setReference(dividerAttempt("correct"));
  store.setComponentLocked(start.source, true);
  store.setComponentLocked(start.meter, true);
  store.setAllowedComponentTypes(["resistor"]);

  // The signal is chosen from the Engine's slots for the circuit, as the panel offers them.
  const labels = new ComponentLabels().labelsFor(start.sketch.schematic);
  const voltage = signalOptions(
    layoutFor(start.sketch.schematic),
    start.sketch.schematic,
    labels,
  ).find((option) => option.key === `${start.meter}:voltage`)!;
  store.addCheck({
    kind: "value",
    id: store.newId(),
    label: "Reading",
    signal: { key: voltage.key, unit: voltage.unit },
    atSeconds: 0,
    expected: 2.5,
    toleranceAbs: 0.025,
    toleranceRel: 0,
  });
  store.addCheck({
    kind: "range",
    id: store.newId(),
    signal: { key: voltage.key, unit: voltage.unit },
    fromSeconds: 0,
    toSeconds: 0.01,
    min: 2.4,
    max: 2.6,
  });
  store.addCheck({
    kind: "componentCount",
    id: store.newId(),
    typeId: "resistor",
    min: 2,
    max: 2,
  });

  return draftToAssignment(store.getState().draft);
}

/** A bridge that only records what the student side sends to be checked. */
class RecordingPort implements AssignmentPort {
  readonly checkRequests: CheckRequest[] = [];
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
  checkDiagnosis(): never {
    throw new Error("not used");
  }
  async checkAssignment(request: CheckRequest) {
    this.checkRequests.push(request);
    // Any answer for this request; the Rust acceptance test gives the real one.
    return {
      documentRevision: request.documentRevision,
      outcome: "passed" as const,
      counts: { pass: 0, fail: 0, notEvaluated: 0 },
      criteria: [],
      locked: {
        verdict: "pass" as const,
        messageKey: "check.locked.pass",
        references: [],
      },
      outcomeKey: "check.outcome.passed",
      diagnostics: [],
    };
  }
}

// --- the fault-finding assignment ------------------------------------------------------------

/** 10 V source, R1 = R2 = 1 kOhm in series, and a 10 MOhm voltmeter across R2. */
function faultCircuit() {
  const sketch = new Sketch();
  const source = sketch.place("dc_voltage_source", p(0, 6), 90, {
    voltageVolts: 10,
    internalResistanceOhms: 0,
  });
  const first = sketch.place("resistor", p(4, 5), 0, { resistanceOhms: 1000 });
  const second = sketch.place("resistor", p(7, 6), 90, {
    resistanceOhms: 1000,
  });
  const meter = sketch.place("voltmeter", p(10, 6), 90, {
    inputResistanceOhms: 1e7,
  });
  const at = (id: StableId, pin: PinId) => sketch.pin(id, pin);

  sketch.wire(at(source, "pin1"), at(first, "pin1"));
  sketch.wire(at(first, "pin2"), at(second, "pin1"));
  sketch.wire(at(second, "pin1"), at(meter, "pin1"));
  sketch.wire(at(source, "pin2"), p(0, 9), p(10, 9), at(meter, "pin2"));
  sketch.wire(at(second, "pin2"), p(7, 9));
  sketch.ground(at(source, "pin2"));

  return { sketch, source, first, second, meter };
}

function authoredFaultFinding(): Assignment {
  const circuit = faultCircuit();
  const store = new AssignmentDraftStore(emptyDraft(ids(0x510)), ids(0xa1));

  store.setTitle("Find the fault");
  store.setInstructions(
    "The voltmeter does not read what the divider should give. Find the faulty part.",
  );
  store.setTaskKind("findFault");
  store.setReveal("afterCheck");
  store.setStartingCircuit(circuit.sketch.schematic);
  for (const id of [
    circuit.source,
    circuit.first,
    circuit.second,
    circuit.meter,
  ]) {
    store.setComponentLocked(id, true);
  }
  store.setAllowedComponentTypes(["voltmeter", "ammeter"]);
  store.addFault({
    id: store.newId(),
    kind: "open",
    componentId: circuit.second,
  });

  return draftToAssignment(store.getState().draft);
}

describe("the Stage 5 circuits, drawn and authored", () => {
  it("draws the divider's starting circuit and every attempt from a blank document", () => {
    const start = dividerStart().sketch.schematic;

    expect(start.components.map((component) => component.typeId)).toEqual([
      "dc_voltage_source",
      "voltmeter",
    ]);
    for (const attempt of ATTEMPTS) {
      const drawn = dividerAttempt(attempt);
      const types = drawn.components.map((component) => component.typeId);

      // A student adds to the starting circuit and never replaces its parts.
      expect(
        drawn.components.slice(0, 2).map((component) => component.id),
      ).toEqual(start.components.map((component) => component.id));
      if (attempt !== "locked-part-changed") {
        expect(drawn.components.slice(0, 2)).toEqual(start.components);
      }
      expect(types.filter((type) => type === "resistor")).toHaveLength(
        {
          correct: 2,
          "three-resistors": 3,
          "equal-resistors": 2,
          "no-resistors": 0,
          "locked-part-changed": 2,
          "source-shorted": 2,
        }[attempt],
      );
    }
  });

  it("authors the divider assignment through the draft store and pins the file", () => {
    const assignment = authoredDivider();

    expect(assignment.task).toEqual({ kind: "build" });
    expect(assignment.restrictions).toEqual({
      allowedComponentTypes: ["resistor"],
      lockedComponents: [
        "00000000-0000-4000-8000-000000000001",
        "00000000-0000-4000-8000-000000000002",
      ],
    });
    expect(assignment.checks).toHaveLength(3);
    expect(assignment.reference?.components).toHaveLength(4);
    // Drawn and authored in the app's own way, the file survives the draft round trip.
    expect(draftToAssignment(assignmentToDraft(assignment))).toEqual(
      assignment,
    );
    pinned("divider-build.ocassignment", assignment);
  });

  it("pins what the student side sends to be checked, for every attempt", async () => {
    const assignment = studentCopyOf(authoredDivider());
    const port = new RecordingPort();

    for (const attempt of ATTEMPTS) {
      const schematic = dividerAttempt(attempt);
      const outcome = await checkStudentCircuit(
        port,
        assignment,
        schematic,
        symbols,
        () => "check-1",
      );
      expect(outcome.status, attempt).toBe("result");
      pinned(`divider-${attempt}.check.json`, port.checkRequests.at(-1));
    }

    // The request carries the student's circuit and the real geometry of its pins.
    const correct = port.checkRequests[0]!;
    expect(correct.schematic).toEqual(dividerAttempt("correct"));
    expect(correct.geometry).toEqual(
      buildGeometryEnvelope(dividerAttempt("correct"), symbols),
    );
    expect(correct.startingCircuit).toEqual(assignment.startingCircuit);
    expect(correct.lockedComponents).toEqual(
      assignment.restrictions?.lockedComponents,
    );
  });

  it("authors the fault-finding assignment and pins the file and the faulted compile request", async () => {
    const assignment = authoredFaultFinding();
    const circuit = faultCircuit();

    expect(assignment.task).toEqual({
      kind: "findFault",
      reveal: "afterCheck",
    });
    expect(assignment.faults).toEqual([
      {
        id: "00000000-0000-4000-8000-0000000000a1",
        kind: "open",
        componentId: circuit.second,
      },
    ]);
    expect(assignment).not.toHaveProperty("checks");
    pinned("fault-finding.ocassignment", assignment);

    // The student's session compiles the circuit the student sees with the faults the Engine
    // applies: this is the request the controller sends.
    const adapter = new PredeterminedEngineAdapter({
      catalog,
      compile: () => ({ status: "failure", diagnostics: [] }),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    const controller = new SimulationController(
      adapter,
      symbols,
      () => "rev-1",
    );
    await controller.setFaults(assignment.faults ?? null);
    await controller.start(assignment.startingCircuit!);

    const request: CompileRequest = adapter.compileRequests[0]!;
    expect(request.faults).toEqual(assignment.faults);
    expect(request.schematic).toEqual(circuit.sketch.schematic);
    // The fault is in the request and nowhere in the schematic.
    expect(JSON.stringify(request.schematic)).not.toContain("faults");
    pinned("fault-finding.request.json", request);
  });
});
