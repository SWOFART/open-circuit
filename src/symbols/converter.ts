import type { SymbolDefinition, SymbolPrimitive } from "./types";

// The simplified frequency converter of Stage 14 (docs/contracts/stage-14-more-parts.md): a DC
// input on the left, three outputs on the right and the neutral below. The drawing shows the
// enabled state the Engine reported and decides nothing.

function line(points: readonly (readonly [number, number])[]): SymbolPrimitive {
  return { kind: "polyline", points: points.map(([x, y]) => ({ x, y })) };
}

const body = (): SymbolPrimitive[] => [
  line([
    [-2, -2.6],
    [2, -2.6],
    [2, 2.6],
    [-2, 2.6],
    [-2, -2.6],
  ]),
  // The leads to the pins at (-3, -2), (-3, 2), (3, -2), (3, 0), (3, 2) and (1, 3).
  line([
    [-3, -2],
    [-2, -2],
  ]),
  line([
    [-3, 2],
    [-2, 2],
  ]),
  line([
    [2, -2],
    [3, -2],
  ]),
  line([
    [2, 0],
    [3, 0],
  ]),
  line([
    [2, 2],
    [3, 2],
  ]),
  line([
    [1, 2.6],
    [1, 3],
  ]),
  // The DC side: two bars (a solid and a dashed one).
  line([
    [-1.6, -0.6],
    [-0.6, -0.6],
  ]),
  line([
    [-1.6, 0.2],
    [-1.4, 0.2],
  ]),
  line([
    [-1.1, 0.2],
    [-0.9, 0.2],
  ]),
  line([
    [-0.8, 0.2],
    [-0.6, 0.2],
  ]),
];

const wave: SymbolPrimitive = line([
  [0.2, -0.2],
  [0.45, -0.7],
  [0.7, -0.2],
  [0.95, 0.3],
  [1.2, -0.2],
]);

const drawn: NonNullable<SymbolDefinition["drawn"]> = (state) => [
  ...body(),
  ...(state?.kind === "frequencyConverter" && state.enabled ? [wave] : []),
];

export const frequencyConverterSymbol: SymbolDefinition = {
  typeId: "frequency_converter",
  pins: [
    { id: "inP", local: { x: -3, y: -2 } },
    { id: "inN", local: { x: -3, y: 2 } },
    { id: "u", local: { x: 3, y: -2 } },
    { id: "v", local: { x: 3, y: 0 } },
    { id: "w", local: { x: 3, y: 2 } },
    { id: "neutral", local: { x: 1, y: 3 } },
  ],
  primitives: drawn(undefined),
  drawn,
};
