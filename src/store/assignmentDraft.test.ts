import { describe, expect, it } from "vitest";
import divider from "../../tests/golden/stage-5-divider-build.ocassignment?raw";
import faultFinding from "../../tests/golden/stage-5-fault-finding.ocassignment?raw";
import fuse from "../../tests/golden/stage-5-fuse-state.ocassignment?raw";
import type { Assignment } from "../generated/Assignment";
import type { Schematic } from "../generated/Schematic";
import {
  AssignmentDraftStore,
  assignmentToDraft,
  draftToAssignment,
  emptyDraft,
} from "./assignmentDraft";

const files: [string, Assignment][] = [
  ["divider", JSON.parse(divider) as Assignment],
  ["fuse", JSON.parse(fuse) as Assignment],
  ["fault finding", JSON.parse(faultFinding) as Assignment],
];

const V1 = "00000000-0000-4000-8000-000000000001";
const VM1 = "00000000-0000-4000-8000-000000000002";

function ids(): () => string {
  let next = 0;
  return () =>
    `00000000-0000-4000-8000-${(0x9000 + next++).toString(16).padStart(12, "0")}`;
}

describe("the draft and the assignment document", () => {
  it.each(files)(
    "round-trips the canonical %s file without losing or adding anything",
    (_name, assignment) => {
      const again = draftToAssignment(assignmentToDraft(assignment));

      // The same document: same fields, same values, same order of the lists.
      expect(again).toEqual(assignment);
      expect(JSON.stringify(again)).toBe(JSON.stringify(assignment));
    },
  );

  it("writes the fields of the task kind and none of the other", () => {
    const build = assignmentToDraft(files[0]![1]);
    const asFault = { ...build, taskKind: "findFault" as const };
    const buildDocument = draftToAssignment(build);
    const faultDocument = draftToAssignment(asFault);

    expect(buildDocument.task).toEqual({ kind: "build" });
    expect(buildDocument).toHaveProperty("checks");
    expect(buildDocument).not.toHaveProperty("faults");
    expect(faultDocument.task).toEqual({
      kind: "findFault",
      reveal: "afterCheck",
    });
    expect(faultDocument).toHaveProperty("faults");
    expect(faultDocument).not.toHaveProperty("checks");
  });

  it("leaves out the circuits it does not have, and keeps null as every type", () => {
    const document = draftToAssignment(emptyDraft(ids()));

    expect(document).not.toHaveProperty("startingCircuit");
    expect(document).not.toHaveProperty("reference");
    expect(document.restrictions).toEqual({ lockedComponents: [] });
    expect(document.schemaVersion).toBe(1);
  });

  it("copies the circuits, so a later edit of the source cannot change the draft", () => {
    const store = new AssignmentDraftStore(emptyDraft(ids()), ids());
    const source = structuredClone(files[0]![1].startingCircuit) as Schematic;

    store.setStartingCircuit(source);
    source.components.length = 0;

    expect(store.getState().draft.startingCircuit?.components).toHaveLength(2);
    expect(draftToAssignment(store.getState().draft).startingCircuit).not.toBe(
      store.getState().draft.startingCircuit,
    );
  });
});

describe("the draft store", () => {
  it("keeps the checks and the faults when the task kind is switched and back", () => {
    const store = new AssignmentDraftStore(
      assignmentToDraft(files[0]![1]),
      ids(),
    );
    const checks = store.getState().draft.checks;

    store.setTaskKind("findFault");
    store.addFault({ id: store.newId(), kind: "open", componentId: VM1 });
    store.setTaskKind("build");

    expect(store.getState().draft.checks).toEqual(checks);
    expect(store.getState().draft.faults).toHaveLength(1);
  });

  it("counts every edit as a new version and tracks unsaved changes", () => {
    const store = new AssignmentDraftStore(emptyDraft(ids()), ids());
    const first = store.getState().version;
    let notified = 0;
    store.subscribe(() => (notified += 1));

    expect(store.getState().isDirty).toBe(false);
    store.setTitle("Divider");
    expect(store.getState().version).toBe(first + 1);
    expect(store.getState().isDirty).toBe(true);

    store.setTitle("");
    expect(store.getState().isDirty).toBe(false);
    expect(notified).toBe(2);
  });

  it("stays dirty when it is edited while a save is under way", () => {
    const store = new AssignmentDraftStore(emptyDraft(ids()), ids());

    store.setTitle("Divider");
    const saving = store.getState().draft;
    store.setTitle("Divider 2");
    store.markSaved(saving);

    expect(store.getState().isDirty).toBe(true);
    store.markSaved();
    expect(store.getState().isDirty).toBe(false);
  });

  it("counts a file that was opened as saved", () => {
    const store = new AssignmentDraftStore(emptyDraft(ids()), ids());

    store.setTitle("Unsaved");
    store.replaceWith(assignmentToDraft(files[0]![1]));

    expect(store.getState().isDirty).toBe(false);
    expect(store.getState().draft.title).toBe("Divider to 2.5 V");
  });

  it("drops a lock on a part the new starting circuit does not have, and nothing else", () => {
    const store = new AssignmentDraftStore(
      assignmentToDraft(files[0]![1]),
      ids(),
    );
    const checks = store.getState().draft.checks;
    const withoutMeter = structuredClone(
      files[0]![1].startingCircuit,
    ) as Schematic;
    withoutMeter.components = withoutMeter.components.filter(
      (component) => component.id !== VM1,
    );

    store.setStartingCircuit(withoutMeter);

    expect(store.getState().draft.lockedComponents).toEqual([V1]);
    expect(store.getState().draft.checks).toEqual(checks);
  });

  it("locks and unlocks a part once, in the order it was locked", () => {
    const store = new AssignmentDraftStore(emptyDraft(ids()), ids());

    store.setComponentLocked(V1, true);
    store.setComponentLocked(V1, true);
    store.setComponentLocked(VM1, true);
    store.setComponentLocked(V1, false);

    expect(store.getState().draft.lockedComponents).toEqual([VM1]);
  });

  it("replaces and removes a check or a fault by its ID", () => {
    const store = new AssignmentDraftStore(
      assignmentToDraft(files[0]![1]),
      ids(),
    );
    const [first, second] = store.getState().draft.checks;

    store.replaceCheck({ ...first!, label: "Reading" });
    store.removeCheck(second!.id);

    expect(store.getState().draft.checks).toHaveLength(2);
    expect(store.getState().draft.checks[0]?.label).toBe("Reading");

    const faults = new AssignmentDraftStore(
      assignmentToDraft(files[2]![1]),
      ids(),
    );
    const fault = faults.getState().draft.faults[0]!;
    faults.replaceFault({ ...fault, kind: "short" });
    expect(faults.getState().draft.faults[0]?.kind).toBe("short");
    faults.removeFault(fault.id);
    expect(faults.getState().draft.faults).toEqual([]);
  });

  it("counts a change of either circuit as an edit, so an answer for the old one goes stale", () => {
    const store = new AssignmentDraftStore(
      assignmentToDraft(files[0]![1]),
      ids(),
    );
    const start = store.getState().version;

    store.setReference(files[0]![1].startingCircuit as Schematic);
    store.setStartingCircuit(files[0]![1].reference as Schematic);

    expect(store.getState().version).toBe(start + 2);
    expect(store.getState().isDirty).toBe(true);
  });

  it("starts a new assignment with a new ID and nothing else", () => {
    const factory = ids();
    const store = new AssignmentDraftStore(
      assignmentToDraft(files[0]![1]),
      factory,
    );
    const before = store.getState().draft.id;

    store.reset();

    expect(store.getState().draft).toEqual(
      emptyDraft(() => store.getState().draft.id),
    );
    expect(store.getState().draft.id).not.toBe(before);
    expect(store.getState().isDirty).toBe(false);
  });

  it("allows every part type with null and a list otherwise, without repeats", () => {
    const store = new AssignmentDraftStore(emptyDraft(ids()), ids());

    store.setAllowedComponentTypes(["resistor", "resistor", "fuse"]);
    expect(store.getState().draft.allowedComponentTypes).toEqual([
      "resistor",
      "fuse",
    ]);
    store.setAllowedComponentTypes(null);
    expect(store.getState().draft.allowedComponentTypes).toBeNull();
  });
});
