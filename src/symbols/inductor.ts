import type { LocalPoint, SymbolDefinition } from "./types";

const HUMPS = 4;
const HUMP_WIDTH = 0.3;
const HUMP_STEPS = 8;
const COIL_START_X = -(HUMPS * HUMP_WIDTH) / 2;

/** One half-turn of the coil: a half circle bulging up from `x` to `x + width`. */
function hump(x: number, width: number): LocalPoint[] {
  const radius = width / 2;

  return Array.from({ length: HUMP_STEPS + 1 }, (_, step) => {
    const angle = Math.PI - (Math.PI * step) / HUMP_STEPS;

    return {
      x: x + radius + radius * Math.cos(angle),
      y: -radius * Math.sin(angle),
    };
  });
}

/**
 * Inductor: leads and a coil of four humps. The small "+" above `pin1` marks
 * the reference of the current and of the optional initial current, which
 * enters `pin1`.
 */
export const inductorSymbol: SymbolDefinition = {
  typeId: "inductor",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: [
    {
      kind: "polyline",
      points: [
        { x: -1, y: 0 },
        { x: COIL_START_X, y: 0 },
      ],
    },
    ...Array.from({ length: HUMPS }, (_, index) => ({
      kind: "polyline" as const,
      points: hump(COIL_START_X + index * HUMP_WIDTH, HUMP_WIDTH),
    })),
    {
      kind: "polyline",
      points: [
        { x: -COIL_START_X, y: 0 },
        { x: 1, y: 0 },
      ],
    },
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
