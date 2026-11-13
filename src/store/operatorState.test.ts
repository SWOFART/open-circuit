import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { Schematic } from "../generated/Schematic";
import type { SetOperatorStateRequest } from "../generated/SetOperatorStateRequest";
import type { SetOperatorStateResult } from "../generated/SetOperatorStateResult";
import { EngineUnavailableError } from "./engineAdapter";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { SimulationController } from "./simulation";
import { symbols, type SymbolRegistry } from "../symbols";
import { resistorSymbol } from "../symbols/resistor";

const RUN_ID = "00000000-0000-4000-8000-0000000000a0";
const SOURCE = "00000000-0000-4000-8000-000000000001";
const SWITCH = "00000000-0000-4000-8000-000000000002";
const RESISTOR = "00000000-0000-4000-8000-000000000003";
const BUTTON = "00000000-0000-4000-8000-000000000004";

/** A switch and a normally closed push button in series with a source and a resistor. */
const schematic: Schematic = {
  schemaVersion: 2,
  components: [
    {
      typeId: "dc_voltage_source",
      id: SOURCE,
      position: { x: 0, y: 0 },
      rotationDeg: 0,
      parameters: { voltageVolts: 10, internalResistanceOhms: 0 },
    },
    {
      typeId: "switch_spst",
      id: SWITCH,
      position: { x: 4, y: 0 },
      rotationDeg: 0,
      parameters: { closedResistanceOhms: 0.01 },
    },
    {
      typeId: "push_button_nc",
      id: BUTTON,
      position: { x: 8, y: 0 },
      rotationDeg: 0,
      parameters: { closedResistanceOhms: 0.01 },
    },
    {
      typeId: "resistor",
      id: RESISTOR,
      position: { x: 12, y: 0 },
      rotationDeg: 0,
      parameters: { resistanceOhms: 1000 },
    },
  ],
  wires: [],
  junctions: [],
  referenceMarkers: [],
  rails: [],
};

const layout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 1,
  slots: [
    {
      key: `${RESISTOR}:current`,
      componentId: RESISTOR,
      quantity: "current",
      unit: "amp",
      signConvention: "currentIntoPin1",
      offset: 0,
    },
  ],
};

function snapshot(
  documentRevision: string,
  frameSequence: bigint,
  closed: boolean,
  pressed = false,
  runId = RUN_ID,
): ResultSnapshot {
  const conducting = closed && !pressed;
  return {
    metadata: {
      runId,
      documentRevision,
      layoutVersion: 1,
      simulationStep: 3n,
      simulationTimeSeconds: 0.003,
      frameSequence,
    },
    values: [conducting ? 0.005 : 0],
    // Ordered by component ID: the switch, then the button.
    states: [
      {
        kind: "switch",
        componentId: SWITCH,
        position: closed ? "closed" : "open",
        conducting: closed,
      },
      {
        kind: "pushButton",
        componentId: BUTTON,
        position: pressed ? "pressed" : "released",
        conducting: !pressed,
      },
    ],
  };
}

const refusal: Diagnostic = {
  code: "InvalidParameter",
  severity: "error",
  localizationKey: "diagnostic.invalidParameter",
  references: [{ kind: "component", componentId: SWITCH }],
};

/**
 * A test double that tracks the positions the way the Engine does: a request must name the
 * current revision, adopts the new one, and answers with a snapshot that carries it.
 */
function adapterFor(
  options: {
    answer?: (
      request: SetOperatorStateRequest,
      engine: { closed: boolean; pressed: boolean; revision: string },
    ) => SetOperatorStateResult | Promise<SetOperatorStateResult>;
  } = {},
): PredeterminedEngineAdapter {
  const engine = { closed: false, pressed: false, revision: "", sequence: 0n };
  return new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request: CompileRequest) => {
      engine.closed = false;
      engine.pressed = false;
      engine.revision = request.documentRevision;
      return {
        status: "success",
        runId: RUN_ID,
        internalSubsteps: 1,
        layout,
        initialSnapshot: snapshot(request.documentRevision, 0n, false),
        diagnostics: [],
      };
    },
    setParameter: () => ({ status: "failure", diagnostics: [] }),
    setOperatorState: (request) => {
      if (options.answer) {
        return options.answer(request, engine);
      }
      if (request.expectedDocumentRevision !== engine.revision) {
        return {
          status: "failure",
          diagnostics: [
            {
              code: "StaleDocumentRevision",
              severity: "error",
              localizationKey: "diagnostic.staleDocumentRevision",
              references: [],
            },
          ],
        };
      }
      if (request.componentId === SWITCH) {
        engine.closed = request.position === "closed";
      } else {
        engine.pressed = request.position === "pressed";
      }
      engine.revision = request.documentRevision;
      engine.sequence += 1n;
      return {
        status: "success",
        internalSubsteps: 1,
        snapshot: snapshot(
          engine.revision,
          engine.sequence,
          engine.closed,
          engine.pressed,
        ),
        diagnostics: [],
      };
    },
  });
}

// The symbols of the Stage 6 devices are drawn by the interface issue; the pins are all this
// test needs, and they are the two pins of a resistor.
const registry: SymbolRegistry = {
  ...symbols,
  switch_spst: resistorSymbol,
  push_button_no: resistorSymbol,
  push_button_nc: resistorSymbol,
};

function controllerFor(adapter: PredeterminedEngineAdapter) {
  let next = 0;
  return new SimulationController(adapter, registry, () => {
    next += 1;
    return `rev-${next}`;
  });
}

function switchOf(snapshotValue: ResultSnapshot | null) {
  const state = snapshotValue?.states.find((entry) => entry.kind === "switch");
  return state?.kind === "switch" ? state : undefined;
}

describe("moving a switch or a push button", () => {
  it("sends the run and revision, shows the answer, and does not recompile", async () => {
    const adapter = adapterFor();
    const controller = controllerFor(adapter);
    await controller.start(schematic);

    await controller.setOperatorState(SWITCH, "closed");

    expect(adapter.operatorRequests).toEqual([
      {
        runId: RUN_ID,
        expectedDocumentRevision: "rev-1",
        documentRevision: "rev-2",
        componentId: SWITCH,
        position: "closed",
      },
    ]);
    const state = controller.getState();
    expect(state.run?.documentRevision).toBe("rev-2");
    expect(switchOf(state.snapshot)?.position).toBe("closed");
    expect(state.snapshot?.values).toEqual([0.005]);
    expect(adapter.compileRequests).toHaveLength(1);
  });

  it("sends requests in the order they were made and never merges a press and a release", async () => {
    const adapter = adapterFor();
    const controller = controllerFor(adapter);
    await controller.start(schematic);

    const pressed = controller.setOperatorState(BUTTON, "pressed");
    const released = controller.setOperatorState(BUTTON, "released");
    await Promise.all([pressed, released]);

    expect(
      adapter.operatorRequests.map((request) => [
        request.position,
        request.expectedDocumentRevision,
        request.documentRevision,
      ]),
    ).toEqual([
      ["pressed", "rev-1", "rev-2"],
      ["released", "rev-2", "rev-3"],
    ]);
    expect(controller.getState().run?.documentRevision).toBe("rev-3");
  });

  it("keeps the shown position when the engine refuses a request", async () => {
    const adapter = adapterFor({
      answer: () => ({ status: "failure", diagnostics: [refusal] }),
    });
    const controller = controllerFor(adapter);
    await controller.start(schematic);
    const before = controller.getState().snapshot;

    await controller.setOperatorState(SWITCH, "closed");

    const state = controller.getState();
    expect(state.snapshot).toBe(before);
    expect(switchOf(state.snapshot)?.position).toBe("open");
    expect(state.run?.documentRevision).toBe("rev-1");
    expect(state.diagnostics).toEqual([refusal]);
    // A refusal never recompiles, because that would put every position back.
    expect(adapter.compileRequests).toHaveLength(1);
    expect(state.updating).toBe(false);
  });

  it("does not show an answer that belongs to another run", async () => {
    const adapter = adapterFor({
      answer: (request) => ({
        status: "success",
        internalSubsteps: 1,
        snapshot: snapshot(
          request.documentRevision,
          1n,
          true,
          false,
          "00000000-0000-4000-8000-0000000000bb",
        ),
        diagnostics: [],
      }),
    });
    const controller = controllerFor(adapter);
    await controller.start(schematic);
    const before = controller.getState();

    await controller.setOperatorState(SWITCH, "closed");

    const state = controller.getState();
    expect(state.snapshot).toBe(before.snapshot);
    expect(state.discardedFrames).toBe(before.discardedFrames + 1);
    expect(state.error).not.toBeNull();
    expect(adapter.compileRequests).toHaveLength(1);
  });

  it("does not show an answer whose state table does not match the document", async () => {
    const adapter = adapterFor({
      answer: (request) => ({
        status: "success",
        internalSubsteps: 1,
        snapshot: {
          ...snapshot(request.documentRevision, 1n, true),
          states: [],
        },
        diagnostics: [],
      }),
    });
    const controller = controllerFor(adapter);
    await controller.start(schematic);

    await controller.setOperatorState(SWITCH, "closed");

    expect(switchOf(controller.getState().snapshot)?.position).toBe("open");
    expect(controller.getState().discardedFrames).toBe(1);
  });

  it("settles a lost answer from the next request without a compile", async () => {
    let calls = 0;
    const adapter = adapterFor({
      answer: (request, engine) => {
        calls += 1;
        if (calls === 1) {
          // The engine applied the first request, but its answer is lost.
          engine.closed = true;
          engine.revision = request.documentRevision;
          throw new EngineUnavailableError("The answer was lost.");
        }
        if (request.expectedDocumentRevision !== engine.revision) {
          return {
            status: "failure",
            diagnostics: [
              {
                code: "StaleDocumentRevision",
                severity: "error",
                localizationKey: "diagnostic.staleDocumentRevision",
                references: [],
              },
            ],
          };
        }
        engine.pressed = request.position === "pressed";
        engine.revision = request.documentRevision;
        return {
          status: "success",
          internalSubsteps: 1,
          snapshot: snapshot(
            engine.revision,
            5n,
            engine.closed,
            engine.pressed,
          ),
          diagnostics: [],
        };
      },
    });
    const controller = controllerFor(adapter);
    await controller.start(schematic);

    await controller.setOperatorState(SWITCH, "closed");
    expect(controller.getState().error).toContain("lost");
    expect(controller.getState().snapshot).not.toBeNull();

    await controller.setOperatorState(BUTTON, "pressed");

    // The second request first named the old revision, was told it is stale, and was sent
    // again with the revision the lost request would have adopted.
    expect(
      adapter.operatorRequests.map(
        (request) => request.expectedDocumentRevision,
      ),
    ).toEqual(["rev-1", "rev-1", "rev-2"]);
    const state = controller.getState();
    expect(state.error).toBeNull();
    expect(state.run?.documentRevision).toBe("rev-3");
    expect(switchOf(state.snapshot)?.position).toBe("closed");
    expect(adapter.compileRequests).toHaveLength(1);
  });

  it("settles a lost answer from the next live frame", async () => {
    const adapter = adapterFor({
      answer: () => {
        throw new EngineUnavailableError("The answer was lost.");
      },
    });
    const controller = controllerFor(adapter);
    await controller.start(schematic);

    await controller.setOperatorState(SWITCH, "closed");
    expect(controller.getState().run?.documentRevision).toBe("rev-1");

    // The engine did apply it: its frames carry the revision the request would have adopted.
    adapter.emitFrame(snapshot("rev-2", 1n, true));

    const state = controller.getState();
    expect(state.run?.documentRevision).toBe("rev-2");
    expect(switchOf(state.snapshot)?.position).toBe("closed");

    // A frame of the revision before it is stale from then on.
    adapter.emitFrame(snapshot("rev-1", 2n, false));
    expect(switchOf(controller.getState().snapshot)?.position).toBe("closed");
  });

  it("accepts frames that carry the new state kinds and rejects a table that is incomplete", async () => {
    const adapter = adapterFor();
    const controller = controllerFor(adapter);
    await controller.start(schematic);

    adapter.emitFrame(snapshot("rev-1", 1n, true, true));
    expect(controller.getState().snapshot?.metadata.frameSequence).toBe(1n);

    adapter.emitFrame({ ...snapshot("rev-1", 2n, true), states: [] });
    expect(controller.getState().snapshot?.metadata.frameSequence).toBe(1n);
    expect(controller.getState().discardedFrames).toBe(1);

    // A position the contract does not define for the kind is refused too.
    const bad = snapshot("rev-1", 3n, true);
    adapter.emitFrame({
      ...bad,
      states: [
        { ...bad.states[0]!, position: "pressed" },
        bad.states[1]!,
      ] as unknown as ResultSnapshot["states"],
    });
    expect(controller.getState().snapshot?.metadata.frameSequence).toBe(1n);
  });

  it("drops requests that were queued for a run a compile replaced", async () => {
    const adapter = adapterFor();
    const controller = controllerFor(adapter);
    await controller.start(schematic);

    const first = controller.setOperatorState(SWITCH, "closed");
    const edited: Schematic = {
      ...schematic,
      components: schematic.components.map((component) =>
        component.typeId === "resistor"
          ? { ...component, parameters: { resistanceOhms: 2000 } }
          : component,
      ),
    };
    // A change that is not a lone parameter edit recompiles: the run is replaced and the
    // positions go back to their initial ones.
    const second = controller.syncDocument({
      ...edited,
      wires: [
        {
          id: "00000000-0000-4000-8000-0000000000c1",
          points: [
            { x: 0, y: 0 },
            { x: 2, y: 0 },
          ],
        },
      ],
    });
    await Promise.all([first, second]);

    expect(adapter.compileRequests).toHaveLength(2);
    expect(switchOf(controller.getState().snapshot)?.position).toBe("open");
    expect(controller.getState().run?.documentRevision).toBe("rev-3");
  });
});
