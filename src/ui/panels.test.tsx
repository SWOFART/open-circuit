import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import expectedFixture from "../../tests/golden/stage-1-divider.expected.json";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Diagnostic } from "../generated/Diagnostic";
import type { ResultLayout } from "../generated/ResultLayout";
import { componentLabels, dividerSchematic } from "../store/fixtures";
import {
  initialSimulationState,
  type SimulationState,
} from "../store/simulation";
import { Inspector } from "./Inspector";
import { ResultPanel } from "./ResultPanel";

const R1 = "00000000-0000-4000-8000-000000000002";
const R2 = "00000000-0000-4000-8000-000000000003";
const RUN_ID = "00000000-0000-4000-8000-0000000000a0";
const labels = componentLabels(dividerSchematic);

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

function readyState(values: number[]): SimulationState {
  return {
    ...initialSimulationState(dividerSchematic),
    status: "ready",
    catalog: catalogFixture as ComponentCatalog,
    run: { runId: RUN_ID, layout, documentRevision: "rev-1" },
    snapshot: {
      metadata: {
        runId: RUN_ID,
        documentRevision: "rev-1",
        layoutVersion: 1,
        simulationStep: 0n,
        simulationTimeSeconds: 0,
        frameSequence: 0n,
      },
      values,
    },
  };
}

const noCommit = () => {};

describe("ResultPanel", () => {
  it("shows the values the backend reported, formatted for reading", () => {
    const markup = renderToStaticMarkup(
      <ResultPanel
        state={readyState([
          expectedFixture.equalResistors.resistor1VoltageVolts,
          expectedFixture.equalResistors.resistor2VoltageVolts,
        ])}
        labels={labels}
      />,
    );

    expect(markup).toContain("R1 voltage");
    expect(markup).toContain("R2 voltage");
    expect(markup).toContain("2.49988 V");
  });

  it("never renders NaN or Infinity as a reading", () => {
    const markup = renderToStaticMarkup(
      <ResultPanel
        state={readyState([Number.NaN, Number.POSITIVE_INFINITY])}
        labels={labels}
      />,
    );

    expect(markup).not.toContain("NaN");
    expect(markup).not.toContain("Infinity");
    expect(markup).toContain("—");
  });

  it("shows a missing backend as an error with no readings", () => {
    const markup = renderToStaticMarkup(
      <ResultPanel
        state={{
          ...initialSimulationState(dividerSchematic),
          status: "unavailable",
          error: "The simulation backend is not connected yet.",
        }}
        labels={labels}
      />,
    );

    expect(markup).toContain('role="alert"');
    expect(markup).toContain("The simulation backend is not connected yet.");
    expect(markup).not.toContain("<dl");
  });

  it("explains an engine rejection with prose and the affected component", () => {
    const diagnostic: Diagnostic = {
      code: "SingularSystem",
      severity: "error",
      localizationKey: "diagnostic.singularSystem",
      references: [{ kind: "component", componentId: R1 }],
    };
    const markup = renderToStaticMarkup(
      <ResultPanel
        state={{
          ...initialSimulationState(dividerSchematic),
          status: "failed",
          diagnostics: [diagnostic],
        }}
        labels={labels}
      />,
    );

    expect(markup).toContain("The engine could not calculate this circuit.");
    expect(markup).toContain("The circuit cannot be solved as drawn.");
    expect(markup).toContain("(R1)");
    expect(markup).not.toContain("diagnostic.singularSystem");
  });

  it("keeps showing the last result while reporting a failed update", () => {
    const markup = renderToStaticMarkup(
      <ResultPanel
        state={{
          ...readyState([2.5, 2.5]),
          error: "The bridge is down.",
          updating: true,
        }}
        labels={labels}
      />,
    );

    expect(markup).toContain("2.5 V");
    expect(markup).toContain("Update failed: The bridge is down.");
    expect(markup).toContain("Updating");
  });
});

describe("Inspector", () => {
  it("offers both resistors with values shown in convenient units", () => {
    const markup = renderToStaticMarkup(
      <Inspector
        state={readyState([1, 2])}
        labels={labels}
        onCommit={noCommit}
      />,
    );

    expect(markup).toContain("R1 resistance");
    expect(markup).toContain("R2 resistance");
    expect(markup.match(/value="1 kΩ"/g)).toHaveLength(2);
    expect(markup).not.toContain("disabled");
  });

  it("disables editing until the backend has produced a result", () => {
    const markup = renderToStaticMarkup(
      <Inspector
        state={{ ...readyState([1, 2]), status: "unavailable" }}
        labels={labels}
        onCommit={noCommit}
      />,
    );

    expect(markup.match(/disabled/g)).toHaveLength(2);
  });

  it("says so when the catalog could not be loaded", () => {
    const markup = renderToStaticMarkup(
      <Inspector
        state={{
          ...initialSimulationState(dividerSchematic),
          status: "unavailable",
          error: "The Rust engine is only available in the desktop app.",
        }}
        labels={labels}
        onCommit={noCommit}
      />,
    );

    expect(markup).toContain("The component catalog is unavailable.");
    expect(markup).not.toContain("Waiting for the component catalog.");
  });

  it("waits for the catalog before offering any field", () => {
    const markup = renderToStaticMarkup(
      <Inspector
        state={initialSimulationState(dividerSchematic)}
        labels={labels}
        onCommit={noCommit}
      />,
    );

    expect(markup).toContain("Waiting for the component catalog.");
    expect(markup).not.toContain("<input");
  });
});
