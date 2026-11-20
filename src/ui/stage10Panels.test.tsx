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
import { operatorPosition } from "./operatorPosition";

const catalog = catalogFixture as ComponentCatalog;
const RUN = "00000000-0000-4000-8000-0000000000b0";

function drawn(type: ComponentTypeId) {
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => "00000000-0000-4000-8000-0000000000b1",
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

/** A ready run with exactly these readings, laid out the way the Engine names its slots. */
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

const poles = (current: number): Reading[] => [
  ["pole1/current", "amp", current],
  ["pole2/current", "amp", current * 2],
  ["pole3/current", "amp", current * 3],
];

describe("the three-pole contactor readout", () => {
  it("shows the state, the coil, the detector and the three poles as the Engine reported", () => {
    const { document, id } = drawn("contactor_three_pole");
    const html = panel(
      document,
      stateWith(
        id,
        [
          ["coil/current", "amp", 0.05],
          ["coil/current_squared", "ampere_squared", 0.0025],
          ...poles(1),
          ["no/current", "amp", 0.2],
          ["nc/current", "amp", 0],
        ],
        [
          {
            kind: "contactor",
            componentId: id,
            state: "picked",
            lastTransitionTimeSeconds: 0.004,
          },
        ],
      ),
    );

    expect(html).toContain("Three-pole contactor");
    expect(html).toContain(">Picked<");
    expect(html).toContain("since 4 ms");
    expect(html).toContain("Coil detector (mean square)");
    expect(html).toContain("0.0025 A²");
    for (const label of ["L1", "L2", "L3"]) {
      expect(html).toContain(`Pole ${label} current`);
    }
    expect(html).toContain(">1 A<");
    expect(html).toContain(">3 A<");
  });

  it("is shown released, and its poles as unknown, before the Engine reports anything", () => {
    const { document, id } = drawn("contactor_three_pole");
    const html = panel(document, stateWith(id, []));

    expect(html).toContain("—");
    expect(html).not.toContain(">Picked<");
  });
});

describe("the breaker readout and its operator control", () => {
  const closed = (id: StableId, conducting: boolean): DeviceState => ({
    kind: "switch",
    componentId: id,
    position: "closed",
    conducting,
  });

  it("is a switch button named after the breaker, with its state from the Engine", () => {
    const { document, id } = drawn("breaker_three_pole");
    const html = panel(
      document,
      stateWith(
        id,
        [["tripped", "dimensionless", 0], ...poles(2)],
        [closed(id, true)],
      ),
      () => {},
    );

    expect(html).toContain('role="switch"');
    expect(html).toContain('aria-label="QF1 breaker"');
    expect(html).toContain('aria-checked="true"');
    expect(html).toContain(">Closed<");
    expect(html).toContain("Conducting");
    expect(html).not.toContain("Open the breaker");
  });

  it("says Tripped and how to reset it, from the Engine's tripped flag", () => {
    const { document, id } = drawn("breaker_three_pole");
    const html = panel(
      document,
      stateWith(
        id,
        [["tripped", "dimensionless", 1], ...poles(0)],
        [closed(id, false)],
      ),
      () => {},
    );

    expect(html).toContain(">Tripped<");
    expect(html).toContain("device-badge-fault");
    expect(html).toContain("To reset");
    expect(html).toContain("Open the breaker");
  });

  it("sends an open when a closed or tripped breaker is toggled, which is how it resets", () => {
    const state = (conducting: boolean): DeviceState => ({
      kind: "switch",
      componentId: "00000000-0000-4000-8000-0000000000b1",
      position: "closed",
      conducting,
    });

    expect(operatorPosition("toggle", state(true))).toBe("open");
    expect(operatorPosition("toggle", state(false))).toBe("open");
    expect(
      operatorPosition("toggle", {
        ...state(false),
        position: "open",
      } as DeviceState),
    ).toBe("closed");
  });

  it("disables the control while there is no run to send to", () => {
    const { document, id } = drawn("breaker_three_pole");
    const html = panel(document, stateWith(id, [], [], "loading"), () => {});

    expect(html).toContain("Waiting for the engine");
    expect(html).not.toContain('role="switch"');
  });
});

describe("the overload relay readout and its reset", () => {
  const overload = (id: StableId, tripped: boolean): DeviceState => ({
    kind: "fuse",
    componentId: id,
    state: tripped ? "tripped" : "intact",
  });

  it("offers the reset button only while the relay is tripped", () => {
    const { document, id } = drawn("thermal_overload_three_pole");
    const readings: Reading[] = [
      ["thermal_dose", "dimensionless", 1],
      ...poles(1),
    ];
    const tripped = panel(
      document,
      stateWith(id, readings, [overload(id, true)]),
      () => {},
    );
    const intact = panel(
      document,
      stateWith(id, readings, [overload(id, false)]),
      () => {},
    );

    expect(tripped).toContain("Reset overload");
    expect(tripped).toContain(">Tripped<");
    expect(tripped).toContain("Thermal dose");
    expect(intact).not.toContain("Reset overload");
    expect(intact).toContain(">Intact<");
  });

  it("asks the Engine for a reset only from a tripped state", () => {
    const id = "00000000-0000-4000-8000-0000000000b1";

    expect(operatorPosition("reset", overload(id, true))).toBe("pressed");
    expect(operatorPosition("reset", overload(id, false))).toBeNull();
    expect(operatorPosition("reset", undefined)).toBeNull();
  });
});

describe("the induction motor readout", () => {
  it("shows speed, torque and the three winding currents, all from the Engine", () => {
    const { document, id } = drawn("induction_motor_three_phase");
    const html = panel(
      document,
      stateWith(id, [
        ["angular_velocity", "radian_per_second", 148.5],
        ["electromagnetic_torque", "newton_meter", 1.25],
        ["u/current", "amp", 3],
        ["v/current", "amp", -1.5],
        ["w/current", "amp", -1.5],
      ]),
    );

    expect(html).toContain("Three-phase induction motor");
    expect(html).toContain("Angular velocity");
    expect(html).toContain("148.5 rad/s");
    expect(html).toContain("Electromagnetic torque");
    expect(html).toContain("1.25 N·m");
    for (const winding of ["U", "V", "W"]) {
      expect(html).toContain(`Winding ${winding} current`);
    }
    expect(html).toContain(">3 A<");
    expect(html).toContain(">-1.5 A<");
  });

  it("shows no value for a reading the run does not contain", () => {
    const { document, id } = drawn("induction_motor_three_phase");
    const html = panel(document, stateWith(id, []));

    expect(html).toContain("—");
    expect(html).not.toContain("rad/s");
  });
});

describe("the transformer and the three-phase loads", () => {
  it("lists every bank's primary and secondary readings", () => {
    const { document, id } = drawn("transformer_three_phase");
    const readings: Reading[] = [1, 2, 3].flatMap((bank): Reading[] => [
      [`primary${bank}/current`, "amp", bank],
      [`secondary${bank}/current`, "amp", bank * 10],
      [`secondary${bank}/voltage`, "volt", bank * 100],
    ]);
    const html = panel(document, stateWith(id, readings));

    expect(html).toContain("Three-phase transformer");
    for (const bank of [1, 2, 3]) {
      expect(html).toContain(`Primary ${bank} current`);
      expect(html).toContain(`Secondary ${bank} voltage`);
    }
    expect(html).toContain(">300 V<");
  });

  it("shows the branch currents of a star load and its neutral", () => {
    const { document, id } = drawn("three_phase_load_star");
    const html = panel(
      document,
      stateWith(id, [
        ["l1/current", "amp", 1],
        ["l2/current", "amp", 2],
        ["l3/current", "amp", 3],
        ["n/current", "amp", 0.5],
      ]),
    );

    expect(html).toContain("Branch L1–N current");
    expect(html).toContain("Neutral current");
    expect(html).toContain(">500 mA<");
  });

  it("shows the branch and line currents of a delta load", () => {
    const { document, id } = drawn("three_phase_load_delta");
    const html = panel(
      document,
      stateWith(id, [
        ["l12/current", "amp", 4],
        ["l23/current", "amp", 5],
        ["l31/current", "amp", 6],
        ["l1/current", "amp", 7],
        ["l2/current", "amp", 8],
        ["l3/current", "amp", 9],
      ]),
    );

    expect(html).toContain("Branch L1–L2 current");
    expect(html).toContain("Line L3 current");
    expect(html).toContain(">9 A<");
  });
});

describe("stale or failed runs", () => {
  // One reading that each device really shows, so the assertion can fail for every type.
  const SHOWN: readonly (readonly [ComponentTypeId, string, Unit, string])[] = [
    ["contactor_three_pole", "pole1/current", "amp", "9 A"],
    ["breaker_three_pole", "pole1/current", "amp", "9 A"],
    ["thermal_overload_three_pole", "pole1/current", "amp", "9 A"],
    ["induction_motor_three_phase", "u/current", "amp", "9 A"],
    ["transformer_three_phase", "primary1/current", "amp", "9 A"],
    ["three_phase_load_star", "n/current", "amp", "9 A"],
    ["three_phase_load_delta", "l12/current", "amp", "9 A"],
  ];

  it.each(SHOWN)(
    "shows the reading of %s when the run is ready, and none unless it is",
    (type, key, unit, text) => {
      const { document, id } = drawn(type);
      const readings: Reading[] = [[key, unit, 9]];

      expect(panel(document, stateWith(id, readings))).toContain(`>${text}<`);
      for (const status of ["loading", "failed"] as const) {
        const html = panel(document, stateWith(id, readings, [], status));

        expect(html).not.toContain(text);
      }
    },
  );
});
