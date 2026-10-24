import type { SymbolDefinition } from "./types";

/** Battery symbol: the long plate is the positive terminal, `pin1`. */
export const dcVoltageSourceSymbol: SymbolDefinition = {
  typeId: "dc_voltage_source",
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
        { x: -0.15, y: -0.4 },
        { x: -0.15, y: 0.4 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 0.15, y: -0.2 },
        { x: 0.15, y: 0.2 },
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
        { x: -0.7, y: -0.3 },
        { x: -0.4, y: -0.3 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: -0.55, y: -0.45 },
        { x: -0.55, y: -0.15 },
      ],
    },
  ],
};
