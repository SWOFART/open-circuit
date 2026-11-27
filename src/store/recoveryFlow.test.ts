import { describe, expect, it } from "vitest";
import type { Schematic } from "../generated/Schematic";
import { SchematicDocumentStore } from "./document";
import { dividerSchematic } from "./fixtures";
import type { ProjectFilePort } from "./projectFiles";
import type {
  RecoveryOffer,
  RecoveryPort,
  RecoveryProbe,
} from "./recoveryFiles";
import { loadRecoveryStartup, restoreRecovery } from "./recoveryFlow";

// What the start-up offer does (docs/contracts/stage-16-editor-improvements.md section 9).

const R1 = "00000000-0000-4000-8000-000000000002";

function port(over: Partial<RecoveryPort> = {}): RecoveryPort {
  return {
    available: true,
    write: () => Promise.resolve(),
    probe: () => Promise.resolve({ kind: "none" }),
    discard: () => Promise.resolve(),
    ...over,
  };
}

const edited: Schematic = {
  ...dividerSchematic,
  components: dividerSchematic.components.slice(0, 2),
};
const offer: RecoveryOffer = {
  writtenAtMs: 1_700_000_000_000,
  sourcePath: "C:/work/lab.ocircuit",
  appVersion: "1.0.0",
  schematic: edited,
};

describe("the look at start", () => {
  it("offers what the native side found", async () => {
    const probe: RecoveryProbe = { kind: "offer", ...offer };

    expect(
      await loadRecoveryStartup(port({ probe: () => Promise.resolve(probe) })),
    ).toEqual(probe);
    expect(
      await loadRecoveryStartup(
        port({
          probe: () =>
            Promise.resolve({ kind: "unreadable", reason: "damaged" }),
        }),
      ),
    ).toEqual({ kind: "unreadable", reason: "damaged" });
  });

  it("starts clean when there is nothing, when there is no native side, and when the look fails", async () => {
    expect(await loadRecoveryStartup(port())).toEqual({ kind: "none" });

    let asked = false;
    expect(
      await loadRecoveryStartup(
        port({
          available: false,
          probe: () => {
            asked = true;
            return Promise.resolve({ kind: "none" });
          },
        }),
      ),
    ).toEqual({ kind: "none" });
    expect(asked).toBe(false);

    expect(
      await loadRecoveryStartup(
        port({ probe: () => Promise.reject(new Error("no folder")) }),
      ),
    ).toEqual({ kind: "none" });
  });
});

describe("restoring", () => {
  function files() {
    const calls: string[] = [];
    const port: Pick<ProjectFilePort, "clearPath"> = {
      clearPath: () => {
        calls.push("clearPath");
        return Promise.resolve();
      },
    };
    return { port, calls };
  }

  it("opens the recovered document as an unsaved document with no path", async () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    const { port: projectFiles, calls } = files();

    await restoreRecovery(offer, store, projectFiles);

    expect(calls).toEqual(["clearPath"]);
    expect(store.getState().schematic).toEqual(edited);
    // It has changes to save: a plain save is a Save As and the original project is not replaced.
    expect(store.getState().isDirty).toBe(true);
  });

  it("starts a fresh history for the restored document", async () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    store.moveComponent(R1, { x: 4, y: 3 });

    await restoreRecovery(offer, store, files().port);

    expect(store.getState().canUndo).toBe(false);
    expect(store.getState().history.past).toEqual([]);
  });

  it("is marked saved by the first explicit save, and not before", async () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    await restoreRecovery(offer, store, files().port);

    store.markSaved();

    expect(store.getState().isDirty).toBe(false);
  });

  it("keeps the old document when the path cannot be cleared", async () => {
    const store = new SchematicDocumentStore(dividerSchematic);
    const broken: Pick<ProjectFilePort, "clearPath"> = {
      clearPath: () => Promise.reject(new Error("session unavailable")),
    };

    await expect(restoreRecovery(offer, store, broken)).rejects.toThrow(
      "session unavailable",
    );

    expect(store.getState().schematic).toEqual(dividerSchematic);
  });
});

describe("a replaced document", () => {
  it("is clean as before unless it is restored as unsaved", () => {
    const store = new SchematicDocumentStore(dividerSchematic);

    store.replaceDocument(edited);
    expect(store.getState().isDirty).toBe(false);

    store.replaceDocument(edited, { unsaved: true });
    expect(store.getState().isDirty).toBe(true);
  });

  it("counts an empty restored document as having nothing to save", () => {
    const store = new SchematicDocumentStore(dividerSchematic);

    store.replaceDocument(
      { ...dividerSchematic, components: [], wires: [], referenceMarkers: [] },
      { unsaved: true },
    );

    expect(store.getState().isDirty).toBe(false);
  });
});
