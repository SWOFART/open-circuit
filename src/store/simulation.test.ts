import { afterEach, describe, expect, it, vi } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import expectedFixture from "../../tests/golden/stage-1-divider.expected.json";
import geometryFixture from "../../tests/golden/stage-1-divider.geometry.json";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { Schematic } from "../generated/Schematic";
import type { SetParameterRequest } from "../generated/SetParameterRequest";
import type { SetParameterResult } from "../generated/SetParameterResult";
import {
  EngineUnavailableError,
  TauriEngineAdapter,
  createEngineAdapter,
  type EngineAdapter,
} from "./engineAdapter";
import { dividerSchematic } from "./fixtures";
import {
  PredeterminedEngineAdapter,
  PredeterminedResponseMissingError,
  createDividerFixtureAdapter,
} from "./mockEngineAdapter";
import {
  SimulationController,
  isSnapshotCurrent,
  readSlot,
  type ActiveRun,
} from "./simulation";
import { symbols } from "../symbols";

const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const RUN_ID = "00000000-0000-4000-8000-0000000000a0";

function revisions(): () => string {
  let next = 0;
  return () => {
    next += 1;
    return `test-revision-${next}`;
  };
}

function controllerFor(adapter: EngineAdapter): SimulationController {
  return new SimulationController(adapter, symbols, revisions());
}

const rejection: Diagnostic = {
  code: "InvalidParameter",
  severity: "error",
  localizationKey: "diagnostic.invalidParameter",
  references: [{ kind: "component", componentId: R1 }],
};

const noFrames = () => Promise.resolve(() => {});

const singleSlotLayout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 1,
  slots: [
    {
      key: "r1.voltage",
      componentId: R1,
      quantity: "voltage",
      unit: "volt",
      signConvention: "voltagePin1MinusPin2",
      offset: 0,
    },
  ],
};

afterEach(() => {
  vi.unstubAllEnvs();
});

describe("compiling the divider through the adapter", () => {
  it("sends the canonical schematic and pin geometry and shows the result", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    const state = controller.getState();

    expect(state.status).toBe("ready");
    expect(state.catalog).toEqual(catalogFixture);
    expect(adapter.compileRequests).toHaveLength(1);

    const request = adapter.compileRequests[0];
    expect(request?.schematic).toEqual(dividerSchematic);
    expect(request?.geometry).toEqual(geometryFixture);
    expect(request?.documentRevision).toBe("test-revision-1");
    expect(readSlot(state.run, state.snapshot, R2, "voltage")).toBe(
      expectedFixture.equalResistors.resistor2VoltageVolts,
    );
  });

  it("does not require every quantity to be present in the layout", async () => {
    const controller = controllerFor(createDividerFixtureAdapter());

    await controller.start(dividerSchematic);
    const { run, snapshot } = controller.getState();

    expect(readSlot(run, snapshot, R2, "current")).toBeUndefined();
  });
});

describe("changing a resistance", () => {
  it("forwards the value as a plain SI number with the run and revision", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    await controller.setParameter(R1, "resistanceOhms", 3000);

    const expected: SetParameterRequest = {
      runId: RUN_ID,
      expectedDocumentRevision: "test-revision-1",
      documentRevision: "test-revision-2",
      componentId: R1,
      parameterId: "resistanceOhms",
      value: 3000,
    };
    expect(adapter.parameterRequests).toEqual([expected]);
    expect(typeof adapter.parameterRequests[0]?.value).toBe("number");
  });

  it("expects the revision the engine last accepted, then adopts the new one", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    await controller.setParameter(R1, "resistanceOhms", 3000);
    await controller.setParameter(R1, "resistanceOhms", 1000);

    const [first, second] = adapter.parameterRequests;
    expect(first?.expectedDocumentRevision).toBe("test-revision-1");
    expect(second?.expectedDocumentRevision).toBe(first?.documentRevision);
    expect(second?.documentRevision).not.toBe(second?.expectedDocumentRevision);
  });

  it("expects the unchanged revision again after the engine rejects an update", async () => {
    const base = createDividerFixtureAdapter();
    const adapter = new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: (request) => base.compile(request),
      setParameter: () => ({ status: "failure", diagnostics: [rejection] }),
    });
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    await controller.setParameter(R1, "resistanceOhms", 3000);
    await controller.setParameter(R1, "resistanceOhms", 4000);

    const [first, second] = adapter.parameterRequests;
    expect(first?.expectedDocumentRevision).toBe("test-revision-1");
    expect(second?.expectedDocumentRevision).toBe("test-revision-1");
  });

  it("shows the new result and restores the original when changed back", async () => {
    const controller = controllerFor(createDividerFixtureAdapter());

    await controller.start(dividerSchematic);
    await controller.setParameter(R1, "resistanceOhms", 3000);

    let state = controller.getState();
    expect(readSlot(state.run, state.snapshot, R2, "voltage")).toBe(
      expectedFixture.resistor1At3000Ohms.resistor2VoltageVolts,
    );
    expect(readSlot(state.run, state.snapshot, R1, "voltage")).toBe(
      expectedFixture.resistor1At3000Ohms.resistor1VoltageVolts,
    );

    await controller.setParameter(R1, "resistanceOhms", 1000);

    state = controller.getState();
    expect(readSlot(state.run, state.snapshot, R2, "voltage")).toBe(
      expectedFixture.equalResistors.resistor2VoltageVolts,
    );
    expect(state.status).toBe("ready");
  });

  it("keeps schematic documents outside simulation state", async () => {
    const controller = controllerFor(createDividerFixtureAdapter());

    await controller.start(dividerSchematic);
    await controller.setParameter(R1, "resistanceOhms", 3000);

    expect(controller.getState()).not.toHaveProperty("schematic");
    expect(dividerSchematic.components[1]).toMatchObject({
      parameters: { resistanceOhms: 1000 },
    });
  });

  it("reports an unknown value as unavailable data, never as a reading", async () => {
    const controller = controllerFor(createDividerFixtureAdapter());

    await controller.start(dividerSchematic);
    const before = controller.getState().snapshot;
    await controller.setParameter(R1, "resistanceOhms", 4700);
    const state = controller.getState();

    expect(state.error).toBe(
      new PredeterminedResponseMissingError(4700, 1000).message,
    );
    expect(state.snapshot).toBe(before);
    expect(state.status).toBe("ready");
  });
});

describe("diagnostics", () => {
  function snapshotFor(
    request: SetParameterRequest,
    sequence: bigint,
  ): ResultSnapshot {
    return {
      metadata: {
        runId: request.runId,
        documentRevision: request.documentRevision,
        layoutVersion: 1,
        simulationStep: 0n,
        simulationTimeSeconds: 0,
        frameSequence: sequence,
      },
      values: [1, 2],
      states: [],
    };
  }

  const layout: ResultLayout = {
    layoutVersion: 1,
    valueCount: 2,
    slots: [
      {
        key: "r1.voltage",
        componentId: R1,
        quantity: "voltage",
        unit: "volt",
        signConvention: "voltagePin1MinusPin2",
        offset: 0,
      },
      {
        key: "r2.voltage",
        componentId: R2,
        quantity: "voltage",
        unit: "volt",
        signConvention: "voltagePin1MinusPin2",
        offset: 1,
      },
    ],
  };

  function adapterWith(
    setParameter: (request: SetParameterRequest) => SetParameterResult,
  ): PredeterminedEngineAdapter {
    return new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: (request) => ({
        status: "success",
        internalSubsteps: 1,
        runId: RUN_ID,
        layout,
        initialSnapshot: {
          metadata: {
            runId: RUN_ID,
            documentRevision: request.documentRevision,
            layoutVersion: 1,
            simulationStep: 0n,
            simulationTimeSeconds: 0,
            frameSequence: 0n,
          },
          values: [1, 2],
          states: [],
        },
        diagnostics: [],
      }),
      setParameter,
    });
  }

  it("shows the engine's rejection and keeps the previous valid result", async () => {
    const controller = controllerFor(
      adapterWith(() => ({ status: "failure", diagnostics: [rejection] })),
    );

    await controller.start(dividerSchematic);
    const before = controller.getState();
    await controller.setParameter(R1, "resistanceOhms", 5);
    const after = controller.getState();

    expect(after.diagnostics).toEqual([rejection]);
    expect(after.snapshot).toBe(before.snapshot);
    expect(after).not.toHaveProperty("schematic");
    expect(after.run).toEqual(before.run);
    expect(after.status).toBe("ready");
    expect(after.updating).toBe(false);
  });

  it("keeps warnings that accompany a valid result", async () => {
    const warning: Diagnostic = {
      code: "NoGroundReference",
      severity: "warning",
      localizationKey: "diagnostic.noGroundReference",
      references: [],
    };
    const controller = controllerFor(
      adapterWith((request) => ({
        status: "success",
        internalSubsteps: 1,
        snapshot: snapshotFor(request, 1n),
        diagnostics: [warning],
      })),
    );

    await controller.start(dividerSchematic);
    await controller.setParameter(R1, "resistanceOhms", 5);

    expect(controller.getState().diagnostics).toEqual([warning]);
    expect(controller.getState().status).toBe("ready");
  });

  it("reports a compile failure without a usable result", async () => {
    const adapter = new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: () => ({ status: "failure", diagnostics: [rejection] }),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    const state = controller.getState();

    expect(state.status).toBe("failed");
    expect(state.diagnostics).toEqual([rejection]);
    expect(state.snapshot).toBeNull();
    expect(state.run).toBeNull();
  });
});

describe("unavailable backend", () => {
  it("never produces a reading when the production adapter has no bridge", async () => {
    const controller = controllerFor(new TauriEngineAdapter());

    await controller.start(dividerSchematic);
    const state = controller.getState();

    expect(state.status).toBe("unavailable");
    expect(state.snapshot).toBeNull();
    expect(state.error).toMatch(/desktop app/);
  });

  it("reports a compile transport error", async () => {
    const adapter: EngineAdapter = {
      getCatalog: () => Promise.resolve(catalogFixture as ComponentCatalog),
      startFrames: noFrames,
      compile: () =>
        Promise.reject(new EngineUnavailableError("The bridge is down.")),
      setParameter: () => Promise.reject(new Error("unused")),
      readTraces: () => Promise.reject(new Error("unused")),
    };
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);

    expect(controller.getState()).toMatchObject({
      status: "unavailable",
      error: "The bridge is down.",
    });
  });

  it("reports an incoherent first result instead of showing it", async () => {
    const adapter = new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: (request) => ({
        status: "success",
        internalSubsteps: 1,
        runId: RUN_ID,
        layout: singleSlotLayout,
        initialSnapshot: {
          metadata: {
            runId: RUN_ID,
            documentRevision: request.documentRevision,
            layoutVersion: 1,
            simulationStep: 0n,
            simulationTimeSeconds: 0,
            frameSequence: 0n,
          },
          values: [Number.NaN],
          states: [],
        },
        diagnostics: [],
      }),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    const state = controller.getState();

    expect(state.status).toBe("unavailable");
    expect(state.snapshot).toBeNull();
    expect(state.discardedFrames).toBe(1);
  });
});

describe("stale results", () => {
  const run: ActiveRun = {
    runId: RUN_ID,
    layout: { layoutVersion: 2, valueCount: 2, slots: [] },
    documentRevision: "rev-5",
  };

  function frame(overrides: {
    runId?: string;
    documentRevision?: string;
    layoutVersion?: number;
    frameSequence?: bigint;
    values?: readonly number[];
    simulationTimeSeconds?: number;
  }): ResultSnapshot {
    return {
      metadata: {
        runId: overrides.runId ?? RUN_ID,
        documentRevision: overrides.documentRevision ?? "rev-5",
        layoutVersion: overrides.layoutVersion ?? 2,
        simulationStep: 0n,
        simulationTimeSeconds: overrides.simulationTimeSeconds ?? 0,
        frameSequence: overrides.frameSequence ?? 4n,
      },
      values: [...(overrides.values ?? [1, 2])],
      states: [],
    };
  }

  const last = frame({ frameSequence: 3n });

  it("accepts a matching frame that is newer than the last applied one", () => {
    expect(isSnapshotCurrent(run, last, frame({}))).toBe(true);
  });

  it("accepts the first frame of a run, whatever its sequence", () => {
    expect(isSnapshotCurrent(run, null, frame({ frameSequence: 0n }))).toBe(
      true,
    );
  });

  it("requires exactly the relay states named by the active schematic", () => {
    const relayId = "00000000-0000-4000-8000-000000000010";
    const schematic: Schematic = {
      ...dividerSchematic,
      components: [
        ...dividerSchematic.components,
        {
          typeId: "relay_spdt",
          id: relayId,
          position: { x: 10, y: 0 },
          rotationDeg: 0,
          parameters: {
            coilResistanceOhms: 100,
            coilInductanceHenries: 0.01,
            pickupCurrentAmps: 0.04,
            dropoutCurrentAmps: 0.02,
            closedContactResistanceOhms: 0.1,
          },
        },
      ],
    };
    const relayFrame: ResultSnapshot = {
      ...frame({}),
      states: [{ componentId: relayId, kind: "relay", state: "released" }],
    };

    expect(isSnapshotCurrent(run, null, frame({}), schematic)).toBe(false);
    expect(isSnapshotCurrent(run, null, relayFrame, schematic)).toBe(true);
    expect(
      isSnapshotCurrent(
        run,
        null,
        {
          ...relayFrame,
          states: [
            {
              componentId: relayId,
              kind: "relay",
              state: "released",
              lastTransitionTimeSeconds: 0.001,
            },
          ],
        },
        schematic,
      ),
    ).toBe(false);
  });

  it.each([
    ["a different run ID", { runId: "00000000-0000-4000-8000-0000000000b1" }],
    ["a different document revision", { documentRevision: "rev-4" }],
    ["a different layout version", { layoutVersion: 1 }],
    ["the same sequence as the last frame", { frameSequence: 3n }],
    ["an older sequence", { frameSequence: 2n }],
    ["too few values", { values: [1] }],
    ["too many values", { values: [1, 2, 3] }],
    ["a NaN value", { values: [1, Number.NaN] }],
    ["an infinite value", { values: [Number.POSITIVE_INFINITY, 2] }],
    ["a non-finite simulation time", { simulationTimeSeconds: Number.NaN }],
  ] as const)("rejects a frame with %s", (_description, overrides) => {
    expect(isSnapshotCurrent(run, last, frame(overrides))).toBe(false);
  });

  it("rejects every frame when there is no active run", () => {
    expect(isSnapshotCurrent(null, null, frame({}))).toBe(false);
  });

  it("compares revisions by equality, not by order", () => {
    expect(
      isSnapshotCurrent(run, last, frame({ documentRevision: "rev-9" })),
    ).toBe(false);
  });

  it("discards a snapshot echoing an old revision and keeps the current result", async () => {
    const stale = new PredeterminedEngineAdapter({
      catalog: catalogFixture as ComponentCatalog,
      compile: (request) => ({
        status: "success",
        internalSubsteps: 1,
        runId: RUN_ID,
        layout: { layoutVersion: 1, valueCount: 0, slots: [] },
        initialSnapshot: {
          metadata: {
            runId: RUN_ID,
            documentRevision: request.documentRevision,
            layoutVersion: 1,
            simulationStep: 0n,
            simulationTimeSeconds: 0,
            frameSequence: 0n,
          },
          values: [],
          states: [],
        },
        diagnostics: [],
      }),
      setParameter: () => ({
        status: "success",
        internalSubsteps: 1,
        snapshot: {
          metadata: {
            runId: RUN_ID,
            documentRevision: "an-older-revision",
            layoutVersion: 1,
            simulationStep: 0n,
            simulationTimeSeconds: 0,
            frameSequence: 1n,
          },
          values: [],
          states: [],
        },
        diagnostics: [],
      }),
    });
    const controller = controllerFor(stale);

    await controller.start(dividerSchematic);
    const before = controller.getState();
    await controller.setParameter(R1, "resistanceOhms", 2000);
    const after = controller.getState();

    expect(after.discardedFrames).toBe(1);
    expect(after.error).toMatch(/unconfirmed/);
    expect(after.snapshot).toBe(before.snapshot);
    expect(after).not.toHaveProperty("schematic");
    expect(after.run).toEqual(before.run);
  });
});

describe("rapid edits", () => {
  interface Pending {
    readonly request: SetParameterRequest;
    readonly resolve: (result: SetParameterResult) => void;
  }

  function slowAdapter(pending: Pending[]): EngineAdapter {
    const layout = singleSlotLayout;
    let sequence = 0n;

    function snapshot(documentRevision: string, value: number): ResultSnapshot {
      const frameSequence = sequence;
      sequence += 1n;
      return {
        metadata: {
          runId: RUN_ID,
          documentRevision,
          layoutVersion: 1,
          simulationStep: 0n,
          simulationTimeSeconds: 0,
          frameSequence,
        },
        values: [value],
        states: [],
      };
    }

    return {
      getCatalog: () => Promise.resolve(catalogFixture as ComponentCatalog),
      startFrames: noFrames,
      compile: (request) =>
        Promise.resolve({
          status: "success",
          internalSubsteps: 1,
          runId: RUN_ID,
          layout,
          initialSnapshot: snapshot(request.documentRevision, 0),
          diagnostics: [],
        }),
      setParameter: (request) =>
        new Promise<SetParameterResult>((resolve) => {
          pending.push({
            request,
            resolve: (result) => resolve(result),
          });
        }),
      readTraces: () => Promise.reject(new Error("unused")),
    };
  }

  let acceptedSequence = 100n;

  function nextSequence(): bigint {
    acceptedSequence += 1n;
    return acceptedSequence;
  }

  function acceptAs(request: SetParameterRequest): SetParameterResult {
    return {
      status: "success",
      internalSubsteps: 1,
      snapshot: {
        metadata: {
          runId: request.runId,
          documentRevision: request.documentRevision,
          layoutVersion: 1,
          simulationStep: 0n,
          simulationTimeSeconds: 0,
          frameSequence: nextSequence(),
        },
        values: [request.value],
        states: [],
      },
      diagnostics: [],
    };
  }

  it("sends one edit at a time and applies only the newest value", async () => {
    const pending: Pending[] = [];
    const controller = controllerFor(slowAdapter(pending));

    await controller.start(dividerSchematic);

    const first = controller.setParameter(R1, "resistanceOhms", 2000);
    void controller.setParameter(R1, "resistanceOhms", 3000);
    const last = controller.setParameter(R1, "resistanceOhms", 4000);

    // Only the first edit has been sent; the others wait and coalesce.
    expect(pending).toHaveLength(1);
    expect(pending[0]?.request.value).toBe(2000);
    expect(controller.getState().updating).toBe(true);

    pending[0]?.resolve(acceptAs(pending[0].request));
    await vi.waitFor(() => expect(pending).toHaveLength(2));

    // The intermediate 3000 was superseded and never reached the engine.
    expect(pending[1]?.request.value).toBe(4000);

    pending[1]?.resolve(acceptAs(pending[1].request));
    await Promise.all([first, last]);

    const state = controller.getState();
    expect(pending).toHaveLength(2);
    expect(state.snapshot?.values).toEqual([4000]);
    expect(state.updating).toBe(false);
    expect(state).not.toHaveProperty("schematic");
  });

  it("never lets an older response overwrite a newer result", async () => {
    const pending: Pending[] = [];
    const controller = controllerFor(slowAdapter(pending));

    await controller.start(dividerSchematic);

    void controller.setParameter(R1, "resistanceOhms", 2000);
    pending[0]?.resolve(acceptAs(pending[0].request));
    await vi.waitFor(() =>
      expect(controller.getState().snapshot?.values).toEqual([2000]),
    );

    const done = controller.setParameter(R1, "resistanceOhms", 3000);
    expect(pending).toHaveLength(2);

    // A late duplicate of the first answer carries a revision that is no longer active.
    const late = acceptAs(pending[0]?.request as SetParameterRequest);
    pending[1]?.resolve(late);
    await done;

    const state = controller.getState();
    expect(state.snapshot?.values).toEqual([2000]);
    expect(state.discardedFrames).toBe(1);
  });

  it("keeps edits of different parameters and sends them in request order", async () => {
    const pending: Pending[] = [];
    const controller = controllerFor(slowAdapter(pending));

    await controller.start(dividerSchematic);

    const done = controller.setParameter(R1, "resistanceOhms", 2000);
    void controller.setParameter(R2, "resistanceOhms", 5000);
    void controller.setParameter(R1, "resistanceOhms", 3000);

    const answered = async (index: number) => {
      await vi.waitFor(() => expect(pending).toHaveLength(index + 1));
      pending[index]?.resolve(acceptAs(pending[index].request));
    };
    await answered(0);
    await answered(1);
    await answered(2);
    await done;

    expect(
      pending.map((entry) => [entry.request.componentId, entry.request.value]),
    ).toEqual([
      [R1, 2000],
      [R2, 5000],
      [R1, 3000],
    ]);

    expect(controller.getState()).not.toHaveProperty("schematic");
  });
});

describe("malformed engine answers", () => {
  const oneSlotSuccess = (request: {
    documentRevision: string;
  }): CompileResult => ({
    status: "success",
    internalSubsteps: 1,
    runId: RUN_ID,
    layout: singleSlotLayout,
    initialSnapshot: {
      metadata: {
        runId: RUN_ID,
        documentRevision: request.documentRevision,
        layoutVersion: 1,
        simulationStep: 0n,
        simulationTimeSeconds: 0,
        frameSequence: 0n,
      },
      values: [1],
      states: [],
    },
    diagnostics: [],
  });

  const catalog = catalogFixture as ComponentCatalog;

  it.each([
    ["fewer slots than values", { layoutVersion: 1, valueCount: 1, slots: [] }],
    [
      "a slot offset outside the value range",
      {
        ...singleSlotLayout,
        slots: singleSlotLayout.slots.map((slot) => ({ ...slot, offset: 3 })),
      },
    ],
    [
      "two slots sharing an offset",
      {
        layoutVersion: 1,
        valueCount: 2,
        slots: [
          ...singleSlotLayout.slots,
          ...singleSlotLayout.slots.map((slot) => ({ ...slot, key: "dup" })),
        ],
      },
    ],
  ] as const)("refuses a compiled layout with %s", async (_name, layout) => {
    const adapter = new PredeterminedEngineAdapter({
      catalog,
      compile: (request) => ({
        ...oneSlotSuccess(request),
        layout: layout as ResultLayout,
      }),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);

    expect(controller.getState().status).toBe("unavailable");
    expect(controller.getState().snapshot).toBeNull();
  });

  it("reports an unreadable compile result instead of throwing", async () => {
    const adapter = new PredeterminedEngineAdapter({
      catalog,
      compile: () =>
        ({
          status: "success",
          internalSubsteps: 1,
          runId: RUN_ID,
          diagnostics: [],
        }) as never,
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    const controller = controllerFor(adapter);

    await expect(controller.start(dividerSchematic)).resolves.toBeUndefined();

    expect(controller.getState()).toMatchObject({ status: "unavailable" });
    expect(controller.getState().error).toMatch(/unreadable result/);
  });

  it("reports an unreadable update result and keeps the last valid one", async () => {
    const adapter = new PredeterminedEngineAdapter({
      catalog,
      compile: oneSlotSuccess,
      setParameter: () =>
        ({ status: "success", internalSubsteps: 1, diagnostics: [] }) as never,
    });
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    const before = controller.getState();

    await expect(
      controller.setParameter(R1, "resistanceOhms", 2000),
    ).resolves.toBeUndefined();
    const after = controller.getState();

    expect(after.error).toMatch(/unreadable result/);
    expect(after.updating).toBe(false);
    expect(after.snapshot).toBe(before.snapshot);
    expect(after).not.toHaveProperty("schematic");
    expect(after.run).toEqual(before.run);
  });
});

describe("lifecycle", () => {
  it("notifies subscribers and stops after unsubscribe", async () => {
    const controller = controllerFor(createDividerFixtureAdapter());
    const listener = vi.fn();
    const unsubscribe = controller.subscribe(listener);

    await controller.start(dividerSchematic);
    expect(listener).toHaveBeenCalled();

    const calls = listener.mock.calls.length;
    unsubscribe();
    await controller.setParameter(R1, "resistanceOhms", 3000);

    expect(listener).toHaveBeenCalledTimes(calls);
  });

  it("ignores a response that arrives after dispose", async () => {
    let resolveCompile: ((value: never) => void) | undefined;
    const adapter: EngineAdapter = {
      getCatalog: () => Promise.resolve(catalogFixture as ComponentCatalog),
      startFrames: noFrames,
      compile: () =>
        new Promise((resolve) => {
          resolveCompile = resolve;
        }),
      setParameter: () => Promise.reject(new Error("unused")),
      readTraces: () => Promise.reject(new Error("unused")),
    };
    const controller = controllerFor(adapter);
    const listener = vi.fn();
    controller.subscribe(listener);

    const started = controller.start(dividerSchematic);
    await vi.waitFor(() => expect(resolveCompile).toBeDefined());
    controller.dispose();
    resolveCompile?.({
      status: "failure",
      diagnostics: [rejection],
    } as never);
    await started;

    expect(controller.getState().status).toBe("loading");
    expect(controller.getState().diagnostics).toEqual([]);
  });

  it("starts only once", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await Promise.all([
      controller.start(dividerSchematic),
      controller.start(dividerSchematic),
    ]);

    expect(adapter.compileRequests).toHaveLength(1);
  });
});

describe("choosing the adapter", () => {
  it("uses the production adapter unless a development mock is requested", async () => {
    vi.stubEnv("DEV", true);
    vi.stubEnv("VITE_ENGINE_ADAPTER", "");

    expect(await createEngineAdapter()).toBeInstanceOf(TauriEngineAdapter);
  });

  it("uses the fixture mock only in development when explicitly requested", async () => {
    vi.stubEnv("DEV", true);
    vi.stubEnv("VITE_ENGINE_ADAPTER", "mock");

    expect(await createEngineAdapter()).toBeInstanceOf(
      PredeterminedEngineAdapter,
    );
  });

  it("never uses the mock in a production build", async () => {
    vi.stubEnv("DEV", false);
    vi.stubEnv("VITE_ENGINE_ADAPTER", "mock");

    expect(await createEngineAdapter()).toBeInstanceOf(TauriEngineAdapter);
  });
});
