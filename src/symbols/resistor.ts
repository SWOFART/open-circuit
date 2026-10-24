import type { SymbolDefinition } from "./types";

export const resistorSymbol: SymbolDefinition = {
  typeId: "resistor",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: [
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
        { x: -0.6, y: -0.25 },
        { x: 0.6, y: -0.25 },
        { x: 0.6, y: 0.25 },
        { x: -0.6, y: 0.25 },
        { x: -0.6, y: -0.25 },
      ],
    },
    {
      kind: "polyline",
      points: [
        { x: 0.6, y: 0 },
        { x: 1, y: 0 },
      ],
    },
  ],
};
