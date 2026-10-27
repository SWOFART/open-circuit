import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import type { ResultSlot } from "../generated/ResultSlot";
import type { StableId } from "../generated/StableId";
import { SchematicDocumentStore } from "../store/document";
import { blankSchematic } from "../store/projectWorkflow";
import {
  initialSimulationState,
  type SimulationState,
} from "../store/simulation";
import { keyOf, layoutFor, valuesFor } from "../store/stage4TestSupport";
import type { ScopeState } from "../store/traces";
import { applyEditorCommand } from "./editorCommands";
import { DevicePanel } from "./DevicePanel";
import { Inspector } from "./Inspector";
import { Oscilloscope } from "./Oscilloscope";
import { ResultPanel } from "./ResultPanel";
import { slotDescription, slotLabel } from "./slotLabels";

const catalog = catalogFixture as ComponentCatalog;
const RUN = "00000000-0000-4000-8000-0000000000a0";

function drawn(types: readonly ComponentTypeId[]) {
  let next = 0x41;
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`,
  );

  const placed = types.map((typeId, index) => {
    applyEditorCommand(
      store,
      {
        kind: "placeComponent",
        typeId,
        position: { x: index * 8, y: 0 },
        rotationDeg: 0,
      },
      catalog,
    );

    return store.getState().schematic.components.at(-1)!.id;
  });

  return { store, ids: placed };
}

function stateWith(
  store: SchematicDocumentStore,
  values: Readonly<Record<string, number>>,
  states: DeviceState[],
  time = 0.007,
): SimulationState {
  const { schematic } = store.getState();
  const layout = layoutFor(schematic);

  return {
    ...initialSimulationState(),
    status: "ready",
    catalog,
    run: { runId: RUN, layout, documentRevision: "rev-1" },
    snapshot: {
      metadata: {
        runId: RUN,
        documentRevision: "rev-1",
        layoutVersion: 1,
        simulationStep: BigInt(Math.round(time * 1000)),
        simulationTimeSeconds: time,
        frameSequence: 5n,
      },
      values: valuesFor(layout, values),
      states,
    },
  };
}

describe("the inspector for the Stage 4 devices", () => {
  const { store } = drawn([
    "relay_spdt",
    "fuse",
    "pmdc_motor",
    "voltmeter",
    "ammeter",
  ]);
  const html = renderToStaticMarkup(
    <Inspector
      state={stateWith(store, {}, [])}
      schematic={store.getState().schematic}
      labels={store.getState().labels}
      onCommit={() => {}}
    />,
  );

  it("shows every required parameter with its catalog default, unit, and name", () => {
    const expected: [string, string][] = [
      ["K1 coil resistance", "100 Ω"],
      ["K1 coil inductance", "10 mH"],
      ["K1 pickup current", "40 mA"],
      ["K1 dropout current", "20 mA"],
      ["K1 closed contact resistance", "100 mΩ"],
      ["F1 intact resistance", "10 mΩ"],
      ["F1 i²t trip threshold", "0.004 A²·s"],
      ["M1 armature resistance", "2 Ω"],
      ["M1 armature inductance", "100 mH"],
      ["M1 motor constant", "100 mN·m/A"],
      ["M1 rotor inertia", "0.01 kg·m²"],
      ["M1 viscous friction", "10 mN·m·s/rad"],
      ["M1 load torque", "0 N·m"],
      ["VM1 input resistance", "10 MΩ"],
      ["AM1 burden resistance", "100 mΩ"],
    ];

    for (const [label, value] of expected) {
      expect(html).toContain(`>${label}</label>`);
      const field = html.slice(html.indexOf(`>${label}</label>`));
      expect(field.slice(0, 400)).toContain(`value="${value}"`);
    }
  });

  it("explains the relay's pickup and dropout rule, the fuse's one-shot trip, and the load torque sign", () => {
    expect(html).toContain("Must be below the pickup current");
    expect(html).toContain(
      "a tripped fuse stays open until the circuit is compiled again",
    );
    expect(html).toContain("A negative torque opposes positive rotation");
  });

  it("offers the motor's two initial conditions as explicit choices, unset by default", () => {
    expect(html).toContain("Set M1 initial angular velocity");
    expect(html).toContain("Set M1 initial armature current");
    expect(html.match(/type="checkbox"/g)).toHaveLength(2);
    expect(html).not.toContain("checked");
    expect(html).toContain("Not set: starts at 0 rad/s.");
    expect(html).toContain(
      "Not set: starts from the DC solution at the initial speed.",
    );
    expect(html).toContain(
      "Positive speed is the rotation that positive current into pin1 drives.",
    );
    expect(html).toContain("Current entering pin1 at time zero.");
  });

  it("shows a set initial condition as a field in its own unit", () => {
    const withSpeed = drawn(["pmdc_motor"]);
    const [motor] = withSpeed.ids;
    withSpeed.store.setParameter(
      motor!,
      "initialAngularVelocityRadiansPerSecond",
      -12.5,
      catalog,
    );
    const markup = renderToStaticMarkup(
      <Inspector
        state={stateWith(withSpeed.store, {}, [])}
        schematic={withSpeed.store.getState().schematic}
        labels={withSpeed.store.getState().labels}
        onCommit={() => {}}
      />,
    );

    expect(markup).toContain(">M1 initial angular velocity</label>");
    expect(markup).toContain('value="-12.5 rad/s"');
    expect(markup.match(/checked=""/g)).toHaveLength(1);
  });

  it("does not offer a field the component's catalog entry does not have", () => {
    const meters = drawn(["voltmeter"]);
    const markup = renderToStaticMarkup(
      <Inspector
        state={stateWith(meters.store, {}, [])}
        schematic={meters.store.getState().schematic}
        labels={meters.store.getState().labels}
        onCommit={() => {}}
      />,
    );

    expect(markup.match(/<input/g)).toHaveLength(1);
  });
});

describe("what a result slot measures", () => {
  const { store } = drawn(["relay_spdt", "fuse", "pmdc_motor", "ammeter"]);
  const layout = layoutFor(store.getState().schematic);
  const labels = store.getState().labels;
  const describeAll = () =>
    layout.slots.map((slot: ResultSlot) => slotLabel(slot, labels));

  it("tells the relay's three voltages and three currents apart", () => {
    const relay = layout.slots.filter(
      (slot) => labels.get(slot.componentId) === "K1",
    );

    expect(relay.map((slot) => slotLabel(slot, labels))).toEqual([
      "K1 coil voltage",
      "K1 coil current",
      "K1 normally open voltage",
      "K1 normally open current",
      "K1 normally closed voltage",
      "K1 normally closed current",
    ]);
  });

  it("names the fuse's accumulated I²t and the motor's angular velocity", () => {
    expect(describeAll()).toContain("F1 accumulated I²t");
    expect(describeAll()).toContain("M1 angular velocity");
  });

  it("gives every slot of a circuit its own label", () => {
    const labelsOfAll = describeAll();

    expect(new Set(labelsOfAll).size).toBe(labelsOfAll.length);
  });

  it("falls back to the quantity for a key that does not follow the Engine's format", () => {
    const [first] = layout.slots;

    expect(slotDescription({ ...first!, key: "something-else" })).toBe(
      first!.quantity,
    );
  });
});

describe("the result list for the Stage 4 devices", () => {
  it("lists every slot with its own name and unit", () => {
    const { store, ids } = drawn(["relay_spdt", "fuse", "pmdc_motor"]);
    const [relay, fuse, motor] = ids as [StableId, StableId, StableId];
    const schematic = store.getState().schematic;
    const state = stateWith(
      store,
      {
        [keyOf(schematic, relay, "coil/current")]: 0.04995005,
        [keyOf(schematic, fuse, "accumulated_i2t")]: 0.004,
        [keyOf(schematic, motor, "angular_velocity")]: 33.33349259,
      },
      [],
    );
    const html = renderToStaticMarkup(
      <ResultPanel state={state} labels={store.getState().labels} />,
    );

    expect(html).toContain("K1 coil current</dt><dd>49.95005 mA");
    expect(html).toContain("F1 accumulated I²t</dt><dd>0.004 A²·s");
    expect(html).toContain("M1 angular velocity</dt><dd>33.33349 rad/s");
    expect(html).not.toMatch(/NaN|Infinity/);
  });
});

describe("the device panel", () => {
  const circuit = drawn([
    "relay_spdt",
    "fuse",
    "pmdc_motor",
    "voltmeter",
    "ammeter",
  ]);
  const [relay, fuse, motor, voltmeter, ammeter] = circuit.ids as [
    StableId,
    StableId,
    StableId,
    StableId,
    StableId,
  ];
  const { store } = circuit;
  const schematic = store.getState().schematic;
  const key = (id: StableId, suffix: string) => keyOf(schematic, id, suffix);

  function render(state: SimulationState): string {
    return renderToStaticMarkup(
      <DevicePanel
        state={state}
        schematic={store.getState().schematic}
        labels={store.getState().labels}
      />,
    );
  }

  const released: DeviceState[] = [
    { kind: "relay", componentId: relay, state: "released" },
    { kind: "fuse", componentId: fuse, state: "intact" },
  ];

  it("shows a released relay and an intact fuse before anything has happened", () => {
    const html = render(stateWith(store, {}, released, 0));

    expect(html).toContain("K1 SPDT relay");
    expect(html).toContain('device-badge-off">Released');
    expect(html).toContain("F1 Fuse");
    expect(html).toContain('device-badge-on">Intact');
    expect(html).not.toContain("Last change");
    expect(html).not.toContain("Tripped<");
  });

  it("shows a picked relay with the time of its last change and the coil current", () => {
    const html = render(
      stateWith(
        store,
        { [key(relay, "coil/current")]: 0.04995005 },
        [
          {
            kind: "relay",
            componentId: relay,
            state: "picked",
            lastTransitionTimeSeconds: 0.002,
          },
          released[1]!,
        ],
        0.005,
      ),
    );

    expect(html).toContain('device-badge-on">Picked');
    expect(html).toContain("Last change</dt><dd>since 2 ms");
    expect(html).toContain("Coil current</dt><dd>49.95005 mA");
  });

  it("shows a tripped fuse as a fault, with the accumulated I²t against its threshold and no current", () => {
    const html = render(
      stateWith(
        store,
        { [key(fuse, "accumulated_i2t")]: 0.004, [key(fuse, "voltage")]: 2.02 },
        [
          released[0]!,
          {
            kind: "fuse",
            componentId: fuse,
            state: "tripped",
            lastTransitionTimeSeconds: 0.001,
          },
        ],
        0.002,
      ),
    );

    expect(html).toContain('device-badge-fault">Tripped');
    expect(html).toContain("Tripped</dt><dd>since 1 ms");
    expect(html).toContain("Accumulated I²t</dt><dd>0.004 A²·s of 0.004 A²·s");
    expect(html).toContain("Current</dt><dd>0 A");
  });

  it("shows the motor's speed, current, and terminal voltage as the Engine reported them", () => {
    const html = render(
      stateWith(
        store,
        {
          [key(motor, "angular_velocity")]: 33.33349259,
          [key(motor, "current")]: 3.33332471,
          [key(motor, "voltage")]: 10,
        },
        released,
        8,
      ),
    );

    expect(html).toContain("M1 DC motor");
    expect(html).toContain("Angular velocity</dt><dd>33.33349 rad/s");
    expect(html).toContain("Armature current</dt><dd>3.333325 A");
    expect(html).toContain("Terminal voltage</dt><dd>10 V");
  });

  it("shows the meters' readings with the loading they cause", () => {
    const html = render(
      stateWith(
        store,
        {
          [key(voltmeter, "voltage")]: 9.09090909,
          [key(voltmeter, "current")]: 9.09091e-7,
          [key(ammeter, "current")]: 0.90909091,
          [key(ammeter, "voltage")]: 0.090909091,
        },
        released,
      ),
    );

    expect(html).toContain("VM1 Voltmeter");
    expect(html).toContain("Reading</dt><dd>9.090909 V");
    expect(html).toContain("Input current</dt><dd>909.091 nA");
    expect(html).toContain("AM1 Ammeter");
    expect(html).toContain("Reading</dt><dd>909.0909 mA");
    expect(html).toContain("Burden voltage</dt><dd>90.90909 mV");
  });

  it("follows the state when a newer frame arrives", () => {
    const before = render(stateWith(store, {}, released, 0.001));
    const after = render(
      stateWith(
        store,
        {},
        [
          {
            kind: "relay",
            componentId: relay,
            state: "picked",
            lastTransitionTimeSeconds: 0.003,
          },
          released[1]!,
        ],
        0.004,
      ),
    );

    expect(before).toContain("Released");
    expect(after).toContain("Picked");
    expect(after).not.toContain("device-badge-off");
  });

  it("is absent for a circuit without these devices", () => {
    const plain = drawn(["resistor", "capacitor"]);

    expect(
      renderToStaticMarkup(
        <DevicePanel
          state={stateWith(plain.store, {}, [])}
          schematic={plain.store.getState().schematic}
          labels={plain.store.getState().labels}
        />,
      ),
    ).toBe("");
  });

  it("says why there are no readings when there is no run", () => {
    const html = render({
      ...initialSimulationState(),
      status: "failed",
      catalog,
    });

    expect(html).toContain("No active run");
    expect(html).not.toContain("device-badge");
    expect(
      render({ ...initialSimulationState(), status: "loading" }),
    ).toContain("Waiting for the engine");
  });

  it("never writes NaN or infinity", () => {
    const html = render(stateWith(store, {}, released));

    expect(html).not.toMatch(/NaN|Infinity/);
  });
});

describe("the oscilloscope with the Stage 4 signals", () => {
  const { store, ids } = drawn(["relay_spdt", "fuse", "pmdc_motor"]);
  const [relay, fuse, motor] = ids as [StableId, StableId, StableId];
  const schematic = store.getState().schematic;
  const simulation = stateWith(store, {}, []);
  const scope = (selectedKeys: string[]): ScopeState => ({
    selectedKeys,
    windowSeconds: 0.1,
    data: null,
    error: null,
    loading: false,
  });
  const render = (selectedKeys: string[], data: ScopeState["data"] = null) =>
    renderToStaticMarkup(
      <Oscilloscope
        simulation={simulation}
        scope={{ ...scope(selectedKeys), data }}
        labels={store.getState().labels}
        onToggleSignal={() => {}}
        onWindowChange={() => {}}
      />,
    );

  it("offers every relay port, the fuse's I²t, and the motor's speed under distinct names", () => {
    const html = render([]);

    for (const name of [
      "K1 coil voltage",
      "K1 coil current",
      "K1 normally open voltage",
      "K1 normally closed current",
      "F1 accumulated I²t",
      "M1 angular velocity",
    ]) {
      expect(html).toContain(name);
    }
  });

  it("draws an angular velocity lane and an I²t lane with their own names and units", () => {
    const speed = key(motor, "angular_velocity");
    const i2t = key(fuse, "accumulated_i2t");
    const times = [0, 0.001, 0.002];
    const html = render([speed, i2t], {
      runId: RUN,
      layoutVersion: 1,
      documentRevision: "rev-1",
      sampleIntervalSeconds: 0.001,
      latestTimeSeconds: 0.002,
      samplesPerPoint: 1,
      signals: [
        {
          key: speed,
          unit: "radian_per_second",
          times,
          minimums: [0, 1, 2],
          maximums: [0, 1, 2],
        },
        {
          key: i2t,
          unit: "ampere_squared_second",
          times,
          minimums: [0, 0.002, 0.004],
          maximums: [0, 0.002, 0.004],
        },
      ],
    });

    expect(html).toContain("Angular velocity (rad/s)");
    expect(html).toContain("Accumulated I²t (A²·s)");
    expect(html).toContain("Angular velocity traces: M1 angular velocity");
    expect(html).toContain("Accumulated I²t traces: F1 accumulated I²t");
    expect(html.match(/<svg/g)).toHaveLength(2);
    expect(html).not.toMatch(/NaN|Infinity/);
  });

  function key(id: StableId, suffix: string): string {
    return keyOf(schematic, id, suffix);
  }

  it("keeps relay signals of different ports on separate checkboxes", () => {
    const html = render([key(relay, "coil/current")]);

    expect(html.match(/checked=""/g)).toHaveLength(1);
  });
});
