import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import expectedFixture from "../../tests/golden/stage-1-divider.expected.json";
import type { CompileRequest } from "../generated/CompileRequest";
import type { CompileResult } from "../generated/CompileResult";
import type { ComponentCatalog } from "../generated/ComponentCatalog";
import type { ResultLayout } from "../generated/ResultLayout";
import type { ResultSnapshot } from "../generated/ResultSnapshot";
import type { SetParameterRequest } from "../generated/SetParameterRequest";
import type { SetParameterResult } from "../generated/SetParameterResult";
import type { StableId } from "../generated/StableId";
import type { EngineAdapter, FrameHandlers } from "./engineAdapter";
import { dividerSchematic } from "./fixtures";

/**
 * Test double with predetermined responses. It performs no calculation and is
 * never imported by production code: `createEngineAdapter` loads it only for
 * explicit development builds.
 */
export interface PredeterminedResponses {
  readonly catalog: ComponentCatalog;
  readonly compile: (
    request: CompileRequest,
  ) => CompileResult | Promise<CompileResult>;
  readonly setParameter: (
    request: SetParameterRequest,
  ) => SetParameterResult | Promise<SetParameterResult>;
}

export class PredeterminedEngineAdapter implements EngineAdapter {
  readonly compileRequests: CompileRequest[] = [];
  readonly parameterRequests: SetParameterRequest[] = [];
  /** Runs for which live frames were requested, in order. */
  readonly startedRuns: StableId[] = [];
  /** How many times a frame stream was stopped. */
  stopCount = 0;
  private handlers: FrameHandlers | null = null;

  constructor(private readonly responses: PredeterminedResponses) {}

  getCatalog(): Promise<ComponentCatalog> {
    return Promise.resolve(this.responses.catalog);
  }

  startFrames(runId: StableId, handlers: FrameHandlers): Promise<() => void> {
    this.startedRuns.push(runId);
    this.handlers = handlers;

    return Promise.resolve(() => {
      if (this.handlers === handlers) {
        this.handlers = null;
      }
      this.stopCount += 1;
    });
  }

  /** Test hook: delivers a frame as the backend would. No-op when not streaming. */
  emitFrame(snapshot: ResultSnapshot): void {
    this.handlers?.onFrame(snapshot);
  }

  /** Test hook: reports a frame that could not be read. */
  emitFrameError(error: Error): void {
    this.handlers?.onError(error);
  }

  async compile(request: CompileRequest): Promise<CompileResult> {
    this.compileRequests.push(request);
    return this.responses.compile(request);
  }

  async setParameter(
    request: SetParameterRequest,
  ): Promise<SetParameterResult> {
    this.parameterRequests.push(request);
    return this.responses.setParameter(request);
  }
}

/** Thrown when the demo adapter has no recorded answer for the inputs. */
export class PredeterminedResponseMissingError extends Error {
  constructor(r1: number, r2: number) {
    super(`No predetermined response for R1 = ${r1} Ω, R2 = ${r2} Ω.`);
    this.name = "PredeterminedResponseMissingError";
  }
}

const DIVIDER_RUN_ID = "00000000-0000-4000-8000-0000000000a0";

const [resistor1, resistor2] = dividerSchematic.components.filter(
  (component) => component.typeId === "resistor",
);

if (!resistor1 || !resistor2) {
  throw new Error("The divider fixture must contain two resistors.");
}

// Only the quantities recorded in tests/golden are offered. The real layout
// comes from the Rust engine and may carry more slots; the UI must cope.
const dividerLayout: ResultLayout = {
  layoutVersion: 1,
  valueCount: 2,
  slots: [
    {
      key: `${resistor1.id}.voltage`,
      componentId: resistor1.id,
      quantity: "voltage",
      unit: "volt",
      signConvention: "voltagePin1MinusPin2",
      offset: 0,
    },
    {
      key: `${resistor2.id}.voltage`,
      componentId: resistor2.id,
      quantity: "voltage",
      unit: "volt",
      signConvention: "voltagePin1MinusPin2",
      offset: 1,
    },
  ],
};

interface DividerCase {
  readonly r1: number;
  readonly r2: number;
  readonly values: readonly [number, number];
}

const dividerCases: readonly DividerCase[] = [
  {
    r1: 1000,
    r2: 1000,
    values: [
      expectedFixture.equalResistors.resistor1VoltageVolts,
      expectedFixture.equalResistors.resistor2VoltageVolts,
    ],
  },
  {
    r1: 3000,
    r2: 1000,
    values: [
      expectedFixture.resistor1At3000Ohms.resistor1VoltageVolts,
      expectedFixture.resistor1At3000Ohms.resistor2VoltageVolts,
    ],
  },
];

function resistanceOf(
  resistances: ReadonlyMap<StableId, number>,
  id: StableId,
): number {
  return resistances.get(id) ?? Number.NaN;
}

/**
 * A demo adapter for the canonical divider. It knows exactly the fixture
 * cases above, matched by the two resistor values, and refuses anything else.
 */
export function createDividerFixtureAdapter(): PredeterminedEngineAdapter {
  let resistances = new Map<StableId, number>();
  let frameSequence = 0n;

  function respond(
    documentRevision: string,
    next: ReadonlyMap<StableId, number>,
  ): ResultSnapshot {
    const r1 = resistanceOf(next, resistor1.id);
    const r2 = resistanceOf(next, resistor2.id);
    const match = dividerCases.find(
      (candidate) => candidate.r1 === r1 && candidate.r2 === r2,
    );

    if (!match) {
      throw new PredeterminedResponseMissingError(r1, r2);
    }

    resistances = new Map(next);
    const snapshot: ResultSnapshot = {
      metadata: {
        runId: DIVIDER_RUN_ID,
        documentRevision,
        layoutVersion: dividerLayout.layoutVersion,
        simulationStep: 0n,
        simulationTimeSeconds: 0,
        frameSequence,
      },
      values: [...match.values],
    };
    frameSequence += 1n;
    return snapshot;
  }

  return new PredeterminedEngineAdapter({
    catalog: catalogFixture as ComponentCatalog,
    compile: (request) => {
      frameSequence = 0n;
      const next = new Map<StableId, number>();

      for (const component of request.schematic.components) {
        if (component.typeId === "resistor") {
          next.set(component.id, component.parameters.resistanceOhms);
        }
      }

      return {
        status: "success",
        runId: DIVIDER_RUN_ID,
        layout: dividerLayout,
        initialSnapshot: respond(request.documentRevision, next),
        diagnostics: [],
      };
    },
    setParameter: (request) => {
      const next = new Map(resistances);

      if (request.parameterId === "resistanceOhms") {
        next.set(request.componentId, request.value);
      }

      return {
        status: "success",
        snapshot: respond(request.documentRevision, next),
        diagnostics: [],
      };
    },
  });
}
