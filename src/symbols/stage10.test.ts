import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import { resolvePinPositions, type Rotation } from "../editor/geometry";
import { hitTest } from "../editor/hitTest";
import { buildScene } from "../editor/scene";
import {
  applyComponentTransform,
  followingWireEdits,
} from "../editor/transform";
import type { ComponentSpec } from "../generated/ComponentSpec";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import type { Schematic } from "../generated/Schematic";
import { symbols, type LocalPoint } from ".";

// The Stage 10 power equipment (docs/contracts/stage-10-three-phase-equipment.md section 2). The
// IDs and the count of the pins against the Rust catalog are checked for every symbol by
// contract.test.ts; this file checks the pin groups, the drawing, hit testing, rotation and
// the wires that follow a moved part.

const STAGE_10 = [
  "contactor_three_pole",
  "breaker_three_pole",
  "thermal_overload_three_pole",
  "induction_motor_three_phase",
  "transformer_three_phase",
  "three_phase_load_star",
  "three_phase_load_delta",
] as const satisfies readonly ComponentTypeId[];

const ID = "00000000-0000-4000-8000-0000000000c1";

function part(
  typeId: ComponentTypeId,
  rotationDeg: Rotation = 0,
): ComponentSpec {
  return {
    typeId,
    id: ID,
    position: { x: 10, y: 10 },
    rotationDeg,
    parameters: {},
  } as unknown as ComponentSpec;
}

function alone(typeId: ComponentTypeId, rotationDeg: Rotation = 0): Schematic {
  return {
    schemaVersion: 2,
    components: [part(typeId, rotationDeg)],
    wires: [],
    junctions: [],
    referenceMarkers: [],
    rails: [],
  };
}

const catalogEntry = (typeId: string) =>
  catalogFixture.componentTypes.find((entry) => entry.typeId === typeId)!;

describe("the Stage 10 symbols", () => {
  it.each(STAGE_10)("exists for %s", (typeId) => {
    expect(symbols[typeId]).toBeDefined();
  });

  it.each(STAGE_10)(
    "puts the two pins of every contracted pin group of %s on different grid points",
    (typeId) => {
      const symbol = symbols[typeId]!;
      const at = new Map<string, LocalPoint>(
        symbol.pins.map((pin) => [pin.id, pin.local]),
      );
      const groups = (
        catalogEntry(typeId) as {
          pinGroups?: { id: string; pins: [string, string] }[];
        }
      ).pinGroups;

      expect(groups?.length).toBeGreaterThan(0);
      for (const group of groups ?? []) {
        const [first, second] = group.pins.map((pin) => at.get(pin));

        expect(first, `${typeId} ${group.id}`).toBeDefined();
        expect(second, `${typeId} ${group.id}`).toBeDefined();
        expect(first).not.toEqual(second);
      }
    },
  );

  it("draws the poles of the three-pole devices as vertical pairs at equal spacing", () => {
    for (const typeId of [
      "contactor_three_pole",
      "breaker_three_pole",
      "thermal_overload_three_pole",
    ] as const) {
      const at = new Map<string, LocalPoint>(
        symbols[typeId]!.pins.map((pin) => [pin.id, pin.local]),
      );
      const xs = [1, 2, 3].map((pole) => {
        const top = at.get(`l${pole}`)!;
        const bottom = at.get(`t${pole}`)!;

        expect(top.x).toBe(bottom.x);
        expect(top.y).toBeLessThan(bottom.y);
        return top.x;
      });

      expect(xs[1]! - xs[0]!).toBe(xs[2]! - xs[1]!);
    }
  });

  it("keeps the motor terminals as on a terminal board and the transformer banks side by side", () => {
    const motor = new Map<string, LocalPoint>(
      symbols.induction_motor_three_phase!.pins.map((pin) => [
        pin.id,
        pin.local,
      ]),
    );
    expect(["u1", "v1", "w1"].map((id) => motor.get(id)!.y)).toEqual([
      -3, -3, -3,
    ]);
    expect(["u2", "v2", "w2"].map((id) => motor.get(id)!.y)).toEqual([3, 3, 3]);

    const transformer = new Map<string, LocalPoint>(
      symbols.transformer_three_phase!.pins.map((pin) => [pin.id, pin.local]),
    );
    for (const bank of [1, 2, 3]) {
      const a = transformer.get(`p${bank}a`)!;
      const b = transformer.get(`p${bank}b`)!;
      const c = transformer.get(`s${bank}a`)!;

      expect(a.y).toBeLessThan(c.y);
      expect(a.x).toBe(c.x);
      expect(b.x - a.x).toBe(2);
    }
  });

  it("draws every primitive with finite coordinates", () => {
    for (const typeId of STAGE_10) {
      for (const primitive of symbols[typeId]!.primitives) {
        const numbers =
          primitive.kind === "polyline"
            ? primitive.points.flatMap((point) => [point.x, point.y])
            : [primitive.center.x, primitive.center.y, primitive.radius];

        expect(numbers.every(Number.isFinite), typeId).toBe(true);
      }
    }
  });
});

describe("the drawing follows the state the Engine reported", () => {
  const stateOf = (state: Record<string, unknown>) =>
    state as unknown as DeviceState;
  const draw = (typeId: ComponentTypeId, state?: DeviceState) =>
    JSON.stringify(symbols[typeId]!.drawn?.(state));

  it("draws the same thing at rest as with no state", () => {
    for (const typeId of [
      "contactor_three_pole",
      "breaker_three_pole",
      "thermal_overload_three_pole",
    ] as const) {
      expect(symbols[typeId]!.drawn?.(undefined)).toEqual(
        symbols[typeId]!.primitives,
      );
    }
  });

  it("closes the contactor poles and the normally open contact when it is picked", () => {
    const released = draw(
      "contactor_three_pole",
      stateOf({ kind: "contactor", componentId: ID, state: "released" }),
    );
    const picked = draw(
      "contactor_three_pole",
      stateOf({ kind: "contactor", componentId: ID, state: "picked" }),
    );

    expect(picked).not.toBe(released);
    expect(released).toBe(draw("contactor_three_pole"));
  });

  it("draws a tripped breaker open although its operator is closed", () => {
    const closed = (conducting: boolean) =>
      draw(
        "breaker_three_pole",
        stateOf({
          kind: "switch",
          componentId: ID,
          position: "closed",
          conducting,
        }),
      );

    expect(closed(true)).not.toBe(closed(false));
    expect(closed(false)).toBe(
      draw(
        "breaker_three_pole",
        stateOf({
          kind: "switch",
          componentId: ID,
          position: "open",
          conducting: false,
        }),
      ),
    );
  });

  it("changes the overload relay's auxiliary contacts when it trips", () => {
    const state = (tripped: boolean) =>
      draw(
        "thermal_overload_three_pole",
        stateOf({
          kind: "fuse",
          componentId: ID,
          state: tripped ? "tripped" : "intact",
        }),
      );

    expect(state(true)).not.toBe(state(false));
    expect(state(false)).toBe(draw("thermal_overload_three_pole"));
  });
});

describe("selecting, rotating and moving the Stage 10 parts", () => {
  it.each(STAGE_10)(
    "selects %s by a point on its body and not far from it",
    (typeId) => {
      const schematic = alone(typeId);
      const scene = buildScene(schematic, symbols);
      const body = symbols[typeId]!.primitives.find(
        (primitive) =>
          primitive.kind === "polyline" && primitive.points.length >= 2,
      );
      if (!body || body.kind !== "polyline") throw new Error("a polyline");
      const middle = {
        x: 10 + (body.points[0]!.x + body.points[1]!.x) / 2,
        y: 10 + (body.points[0]!.y + body.points[1]!.y) / 2,
      };

      expect(hitTest(scene, middle, 0.2)).toEqual({
        kind: "component",
        id: ID,
      });
      expect(hitTest(scene, { x: 40, y: 40 }, 0.2)).toBeNull();
    },
  );

  it.each(STAGE_10)(
    "keeps the pin groups of %s distinct at every rotation",
    (typeId) => {
      for (const rotation of [0, 90, 180, 270] as const) {
        const pins = resolvePinPositions(part(typeId, rotation), symbols);
        const at = new Map(pins.map((pin) => [pin.pinId, pin.position]));
        const groups = (
          catalogEntry(typeId) as { pinGroups?: { pins: [string, string] }[] }
        ).pinGroups;

        for (const group of groups ?? []) {
          expect(at.get(group.pins[0] as never)).not.toEqual(
            at.get(group.pins[1] as never),
          );
        }
      }
    },
  );

  it("turns a three-pole contactor a quarter turn clockwise about its origin", () => {
    const upright = new Map(
      resolvePinPositions(part("contactor_three_pole", 0), symbols).map(
        (pin) => [pin.pinId, pin.position],
      ),
    );
    const turned = new Map(
      resolvePinPositions(part("contactor_three_pole", 90), symbols).map(
        (pin) => [pin.pinId, pin.position],
      ),
    );

    // The poles ran top to bottom; after the turn they run left to right at the same distance.
    expect(upright.get("l1")!.x).toBe(upright.get("t1")!.x);
    expect(turned.get("l1")!.y).toBe(turned.get("t1")!.y);
    expect(Math.abs(turned.get("l1")!.x - turned.get("t1")!.x)).toBe(
      Math.abs(upright.get("l1")!.y - upright.get("t1")!.y),
    );
  });

  it.each(STAGE_10)("lets the wires follow %s when it is turned", (typeId) => {
    const first = symbols[typeId]!.pins[0]!;
    const origin = { x: 10, y: 10 };
    const pinAt = (rotation: Rotation) =>
      resolvePinPositions(part(typeId, rotation), symbols).find(
        (pin) => pin.pinId === first.id,
      )!.position;
    const start = pinAt(0);
    const schematic: Schematic = {
      ...alone(typeId),
      wires: [
        {
          id: "00000000-0000-4000-8000-0000000000c2",
          points: [
            start,
            { x: start.x - 5, y: start.y },
            { x: start.x - 5, y: start.y - 3 },
          ],
        },
      ],
    } as unknown as Schematic;
    const placement = { position: origin, rotationDeg: 90 as Rotation };
    const edits = followingWireEdits(
      schematic,
      ID as never,
      placement,
      symbols,
    );
    const moved = applyComponentTransform(
      schematic,
      ID as never,
      placement,
      edits,
    );
    const wire = moved.wires[0]!;

    expect(wire.points[0]).toEqual(pinAt(90));
    // The far end of the wire stays where it was.
    expect(wire.points.at(-1)).toEqual({ x: start.x - 5, y: start.y - 3 });
  });
});
