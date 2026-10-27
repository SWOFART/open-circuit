import type { SymbolDefinition } from "./types";

/**
 * Ammeter: a circle with an "A". The small "+" above `pin1` marks the reference of the
 * reading, which is the current entering `pin1`.
 */
export const ammeterSymbol: SymbolDefinition = {
  typeId: "ammeter",
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
    // The letter A.
    {
      kind: "polyline",
      points: [
        { x: -0.25, y: 0.28 },
        { x: 0, y: -0.28 },
        { x: 0.25, y: 0.28 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.13, y: 0.08 },
        { x: 0.13, y: 0.08 },
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
