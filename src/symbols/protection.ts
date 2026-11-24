import type { DeviceState } from "../generated/DeviceState";
import { contact, dashedLink, dot, line } from "./threePhase";
import type { SymbolDefinition, SymbolPrimitive } from "./types";

// The Stage 13 protection devices (docs/contracts/stage-13-protection-and-safety.md sections 2, 4
// and 7). Every pin sits on an integer grid point and carries the ID of the Rust catalog; a drawing
// follows the discrete state the Engine reported and decides nothing by itself.

/**
 * Earth electrode: one pin `pe` on a lead that ends in the three bars of the earth symbol. It is a
 * resistor to the earth node in the Engine; the picture is only the usual mark for it.
 */
export const earthElectrodeSymbol: SymbolDefinition = {
  typeId: "earth_electrode",
  pins: [{ id: "pe", local: { x: 0, y: -2 } }],
  primitives: [
    line([0, -2], [0, 0]),
    line([-1, 0], [1, 0]),
    line([-0.65, 0.4], [0.65, 0.4]),
    line([-0.3, 0.8], [0.3, 0.8]),
  ],
};

/**
 * Exposed part: one pin `ep` on a lead to a housing, drawn as a box with a diagonal across it. The
 * Engine reads the touch voltage of its net; whether it is hazardous is a result, not a drawing.
 */
export const exposedPartSymbol: SymbolDefinition = {
  typeId: "exposed_part",
  pins: [{ id: "ep", local: { x: 0, y: -2 } }],
  primitives: [
    line([0, -2], [0, -1]),
    dot(0, -1),
    line([-1.5, -1], [1.5, -1], [1.5, 1.5], [-1.5, 1.5], [-1.5, -1]),
    line([-1.5, 1.5], [1.5, -1]),
  ],
};

function rcdPrimitives(
  poles: number,
  state: DeviceState | undefined,
): readonly SymbolPrimitive[] {
  const conducting = state?.kind === "switch" && state.conducting;
  const xs = Array.from({ length: poles }, (_, index) => 2 * index);
  const middle = (xs[0]! + xs[xs.length - 1]!) / 2;

  return [
    // The summation current transformer that the poles pass through, then the poles as power
    // switches joined by a dashed link.
    {
      kind: "circle",
      center: { x: middle, y: 0 },
      radius: xs.length * 0.7 + 0.5,
    },
    ...dashedLink(0, xs[xs.length - 1]! + 0.2),
    ...xs.flatMap((x) => contact(x, conducting, { cross: true })),
  ];
}

function rcdSymbol(
  typeId: "rcd_two_pole" | "rcd_four_pole",
  poles: number,
): SymbolDefinition {
  return {
    typeId,
    pins: Array.from({ length: poles }, (_, index) => [
      {
        id: ["l1", "l2", "l3", "l4"][index] as "l1",
        local: { x: 2 * index, y: -2 },
      },
      {
        id: ["t1", "t2", "t3", "t4"][index] as "t1",
        local: { x: 2 * index, y: 2 },
      },
    ]).flat(),
    primitives: rcdPrimitives(poles, undefined),
    drawn: (state) => rcdPrimitives(poles, state),
  };
}

/**
 * Residual-current device with two poles `l1`-`t1` and `l2`-`t2`: power switches in a ring that
 * stands for the summation transformer. The contacts follow the state the Engine reported, so a
 * tripped device is drawn open.
 */
export const rcdTwoPoleSymbol = rcdSymbol("rcd_two_pole", 2);

/** The same device with four poles, `l1`-`t1` to `l4`-`t4` (three phases and the neutral). */
export const rcdFourPoleSymbol = rcdSymbol("rcd_four_pole", 4);
