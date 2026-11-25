import type { DeviceState } from "../generated/DeviceState";
import type { SymbolDefinition, SymbolPrimitive } from "./types";

// The indicator and actuator loads of Stage 14 (docs/contracts/stage-14-more-parts.md,
// "Indicators and actuators"). Each is a two-pin part on the common layout p=(-1,0), n=(1,0).
// The drawing shows only the state the Engine reported; it decides nothing.

function line(points: readonly (readonly [number, number])[]): SymbolPrimitive {
  return { kind: "polyline", points: points.map(([x, y]) => ({ x, y })) };
}

const leads = (inner: number): SymbolPrimitive[] => [
  line([
    [-1, 0],
    [-inner, 0],
  ]),
  line([
    [inner, 0],
    [1, 0],
  ]),
];

/** Rays around a lit lamp or indicator. */
const rays: SymbolPrimitive[] = [
  line([
    [0.5, -0.5],
    [0.8, -0.8],
  ]),
  line([
    [-0.5, -0.5],
    [-0.8, -0.8],
  ]),
  line([
    [0.5, 0.5],
    [0.8, 0.8],
  ]),
  line([
    [-0.5, 0.5],
    [-0.8, 0.8],
  ]),
];

const rectangle = (halfWidth: number, halfHeight: number): SymbolPrimitive =>
  line([
    [-halfWidth, -halfHeight],
    [halfWidth, -halfHeight],
    [halfWidth, halfHeight],
    [-halfWidth, halfHeight],
    [-halfWidth, -halfHeight],
  ]);

const pins = [
  { id: "p", local: { x: -1, y: 0 } },
  { id: "n", local: { x: 1, y: 0 } },
] as const;

function definition(
  typeId: SymbolDefinition["typeId"],
  drawn: NonNullable<SymbolDefinition["drawn"]>,
): SymbolDefinition {
  return { typeId, pins, primitives: drawn(undefined), drawn };
}

const lampBody: SymbolPrimitive[] = [
  { kind: "circle", center: { x: 0, y: 0 }, radius: 0.4 },
  line([
    [-0.28, -0.28],
    [0.28, 0.28],
  ]),
  line([
    [-0.28, 0.28],
    [0.28, -0.28],
  ]),
];

export const lampSymbol = definition(
  "lamp",
  (state: DeviceState | undefined) => [
    ...leads(0.4),
    ...lampBody,
    ...(state?.kind === "lamp" && state.lit ? rays : []),
  ],
);

export const indicatorSymbol = definition(
  "indicator",
  (state: DeviceState | undefined) => [
    ...leads(0.4),
    { kind: "circle", center: { x: 0, y: 0 }, radius: 0.4 },
    { kind: "circle", center: { x: 0, y: 0 }, radius: 0.12 },
    ...(state?.kind === "indicator" && state.lit ? rays : []),
  ],
);

export const buzzerSymbol = definition(
  "buzzer",
  (state: DeviceState | undefined) => [
    ...leads(0.5),
    rectangle(0.5, 0.4),
    line([
      [-0.2, 0.2],
      [-0.2, -0.2],
      [0.05, -0.2],
      [0.2, -0.05],
      [0.05, 0.1],
      [-0.2, 0.1],
    ]),
    ...(state?.kind === "buzzer" && state.active
      ? [
          line([
            [0.65, -0.3],
            [0.8, -0.15],
          ]),
          line([
            [0.65, 0.3],
            [0.8, 0.15],
          ]),
        ]
      : []),
  ],
);

const coil = (): SymbolPrimitive[] => [...leads(0.5), rectangle(0.5, 0.4)];

export const solenoidSymbol = definition(
  "solenoid",
  (state: DeviceState | undefined) => {
    const pulled = state?.kind === "solenoid" && state.energized;
    // The plunger is drawn below the coil and moves up into it when the coil is energized.
    const y = pulled ? 0.55 : 0.8;
    return [
      ...coil(),
      line([
        [-0.3, y],
        [0.3, y],
      ]),
      line([
        [0, y],
        [0, 1],
      ]),
    ];
  },
);

export const valveSymbol = definition(
  "valve",
  (state: DeviceState | undefined) => {
    const open = state?.kind === "valve" && state.position === "open";
    return [
      ...coil(),
      // The valve body: two triangles that meet at their tips, with the flow path open or shut.
      line([
        [-0.4, 0.55],
        [0, 0.8],
        [-0.4, 1.05],
        [-0.4, 0.55],
      ]),
      line([
        [0.4, 0.55],
        [0, 0.8],
        [0.4, 1.05],
        [0.4, 0.55],
      ]),
      ...(open
        ? [
            line([
              [-0.4, 0.8],
              [0.4, 0.8],
            ]),
          ]
        : []),
    ];
  },
);
