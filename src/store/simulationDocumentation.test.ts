import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { Schematic } from "../generated/Schematic";
import { dividerSchematic } from "./fixtures";
import { PredeterminedEngineAdapter } from "./mockEngineAdapter";
import { SimulationController, hasSameElectricalPart } from "./simulation";
import { symbols } from "../symbols";

const R1 = "00000000-0000-4000-8000-000000000002";
const RUN_ID = "00000000-0000-4000-8000-0000000000a0";

function adapter(compiled: CompileRequest[]): PredeterminedEngineAdapter {
  return new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request) => {
      compiled.push(request);
      return {
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
      };
    },
    setParameter: (request) => ({
      status: "success",
      internalSubsteps: 1,
      snapshot: {
        metadata: {
          runId: RUN_ID,
          documentRevision: request.documentRevision,
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
}

function documented(schematic: Schematic, text: string): Schematic {
  return {
    ...schematic,
    documentation: {
      ...schematic.documentation,
      notes: [
        {
          id: "00000000-0000-4000-8000-000000001201",
          position: { x: 0, y: 0 },
          text,
        },
      ],
    },
  };
}

function controllerFor(compiled: CompileRequest[]): SimulationController {
  let next = 0;
  return new SimulationController(
    adapter(compiled),
    symbols,
    () => `revision-${(next += 1)}`,
  );
}

describe("documentation edits and the run", () => {
  it("recognizes the electrical part by identity of every circuit list", () => {
    expect(
      hasSameElectricalPart(
        dividerSchematic,
        documented(dividerSchematic, "x"),
      ),
    ).toBe(true);
    expect(
      hasSameElectricalPart(dividerSchematic, {
        ...dividerSchematic,
        wires: [...dividerSchematic.wires],
      }),
    ).toBe(false);
    expect(
      hasSameElectricalPart(dividerSchematic, {
        ...dividerSchematic,
        rails: [...dividerSchematic.rails],
      }),
    ).toBe(false);
  });

  it("sends the Engine nothing for a documentation-only edit and keeps the run", async () => {
    const compiled: CompileRequest[] = [];
    const controller = controllerFor(compiled);
    await controller.start(dividerSchematic);
    expect(compiled).toHaveLength(1);
    const before = controller.getState();

    await controller.syncDocument(documented(dividerSchematic, "one"));
    await controller.syncDocument(documented(dividerSchematic, "two"));

    expect(compiled).toHaveLength(1);
    expect(controller.getState().run).toBe(before.run);
    expect(controller.getState().snapshot).toBe(before.snapshot);
    expect(controller.getState().status).toBe("ready");
  });

  it("still recompiles for an electrical edit, and a later parameter edit stays lightweight", async () => {
    const compiled: CompileRequest[] = [];
    const controller = controllerFor(compiled);
    await controller.start(dividerSchematic);
    await controller.syncDocument(documented(dividerSchematic, "one"));
    expect(compiled).toHaveLength(1);

    const rewired: Schematic = {
      ...dividerSchematic,
      wires: dividerSchematic.wires.slice(0, -1),
    };
    await controller.syncDocument(documented(rewired, "one"));
    expect(compiled).toHaveLength(2);

    // The compile sent to the Engine is the electrical document, as drawn.
    expect(compiled[1]?.schematic.wires).toEqual(rewired.wires);

    const edited: Schematic = {
      ...rewired,
      components: rewired.components.map((component) =>
        component.id === R1 && component.typeId === "resistor"
          ? { ...component, parameters: { resistanceOhms: 2200 } }
          : component,
      ),
    };
    await controller.syncDocument(edited, {
      componentId: R1,
      parameterId: "resistanceOhms",
      value: 2200,
    });
    expect(compiled).toHaveLength(2);
  });
});
