import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import type { ResultLayout } from "../generated/ResultLayout";
import type { StableId } from "../generated/StableId";
import type { Unit } from "../generated/Unit";
import { SchematicDocumentStore } from "../store/document";
import { blankSchematic } from "../store/projectWorkflow";
import {
  initialSimulationState,
  type SimulationState,
} from "../store/simulation";
import { DevicePanel } from "./DevicePanel";
import { applyEditorCommand } from "./editorCommands";

const catalog = catalogFixture as ComponentCatalog;
const RUN = "00000000-0000-4000-8000-0000000000f0";

function drawn(type: ComponentTypeId) {
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => "00000000-0000-4000-8000-0000000000f1",
  );
  applyEditorCommand(
    store,
    {
      kind: "placeComponent",
      typeId: type,
      position: { x: 0, y: 0 },
      rotationDeg: 0,
    },
    catalog,
  );
  const document = store.getState();

  return { document, id: document.schematic.components[0]!.id };
}

type Reading = readonly [key: string, unit: Unit, value: number];

/** A run with exactly these readings, laid out the way the Engine names its slots. */
function stateWith(
  id: StableId,
  readings: readonly Reading[],
  states: DeviceState[] = [],
  status: SimulationState["status"] = "ready",
): SimulationState {
  const layout: ResultLayout = {
    layoutVersion: 1,
    valueCount: readings.length,
    slots: readings.map(([key, unit], offset) => ({
      key: `${id}/${key}`,
      componentId: id,
      quantity: "current",
      unit,
      signConvention: "currentIntoPin1",
      offset,
    })),
  };

  return {
    ...initialSimulationState(),
    status,
    catalog,
    run: { runId: RUN, layout, documentRevision: "rev-1" },
    snapshot: {
      metadata: {
        runId: RUN,
        documentRevision: "rev-1",
        layoutVersion: 1,
        simulationStep: 10n,
        simulationTimeSeconds: 0.01,
        frameSequence: 3n,
      },
      values: readings.map(([, , value]) => value),
      states,
    },
  };
}

function panel(
  document: ReturnType<typeof drawn>["document"],
  state: SimulationState,
  onOperate?: () => void,
) {
  return renderToStaticMarkup(
    <DevicePanel
      state={state}
      schematic={document.schematic}
      labels={document.labels}
      onOperate={onOperate}
    />,
  );
}

describe("the lamp and the indicator readouts", () => {
  const lamp = (lit: boolean): [Reading[], DeviceState[]] => {
    return [
      [
        ["voltageVolts", "volt", 12],
        ["currentAmps", "amp", 0.1],
        ["powerWatts", "watt", 1.2],
      ],
      [{ kind: "lamp", componentId: "", lit }],
    ];
  };

  it("shows Lit or Dark from the Engine's state, with voltage, current and power", () => {
    const { document, id } = drawn("lamp");
    for (const lit of [true, false]) {
      const [readings, states] = lamp(lit);
      const html = panel(
        document,
        stateWith(
          id,
          readings,
          states.map((state) => ({ ...state, componentId: id }) as DeviceState),
        ),
      );

      expect(html).toContain(lit ? ">Lit<" : ">Dark<");
      expect(html).toContain("device-badge-" + (lit ? "on" : "off"));
      expect(html).toContain("Power");
      expect(html).toContain("Voltage");
    }
  });

  it("shows an indicator without a power row", () => {
    const { document, id } = drawn("indicator");
    const html = panel(
      document,
      stateWith(
        id,
        [
          ["voltageVolts", "volt", 12],
          ["currentAmps", "amp", 0.005],
        ],
        [{ kind: "indicator", componentId: id, lit: true }],
      ),
    );

    expect(html).toContain(">Lit<");
    expect(html).not.toContain("Power");
  });

  it("shows a dash for a missing reading and no badge without a state", () => {
    const { document, id } = drawn("lamp");
    const html = panel(document, stateWith(id, []));

    expect(html).toContain("—");
    expect(html).not.toContain(">Lit<");
    expect(html).not.toContain(">Dark<");
  });

  it("waits for the engine while the run is loading", () => {
    const { document, id } = drawn("lamp");

    expect(panel(document, stateWith(id, [], [], "loading"))).toContain(
      "Waiting for the engine",
    );
  });
});

describe("the buzzer readout", () => {
  it.each([
    [true, ">Sounding<"],
    [false, ">Silent<"],
  ])("shows %s as %s", (active, text) => {
    const { document, id } = drawn("buzzer");
    const html = panel(
      document,
      stateWith(
        id,
        [
          ["voltageVolts", "volt", 6],
          ["currentAmps", "amp", 0.025],
        ],
        [{ kind: "buzzer", componentId: id, active }],
      ),
    );

    expect(html).toContain(text);
  });
});

describe("the solenoid and valve readouts", () => {
  const readings: Reading[] = [
    ["voltageVolts", "volt", 24],
    ["currentAmps", "amp", 1],
  ];

  it.each([
    [true, ">Energized<"],
    [false, ">Released<"],
  ])("shows a solenoid energized=%s as %s", (energized, text) => {
    const { document, id } = drawn("solenoid");
    const html = panel(
      document,
      stateWith(id, readings, [
        { kind: "solenoid", componentId: id, energized },
      ]),
    );

    expect(html).toContain(text);
    expect(html).toContain("Coil current");
  });

  it.each([
    ["open", ">Open<"],
    ["closed", ">Closed<"],
  ] as const)("shows a valve %s as %s", (position, text) => {
    const { document, id } = drawn("valve");
    const html = panel(
      document,
      stateWith(id, readings, [{ kind: "valve", componentId: id, position }]),
    );

    expect(html).toContain(text);
  });
});

describe("the frequency converter readout", () => {
  const readings: Reading[] = [
    ["inputVoltageVolts", "volt", 24],
    ["inputCurrentAmps", "amp", 0.0024],
    ["uVoltageVolts", "volt", 0],
    ["uCurrentAmps", "amp", 0],
    ["vVoltageVolts", "volt", -58.7],
    ["vCurrentAmps", "amp", 0.58],
    ["wVoltageVolts", "volt", 58.7],
    ["wCurrentAmps", "amp", -0.58],
  ];

  it.each([
    [true, ">Enabled<"],
    [false, ">Disabled<"],
  ])(
    "shows enabled=%s as %s with the input and the three outputs",
    (enabled, text) => {
      const { document, id } = drawn("frequency_converter");
      const html = panel(
        document,
        stateWith(id, readings, [
          { kind: "frequencyConverter", componentId: id, enabled },
        ]),
      );

      expect(html).toContain(text);
      expect(html).toContain("Input voltage");
      for (const phase of ["U", "V", "W"]) {
        expect(html).toContain(`Output ${phase} voltage`);
        expect(html).toContain(`Output ${phase} current`);
      }
    },
  );

  it("shows a dash for a missing reading and no badge without a state", () => {
    const { document, id } = drawn("frequency_converter");
    const html = panel(document, stateWith(id, []));

    expect(html).toContain("—");
    expect(html).not.toContain(">Enabled<");
    expect(html).not.toContain(">Disabled<");
  });
});

describe("the LED, zener diode, MOSFET and amplifier readouts", () => {
  it.each([
    [true, ">Lit<"],
    [false, ">Dark<"],
  ])("shows an LED lit=%s as %s", (lit, text) => {
    const { document, id } = drawn("led");
    const html = panel(
      document,
      stateWith(
        id,
        [
          ["voltageVolts", "volt", 2],
          ["currentAmps", "amp", 0.005],
        ],
        [{ kind: "led", componentId: id, lit }],
      ),
    );

    expect(html).toContain(text);
    expect(html).toContain("Voltage");
  });

  it.each([
    [true, ">In breakdown<"],
    [false, ">Not in breakdown<"],
  ])("shows a zener diode inBreakdown=%s as %s", (inBreakdown, text) => {
    const { document, id } = drawn("zener_diode");
    const html = panel(
      document,
      stateWith(
        id,
        [
          ["voltageVolts", "volt", -5.2],
          ["currentAmps", "amp", -0.014],
        ],
        [{ kind: "zenerDiode", componentId: id, inBreakdown }],
      ),
    );

    expect(html).toContain(text);
  });

  it.each([
    [true, ">Conducting<"],
    [false, ">Off<"],
  ])(
    "shows a MOSFET conducting=%s as %s with its three readings",
    (conducting, text) => {
      const { document, id } = drawn("mosfet_n");
      const html = panel(
        document,
        stateWith(
          id,
          [
            ["gateSourceVoltageVolts", "volt", 5],
            ["drainSourceVoltageVolts", "volt", 3],
            ["drainCurrentAmps", "amp", 0.2],
          ],
          [{ kind: "mosfetN", componentId: id, conducting }],
        ),
      );

      expect(html).toContain(text);
      expect(html).toContain("Gate-source voltage");
      expect(html).toContain("Drain-source voltage");
      expect(html).toContain("Drain current");
    },
  );

  it.each([
    ["linear", ">Linear<"],
    ["clippedLow", ">Clipped low<"],
    ["clippedHigh", ">Clipped high<"],
  ] as const)("shows an amplifier at %s as %s", (position, text) => {
    const { document, id } = drawn("operational_amplifier");
    const html = panel(
      document,
      stateWith(
        id,
        [
          ["differentialVoltageVolts", "volt", 0.003],
          ["outputVoltageVolts", "volt", 3],
          ["outputCurrentAmps", "amp", -0.003],
        ],
        [{ kind: "operationalAmplifier", componentId: id, state: position }],
      ),
    );

    expect(html).toContain(text);
    expect(html).toContain("Differential input");
  });

  it("shows a dash and no badge while there is no state or reading", () => {
    const { document, id } = drawn("mosfet_n");
    const html = panel(document, stateWith(id, []));

    expect(html).toContain("—");
    expect(html).not.toContain(">Conducting<");
    expect(html).not.toContain(">Off<");
  });
});
