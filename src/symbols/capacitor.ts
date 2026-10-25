import type { SymbolDefinition } from "./types";

/**
 * Non-polarized capacitor: two plates across the pin axis. The small "+" above
 * `pin1` marks the reference of the voltage and of the optional initial
 * voltage, `V(pin1) - V(pin2)`; it does not make the part polarized.
 */
export const capacitorSymbol: SymbolDefinition = {
  typeId: "capacitor",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: [
    {
      kind: "polyline",
      points: [
        { x: -1, y: 0 },
        { x: -0.15, y: 0 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.15, y: -0.5 },
        { x: -0.15, y: 0.5 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 0.15, y: -0.5 },
        { x: 0.15, y: 0.5 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 0.15, y: 0 },
        { x: 1, y: 0 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.95, y: -0.3 },
        { x: -0.65, y: -0.3 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.8, y: -0.45 },
        { x: -0.8, y: -0.15 },
      ],
    },
  ],
};
