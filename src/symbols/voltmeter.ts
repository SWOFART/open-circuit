import type { SymbolDefinition } from "./types";

/**
 * Voltmeter: a circle with a "V". The small "+" above `pin1` marks the reference of
 * the reading, which is V(pin1) - V(pin2): the reading is positive when `pin1` is at
 * the higher potential.
 */
export const voltmeterSymbol: SymbolDefinition = {
  typeId: "voltmeter",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: [
    {
      kind: "circle",
      center: { x: 0, y: 0 },
      radius: 0.6,
    },
    {
      kind: "polyline",
      points: [
        { x: -1, y: 0 },
        { x: -0.6, y: 0 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 0.6, y: 0 },
        { x: 1, y: 0 },
      ],
    },
    // The letter V.
    {
      kind: "polyline",
      points: [
        { x: -0.25, y: -0.28 },
        { x: 0, y: 0.28 },
        { x: 0.25, y: -0.28 },
      ],
    },
    // The positive mark of pin1.
    {
      kind: "polyline",
      points: [
        { x: -0.95, y: -0.4 },
        { x: -0.65, y: -0.4 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.8, y: -0.55 },
        { x: -0.8, y: -0.25 },
      ],
    },
  ],
};
