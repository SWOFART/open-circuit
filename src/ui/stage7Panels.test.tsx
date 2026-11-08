import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { Diagnostic } from "../generated/Diagnostic";
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
import { Oscilloscope } from "./Oscilloscope";
import { Palette } from "./Palette";
import { ResultPanel } from "./ResultPanel";
import { describeRange, evaluateParameterDraft, formatSi } from "./units";

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
  overrides: Partial<SimulationState> = {},
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
        simulationStep: 7n,
        simulationTimeSeconds: 0.007,
        frameSequence: 5n,
      },
      values: valuesFor(layout, values),
      states: [],
    },
    ...overrides,
  };
}

const nonConvergence: Diagnostic = {
  code: "NonConvergence",
  severity: "error",
  localizationKey: "diagnostic.nonConvergence",
  references: [],
};

describe("the Stage 7 palette and names", () => {
  it("offers the diode and both transistors from the Engine catalog, in catalog order", () => {
    const markup = renderToStaticMarkup(
      <Palette
        catalog={catalog}
        tool={{ kind: "select" }}
        onSelectTool={() => {}}
      />,
    );

    for (const label of ["Diode", "NPN transistor", "PNP transistor"]) {
      expect(markup).toContain(`>${label}<`);
    }
    expect(markup.indexOf(">Diode<")).toBeLessThan(
      markup.indexOf(">NPN transistor<"),
    );
  });

  it("names diodes D and transistors T, counting each prefix on its own", () => {
    const { store, ids } = drawn(["diode", "bjt_npn", "bjt_pnp", "diode"]);
    const labels = store.getState().labels;

    expect(ids.map((id) => labels.get(id))).toEqual(["D1", "T1", "T2", "D2"]);
  });

  it("starts each new device from the catalog defaults and nothing else", () => {
    const { store } = drawn(["diode", "bjt_npn", "bjt_pnp"]);

    expect(
      store.getState().schematic.components.map((item) => item.parameters),
    ).toEqual([
      { saturationCurrentAmps: 1e-14, emissionCoefficient: 1 },
      { saturationCurrentAmps: 1e-14, forwardBeta: 100, reverseBeta: 1 },
      { saturationCurrentAmps: 1e-14, forwardBeta: 100, reverseBeta: 1 },
    ]);
  });
});

describe("the inspector for the Stage 7 devices", () => {
  const { store } = drawn(["diode", "bjt_npn"]);
  const html = renderToStaticMarkup(
    <Inspector
      state={stateWith(store, {})}
      schematic={store.getState().schematic}
      labels={store.getState().labels}
      onCommit={() => {}}
    />,
  );

  it("shows every parameter with its name, unit, and catalog default", () => {
    const expected: [string, string][] = [
      ["D1 saturation current", "10 fA"],
      ["D1 emission coefficient", "1"],
      ["T1 saturation current", "10 fA"],
      ["T1 forward beta", "100"],
      ["T1 reverse beta", "1"],
    ];

    for (const [label, value] of expected) {
      expect(html).toContain(`>${label}</label>`);
      const field = html.slice(html.indexOf(`>${label}</label>`));
      expect(field.slice(0, 400)).toContain(`value="${value}"`);
    }
  });

  it("states the allowed range the catalog gives, in the parameter's own unit", () => {
    expect(html).toContain("Allowed range: 0.001 fA to 1 nA.");
    expect(html).toContain("Allowed range: 1 to 2.");
    expect(html).toContain("Allowed range: 1 to 1000.");
    expect(html).toContain("Allowed range: 0.01 to 100.");
  });

  it("reads the catalog's bounds, not numbers of its own", () => {
    const saturation = catalog.componentTypes
      .find((type) => type.typeId === "diode")!
      .parameters.find(
        (parameter) => parameter.id === "saturationCurrentAmps",
      )!;

    expect(describeRange(saturation)).toBe("0.001 fA to 1 nA");
    expect(describeRange({ ...saturation, maximumValue: undefined })).toBe(
      "at least 0.001 fA",
    );
    expect(describeRange({ ...saturation, minimumValue: undefined })).toBe(
      "at most 1 nA",
    );
    expect(
      describeRange({
        ...saturation,
        minimumValue: undefined,
        maximumValue: undefined,
      }),
    ).toBeNull();
  });
});

describe("typing Stage 7 values", () => {
  const saturation = catalog.componentTypes
    .find((type) => type.typeId === "diode")!
    .parameters.find((parameter) => parameter.id === "saturationCurrentAmps")!;
  const beta = catalog.componentTypes
    .find((type) => type.typeId === "bjt_npn")!
    .parameters.find((parameter) => parameter.id === "forwardBeta")!;

  it.each([
    ["20 fA", 2e-14],
    ["20f", 2e-14],
    ["2.5 pA", 2.5e-12],
    ["1e-15", 1e-15],
    ["1 nA", 1e-9],
  ])("converts %s to SI amperes", (text, value) => {
    expect(evaluateParameterDraft(text, saturation, 1e-14)).toEqual({
      state: "commit",
      value,
    });
  });

  it("refuses a value outside the finite range, with the range in the message", () => {
    for (const text of ["0.0005 fA", "2 nA", "1"]) {
      expect(evaluateParameterDraft(text, saturation, 1e-14)).toEqual({
        state: "invalid",
        reason: "range",
      });
    }
    for (const text of ["0.5", "1001"]) {
      expect(evaluateParameterDraft(text, beta, 100)).toEqual({
        state: "invalid",
        reason: "range",
      });
    }
  });

  it("still tells a negative or non-numeric entry from an out-of-range one", () => {
    expect(evaluateParameterDraft("abc", beta, 100)).toMatchObject({
      state: "invalid",
      reason: "format",
    });
    expect(evaluateParameterDraft("-5", beta, 100)).toMatchObject({
      state: "invalid",
    });
  });

  it("shows a ratio as a plain number", () => {
    expect(formatSi(100, "dimensionless")).toBe("100");
    expect(formatSi(0.01, "dimensionless")).toBe("0.01");
  });
});

describe("the device panel for the Stage 7 devices", () => {
  const { store, ids } = drawn(["diode", "bjt_npn", "bjt_pnp"]);
  const [diode, npn] = ids as [string, string, string];
  const { schematic, labels } = store.getState();
  const state = stateWith(store, {
    [keyOf(schematic, diode, "voltage")]: 0.7,
    [keyOf(schematic, diode, "current")]: 0.012,
    [keyOf(schematic, npn, "base_emitter/voltage")]: 0.65,
    [keyOf(schematic, npn, "collector_emitter/voltage")]: 3.1,
    [keyOf(schematic, npn, "collector/current")]: 0.002,
    [keyOf(schematic, npn, "base/current")]: 0.00002,
    [keyOf(schematic, npn, "emitter/current")]: -0.00202,
  });
  const html = renderToStaticMarkup(
    <DevicePanel state={state} schematic={schematic} labels={labels} />,
  );

  it("shows the diode's voltage and current with their direction in the name", () => {
    expect(html).toContain("D1 Diode");
    expect(html).toContain("Voltage (anode − cathode)</dt><dd>700 mV");
    expect(html).toContain("Current (into the anode)</dt><dd>12 mA");
  });

  it("shows the transistor's five quantities, each named by its terminals", () => {
    expect(html).toContain("T1 NPN transistor");
    expect(html).toContain("Base-emitter voltage</dt><dd>650 mV");
    expect(html).toContain("Collector-emitter voltage</dt><dd>3.1 V");
    expect(html).toContain(
      "Collector current (into the collector)</dt><dd>2 mA",
    );
    expect(html).toContain("Base current (into the base)</dt><dd>20 µA");
    expect(html).toContain(
      "Emitter current (into the emitter)</dt><dd>-2.02 mA",
    );
    expect(html).toContain("T2 PNP transistor");
  });

  it("shows no operating region and no state the interface made up", () => {
    expect(html).not.toMatch(
      /region|saturat|cut-?off|active|forward bias|reverse bias|conducting/i,
    );
  });

  it("shows no reading before the Engine has answered", () => {
    const loading = renderToStaticMarkup(
      <DevicePanel
        state={{ ...initialSimulationState(), status: "loading" }}
        schematic={schematic}
        labels={labels}
      />,
    );

    expect(loading).not.toContain("700 mV");
    expect(loading).not.toContain("<dd>");
  });
});

describe("the result panel on a nonlinear failure", () => {
  const { store } = drawn(["diode"]);

  it("says a compile that failed has no result and shows its diagnostics", () => {
    const html = renderToStaticMarkup(
      <ResultPanel
        state={{
          ...initialSimulationState(),
          status: "failed",
          catalog,
          diagnostics: [nonConvergence],
        }}
        labels={store.getState().labels}
      />,
    );

    expect(html).toContain("could not calculate this circuit");
    expect(html).toContain("There is no result");
    expect(html).toContain("The nonlinear circuit did not converge.");
    expect(html).not.toContain("Simulation time");
    expect(html).not.toContain("stopped at step");
    expect(html).not.toContain("refused the last change");
  });

  it("says a refused live edit was not applied and the earlier result stands", () => {
    const html = renderToStaticMarkup(
      <ResultPanel
        state={stateWith(
          store,
          {},
          { refusedUpdate: true, diagnostics: [nonConvergence] },
        )}
        labels={store.getState().labels}
      />,
    );

    expect(html).toContain("refused the last change and did not apply it");
    expect(html).toContain("The nonlinear circuit did not converge.");
    expect(html).toContain("Simulation time 0.007 s");
    expect(html).not.toContain("stopped at step");
    expect(html).not.toContain("There is no result");
  });

  it("says a stopped run keeps the values of the last step before the stop", () => {
    const html = renderToStaticMarkup(
      <ResultPanel
        state={stateWith(
          store,
          {},
          {
            stopped: {
              kind: "runStopped",
              runId: RUN,
              documentRevision: "rev-1",
              simulationStep: 7,
              diagnostics: [nonConvergence],
            } as SimulationState["stopped"],
            diagnostics: [nonConvergence],
          },
        )}
        labels={store.getState().labels}
      />,
    );

    expect(html).toContain("stopped at step 7");
    expect(html).toContain(
      "values shown are from the last step before the stop",
    );
    expect(html).toContain("readings-unsettled");
    expect(html).not.toContain("refused the last change");
    expect(html).not.toContain("There is no result");
  });

  it("marks the values as unsettled while an update is under way", () => {
    const html = renderToStaticMarkup(
      <ResultPanel
        state={stateWith(store, {}, { updating: true })}
        labels={store.getState().labels}
      />,
    );

    expect(html).toContain('aria-busy="true"');
    expect(html).toContain("readings-unsettled");
  });

  it("shows settled values plainly", () => {
    const html = renderToStaticMarkup(
      <ResultPanel
        state={stateWith(store, {})}
        labels={store.getState().labels}
      />,
    );

    expect(html).toContain('aria-busy="false"');
    expect(html).not.toContain("readings-unsettled");
  });
});

describe("the oscilloscope for the Stage 7 devices", () => {
  it("offers the transistor's five slots by terminal and the diode's two", () => {
    const { store } = drawn(["diode", "bjt_npn"]);
    const html = renderToStaticMarkup(
      <Oscilloscope
        simulation={stateWith(store, {})}
        scope={{
          selectedKeys: [],
          windowSeconds: 0.1,
          data: null,
          loading: false,
          error: null,
        }}
        labels={store.getState().labels}
        onToggleSignal={() => {}}
        onWindowChange={() => {}}
      />,
    );

    for (const text of [
      "D1 voltage",
      "D1 current",
      "T1 base-emitter voltage",
      "T1 collector-emitter voltage",
      "T1 collector current",
      "T1 base current",
      "T1 emitter current",
    ]) {
      expect(html).toContain(text);
    }
  });
});
