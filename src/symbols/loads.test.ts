import { describe, expect, it } from "vitest";
import catalogFixture from "../../tests/golden/stage-1-catalog.json";
import type { ComponentTypeId } from "../generated/ComponentTypeId";
import type { DeviceState } from "../generated/DeviceState";
import { symbols } from ".";

// The Stage 14 indicator and actuator loads (docs/contracts/stage-14-more-parts.md): the pin
// layout is the contract's common two-pin one, and the drawing follows the state the Engine
// reported.

const LOADS = [
  "lamp",
  "indicator",
  "solenoid",
  "valve",
  "buzzer",
] as const satisfies readonly ComponentTypeId[];

const ID = "00000000-0000-4000-8000-0000000000d2";

const draw = (typeId: (typeof LOADS)[number], state?: DeviceState) =>
  JSON.stringify(symbols[typeId]!.drawn?.(state));

describe("the Stage 14 load symbols", () => {
  it.each(LOADS)("%s has the catalog pins on the contract layout", (typeId) => {
    const entry = catalogFixture.componentTypes.find(
      (candidate) => candidate.typeId === typeId,
    )!;

    expect(symbols[typeId]!.pins).toEqual([
      { id: "p", local: { x: -1, y: 0 } },
      { id: "n", local: { x: 1, y: 0 } },
    ]);
    expect(symbols[typeId]!.pins.map((pin) => pin.id)).toEqual(
      entry.pins.map((pin) => pin.id),
    );
  });

  it.each(LOADS)("%s has no operating area and finite drawing", (typeId) => {
    expect(symbols[typeId]!.operatingArea).toBeUndefined();
    for (const primitive of symbols[typeId]!.primitives) {
      const numbers =
        primitive.kind === "polyline"
          ? primitive.points.flatMap((point) => [point.x, point.y])
          : [primitive.center.x, primitive.center.y, primitive.radius];

      expect(numbers.every(Number.isFinite)).toBe(true);
    }
  });

  it.each(LOADS)("%s reaches both pins with its leads", (typeId) => {
    const ends = symbols[typeId]!.primitives.flatMap((primitive) =>
      primitive.kind === "polyline" ? primitive.points : [],
    );

    expect(ends.some((point) => point.x === -1 && point.y === 0)).toBe(true);
    expect(ends.some((point) => point.x === 1 && point.y === 0)).toBe(true);
  });

  it("is drawn the same at rest as with no state", () => {
    for (const typeId of LOADS) {
      expect(symbols[typeId]!.drawn?.(undefined)).toEqual(
        symbols[typeId]!.primitives,
      );
    }
  });
});

describe("the drawing follows the reported state", () => {
  const cases: readonly [(typeof LOADS)[number], DeviceState, DeviceState][] = [
    [
      "lamp",
      { kind: "lamp", componentId: ID, lit: true },
      { kind: "lamp", componentId: ID, lit: false },
    ],
    [
      "indicator",
      { kind: "indicator", componentId: ID, lit: true },
      { kind: "indicator", componentId: ID, lit: false },
    ],
    [
      "solenoid",
      { kind: "solenoid", componentId: ID, energized: true },
      { kind: "solenoid", componentId: ID, energized: false },
    ],
    [
      "valve",
      { kind: "valve", componentId: ID, position: "open" },
      { kind: "valve", componentId: ID, position: "closed" },
    ],
    [
      "buzzer",
      { kind: "buzzer", componentId: ID, active: true },
      { kind: "buzzer", componentId: ID, active: false },
    ],
  ];

  it.each(cases)("%s differs between its two states", (typeId, on, off) => {
    expect(draw(typeId, on)).not.toBe(draw(typeId, off));
    // The off state is the rest drawing.
    expect(draw(typeId, off)).toBe(draw(typeId));
  });

  it("ignores a state of another kind", () => {
    for (const [typeId] of cases) {
      expect(
        draw(typeId, {
          kind: "fuse",
          componentId: ID,
          state: "tripped",
        } as DeviceState),
      ).toBe(draw(typeId));
    }
  });
});

describe("the frequency converter symbol", () => {
  const converter = symbols.frequency_converter!;

  it("has the pins of the catalog on the contract layout", () => {
    const entry = catalogFixture.componentTypes.find(
      (candidate) => candidate.typeId === "frequency_converter",
    )!;

    expect(converter.pins.map((pin) => pin.id)).toEqual(
      entry.pins.map((pin) => pin.id),
    );
    expect(converter.pins.map((pin) => [pin.local.x, pin.local.y])).toEqual([
      [-3, -2],
      [-3, 2],
      [3, -2],
      [3, 0],
      [3, 2],
      [1, 3],
    ]);
  });

  it("reaches every pin and shows the enabled state only when reported", () => {
    const ends = converter.primitives.flatMap((primitive) =>
      primitive.kind === "polyline" ? primitive.points : [],
    );
    for (const pin of converter.pins) {
      expect(
        ends.some(
          (point) => point.x === pin.local.x && point.y === pin.local.y,
        ),
      ).toBe(true);
    }
    const on = JSON.stringify(
      converter.drawn?.({
        kind: "frequencyConverter",
        componentId: ID,
        enabled: true,
      }),
    );
    const off = JSON.stringify(
      converter.drawn?.({
        kind: "frequencyConverter",
        componentId: ID,
        enabled: false,
      }),
    );

    expect(on).not.toBe(off);
    expect(off).toBe(JSON.stringify(converter.drawn?.(undefined)));
  });
});
