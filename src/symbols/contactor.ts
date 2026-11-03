import type { DeviceState } from "../generated/DeviceState";
import type { SymbolDefinition, SymbolPrimitive } from "./types";

const line = (...points: [number, number][]): SymbolPrimitive => ({
  kind: "polyline",
  points: points.map(([x, y]) => ({ x, y })),
});

/**
 * One contact between `(x, -2)` and `(x, 2)`: leads to two contact points, and a blade that
 * lies on them when the contact is closed and swings away when it is open. A normally
 * closed contact carries a bar across the blade.
 */
function contact(
  x: number,
  closed: boolean,
  normallyClosed: boolean,
): SymbolPrimitive[] {
  return [
    line([x, -2], [x, -1]),
    line([x, 1], [x, 2]),
    { kind: "circle", center: { x, y: -1 }, radius: 0.08 },
    { kind: "circle", center: { x, y: 1 }, radius: 0.08 },
    closed ? line([x, -1], [x, 1]) : line([x, -1], [x - 0.7, 0.75]),
    ...(normallyClosed ? [line([x - 0.3, 0], [x + 0.3, 0])] : []),
  ];
}

/**
 * The drawing of a contactor: the coil on the left, then the normally open contact and the
 * normally closed contact, joined to the coil by a dashed mechanical link. The contacts
 * follow the state the Engine reported: picked closes the normally open one and opens the
 * normally closed one.
 */
export function contactorPrimitives(
  state: DeviceState | undefined,
): readonly SymbolPrimitive[] {
  const picked = state?.kind === "contactor" && state.state === "picked";

  return [
    // The coil box and its leads to A1 and A2.
    line([-3, -1], [-1, -1], [-1, 1], [-3, 1], [-3, -1]),
    line([-2, -2], [-2, -1]),
    line([-2, 1], [-2, 2]),
    // The positive mark of the coil, beside A1.
    line([-1.7, -1.8], [-1.4, -1.8]),
    line([-1.55, -1.95], [-1.55, -1.65]),
    // The dashed mechanical link from the coil to both contacts.
    line([-1, 0], [-0.6, 0]),
    line([-0.3, 0], [0.1, 0]),
    line([0.4, 0], [0.8, 0]),
    line([1.1, 0], [1.5, 0]),
    line([2.3, 0], [2.7, 0]),
    line([3, 0], [3.4, 0]),
    line([3.7, 0], [4.1, 0]),
    ...contact(2, picked, false),
    ...contact(5, !picked, true),
  ];
}

/**
 * Contactor: a coil on `a1` (positive reference) and `a2`, and two galvanically isolated
 * auxiliary contacts, one normally open (`no_1`, `no_2`) and one normally closed (`nc_1`,
 * `nc_2`). Pin positions are those of docs/contracts/stage-6-control-circuits.md section 6.1.
 */
export const contactorSymbol: SymbolDefinition = {
  typeId: "contactor",
  pins: [
    { id: "a1", local: { x: -2, y: -2 } },
    { id: "a2", local: { x: -2, y: 2 } },
    { id: "no_1", local: { x: 2, y: -2 } },
    { id: "no_2", local: { x: 2, y: 2 } },
    { id: "nc_1", local: { x: 5, y: -2 } },
    { id: "nc_2", local: { x: 5, y: 2 } },
  ],
  primitives: contactorPrimitives(undefined),
  drawn: contactorPrimitives,
};
