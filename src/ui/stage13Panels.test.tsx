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
const RUN = "00000000-0000-4000-8000-0000000000e0";

function drawn(type: ComponentTypeId) {
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => "00000000-0000-4000-8000-0000000000e1",
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

const closed = (id: StableId, conducting: boolean): DeviceState => ({
  kind: "switch",
  componentId: id,
  position: "closed",
  conducting,
});

describe("the residual-current device readout and its operator control", () => {
  const readings = (tripped: number): Reading[] => [
    ["pole1/current", "amp", 0.09],
    ["pole2/current", "amp", 0],
    ["tripped", "dimensionless", tripped],
    ["residual_rms", "amp", 0.031],
    ["trip_dose", "dimensionless", 0.4],
  ];

  it("is a switch button named after the device, with the Engine's state, residual and dose", () => {
    const { document, id } = drawn("rcd_two_pole");
    const html = panel(
      document,
      stateWith(id, readings(0), [closed(id, true)]),
      () => {},
    );

    expect(html).toContain('role="switch"');
    expect(html).toContain('aria-label="FI1 residual-current device"');
    expect(html).toContain('aria-checked="true"');
    expect(html).toContain(">Closed<");
    expect(html).toContain("Residual current (detector RMS)");
    expect(html).toContain("Trip dose");
    expect(html).toContain("Pole L current");
    expect(html).toContain("Pole N current");
    expect(html).not.toContain("Open the device");
  });

  it("says Tripped and how to reset it, from the Engine's tripped flag", () => {
    const { document, id } = drawn("rcd_two_pole");
    const html = panel(
      document,
      stateWith(id, readings(1), [closed(id, false)]),
      () => {},
    );

    expect(html).toContain(">Tripped<");
    expect(html).toContain("device-badge-fault");
    expect(html).toContain("To reset");
    expect(html).toContain("Open the device");
  });

  it("lists the four poles of the four-pole device as L1, L2, L3 and N", () => {
    const { document, id } = drawn("rcd_four_pole");
    const html = panel(
      document,
      stateWith(
        id,
        [
          ["pole1/current", "amp", 1],
          ["pole2/current", "amp", 2],
          ["pole3/current", "amp", 3],
          ["pole4/current", "amp", 4],
          ["tripped", "dimensionless", 0],
          ["residual_rms", "amp", 0],
          ["trip_dose", "dimensionless", 0],
        ],
        [closed(id, true)],
      ),
      () => {},
    );

    for (const pole of ["L1", "L2", "L3", "N"]) {
      expect(html).toContain(`Pole ${pole} current`);
    }
  });

  it("shows a dash for a reading the layout does not have, and no control without a run", () => {
    const { document, id } = drawn("rcd_two_pole");
    const empty = panel(
      document,
      stateWith(id, [], [closed(id, true)]),
      () => {},
    );
    expect(empty).toContain("—");

    const loading = panel(document, stateWith(id, [], [], "loading"), () => {});
    expect(loading).toContain("Waiting for the engine");
    expect(loading).not.toContain('role="switch"');
  });
});

describe("the exposed part readout", () => {
  const touch = (valid: number, hazard: number, rms = 45): Reading[] => [
    ["touch_voltage_rms", "volt", rms],
    ["touch_voltage_peak", "volt", rms * 1.41],
    ["hazard", "dimensionless", hazard],
    ["valid", "dimensionless", valid],
  ];

  it("shows the touch voltage, the limit and a Hazard badge when the Engine says so", () => {
    const { document, id } = drawn("exposed_part");
    const html = panel(document, stateWith(id, touch(1, 1)));

    expect(html).toContain(">Hazard<");
    expect(html).toContain("device-badge-fault");
    expect(html).toContain("Touch voltage (RMS)");
    expect(html).toContain("Touch voltage (peak)");
    expect(html).toContain("Limit");
    expect(html).toContain("50");
  });

  it("says the part is below the limit when the reading is valid and not hazardous", () => {
    const { document, id } = drawn("exposed_part");
    const html = panel(document, stateWith(id, touch(1, 0, 12)));

    expect(html).toContain(">Below the limit<");
    expect(html).toContain("device-badge-on");
    expect(html).not.toContain(">Hazard<");
  });

  it("shows no reading and no hazard while the Engine says it is not valid", () => {
    const { document, id } = drawn("exposed_part");
    // Even a stale non-zero value or hazard flag is not shown while valid is 0.
    const html = panel(document, stateWith(id, touch(0, 1, 99)));

    expect(html).toContain(">Not yet valid<");
    expect(html).not.toContain(">Hazard<");
    expect(html).not.toContain("99");
  });

  it("always says that the value is a simulation for teaching", () => {
    const { document, id } = drawn("exposed_part");

    for (const readings of [touch(1, 1), touch(1, 0), touch(0, 0)]) {
      expect(panel(document, stateWith(id, readings))).toContain(
        "A simulation for teaching, not a safety statement",
      );
    }
  });

  it("takes the limit from the part, not from the interface", () => {
    const { document, id } = drawn("exposed_part");
    const edited = new SchematicDocumentStore(
      {
        ...document.schematic,
        components: document.schematic.components.map((component) =>
          component.typeId === "exposed_part"
            ? {
                ...component,
                parameters: {
                  ...component.parameters,
                  touchVoltageLimitVolts: 12,
                },
              }
            : component,
        ),
      },
      () => "00000000-0000-4000-8000-0000000000e2",
    ).getState();

    expect(panel(edited, stateWith(id, touch(1, 1)))).toContain("12");
  });
});

describe("the breaker with a trip class", () => {
  it("shows its thermal dose, and a breaker without a class does not", () => {
    const { document, id } = drawn("breaker_three_pole");
    const base: Reading[] = [
      ["pole1/current", "amp", 1],
      ["pole2/current", "amp", 1],
      ["pole3/current", "amp", 1],
      ["tripped", "dimensionless", 0],
    ];

    expect(
      panel(document, stateWith(id, base, [closed(id, true)])),
    ).not.toContain("Thermal dose");
    expect(
      panel(
        document,
        stateWith(
          id,
          [...base, ["thermal_dose", "dimensionless", 0.5]],
          [closed(id, true)],
        ),
      ),
    ).toContain("Thermal dose");
  });
});

describe("the earth electrode", () => {
  it("has no readout of its own: it carries no result", () => {
    const { document, id } = drawn("earth_electrode");

    expect(panel(document, stateWith(id, []))).toBe("");
  });
});
