import type { SymbolDefinition } from "./types";

/**
 * Junction diode (IEC 60617): a triangle that points from the anode to the cathode, with a
 * bar at the cathode. The pins are `anode` on the left and `cathode` on the right, at the
 * positions of docs/contracts/stage-7-nonlinear-circuits.md section 8. The drawing says
 * nothing about the bias: whether and how much it conducts is the Engine's result.
 */
export const diodeSymbol: SymbolDefinition = {
  typeId: "diode",
  pins: [
    { id: "anode", local: { x: -1, y: 0 } },
    { id: "cathode", local: { x: 1, y: 0 } },
  ],
  primitives: [
    {
      kind: "polyline",
      points: [
        { x: -1, y: 0 },
        { x: -0.4, y: 0 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 0.4, y: 0 },
        { x: 1, y: 0 },
      ],
    },
    // The triangle, closed.
    {
      kind: "polyline",
      points: [
        { x: -0.4, y: -0.5 },
        { x: -0.4, y: 0.5 },
        { x: 0.4, y: 0 },
        { x: -0.4, y: -0.5 },
      ],
    },
    // The cathode bar.
    {
      kind: "polyline",
      points: [
        { x: 0.4, y: -0.5 },
        { x: 0.4, y: 0.5 },
      ],
    },
  ],
};
