import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import fixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { ResultLayout } from "../generated/ResultLayout";
import { SchematicDocumentStore } from "../store/document";
import { PredeterminedEngineAdapter } from "../store/mockEngineAdapter";
import { blankSchematic } from "../store/projectWorkflow";
import {
  initialSimulationState,
  SimulationController,
} from "../store/simulation";
import { initialScopeState } from "../store/traces";
import { symbols } from "../symbols";
import { DevicePanel } from "./DevicePanel";
import { Inspector } from "./Inspector";
import { Oscilloscope } from "./Oscilloscope";
import { ResultPanel } from "./ResultPanel";
import { searchKeyAction } from "./drawer/drawerModel";
import { searchParts } from "./drawer/partSearch";
import { applyEditorCommand } from "./editorCommands";
import { evaluateParameterDraft } from "./units";

const catalog = fixture as ComponentCatalog;
const meter = "00000000-0000-4000-8000-000000000041";
const runA = "00000000-0000-4000-8000-0000000000a1";
const runB = "00000000-0000-4000-8000-0000000000b2";

function drawn(...types: ComponentTypeId[]) {
  let id = 0x41;
  const store = new SchematicDocumentStore(
    blankSchematic,
    () => `00000000-0000-4000-8000-0000000000${(id++).toString(16)}`,
  );
  for (const [index, typeId] of types.entries()) {
    applyEditorCommand(
      store,
      {
        kind: "placeComponent",
        typeId,
        position: { x: index * 6, y: 0 },
        rotationDeg: 0,
      },
      catalog,
    );
  }
  return store;
}

const layout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 4,
  slots: [
    {
      key: `${meter}/rms_voltage`,
      componentId: meter,
      quantity: "rmsVoltage",
      unit: "volt",
      signConvention: "unsigned",
      offset: 0,
    },
    {
      key: `${meter}/frequency`,
      componentId: meter,
      quantity: "frequency",
      unit: "hertz",
      signConvention: "unsigned",
      offset: 1,
    },
    {
      key: `${meter}/frequency_valid`,
      componentId: meter,
      quantity: "valid",
      unit: "dimensionless",
      signConvention: "flag",
      offset: 2,
    },
    {
      key: `${meter}/valid`,
      componentId: meter,
      quantity: "valid",
      unit: "dimensionless",
      signConvention: "flag",
      offset: 3,
    },
  ],
};

function result(
  runId: string,
  documentRevision: string,
  values: number[],
): Extract<CompileResult, { status: "success" }> {
  return {
    status: "success",
    internalSubsteps: 4,
    runId,
    layout,
    diagnostics: [],
    initialSnapshot: {
      metadata: {
        runId,
        documentRevision,
        simulationStep: 0n,
        simulationTimeSeconds: 0,
        frameSequence: 0n,
        layoutVersion: 1,
      },
      values,
      states: [],
    },
  };
}

describe("Stage 9 editor", () => {
  it.each([
    [
      "ac_voltage_source",
      [
        ["pin1", -1, 0],
        ["pin2", 1, 0],
      ],
    ],
    [
      "three_phase_source_star",
      [
        ["l1", 2, -2],
        ["l2", 2, 0],
        ["l3", 2, 2],
        ["n", -2, 0],
      ],
    ],
    [
      "three_phase_source_delta",
      [
        ["l1", 2, -2],
        ["l2", 2, 0],
        ["l3", 2, 2],
      ],
    ],
    [
      "ac_voltmeter",
      [
        ["pin1", -2, 0],
        ["pin2", 2, 0],
      ],
    ],
    [
      "ac_ammeter",
      [
        ["pin1", -2, 0],
        ["pin2", 2, 0],
      ],
    ],
    [
      "wattmeter",
      [
        ["v1", -2, -1],
        ["v2", 2, -1],
        ["i1", -2, 1],
        ["i2", 2, 1],
      ],
    ],
  ] as const)("uses the contracted pins for %s", (typeId, expected) => {
    expect(
      symbols[typeId]?.pins.map(({ id, local }) => [id, local.x, local.y]),
    ).toEqual(expected);
  });

  it.each([
    ["voltageRmsVolts", "0.4 kV", 400, "101 kV"],
    ["frequencyHz", "0.06 kHz", 60, "401 Hz"],
    ["phaseOffsetDegrees", "-90 degrees", -90, "361 °"],
  ] as const)(
    "uses the catalog units and ranges for %s",
    (id, input, value, outOfRange) => {
      const definition = catalog.componentTypes
        .find((part) => part.typeId === "ac_voltage_source")!
        .parameters.find((parameter) => parameter.id === id)!;
      expect(
        evaluateParameterDraft(input, definition, definition.defaultValue),
      ).toEqual({ state: "commit", value });
      expect(
        evaluateParameterDraft(outOfRange, definition, definition.defaultValue),
      ).toEqual({ state: "invalid", reason: "range" });
    },
  );

  it("places a wattmeter from keyboard search and exposes labeled inspector inputs", () => {
    const query = "wattmeter";
    const results = searchParts(catalog.componentTypes, query);
    const action = searchKeyAction({
      key: "Enter",
      shiftKey: true,
      highlight: -1,
      count: results.length,
      query,
    });
    expect(action).toEqual({ kind: "choose", index: 0, placeAtCenter: true });
    const word = action.kind === "choose" ? results[action.index] : undefined;
    expect(word?.typeId).toBe("wattmeter");
    const { schematic, labels } = drawn(
      word!.typeId,
      "three_phase_source_star",
    ).getState();
    const html = renderToStaticMarkup(
      <Inspector
        state={{ ...initialSimulationState(), catalog }}
        schematic={schematic}
        labels={labels}
        onCommit={() => {}}
      />,
    );
    expect(html).toContain("PW1 measurement frequency");
    expect(html).toContain("50 Hz");
    expect(html).toContain("G1 reversed phase sequence");
    expect(html).toContain(
      '<option value="standard" selected="">Standard</option>',
    );
    expect(html).toContain('<option value="reversed">Reversed</option>');
    expect(html).toContain("Allowed range");
  });

  it("shows readings only when the Engine's matching flag is set", () => {
    const { schematic, labels } = drawn("ac_voltmeter").getState();
    function render(values: number[]) {
      const state = {
        ...initialSimulationState(),
        status: "ready" as const,
        run: { runId: runA, layout, documentRevision: "rev-1" },
        snapshot: result(runA, "rev-1", values).initialSnapshot,
      };
      return renderToStaticMarkup(
        <>
          <DevicePanel state={state} schematic={schematic} labels={labels} />
          <ResultPanel state={state} labels={labels} />
        </>,
      );
    }
    const waiting = render([230, 50, 0, 0]);
    expect(waiting.match(/Not yet valid/g)).toHaveLength(4);
    expect(waiting).not.toContain("230 V");
    const rmsReady = render([230, 50, 0, 1]);
    expect(rmsReady.match(/230 V/g)).toHaveLength(2);
    expect(rmsReady).not.toContain("50 Hz");
    expect(render([230, 50, 1, 1]).match(/50 Hz/g)).toHaveLength(2);
  });

  it("formats the signed wattmeter readings directly from the layout", () => {
    const { schematic, labels } = drawn("wattmeter").getState();
    const wattLayout: ResultLayout = {
      layoutVersion: 1,
      valueCount: 4,
      slots: [
        {
          key: `${meter}/real_power`,
          componentId: meter,
          quantity: "realPower",
          unit: "watt",
          signConvention: "loadConvention",
          offset: 0,
        },
        {
          key: `${meter}/reactive_power`,
          componentId: meter,
          quantity: "reactivePower",
          unit: "volt_ampere_reactive",
          signConvention: "loadConvention",
          offset: 1,
        },
        {
          key: `${meter}/phase_angle`,
          componentId: meter,
          quantity: "phaseAngle",
          unit: "degree",
          signConvention: "loadConvention",
          offset: 2,
        },
        layout.slots[3]!,
      ],
    };
    const html = renderToStaticMarkup(
      <DevicePanel
        state={{
          ...initialSimulationState(),
          status: "ready",
          run: { runId: runA, layout: wattLayout, documentRevision: "rev-1" },
          snapshot: result(runA, "rev-1", [-12, 5, -30, 1]).initialSnapshot,
        }}
        schematic={schematic}
        labels={labels}
      />,
    );
    expect(html).toContain("PW1 Wattmeter");
    expect(html).toContain("-12 W");
    expect(html).toContain("5 var");
    expect(html).toContain("-30°");
  });

  it("offers the AC trace and a labeled window with several 50 Hz periods", () => {
    const { labels } = drawn("ac_voltmeter").getState();
    const html = renderToStaticMarkup(
      <Oscilloscope
        simulation={{
          ...initialSimulationState(),
          status: "ready",
          run: { runId: runA, layout, documentRevision: "rev-1" },
        }}
        scope={{
          ...initialScopeState(),
          selectedKeys: [`${meter}/rms_voltage`],
        }}
        labels={labels}
        onToggleSignal={() => {}}
        onWindowChange={() => {}}
      />,
    );
    expect(html).toContain("PV1 RMS voltage");
    expect(html).toContain('type="checkbox" checked=""');
    expect(html).toContain("Time window");
    expect(html).toContain('<option value="0.1" selected="">100 ms</option>');
  });

  it("discards a delayed compile and frame when the AC drawing has been replaced", async () => {
    const first = drawn("ac_voltmeter", "ac_voltage_source").getState()
      .schematic;
    const second = drawn("ac_voltmeter", "three_phase_source_star").getState()
      .schematic;
    const pending: {
      request: CompileRequest;
      resolve: (value: CompileResult) => void;
    }[] = [];
    const adapter = new PredeterminedEngineAdapter({
      catalog,
      compile: (request) =>
        new Promise((resolve) => pending.push({ request, resolve })),
      setParameter: () => {
        throw new Error("no parameter update expected");
      },
    });
    let revision = 0;
    const controller = new SimulationController(
      adapter,
      symbols,
      () => `rev-${++revision}`,
    );
    const started = controller.start(first);
    await vi.waitFor(() => expect(pending).toHaveLength(1));
    const updated = controller.syncDocument(second);
    const old = result(
      runA,
      pending[0]!.request.documentRevision,
      [230, 50, 1, 1],
    );
    pending[0]!.resolve(old);
    await vi.waitFor(() => expect(pending).toHaveLength(2));
    expect(controller.getState().snapshot).toBeNull();
    expect(pending[1]!.request.schematic).toEqual(second);
    pending[1]!.resolve(
      result(runB, pending[1]!.request.documentRevision, [0, 0, 0, 0]),
    );
    await Promise.all([started, updated]);
    adapter.emitFrame(old.initialSnapshot);
    expect(controller.getState().run?.runId).toBe(runB);
    expect(controller.getState().snapshot?.values).toEqual([0, 0, 0, 0]);
    controller.dispose();
  });
});
