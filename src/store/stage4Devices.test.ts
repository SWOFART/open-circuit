import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import type { Schematic } from "../generated/Schematic";
import type { StableId } from "../generated/StableId";
import { symbols } from "../symbols";
import { applyEditorCommand, liveParameterUpdate } from "../ui/editorCommands";
import { describeDiagnostic } from "../ui/messages";
import { SchematicDocumentStore } from "./document";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import type { ProjectFilePort } from "./projectFiles";
import { blankSchematic, ProjectWorkflow } from "./projectWorkflow";
import { SimulationController } from "./simulation";
import { layoutFor, valuesFor } from "./stage4TestSupport";

const catalog = catalogFixture as ComponentCatalog;
const STAGE_4: readonly ComponentTypeId[] = [
  "relay_spdt",
  "fuse",
  "pmdc_motor",
  "voltmeter",
  "ammeter",
];

function ids(): () => StableId {
  let next = 0x41;
  return () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`;
}

function newStore(): SchematicDocumentStore {
  return new SchematicDocumentStore(blankSchematic, ids());
}

function place(
  store: SchematicDocumentStore,
  typeId: ComponentTypeId,
  x = 0,
): StableId {
  applyEditorCommand(
    store,
    { kind: "placeComponent", typeId, position: { x, y: 0 }, rotationDeg: 0 },
    catalog,
  );

  return store.getState().schematic.components.at(-1)!.id;
}

function allDevices() {
  const store = newStore();
  const [relay, fuse, motor, voltmeter, ammeter] = STAGE_4.map(
    (typeId, index) => place(store, typeId, index * 8),
  ) as [StableId, StableId, StableId, StableId, StableId];

  return { store, relay, fuse, motor, voltmeter, ammeter };
}

function parametersOf(store: SchematicDocumentStore, id: StableId) {
  return store.getState().schematic.components.find((item) => item.id === id)
    ?.parameters as Record<string, number>;
}

describe("placing the Stage 4 devices", () => {
  it.each(STAGE_4)(
    "places a %s with exactly the catalog's required defaults and no initial condition",
    (typeId) => {
      const store = newStore();
      const id = place(store, typeId);
      const entry = catalog.componentTypes.find(
        (item) => item.typeId === typeId,
      )!;
      const required = entry.parameters.filter(
        (parameter) => !parameter.optional,
      );

      expect(parametersOf(store, id)).toEqual(
        Object.fromEntries(
          required.map((parameter) => [parameter.id, parameter.defaultValue]),
        ),
      );
    },
  );

  it("takes the motor's two optional initial conditions from the catalog, as optional", () => {
    const motor = catalog.componentTypes.find(
      (item) => item.typeId === "pmdc_motor",
    )!;
    const optional = motor.parameters.filter((parameter) => parameter.optional);

    expect(optional.map((parameter) => parameter.id)).toEqual([
      "initialAngularVelocityRadiansPerSecond",
      "initialArmatureCurrentAmps",
    ]);
    expect(optional.every((parameter) => parameter.initialCondition)).toBe(
      true,
    );
  });

  it("names the devices K1, F1, M1, VM1, and AM1, and never reuses a name", () => {
    const { store, relay } = allDevices();

    expect([...store.getState().labels.values()]).toEqual([
      "K1",
      "F1",
      "M1",
      "VM1",
      "AM1",
    ]);

    store.removeComponent(relay);
    const second = place(store, "relay_spdt", 40);

    expect(store.getState().labels.get(second)).toBe("K2");
  });

  it("keeps IDs and names through move, rotate, delete, and undo", () => {
    const { store, relay } = allDevices();
    const before = store.getState().labels.get(relay);

    store.moveComponent(relay, { x: 3, y: 4 });
    store.rotateComponent(relay, 90);
    store.removeComponent(relay);
    store.undo();

    const restored = store
      .getState()
      .schematic.components.find((item) => item.id === relay);

    expect(restored).toMatchObject({
      position: { x: 3, y: 4 },
      rotationDeg: 90,
    });
    expect(store.getState().labels.get(relay)).toBe(before);
  });

  it("has a symbol for every one of them", () => {
    for (const typeId of STAGE_4) {
      expect(symbols[typeId]).toBeDefined();
    }
  });
});

describe("editing the Stage 4 parameters", () => {
  it("edits every parameter of every device within its catalog domain, with undo and redo", () => {
    const { store, relay, fuse, motor, voltmeter, ammeter } = allDevices();
    const edits: [StableId, string, number][] = [
      [relay, "coilResistanceOhms", 120],
      [relay, "coilInductanceHenries", 0.02],
      [relay, "pickupCurrentAmps", 0.05],
      [relay, "dropoutCurrentAmps", 0],
      [relay, "closedContactResistanceOhms", 0.2],
      [fuse, "intactResistanceOhms", 0.02],
      [fuse, "tripI2tAmpereSquaredSeconds", 0.01],
      [motor, "armatureResistanceOhms", 3],
      [motor, "armatureInductanceHenries", 0.2],
      [motor, "motorConstantNewtonMetersPerAmp", 0.2],
      [motor, "rotorInertiaKilogramMetersSquared", 0.02],
      [motor, "viscousFrictionNewtonMeterSecondsPerRadian", 0],
      [motor, "loadTorqueNewtonMeters", -0.05],
      [voltmeter, "inputResistanceOhms", 1e6],
      [ammeter, "burdenResistanceOhms", 1],
    ];

    for (const [id, parameter, value] of edits) {
      const before = parametersOf(store, id)[parameter];

      store.setParameter(id, parameter as never, value, catalog);
      expect(parametersOf(store, id)[parameter]).toBe(value);
      store.undo();
      expect(parametersOf(store, id)[parameter]).toBe(before);
      store.redo();
      expect(parametersOf(store, id)[parameter]).toBe(value);
    }
  });

  it("refuses values outside the domain without a history entry", () => {
    const { store, relay, fuse, motor, ammeter } = allDevices();
    const refused: [StableId, string, number][] = [
      [relay, "pickupCurrentAmps", 0],
      [relay, "coilResistanceOhms", -1],
      [relay, "dropoutCurrentAmps", -0.01],
      [fuse, "tripI2tAmpereSquaredSeconds", 0],
      [motor, "motorConstantNewtonMetersPerAmp", 0],
      [motor, "viscousFrictionNewtonMeterSecondsPerRadian", -0.1],
      [motor, "loadTorqueNewtonMeters", Number.NaN],
      [
        motor,
        "initialAngularVelocityRadiansPerSecond",
        Number.POSITIVE_INFINITY,
      ],
      [ammeter, "burdenResistanceOhms", 0],
    ];
    const undoable = store.getState().canUndo;
    const schematic = store.getState().schematic;

    for (const [id, parameter, value] of refused) {
      expect(() =>
        store.setParameter(id, parameter as never, value, catalog),
      ).toThrow();
    }

    expect(store.getState().schematic).toBe(schematic);
    expect(store.getState().canUndo).toBe(undoable);
  });

  it("leaves a relay whose dropout is not below its pickup to the Engine, and stays editable", () => {
    const { store, relay } = allDevices();

    // The catalog domains accept each value alone; the cross-parameter rule is the
    // Engine's (stage-4-components.md section 2), and it answers with a diagnostic.
    store.setParameter(relay, "dropoutCurrentAmps", 0.05, catalog);

    expect(parametersOf(store, relay)).toMatchObject({
      pickupCurrentAmps: 0.04,
      dropoutCurrentAmps: 0.05,
    });
    store.setParameter(relay, "dropoutCurrentAmps", 0.02, catalog);
    expect(store.getState().canUndo).toBe(true);
  });

  it("sets, clears, undoes, and saves the motor's optional initial conditions", () => {
    const { store, motor } = allDevices();

    store.setParameter(
      motor,
      "initialAngularVelocityRadiansPerSecond",
      -12.5,
      catalog,
    );
    store.setParameter(motor, "initialArmatureCurrentAmps", 0, catalog);
    expect(parametersOf(store, motor)).toMatchObject({
      initialAngularVelocityRadiansPerSecond: -12.5,
      initialArmatureCurrentAmps: 0,
    });

    store.clearParameter(
      motor,
      "initialAngularVelocityRadiansPerSecond",
      catalog,
    );
    expect(
      "initialAngularVelocityRadiansPerSecond" in parametersOf(store, motor),
    ).toBe(false);
    store.undo();
    expect(
      parametersOf(store, motor).initialAngularVelocityRadiansPerSecond,
    ).toBe(-12.5);
    store.redo();
    store.clearParameter(motor, "initialArmatureCurrentAmps", catalog);

    const saved = JSON.stringify(store.getState().schematic);
    expect(saved).not.toContain("initialAngularVelocityRadiansPerSecond");
    expect(saved).not.toContain("initialArmatureCurrentAmps");
    expect(saved).not.toContain("null");
  });

  it("clears only optional parameters", () => {
    const { store, relay } = allDevices();

    expect(() =>
      store.clearParameter(relay, "pickupCurrentAmps", catalog),
    ).toThrow();
  });
});

describe("forwarding Stage 4 parameters to the Engine", () => {
  it("sends a live update for every parameter that is not an initial condition", () => {
    const { store, relay, fuse, motor, voltmeter, ammeter } = allDevices();
    const schematic = store.getState().schematic;
    const expectUpdate = (id: StableId, parameter: string, value: number) =>
      expect(
        liveParameterUpdate(catalog, schematic, id, parameter as never, value),
      ).toEqual({ componentId: id, parameterId: parameter, value });

    expectUpdate(relay, "pickupCurrentAmps", 0.05);
    expectUpdate(relay, "dropoutCurrentAmps", 0.01);
    expectUpdate(fuse, "tripI2tAmpereSquaredSeconds", 0.01);
    expectUpdate(motor, "loadTorqueNewtonMeters", 0.2);
    expectUpdate(voltmeter, "inputResistanceOhms", 1e6);
    expectUpdate(ammeter, "burdenResistanceOhms", 1);
  });

  it("recompiles instead for the motor's initial conditions, as the catalog flags say", () => {
    const { store, motor } = allDevices();
    const schematic = store.getState().schematic;

    for (const parameter of [
      "initialAngularVelocityRadiansPerSecond",
      "initialArmatureCurrentAmps",
    ] as const) {
      expect(
        liveParameterUpdate(catalog, schematic, motor, parameter, 1),
      ).toBeUndefined();
    }
  });

  it("hands the controller the plain SI value, run, and revision", async () => {
    const { store, fuse } = allDevices();
    const schematic = store.getState().schematic;
    const layout = layoutFor(schematic);
    const states = stateRecords(schematic);
    const adapter = new PredeterminedEngineAdapter({
      catalog,
      compile: (request) =>
        compileSuccess(layout, states, request.documentRevision),
      setParameter: (request) => ({
        status: "success",
        internalSubsteps: 1,
        snapshot: snapshotOf(layout, states, request.documentRevision, 1n),
        diagnostics: [],
      }),
    });
    const controller = new SimulationController(adapter, symbols, revisions());

    await controller.start(schematic);
    await controller.setParameter(fuse, "tripI2tAmpereSquaredSeconds", 0.01);

    expect(adapter.parameterRequests).toHaveLength(1);
    expect(adapter.parameterRequests[0]).toMatchObject({
      componentId: fuse,
      parameterId: "tripI2tAmpereSquaredSeconds",
      value: 0.01,
      expectedDocumentRevision: "rev-1",
      documentRevision: "rev-2",
    });
    expect(controller.getState().status).toBe("ready");
  });
});

function revisions(): () => string {
  let next = 0;
  return () => `rev-${(next += 1)}`;
}

function stateRecords(schematic: Schematic): DeviceState[] {
  return [...schematic.components]
    .filter(
      (component) =>
        component.typeId === "relay_spdt" || component.typeId === "fuse",
    )
    .sort((a, b) => (a.id < b.id ? -1 : 1))
    .map((component) =>
      component.typeId === "relay_spdt"
        ? { kind: "relay", componentId: component.id, state: "released" }
        : { kind: "fuse", componentId: component.id, state: "intact" },
    ) as DeviceState[];
}

const RUN = "00000000-0000-4000-8000-0000000000a0";

function snapshotOf(
  layout: ReturnType<typeof layoutFor>,
  states: DeviceState[],
  documentRevision: string,
  frameSequence: bigint,
) {
  return {
    metadata: {
      runId: RUN,
      documentRevision,
      layoutVersion: layout.layoutVersion,
      simulationStep: 0n,
      simulationTimeSeconds: 0,
      frameSequence,
    },
    values: valuesFor(layout, {}),
    states,
  };
}

function compileSuccess(
  layout: ReturnType<typeof layoutFor>,
  states: DeviceState[],
  documentRevision: string,
): CompileResult {
  return {
    status: "success",
    runId: RUN,
    layout,
    internalSubsteps: 1,
    initialSnapshot: snapshotOf(layout, states, documentRevision, 0n),
    diagnostics: [],
  };
}

describe("the Engine's diagnostics for Stage 4 circuits", () => {
  it("shows an InvalidParameter on a relay and keeps the document as drawn", async () => {
    const { store, relay } = allDevices();
    store.setParameter(relay, "dropoutCurrentAmps", 0.05, catalog);
    const schematic = store.getState().schematic;
    const adapter = new PredeterminedEngineAdapter({
      catalog,
      compile: () => ({
        status: "failure",
        diagnostics: [
          {
            code: "InvalidParameter",
            severity: "error",
            localizationKey: "diagnostic.invalidParameter",
            references: [{ kind: "component", componentId: relay }],
          },
        ],
      }),
      setParameter: () => ({ status: "failure", diagnostics: [] }),
    });
    const controller = new SimulationController(adapter, symbols, revisions());

    await controller.start(schematic);
    const state = controller.getState();

    expect(state.status).toBe("failed");
    expect(state.run).toBeNull();
    expect(state.diagnostics.map((item) => item.code)).toEqual([
      "InvalidParameter",
    ]);
    expect(store.getState().schematic).toBe(schematic);
    expect(store.getState().canUndo).toBe(true);
  });

  it.each([
    ["EventChatter", "diagnostic.eventChatter", /again and again/],
    ["NonFiniteState", "diagnostic.nonFiniteState", /too large to represent/],
  ] as const)("says what %s means in words", (code, localizationKey, words) => {
    const text = describeDiagnostic({
      code,
      severity: "error",
      localizationKey,
      references: [],
    });

    expect(text).toMatch(words);
    expect(text).not.toBe(localizationKey);
  });
});

describe("opening and saving a circuit with the Stage 4 devices", () => {
  function memoryFiles() {
    let saved = "";
    const files: ProjectFilePort = {
      open: () => Promise.resolve(JSON.parse(saved) as Schematic),
      save: (schematic) => {
        saved = JSON.stringify(schematic);
        return Promise.resolve(true);
      },
      saveAs: (schematic) => {
        saved = JSON.stringify(schematic);
        return Promise.resolve(true);
      },
      clearPath: () => Promise.resolve(),
    };

    return { files, text: () => saved };
  }

  it("round-trips the devices and their parameters, with no runtime state", async () => {
    const { store, motor } = allDevices();
    store.setParameter(
      motor,
      "initialAngularVelocityRadiansPerSecond",
      3,
      catalog,
    );
    const { files, text } = memoryFiles();
    await new ProjectWorkflow(store, files).save();

    for (const word of [
      "released",
      "picked",
      '"state"',
      "tripped",
      "accumulated",
      "states",
      "snapshot",
      "lastTransition",
    ]) {
      expect(text()).not.toContain(word);
    }

    const reopened = new SchematicDocumentStore(blankSchematic, ids());
    await new ProjectWorkflow(reopened, files).perform("open");

    expect(reopened.getState().schematic).toEqual(store.getState().schematic);
    expect(reopened.getState().isDirty).toBe(false);
    expect([...reopened.getState().labels.values()]).toEqual([
      "K1",
      "F1",
      "M1",
      "VM1",
      "AM1",
    ]);
  });
});
