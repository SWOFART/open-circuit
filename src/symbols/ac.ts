import type { LocalPoint, SymbolDefinition, SymbolPrimitive } from "./types";

function line(...points: readonly [number, number][]): SymbolPrimitive {
  return { kind: "polyline", points: points.map(([x, y]) => ({ x, y })) };
}

const sine = line(
  [-0.36, 0],
  [-0.24, -0.16],
  [-0.12, -0.16],
  [0, 0],
  [0.12, 0.16],
  [0.24, 0.16],
  [0.36, 0],
);

export const acVoltageSourceSymbol: SymbolDefinition = {
  typeId: "ac_voltage_source",
  pins: [
    { id: "pin1", local: { x: -1, y: 0 } },
    { id: "pin2", local: { x: 1, y: 0 } },
  ],
  primitives: [
    { kind: "circle", center: { x: 0, y: 0 }, radius: 0.65 },
    line([-1, 0], [-0.65, 0]),
    line([0.65, 0], [1, 0]),
    sine,
  ],
};

const phasePins = [
  { id: "l1", local: { x: 2, y: -2 } },
  { id: "l2", local: { x: 2, y: 0 } },
  { id: "l3", local: { x: 2, y: 2 } },
] as const;

function terminalLetter(
  points: readonly LocalPoint[],
  x: number,
  y: number,
): SymbolPrimitive {
  return {
    kind: "polyline",
    points: points.map((point) => ({ x: x + point.x, y: y + point.y })),
  };
}

const numerals: readonly (readonly LocalPoint[])[] = [
  [
    { x: 0.02, y: 0.08 },
    { x: 0.12, y: 0 },
    { x: 0.12, y: 0.28 },
  ],
  [
    { x: 0, y: 0.03 },
    { x: 0.17, y: 0.03 },
    { x: 0.17, y: 0.11 },
    { x: 0, y: 0.28 },
    { x: 0.17, y: 0.28 },
  ],
  [
    { x: 0, y: 0.03 },
    { x: 0.17, y: 0.03 },
    { x: 0.17, y: 0.28 },
    { x: 0, y: 0.28 },
  ],
];

const phaseDrawing: readonly SymbolPrimitive[] = [
  line([-1.15, -2.3], [1.15, -2.3], [1.15, 2.3], [-1.15, 2.3], [-1.15, -2.3]),
  ...phasePins.flatMap(({ local: { y } }, index) => [
    line([1.15, y], [2, y]),
    line([0.48, y - 0.15], [0.48, y + 0.13], [0.62, y + 0.13]),
    terminalLetter(numerals[index]!, 0.69, y - 0.15),
    ...(index === 2 ? [line([0.71, y], [0.86, y])] : []),
  ]),
];

export const threePhaseStarSymbol: SymbolDefinition = {
  typeId: "three_phase_source_star",
  pins: [...phasePins, { id: "n", local: { x: -2, y: 0 } }],
  primitives: [
    ...phaseDrawing,
    line([-2, 0], [-1.15, 0]),
    line([-0.95, 0.13], [-0.95, -0.15], [-0.77, 0.13], [-0.77, -0.15]),
    line([-0.34, -0.38], [0, 0], [0.34, -0.38]),
    line([0, 0], [0, 0.5]),
  ],
};

export const threePhaseDeltaSymbol: SymbolDefinition = {
  typeId: "three_phase_source_delta",
  pins: phasePins,
  primitives: [
    ...phaseDrawing,
    line([0, -0.49], [0.39, 0.28], [-0.39, 0.28], [0, -0.49]),
  ],
};

function acMeter(
  typeId: "ac_voltmeter" | "ac_ammeter",
  letter: readonly SymbolPrimitive[],
): SymbolDefinition {
  return {
    typeId,
    pins: [
      { id: "pin1", local: { x: -2, y: 0 } },
      { id: "pin2", local: { x: 2, y: 0 } },
    ],
    primitives: [
      { kind: "circle", center: { x: 0, y: 0 }, radius: 0.82 },
      line([-2, 0], [-0.82, 0]),
      line([0.82, 0], [2, 0]),
      ...letter,
      line(
        [-0.36, 0.42],
        [-0.24, 0.3],
        [-0.12, 0.3],
        [0.12, 0.54],
        [0.24, 0.54],
        [0.36, 0.42],
      ),
    ],
  };
}

export const acVoltmeterSymbol = acMeter("ac_voltmeter", [
  line([-0.25, -0.42], [0, 0.16], [0.25, -0.42]),
]);

export const acAmmeterSymbol = acMeter("ac_ammeter", [
  line([-0.25, 0.16], [0, -0.42], [0.25, 0.16]),
  line([-0.14, -0.07], [0.14, -0.07]),
]);

export const wattmeterSymbol: SymbolDefinition = {
  typeId: "wattmeter",
  pins: [
    { id: "v1", local: { x: -2, y: -1 } },
    { id: "v2", local: { x: 2, y: -1 } },
    { id: "i1", local: { x: -2, y: 1 } },
    { id: "i2", local: { x: 2, y: 1 } },
  ],
  primitives: [
    line([-1, -1.27], [1, -1.27], [1, 1.27], [-1, 1.27], [-1, -1.27]),
    line([-2, -1], [-1, -1]),
    line([1, -1], [2, -1]),
    line([-2, 1], [-1, 1]),
    line([1, 1], [2, 1]),
    line(
      [-0.44, -0.53],
      [-0.25, 0.16],
      [0, -0.19],
      [0.25, 0.16],
      [0.44, -0.53],
    ),
    line(
      [-0.36, 0.58],
      [-0.24, 0.46],
      [-0.12, 0.46],
      [0.12, 0.7],
      [0.24, 0.7],
      [0.36, 0.58],
    ),
  ],
};
