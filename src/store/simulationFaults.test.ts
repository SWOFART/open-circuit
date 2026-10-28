import { describe, expect, it } from "vitest";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { FaultSpec } from "../generated/FaultSpec";
import { symbols } from "../symbols";
import { dividerSchematic } from "./fixtures";
import {
  PredeterminedEngineAdapter,
  createDividerFixtureAdapter,
} from "./mockEngineAdapter";
import { SimulationController } from "./simulation";

const R2 = "00000000-0000-4000-8000-000000000003";
const fault: FaultSpec = {
  id: "00000000-0000-4000-8000-0000000000a1",
  kind: "open",
  componentId: R2,
};

function controllerFor(adapter: PredeterminedEngineAdapter) {
  let next = 0;
  return new SimulationController(adapter, symbols, () => `rev-${(next += 1)}`);
}

describe("compiling with faults", () => {
  it("sends the faults with the document, and none unless they were set", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    await controller.setFaults([fault]);
    await controller.setFaults(null);

    expect(adapter.compileRequests).toHaveLength(3);
    expect(adapter.compileRequests[0]).not.toHaveProperty("faults");
    expect(adapter.compileRequests[1]?.faults).toEqual([fault]);
    expect(adapter.compileRequests[1]?.schematic).toEqual(dividerSchematic);
    expect(adapter.compileRequests[2]).not.toHaveProperty("faults");
  });

  it("starts the first compile with the faults that were set before it", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.setFaults([fault]);
    await controller.start(dividerSchematic);

    expect(adapter.compileRequests).toHaveLength(1);
    expect(adapter.compileRequests[0]?.faults).toEqual([fault]);
  });

  it("treats the same faults, an empty list, and none as no change", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    await controller.setFaults([]);
    await controller.setFaults(null);
    expect(adapter.compileRequests).toHaveLength(1);

    await controller.setFaults([fault]);
    await controller.setFaults([{ ...fault }]);
    expect(adapter.compileRequests).toHaveLength(2);
  });

  it("gives every compile a new revision, so a result of the other faults is stale", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    const healthy = controller.getState().run?.documentRevision;
    await controller.setFaults([fault]);

    expect(controller.getState().run?.documentRevision).not.toBe(healthy);
    expect(controller.getState().status).toBe("ready");
  });

  it("does not show the result of a compile that was asked for other faults", async () => {
    const releases: (() => void)[] = [];
    const inner = createDividerFixtureAdapter();
    const requests: CompileRequest[] = [];
    const adapter = new PredeterminedEngineAdapter({
      catalog: await inner.getCatalog(),
      compile: async (request): Promise<CompileResult> => {
        requests.push(request);
        // The first compile (without faults) is held until the faults have been set.
        if (requests.length === 1) {
          await new Promise<void>((resolve) => releases.push(resolve));
        }
        return inner.compile(request);
      },
      setParameter: (request) => inner.setParameter(request),
    });
    const controller = controllerFor(adapter);

    const started = controller.start(dividerSchematic);
    await new Promise((resolve) => setTimeout(resolve, 0));
    const faulted = controller.setFaults([fault]);
    releases.forEach((release) => release());
    await Promise.all([started, faulted]);

    // The answer for the unfaulted compile was dropped; the faulted one is on screen.
    expect(requests.map((request) => request.faults ?? null)).toEqual([
      null,
      [fault],
    ]);
    expect(controller.getState().run?.documentRevision).toBe("rev-2");
  });

  it("makes no request after the controller is disposed", async () => {
    const adapter = createDividerFixtureAdapter();
    const controller = controllerFor(adapter);

    await controller.start(dividerSchematic);
    controller.dispose();
    await controller.setFaults([fault]);

    expect(adapter.compileRequests).toHaveLength(1);
  });
});
