import type { DeviceState } from "../generated/DeviceState";
import type { LocalPoint, SymbolDefinition, SymbolPrimitive } from "./types";

// The Stage 10 power equipment (docs/contracts/stage-10-three-phase-equipment.md section 2).
// Every pin sits on an integer grid point and carries the ID of the Rust catalog; a drawing
// follows the discrete state the Engine reported and decides nothing by itself.

const line = (...points: [number, number][]): SymbolPrimitive => ({
  kind: "polyline",
  points: points.map(([x, y]) => ({ x, y })),
});

const dot = (x: number, y: number, radius = 0.08): SymbolPrimitive => ({
  kind: "circle",
  center: { x, y },
  radius,
});

/**
 * One contact between `(x, -2)` and `(x, 2)`: leads to two contact points and a blade that lies
 * on them when the contact is closed and swings away when it is open. A normally closed contact
 * carries a bar across the blade, and a power switching device a small cross on its lead.
 */
function contact(
  x: number,
  closed: boolean,
  options: { readonly normallyClosed?: boolean; readonly cross?: boolean } = {},
): SymbolPrimitive[] {
  return [
    line([x, -2], [x, -1]),
    line([x, 1], [x, 2]),
    dot(x, -1),
    dot(x, 1),
    closed ? line([x, -1], [x, 1]) : line([x, -1], [x - 0.7, 0.75]),
    ...(options.normallyClosed ? [line([x - 0.3, 0], [x + 0.3, 0])] : []),
    ...(options.cross
      ? [
          line([x - 0.2, -1.8], [x + 0.2, -1.4]),
          line([x + 0.2, -1.8], [x - 0.2, -1.4]),
        ]
      : []),
  ];
}

/** A dashed horizontal mechanical link from `from` to `to` at height `y`. */
function dashedLink(from: number, to: number, y = 0): SymbolPrimitive[] {
  const dashes: SymbolPrimitive[] = [];

  for (let x = from; x + 0.4 <= to + 1e-9; x += 0.7) {
    dashes.push(line([x, y], [x + 0.4, y]));
  }

  return dashes;
}

/** A resistor-style box with leads on the segment from `from` to `to`. */
function impedance(
  from: LocalPoint,
  to: LocalPoint,
  length: number,
  width: number,
): SymbolPrimitive[] {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const total = Math.hypot(dx, dy);
  const ux = dx / total;
  const uy = dy / total;
  const nx = -uy;
  const ny = ux;
  const start = (total - length) / 2;
  const at = (along: number, across: number): [number, number] => [
    Math.round((from.x + ux * along + nx * across) * 1000) / 1000,
    Math.round((from.y + uy * along + ny * across) * 1000) / 1000,
  ];
  const half = width / 2;

  return [
    line([from.x, from.y], at(start, 0)),
    line(
      at(start, -half),
      at(start + length, -half),
      at(start + length, half),
      at(start, half),
      at(start, -half),
    ),
    line(at(start + length, 0), [to.x, to.y]),
  ];
}

/** `count` half-turn bumps of a winding from `x0` to `x1` on the baseline `y`, bulging by `dir`. */
function winding(
  x0: number,
  x1: number,
  y: number,
  dir: 1 | -1,
  count = 3,
): SymbolPrimitive {
  const width = (x1 - x0) / count;
  const radius = width / 2;
  const points: [number, number][] = [[x0, y]];

  for (let turn = 0; turn < count; turn += 1) {
    const centerX = x0 + width * (turn + 0.5);

    for (let step = 1; step <= 6; step += 1) {
      const angle = Math.PI - (Math.PI * step) / 6;
      points.push([
        Math.round((centerX + radius * Math.cos(angle)) * 1000) / 1000,
        Math.round((y + dir * radius * Math.sin(angle)) * 1000) / 1000,
      ]);
    }
  }

  return line(...points);
}

/** The three power poles of a switching device at `xs`, each between `(x, -2)` and `(x, 2)`. */
const POLE_XS = [2, 4, 6] as const;

function contactorPrimitives(
  state: DeviceState | undefined,
): readonly SymbolPrimitive[] {
  const picked = state?.kind === "contactor" && state.state === "picked";

  return [
    // The coil box and its leads to A1 and A2, with the positive mark beside A1.
    line([-3, -1], [-1, -1], [-1, 1], [-3, 1], [-3, -1]),
    line([-2, -2], [-2, -1]),
    line([-2, 1], [-2, 2]),
    line([-1.7, -1.8], [-1.4, -1.8]),
    line([-1.55, -1.95], [-1.55, -1.65]),
    // The dashed link from the coil to every contact.
    ...dashedLink(-1, 10.2),
    ...POLE_XS.flatMap((x) => contact(x, picked)),
    ...contact(8, picked),
    ...contact(10, !picked, { normallyClosed: true }),
  ];
}

/**
 * Three-pole contactor: the coil `a1` (positive reference), `a2`, three power poles `l1`-`t1`,
 * `l2`-`t2` and `l3`-`t3`, and two auxiliary contacts, one normally open (`no_1`, `no_2`) and one
 * normally closed (`nc_1`, `nc_2`). A pickup closes the poles and the normally open contact and
 * opens the normally closed one, as the Engine reports.
 */
export const contactorThreePoleSymbol: SymbolDefinition = {
  typeId: "contactor_three_pole",
  pins: [
    { id: "a1", local: { x: -2, y: -2 } },
    { id: "a2", local: { x: -2, y: 2 } },
    { id: "l1", local: { x: 2, y: -2 } },
    { id: "t1", local: { x: 2, y: 2 } },
    { id: "l2", local: { x: 4, y: -2 } },
    { id: "t2", local: { x: 4, y: 2 } },
    { id: "l3", local: { x: 6, y: -2 } },
    { id: "t3", local: { x: 6, y: 2 } },
    { id: "no_1", local: { x: 8, y: -2 } },
    { id: "no_2", local: { x: 8, y: 2 } },
    { id: "nc_1", local: { x: 10, y: -2 } },
    { id: "nc_2", local: { x: 10, y: 2 } },
  ],
  primitives: contactorPrimitives(undefined),
  drawn: contactorPrimitives,
};

function breakerPrimitives(
  state: DeviceState | undefined,
): readonly SymbolPrimitive[] {
  // The blades follow actual conduction: a tripped breaker is drawn open although its
  // operator is still closed.
  const conducting = state?.kind === "switch" && state.conducting;

  return [
    ...dashedLink(0, 4.2),
    ...[0, 2, 4].flatMap((x) => contact(x, conducting, { cross: true })),
  ];
}

/**
 * Three-pole circuit-breaker: the poles `l1`-`t1`, `l2`-`t2` and `l3`-`t3`, drawn as power
 * switches (a cross on each lead) joined by a dashed link.
 */
export const breakerThreePoleSymbol: SymbolDefinition = {
  typeId: "breaker_three_pole",
  pins: [
    { id: "l1", local: { x: 0, y: -2 } },
    { id: "t1", local: { x: 0, y: 2 } },
    { id: "l2", local: { x: 2, y: -2 } },
    { id: "t2", local: { x: 2, y: 2 } },
    { id: "l3", local: { x: 4, y: -2 } },
    { id: "t3", local: { x: 4, y: 2 } },
  ],
  primitives: breakerPrimitives(undefined),
  drawn: breakerPrimitives,
};

function overloadPrimitives(
  state: DeviceState | undefined,
): readonly SymbolPrimitive[] {
  const tripped = state?.kind === "fuse" && state.state === "tripped";

  return [
    // Three series sensing elements, each a box on its pole.
    ...[0, 2, 4].flatMap((x) => [
      line([x, -2], [x, -1]),
      line(
        [x - 0.35, -1],
        [x + 0.35, -1],
        [x + 0.35, 1],
        [x - 0.35, 1],
        [x - 0.35, -1],
      ),
      line([x, 1], [x, 2]),
      // The bimetal strip.
      line([x - 0.2, 0.6], [x + 0.2, -0.6]),
    ]),
    ...dashedLink(0.4, 8.2),
    // Tripping closes the normally open contact and opens the normally closed one.
    ...contact(6, tripped),
    ...contact(8, !tripped, { normallyClosed: true }),
  ];
}

/**
 * Thermal overload relay: three series sensing elements on `l1`-`t1`, `l2`-`t2`, `l3`-`t3`, and
 * auxiliary contacts `no_1`-`no_2` (normally open) and `nc_1`-`nc_2` (normally closed) that
 * change when the Engine reports a trip.
 */
export const thermalOverloadThreePoleSymbol: SymbolDefinition = {
  typeId: "thermal_overload_three_pole",
  pins: [
    { id: "l1", local: { x: 0, y: -2 } },
    { id: "t1", local: { x: 0, y: 2 } },
    { id: "l2", local: { x: 2, y: -2 } },
    { id: "t2", local: { x: 2, y: 2 } },
    { id: "l3", local: { x: 4, y: -2 } },
    { id: "t3", local: { x: 4, y: 2 } },
    { id: "no_1", local: { x: 6, y: -2 } },
    { id: "no_2", local: { x: 6, y: 2 } },
    { id: "nc_1", local: { x: 8, y: -2 } },
    { id: "nc_2", local: { x: 8, y: 2 } },
  ],
  primitives: overloadPrimitives(undefined),
  drawn: overloadPrimitives,
};

/**
 * Three-phase induction motor: a circle with an "M" and a "~", the six winding terminals as on
 * a terminal board: `u1`, `v1`, `w1` above and `w2`, `u2`, `v2` below. How the windings are
 * joined (star or delta) is drawn with wires, as on the real machine.
 */
export const inductionMotorThreePhaseSymbol: SymbolDefinition = {
  typeId: "induction_motor_three_phase",
  pins: [
    { id: "u1", local: { x: -2, y: -3 } },
    { id: "u2", local: { x: 0, y: 3 } },
    { id: "v1", local: { x: 0, y: -3 } },
    { id: "v2", local: { x: 2, y: 3 } },
    { id: "w1", local: { x: 2, y: -3 } },
    { id: "w2", local: { x: -2, y: 3 } },
  ],
  primitives: [
    { kind: "circle", center: { x: 0, y: 0 }, radius: 2.3 },
    // Leads from the terminals to the circle.
    line([-2, -3], [-2, -1.13]),
    line([0, -3], [0, -2.3]),
    line([2, -3], [2, -1.13]),
    line([-2, 3], [-2, 1.13]),
    line([0, 3], [0, 2.3]),
    line([2, 3], [2, 1.13]),
    // The letter M.
    line([-0.7, 0.2], [-0.7, -0.8], [0, -0.15], [0.7, -0.8], [0.7, 0.2]),
    // The sign for alternating current.
    line(
      [-0.7, 0.85],
      [-0.5, 0.7],
      [-0.3, 0.7],
      [0.3, 1.05],
      [0.5, 1.05],
      [0.7, 0.85],
    ),
  ],
};

function transformerBank(centerX: number): SymbolPrimitive[] {
  return [
    // Leads to the four terminals of the bank.
    line([centerX - 1, -3], [centerX - 1, -1]),
    line([centerX + 1, -3], [centerX + 1, -1]),
    line([centerX - 1, 3], [centerX - 1, 1]),
    line([centerX + 1, 3], [centerX + 1, 1]),
    winding(centerX - 1, centerX + 1, -1, -1),
    winding(centerX - 1, centerX + 1, 1, 1),
    // The core between the windings.
    line([centerX - 1, -0.15], [centerX + 1, -0.15]),
    line([centerX - 1, 0.15], [centerX + 1, 0.15]),
    // The dots of the first terminals of both windings.
    dot(centerX - 1.4, -2.5, 0.1),
    dot(centerX - 1.4, 2.5, 0.1),
  ];
}

/**
 * Three-phase transformer: three independent banks side by side, each a primary
 * (`p1a`, `p1b` ... `p3a`, `p3b`) above its secondary (`s1a`, `s1b` ... `s3a`, `s3b`). The dots
 * mark the first terminals. Star or delta wiring is drawn with wires.
 */
export const transformerThreePhaseSymbol: SymbolDefinition = {
  typeId: "transformer_three_phase",
  pins: [
    { id: "p1a", local: { x: -5, y: -3 } },
    { id: "p1b", local: { x: -3, y: -3 } },
    { id: "p2a", local: { x: -1, y: -3 } },
    { id: "p2b", local: { x: 1, y: -3 } },
    { id: "p3a", local: { x: 3, y: -3 } },
    { id: "p3b", local: { x: 5, y: -3 } },
    { id: "s1a", local: { x: -5, y: 3 } },
    { id: "s1b", local: { x: -3, y: 3 } },
    { id: "s2a", local: { x: -1, y: 3 } },
    { id: "s2b", local: { x: 1, y: 3 } },
    { id: "s3a", local: { x: 3, y: 3 } },
    { id: "s3b", local: { x: 5, y: 3 } },
  ],
  primitives: [-4, 0, 4].flatMap((centerX) => transformerBank(centerX)),
};

/**
 * Star-connected three-phase load: three impedance branches from `l1`, `l2` and `l3` to the
 * common point, which is the pin `n` and may float.
 */
export const threePhaseLoadStarSymbol: SymbolDefinition = {
  typeId: "three_phase_load_star",
  pins: [
    { id: "l1", local: { x: -2, y: -3 } },
    { id: "l2", local: { x: 0, y: -3 } },
    { id: "l3", local: { x: 2, y: -3 } },
    { id: "n", local: { x: 0, y: 3 } },
  ],
  primitives: [
    ...[-2, 0, 2].flatMap((x) =>
      impedance({ x, y: -3 }, { x, y: 0.8 }, 2.2, 0.6),
    ),
    line([-2, 0.8], [2, 0.8]),
    dot(0, 0.8, 0.1),
    line([0, 0.8], [0, 3]),
  ],
};

/**
 * Delta-connected three-phase load: three impedance branches between the corners, `l1` at the
 * upper left, `l3` at the upper right and `l2` at the bottom.
 */
export const threePhaseLoadDeltaSymbol: SymbolDefinition = {
  typeId: "three_phase_load_delta",
  pins: [
    { id: "l1", local: { x: -2, y: -3 } },
    { id: "l2", local: { x: 0, y: 3 } },
    { id: "l3", local: { x: 2, y: -3 } },
  ],
  primitives: [
    line([-2, -3], [-2, -1]),
    line([2, -3], [2, -1]),
    line([0, 3], [0, 2]),
    dot(-2, -1, 0.1),
    dot(2, -1, 0.1),
    dot(0, 2, 0.1),
    ...impedance({ x: -2, y: -1 }, { x: 2, y: -1 }, 1.6, 0.5),
    ...impedance({ x: 2, y: -1 }, { x: 0, y: 2 }, 1.6, 0.5),
    ...impedance({ x: 0, y: 2 }, { x: -2, y: -1 }, 1.6, 0.5),
  ],
};
