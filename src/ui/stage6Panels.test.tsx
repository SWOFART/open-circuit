import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import { SchematicDocumentStore } from "../store/document";
import { blankSchematic } from "../store/projectWorkflow";
import {
  initialSimulationState,
  type SimulationState,
} from "../store/simulation";
import { keyOf, layoutFor, valuesFor } from "../store/stage4TestSupport";
import { applyEditorCommand } from "./editorCommands";
import { DevicePanel } from "./DevicePanel";
import { Inspector } from "./Inspector";
import { Palette } from "./Palette";

const catalog = catalogFixture as ComponentCatalog;
const RUN = "00000000-0000-4000-8000-0000000000a0";

function drawn(types: readonly ComponentTypeId[]) {
  let next = 0x41;
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => `00000000-0000-4000-8000-0000000000${(next++).toString(16)}`,
  );
  const ids = types.map((typeId, index) => {
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

  return { store, ids };
}

function stateWith(
  store: SchematicDocumentStore,
  values: Readonly<Record<string, number>>,
  states: DeviceState[],
  status: SimulationState["status"] = "ready",
): SimulationState {
  const { schematic } = store.getState();
  const layout = layoutFor(schematic);

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
        simulationStep: 7n,
        simulationTimeSeconds: 0.007,
        frameSequence: 5n,
      },
      values: valuesFor(layout, values),
      states,
    },
  };
}

describe("the palette", () => {
  it("offers the switch, both push buttons, and the contactor from the Engine catalog", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={catalog}
        tool={{ kind: "select" }}
        onSelectTool={() => {}}
      />,
    );

    for (const label of [
      "Switch",
      "Push button (normally open)",
      "Push button (normally closed)",
      "Contactor",
    ]) {
      expect(markup).toContain(`>${label}<`);
    }
  });
});

describe("placing the Stage 6 devices", () => {
  it("names a switch and the push buttons S1, S2, and S3 and the contactor Q1", () => {
    const { store, ids } = drawn([
      "switch_spst",
      "push_button_no",
      "push_button_nc",
      "contactor",
    ]);
    const labels = store.getState().labels;

    expect(ids.map((id) => labels.get(id))).toEqual(["S1", "S2", "S3", "Q1"]);
  });

  it("stores no initial position and no position of any kind in the document", () => {
    const { store } = drawn(["switch_spst", "push_button_nc", "contactor"]);

    expect(JSON.stringify(store.getState().schematic)).not.toMatch(
      /initialClosed|closed"|pressed|picked|conducting/,
    );
    expect(
      store.getState().schematic.components.map((item) => item.parameters),
    ).toEqual([
      { closedResistanceOhms: 0.01 },
      { closedResistanceOhms: 0.01 },
      {
        coilResistanceOhms: 100,
        coilInductanceHenries: 0.01,
        pickupCurrentAmps: 0.04,
        dropoutCurrentAmps: 0.02,
        closedContactResistanceOhms: 0.1,
      },
    ]);
  });
});

describe("the inspector for the Stage 6 devices", () => {
  const { store } = drawn([
    "switch_spst",
    "push_button_no",
    "push_button_nc",
    "contactor",
  ]);
  const html = renderToStaticMarkup(
    <Inspector
      state={stateWith(store, {}, [])}
      schematic={store.getState().schematic}
      labels={store.getState().labels}
      onCommit={() => {}}
    />,
  );

  it("shows every parameter with its catalog default, unit, and name", () => {
    const expected: [string, string][] = [
      ["S1 closed resistance", "10 mΩ"],
      ["S2 closed resistance", "10 mΩ"],
      ["S3 closed resistance", "10 mΩ"],
      ["Q1 coil resistance", "100 Ω"],
      ["Q1 coil inductance", "10 mH"],
      ["Q1 pickup current", "40 mA"],
      ["Q1 dropout current", "20 mA"],
      ["Q1 closed contact resistance", "100 mΩ"],
    ];

    for (const [label, value] of expected) {
      expect(html).toContain(`>${label}</label>`);
      const field = html.slice(html.indexOf(`>${label}</label>`));
      expect(field.slice(0, 400)).toContain(`value="${value}"`);
    }
  });

  it("offers a switch's initial position as a choice, open unless it was set", () => {
    expect(html).toContain(">S1 initial position</label>");
    const select = html.slice(html.indexOf(">S1 initial position</label>"));
    expect(select.slice(0, 200)).toContain("<select");
    expect(select.slice(0, 300)).toContain(
      '<option value="open" selected="">Open</option>',
    );
    expect(select.slice(0, 300)).toContain(
      '<option value="closed">Closed</option>',
    );
    // It is a choice, not a number field, and only the switch has one.
    expect(html.match(/initial position/g)).toHaveLength(1);
    expect(html).not.toContain('value="0"');
  });

  it("shows a stored closed initial position as Closed", () => {
    const closed = new SchematicDocumentStore({
      ...store.getState().schematic,
      components: store
        .getState()
        .schematic.components.map((item) =>
          item.typeId === "switch_spst"
            ? { ...item, parameters: { ...item.parameters, initialClosed: 1 } }
            : item,
        ),
    });
    const markup = renderToStaticMarkup(
      <Inspector
        state={stateWith(closed, {}, [])}
        schematic={closed.getState().schematic}
        labels={closed.getState().labels}
        onCommit={() => {}}
      />,
    );

    expect(markup).toMatch(/<select[^>]*>/);
    expect(markup).toContain('<option value="closed" selected="">Closed');
  });
});

describe("the device panel for the Stage 6 devices", () => {
  const circuit = drawn([
    "switch_spst",
    "push_button_no",
    "push_button_nc",
    "contactor",
  ]);
  const [switchId, noId, ncId, contactorId] = circuit.ids as [
    string,
    string,
    string,
    string,
  ];
  const { store } = circuit;
  const schematic = store.getState().schematic;

  const states = (closed: boolean, pressed: boolean): DeviceState[] => [
    {
      kind: "switch",
      componentId: switchId,
      position: closed ? "closed" : "open",
      conducting: closed,
      ...(closed ? { lastTransitionTimeSeconds: 0.004 } : {}),
    },
    {
      kind: "pushButton",
      componentId: noId,
      position: pressed ? "pressed" : "released",
      conducting: pressed,
    },
    {
      kind: "pushButton",
      componentId: ncId,
      position: pressed ? "pressed" : "released",
      conducting: !pressed,
    },
    {
      kind: "contactor",
      componentId: contactorId,
      state: closed ? "picked" : "released",
    },
  ];

  const render = (state: SimulationState, withOperate = true) =>
    renderToStaticMarkup(
      <DevicePanel
        state={state}
        schematic={schematic}
        labels={store.getState().labels}
        onOperate={withOperate ? () => {} : undefined}
      />,
    );

  it("gives every device an accessible name and its state in words", () => {
    const html = render(
      stateWith(
        store,
        { [keyOf(schematic, switchId, "voltage")]: 0.05 },
        states(true, false),
      ),
    );

    expect(html).toContain('role="switch"');
    expect(html).toContain('aria-label="S1 switch"');
    expect(html).toContain('aria-checked="true"');
    expect(html).toContain(">Closed</button>");
    expect(html).toContain('aria-label="S2 push button"');
    expect(html).toContain('aria-label="S3 push button"');
    expect(html.match(/aria-pressed="false"/g)).toHaveLength(2);
    expect(html).toContain(">Hold to press</button>");
    expect(html).toContain("S1 Switch");
    expect(html).toContain("S2 Push button (normally open)");
    expect(html).toContain("S3 Push button (normally closed)");
    expect(html).toContain("Q1 Contactor");
    expect(html).toContain("device-badge-on");
    expect(html).toContain(">Picked<");
  });

  it("shows exactly what the Engine reported, including conduction and the last change", () => {
    const html = render(stateWith(store, {}, states(true, true)));

    // The pressed normally closed button does not conduct, and the Engine said so.
    expect(html.match(/aria-pressed="true"/g)).toHaveLength(2);
    expect(html).toContain(">Pressed</button>");
    expect(html).toContain("<dt>Conducting</dt><dd>No</dd>");
    expect(html).toContain("<dt>Conducting</dt><dd>Yes</dd>");
    expect(html).toContain("since 4 ms");
    expect(html).toContain("Coil current");
    expect(html).toContain("Normally open contact current");
    expect(html).toContain("Normally closed contact current");
  });

  it("shows the contactor released and the contacts at rest before anything is picked", () => {
    const html = render(stateWith(store, {}, states(false, false)));

    expect(html).toContain('aria-checked="false"');
    expect(html).toContain(">Open</button>");
    expect(html).toContain(">Released<");
    expect(html).not.toContain(">Picked<");
  });

  it("shows no devices and no controls when the engine could not calculate the circuit", () => {
    const html = render(stateWith(store, {}, states(false, false), "failed"));

    expect(html).not.toContain('role="switch"');
    expect(html).not.toContain("operator-control");
    expect(html).toContain("No active run");
  });

  it("disables the controls when there is nothing to send a position to", () => {
    const noSender = render(stateWith(store, {}, states(false, false)), false);
    expect(noSender).toContain('role="switch"');
    expect(noSender.match(/<button[^>]*disabled=""/g)).toHaveLength(3);

    const withSender = render(stateWith(store, {}, states(false, false)));
    expect(withSender.match(/<button[^>]*disabled=""/g)).toBeNull();
  });
});
