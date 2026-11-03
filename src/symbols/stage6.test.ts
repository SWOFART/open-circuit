import { describe, expect, it } from "vitest";
import type { DeviceState } from "../generated/DeviceState";
import { symbols } from ".";

// Pin positions of docs/contracts/stage-6-control-circuits.md section 6.1. Their IDs and
// count against the Engine catalog are checked by contract.test.ts for every symbol.

const stateOf = (state: DeviceState | Record<string, unknown>): DeviceState =>
  state as DeviceState;

describe("the Stage 6 symbols", () => {
  it.each(["switch_spst", "push_button_no", "push_button_nc"] as const)(
    "puts the pins of %s at (-1, 0) and (1, 0), like the other two-terminal parts",
    (typeId) => {
      expect(symbols[typeId]?.pins).toEqual([
        { id: "pin1", local: { x: -1, y: 0 } },
        { id: "pin2", local: { x: 1, y: 0 } },
      ]);
    },
  );

  it("puts the pins of the contactor where the contract says", () => {
    expect(symbols.contactor?.pins).toEqual([
      { id: "a1", local: { x: -2, y: -2 } },
      { id: "a2", local: { x: -2, y: 2 } },
      { id: "no_1", local: { x: 2, y: -2 } },
      { id: "no_2", local: { x: 2, y: 2 } },
      { id: "nc_1", local: { x: 5, y: -2 } },
      { id: "nc_2", local: { x: 5, y: 2 } },
    ]);
  });

  it("gives only the switch and the push buttons an operating area", () => {
    expect(symbols.switch_spst?.operatingArea).toBeDefined();
    expect(symbols.push_button_no?.operatingArea).toBeDefined();
    expect(symbols.push_button_nc?.operatingArea).toBeDefined();
    expect(symbols.contactor?.operatingArea).toBeUndefined();
    expect(symbols.resistor?.operatingArea).toBeUndefined();
  });

  it("keeps the operating area inside the symbol's extent and clear of the pins", () => {
    for (const typeId of [
      "switch_spst",
      "push_button_no",
      "push_button_nc",
    ] as const) {
      const symbol = symbols[typeId]!;
      const area = symbol.operatingArea!;

      for (const pin of symbol.pins) {
        expect(
          Math.hypot(pin.local.x - area.center.x, pin.local.y - area.center.y),
        ).toBeGreaterThan(area.radius);
      }
    }
  });

  it("draws the same thing at rest as with no state", () => {
    for (const symbol of [
      symbols.switch_spst,
      symbols.push_button_no,
      symbols.push_button_nc,
      symbols.contactor,
    ]) {
      expect(symbol?.drawn?.(undefined)).toEqual(symbol?.primitives);
    }
  });

  it("draws every state differently where the state changes what the part looks like", () => {
    const draw = (typeId: keyof typeof symbols, state: DeviceState) =>
      JSON.stringify(symbols[typeId]?.drawn?.(state));
    const id = "00000000-0000-4000-8000-0000000000aa";

    expect(
      draw(
        "switch_spst",
        stateOf({
          kind: "switch",
          componentId: id,
          position: "closed",
          conducting: true,
        }),
      ),
    ).not.toEqual(
      draw(
        "switch_spst",
        stateOf({
          kind: "switch",
          componentId: id,
          position: "open",
          conducting: false,
        }),
      ),
    );

    for (const typeId of ["push_button_no", "push_button_nc"] as const) {
      const pressed = draw(
        typeId,
        stateOf({
          kind: "pushButton",
          componentId: id,
          position: "pressed",
          conducting: typeId === "push_button_no",
        }),
      );
      const released = draw(
        typeId,
        stateOf({
          kind: "pushButton",
          componentId: id,
          position: "released",
          conducting: typeId === "push_button_nc",
        }),
      );

      expect(pressed).not.toEqual(released);
    }

    expect(
      draw(
        "contactor",
        stateOf({ kind: "contactor", componentId: id, state: "picked" }),
      ),
    ).not.toEqual(
      draw(
        "contactor",
        stateOf({ kind: "contactor", componentId: id, state: "released" }),
      ),
    );
  });

  it("tells the normally open and normally closed buttons apart at rest", () => {
    expect(JSON.stringify(symbols.push_button_nc?.primitives)).not.toEqual(
      JSON.stringify(symbols.push_button_no?.primitives),
    );
  });

  it("draws what the state says, not what the type suggests", () => {
    const id = "00000000-0000-4000-8000-0000000000aa";
    // A normally closed button the Engine says does not conduct, though it is released,
    // is drawn with its bridge lifted: the symbol follows `conducting`.
    const lifted = JSON.stringify(
      symbols.push_button_nc?.drawn?.(
        stateOf({
          kind: "pushButton",
          componentId: id,
          position: "released",
          conducting: false,
        }),
      ),
    );

    expect(lifted).not.toEqual(
      JSON.stringify(symbols.push_button_nc?.primitives),
    );
  });
});
