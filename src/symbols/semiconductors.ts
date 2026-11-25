import type { SymbolDefinition, SymbolPrimitive } from "./types";

// The Stage 14 LED, zener diode, N-channel MOSFET and operational amplifier
// (docs/contracts/stage-14-more-parts.md). Each drawing follows the state the Engine reported
// and decides nothing.

function line(points: readonly (readonly [number, number])[]): SymbolPrimitive {
  return { kind: "polyline", points: points.map(([x, y]) => ({ x, y })) };
}

/** A diode on the two-pin layout p=(-1, 0) (anode) and n=(1, 0) (cathode). */
const diodeBody = (): SymbolPrimitive[] => [
  line([
    [-1, 0],
    [-0.4, 0],
  ]),
  line([
    [0.4, 0],
    [1, 0],
  ]),
  line([
    [-0.4, -0.5],
    [-0.4, 0.5],
    [0.4, 0],
    [-0.4, -0.5],
  ]),
];

const bar: SymbolPrimitive = line([
  [0.4, -0.5],
  [0.4, 0.5],
]);

/** The bar of a zener diode, bent at its ends. */
const zenerBar: SymbolPrimitive = line([
  [0.2, -0.5],
  [0.4, -0.5],
  [0.4, 0.5],
  [0.6, 0.5],
]);

const lightRays: SymbolPrimitive[] = [
  line([
    [-0.1, -0.7],
    [0.3, -1.1],
  ]),
  line([
    [0.2, -0.6],
    [0.6, -1],
  ]),
];

const twoPin = [
  { id: "p", local: { x: -1, y: 0 } },
  { id: "n", local: { x: 1, y: 0 } },
] as const;

const ledDrawn: NonNullable<SymbolDefinition["drawn"]> = (state) => [
  ...diodeBody(),
  bar,
  ...(state?.kind === "led" && state.lit ? lightRays : []),
];

export const ledSymbol: SymbolDefinition = {
  typeId: "led",
  pins: twoPin,
  primitives: ledDrawn(undefined),
  drawn: ledDrawn,
};

const zenerDrawn: NonNullable<SymbolDefinition["drawn"]> = (state) => [
  ...diodeBody(),
  zenerBar,
  // The reverse breakdown is shown by a short arrow at the cathode.
  ...(state?.kind === "zenerDiode" && state.inBreakdown
    ? [
        line([
          [0.9, -0.55],
          [0.7, -0.35],
        ]),
        line([
          [0.9, -0.55],
          [0.9, -0.3],
        ]),
      ]
    : []),
];

export const zenerDiodeSymbol: SymbolDefinition = {
  typeId: "zener_diode",
  pins: twoPin,
  primitives: zenerDrawn(undefined),
  drawn: zenerDrawn,
};

/** The MOSFET: gate g=(-2, 0), drain d=(2, -2), source s=(2, 2). */
const mosfetDrawn: NonNullable<SymbolDefinition["drawn"]> = (state) => {
  const conducting = state?.kind === "mosfetN" && state.conducting;
  return [
    line([
      [-2, 0],
      [-0.8, 0],
    ]),
    // The insulated gate plate.
    line([
      [-0.8, -1.3],
      [-0.8, 1.3],
    ]),
    // The channel: one solid bar when it conducts, three segments when it does not.
    ...(conducting
      ? [
          line([
            [-0.4, -1.3],
            [-0.4, 1.3],
          ]),
        ]
      : [
          line([
            [-0.4, -1.3],
            [-0.4, -0.9],
          ]),
          line([
            [-0.4, -0.5],
            [-0.4, 0.5],
          ]),
          line([
            [-0.4, 0.9],
            [-0.4, 1.3],
          ]),
        ]),
    line([
      [-0.4, -1],
      [2, -1],
      [2, -2],
    ]),
    line([
      [-0.4, 1],
      [2, 1],
      [2, 2],
    ]),
    // The arrow into the channel at the source.
    line([
      [0.2, 0.7],
      [-0.4, 1],
      [0.2, 1.3],
    ]),
  ];
};

export const mosfetNSymbol: SymbolDefinition = {
  typeId: "mosfet_n",
  pins: [
    { id: "g", local: { x: -2, y: 0 } },
    { id: "d", local: { x: 2, y: -2 } },
    { id: "s", local: { x: 2, y: 2 } },
  ],
  primitives: mosfetDrawn(undefined),
  drawn: mosfetDrawn,
};

/** The amplifier: plus (-2, -1), minus (-2, 1), out (2, 0), ref (0, 2). */
const amplifierDrawn: NonNullable<SymbolDefinition["drawn"]> = (state) => {
  const clipped =
    state?.kind === "operationalAmplifier" && state.state !== "linear";
  return [
    line([
      [-1.5, -2],
      [1.5, 0],
      [-1.5, 2],
      [-1.5, -2],
    ]),
    line([
      [-2, -1],
      [-1.5, -1],
    ]),
    line([
      [-2, 1],
      [-1.5, 1],
    ]),
    line([
      [1.5, 0],
      [2, 0],
    ]),
    line([
      [0, 1],
      [0, 2],
    ]),
    // The plus and minus marks on the inputs.
    line([
      [-1.2, -1],
      [-0.8, -1],
    ]),
    line([
      [-1, -1.2],
      [-1, -0.8],
    ]),
    line([
      [-1.2, 1],
      [-0.8, 1],
    ]),
    // A flat top and bottom on the output side show that it is at a limit.
    ...(clipped
      ? [
          line([
            [0.2, -0.3],
            [0.6, -0.3],
          ]),
        ]
      : []),
  ];
};

export const operationalAmplifierSymbol: SymbolDefinition = {
  typeId: "operational_amplifier",
  pins: [
    { id: "plus", local: { x: -2, y: -1 } },
    { id: "minus", local: { x: -2, y: 1 } },
    { id: "out", local: { x: 2, y: 0 } },
    { id: "ref", local: { x: 0, y: 2 } },
  ],
  primitives: amplifierDrawn(undefined),
  drawn: amplifierDrawn,
};
