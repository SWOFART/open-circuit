import type { DeviceState } from "../generated/DeviceState";
import type { SymbolDefinition, SymbolPrimitive } from "./types";

const line = (...points: [number, number][]): SymbolPrimitive => ({
  kind: "polyline",
  points: points.map(([x, y]) => ({ x, y })),
});

/** The drawing of a latching switch (IEC 60617): two contacts and a lever. */
export function switchPrimitives(
  state: DeviceState | undefined,
): readonly SymbolPrimitive[] {
  const closed = state?.kind === "switch" && state.position === "closed";

  return [
    line([-1, 0], [-0.5, 0]),
    line([0.5, 0], [1, 0]),
    { kind: "circle", center: { x: -0.5, y: 0 }, radius: 0.08 },
    { kind: "circle", center: { x: 0.5, y: 0 }, radius: 0.08 },
    // The lever rests lifted off the right contact, and lies on it when closed.
    closed ? line([-0.5, 0], [0.5, 0]) : line([-0.5, 0], [0.4, -0.75]),
  ];
}

/**
 * Single-pole, single-throw latching switch. It is unpolarized. The lever is drawn
 * open at rest; once the Engine reports a position, the lever follows it.
 */
export const switchSpstSymbol: SymbolDefinition = {
  typeId: "switch_spst",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: switchPrimitives(undefined),
  drawn: switchPrimitives,
  operatingArea: { center: { x: 0, y: -0.4 }, radius: 0.65 },
};
