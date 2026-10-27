import type { SymbolDefinition } from "./types";

/**
 * Fuse (IEC): a narrow box with the conductor running through it. The fuse is
 * unpolarized, so there is no sign mark; its state (intact or tripped) is shown in the
 * device panel.
 */
export const fuseSymbol: SymbolDefinition = {
  typeId: "fuse",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: [
    {
      kind: "polyline",
      points: [
        { x: -1, y: 0 },
        { x: 1, y: 0 },
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
  ],
};
