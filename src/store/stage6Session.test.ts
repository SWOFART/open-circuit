import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { DeviceState } from "../generated/DeviceState";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { SetOperatorStateRequest } from "../generated/SetOperatorStateRequest";
import type { SetOperatorStateResult } from "../generated/SetOperatorStateResult";
import { createElement } from "react";
import { applyEditorCommand } from "../ui/editorCommands";
import { DevicePanel } from "../ui/DevicePanel";
import { operatorPosition } from "../ui/operatorPosition";
import { SchematicDocumentStore } from "./document";
import { EngineUnavailableError } from "./engineAdapter";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { blankSchematic } from "./projectWorkflow";
import { SimulationController } from "./simulation";
import { layoutFor } from "./stage4TestSupport";

const catalog = catalogFixture as ComponentCatalog;
const RUN = "00000000-0000-4000-8000-0000000000a0";

function drawnDocument() {
  let next = 0x41;
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`,
  );
  const place = (typeId: "switch_spst" | "push_button_no" | "resistor") => {
    applyEditorCommand(
      store,
      {
        kind: "placeComponent",
        typeId,
        position: { x: 8 * store.getState().schematic.components.length, y: 0 },
        rotationDeg: 0,
      },
      catalog,
    );
    return store.getState().schematic.components.at(-1)!.id;
  };

  return { store, place };
}

const refusal: Diagnostic = {
  code: "InvalidParameter",
  severity: "error",
  localizationKey: "diagnostic.invalidParameter",
  references: [],
};

/**
 * An Engine double that tracks positions the way the Engine does: a compile puts every
 * position back to its initial one, and a request names the current revision.
 */
function engineDouble(
  documentOf: () => CompileRequest["schematic"],
  options: {
    answer?: (
      request: SetOperatorStateRequest,
    ) => SetOperatorStateResult | Promise<SetOperatorStateResult>;
  } = {},
) {
  const closed = new Set<string>();
  const pressed = new Set<string>();
  let revision = "";
  let sequence = 0n;

  const snapshotOf = (): ResultSnapshot => {
    const schematic = documentOf();
    const states: DeviceState[] = [];
    for (const component of [...schematic.components].sort((a, b) =>
      a.id < b.id ? -1 : 1,
    )) {
      if (component.typeId === "switch_spst") {
        states.push({
          kind: "switch",
          componentId: component.id,
          position: closed.has(component.id) ? "closed" : "open",
          conducting: closed.has(component.id),
        });
      } else if (component.typeId === "push_button_no") {
        states.push({
          kind: "pushButton",
          componentId: component.id,
          position: pressed.has(component.id) ? "pressed" : "released",
          conducting: pressed.has(component.id),
        });
      }
    }
    sequence += 1n;
    return {
      metadata: {
        runId: RUN,
        documentRevision: revision,
        layoutVersion: 1,
        simulationStep: 3n,
        simulationTimeSeconds: 0.003,
        frameSequence: sequence,
      },
      values: layoutFor(schematic).slots.map(() => 0),
      states,
    };
  };

  return new PredeterminedEngineAdapter({
    catalog,
    compile: (request) => {
      closed.clear();
      pressed.clear();
      revision = request.documentRevision;
      sequence = 0n;
      return {
        status: "success",
        runId: RUN,
        internalSubsteps: 1,
        layout: layoutFor(request.schematic),
        initialSnapshot: snapshotOf(),
        diagnostics: [],
      };
    },
    setParameter: () => ({ status: "failure", diagnostics: [] }),
    setOperatorState: (request) => {
      if (options.answer) {
        return options.answer(request);
      }
      if (request.expectedDocumentRevision !== revision) {
        return { status: "failure", diagnostics: [refusal] };
      }
      const set =
        request.position === "closed" || request.position === "pressed"
          ? "add"
          : "delete";
      const target =
        request.position === "closed" || request.position === "open"
          ? closed
          : pressed;
      target[set](request.componentId);
      revision = request.documentRevision;
      return {
        status: "success",
        internalSubsteps: 1,
        snapshot: snapshotOf(),
        diagnostics: [],
      };
    },
  });
}

function controllerFor(adapter: PredeterminedEngineAdapter) {
  let next = 0;
  return new SimulationController(adapter, undefined, () => {
    next += 1;
    return `rev-${next}`;
  });
}

function panel(
  controller: SimulationController,
  store: SchematicDocumentStore,
): string {
  return renderToStaticMarkup(
    createElement(DevicePanel, {
      state: controller.getState(),
      schematic: store.getState().schematic,
      labels: store.getState().labels,
      onOperate: () => {},
    }),
  );
}

async function operate(
  controller: SimulationController,
  componentId: string,
  action: "toggle" | "press" | "release",
) {
  const state = controller
    .getState()
    .snapshot?.states.find((entry) => entry.componentId === componentId);
  const position = operatorPosition(action, state);
  if (position) {
    await controller.setOperatorState(componentId, position);
  }
}

describe("operating the devices of a drawn circuit", () => {
  it("shows the position the Engine returns after a toggle, and nothing in the document changes", async () => {
    const { store, place } = drawnDocument();
    const switchId = place("switch_spst");
    place("resistor");
    const controller = controllerFor(
      engineDouble(() => store.getState().schematic),
    );
    await controller.start(store.getState().schematic);
    const before = store.getState();

    expect(panel(controller, store)).toContain('aria-checked="false"');

    await operate(controller, switchId, "toggle");

    expect(panel(controller, store)).toContain('aria-checked="true"');
    expect(panel(controller, store)).toContain(
      "<dt>Conducting</dt><dd>Yes</dd>",
    );
    // The document, its undo history, and its saved state are untouched.
    const after = store.getState();
    expect(after.schematic).toBe(before.schematic);
    expect(after.canUndo).toBe(before.canUndo);
    expect(after.isDirty).toBe(before.isDirty);

    await operate(controller, switchId, "toggle");
    expect(panel(controller, store)).toContain('aria-checked="false"');
  });

  it("holds a push button for as long as it is pressed and lets it go on release", async () => {
    const { store, place } = drawnDocument();
    const buttonId = place("push_button_no");
    const controller = controllerFor(
      engineDouble(() => store.getState().schematic),
    );
    await controller.start(store.getState().schematic);

    await operate(controller, buttonId, "press");
    expect(panel(controller, store)).toContain('aria-pressed="true"');
    expect(panel(controller, store)).toContain(">Pressed</button>");

    await operate(controller, buttonId, "release");
    expect(panel(controller, store)).toContain('aria-pressed="false"');
  });

  it("puts every position back when a document change starts a new run", async () => {
    const { store, place } = drawnDocument();
    const switchId = place("switch_spst");
    const adapter = engineDouble(() => store.getState().schematic);
    const controller = controllerFor(adapter);
    await controller.start(store.getState().schematic);

    await operate(controller, switchId, "toggle");
    expect(panel(controller, store)).toContain('aria-checked="true"');

    // Any edit that compiles is a new run.
    place("resistor");
    await controller.syncDocument(store.getState().schematic);

    expect(adapter.compileRequests).toHaveLength(2);
    expect(panel(controller, store)).toContain('aria-checked="false"');
    expect(panel(controller, store)).toContain(">Open</button>");
  });

  it("keeps the shown position and the document editable when the Engine refuses a request", async () => {
    const { store, place } = drawnDocument();
    const switchId = place("switch_spst");
    const controller = controllerFor(
      engineDouble(() => store.getState().schematic, {
        answer: () => ({ status: "failure", diagnostics: [refusal] }),
      }),
    );
    await controller.start(store.getState().schematic);

    await operate(controller, switchId, "toggle");

    expect(panel(controller, store)).toContain('aria-checked="false"');
    expect(controller.getState().diagnostics).toEqual([refusal]);
    expect(controller.getState().status).toBe("ready");

    // The document still takes edits, and its change reaches the Engine.
    place("resistor");
    expect(store.getState().canUndo).toBe(true);
    await controller.syncDocument(store.getState().schematic);
    expect(controller.getState().status).toBe("ready");
  });

  it("reports an unavailable Engine, keeps the last result, and stays editable", async () => {
    const { store, place } = drawnDocument();
    const switchId = place("switch_spst");
    const controller = controllerFor(
      engineDouble(() => store.getState().schematic, {
        answer: () => {
          throw new EngineUnavailableError("The engine is not running.");
        },
      }),
    );
    await controller.start(store.getState().schematic);

    await operate(controller, switchId, "toggle");

    const state = controller.getState();
    expect(state.error).toContain("not running");
    expect(state.status).toBe("ready");
    expect(panel(controller, store)).toContain('aria-checked="false"');

    place("resistor");
    expect(store.getState().canUndo).toBe(true);
  });
});
