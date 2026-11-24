// The Stage 13 protection oracles: closed-form results for earth faults, the touch voltage, the
// residual-current device, the breaker trip classes and short circuits, each with a wrong variant.
// They have no dependency on the application or the Engine, only on Node.
//
//   node scripts/stage13-references.mjs --check     verify the committed files (no Engine run)
//   node scripts/stage13-references.mjs --write     regenerate tests/golden/stage-13/
//
// The Engine is compared with these data by crates/engine/tests/stage_13_references.rs.
import { withDocumentation } from "./documentation-defaults.mjs";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const directory = fileURLToPath(
  new URL("../tests/golden/stage-13/", import.meta.url),
);

const digest = (value) => createHash("sha256").update(value).digest("hex");
const text = (value) => `${JSON.stringify(value, null, 2)}\n`;
const sqrt2 = Math.sqrt(2);

// ---------------------------------------------------------------- the circuits

const uuid = (group, n) =>
  `00000000-0000-4000-8000-${(group * 4096 + n).toString(16).padStart(12, "0")}`;

const part = (id, typeId, parameters, pins) => ({
  component: {
    typeId,
    id,
    position: { x: 0, y: 0 },
    rotationDeg: 0,
    parameters,
  },
  geometry: {
    componentId: id,
    pins: pins.map(([pinId, [x, y]]) => ({ pinId, position: { x, y } })),
  },
});

function request(parts, { marker = true, faults, revision = "rev-1" } = {}) {
  const compile = {
    schematic: {
      // Described in the earlier shape; `withDocumentation` makes it the live version 3.
      schemaVersion: 2,
      components: parts.map((p) => p.component),
      wires: [],
      junctions: [],
      referenceMarkers: marker
        ? [{ id: uuid(15, 0), position: { x: 2, y: 2 } }]
        : [],
      rails: [],
    },
    geometry: { components: parts.map((p) => p.geometry) },
    documentRevision: revision,
  };
  if (faults) compile.faults = faults;
  return withDocumentation(compile);
}

// Node A (0,0): the source's positive pin. Node D (2,2): its negative pin, with the ground marker.
// Node B (10,0): the frame and, for a bonded frame, the electrode.
const ids = (group) => ({
  source: uuid(group, 1),
  second: uuid(group, 2),
  third: uuid(group, 3),
  fault: uuid(group, 10),
});

const dcSource = (id, volts, rs) =>
  part(
    id,
    "dc_voltage_source",
    { voltageVolts: volts, internalResistanceOhms: rs },
    [
      ["pin1", [0, 0]],
      ["pin2", [2, 2]],
    ],
  );
const acSource = (id, rms, rs) =>
  part(
    id,
    "ac_voltage_source",
    {
      voltageRmsVolts: rms,
      frequencyHz: 50,
      phaseOffsetDegrees: 0,
      internalResistanceOhms: rs,
    },
    [
      ["pin1", [0, 0]],
      ["pin2", [2, 2]],
    ],
  );
const electrode = (id, re, at) =>
  part(id, "earth_electrode", { earthResistanceOhms: re }, [["pe", at]]);
const frame = (id, limit) =>
  part(id, "exposed_part", { frequencyHz: 50, touchVoltageLimitVolts: limit }, [
    ["ep", [10, 0]],
  ]);
const leak = (group, sourceId, frameId, value, pinId = "pin1") => [
  {
    id: ids(group).fault,
    kind: "leak",
    componentId: sourceId,
    pinId,
    frameId,
    value,
  },
];

// ------------------------------------------------------------- the closed forms

/** DC earth-fault loop: source E (internal Rs) through a leak Rf to a frame bonded to Re. */
const bondedLoop = ({ E, Rs, Rf, Re }) => {
  const I = E / (Rs + Rf + Re);
  return { current: I, frame: Re * I };
};
/** The same loop with the frame not bonded: its only path is the exposed part's 1 GOhm. */
const unbondedLoop = ({ E, Rs, Rf }) => {
  const earth = 1e9;
  const I = E / (Rs + Rf + earth);
  return { current: I, frame: E * (earth / (Rs + Rf + earth)) };
};

/**
 * Trip time (s) of a constant residual `ratio` times the rated current, from rest: the detector
 * is I^2 (1 - exp(-t/tau)); while its RMS is at least In the dose gains (I/In)^2 / T per second,
 * below In it is cleared. Null when it never trips within `horizon` seconds.
 */
function rcdTripTime(ratio, { tau, T }, horizon = 30) {
  const step = 1e-6;
  let dose = 0;
  for (let t = step; t < horizon; t += step) {
    const level = ratio * Math.sqrt(1 - Math.exp(-t / tau));
    dose = level >= 1 ? dose + (step * level * level) / T : 0;
    if (dose >= 1) return t;
  }
  return null;
}

/** Thermal trip time (s) of a breaker at `ratio` times the rated current, from cold. */
function breakerThermalTime(ratio, { tau, K, cool }, horizon = 60) {
  const step = 1e-6;
  let dose = 0;
  for (let t = step; t < horizon; t += step) {
    const q = ratio * ratio * (1 - Math.exp(-t / tau));
    dose = q > 1 ? dose + (step * (q - 1)) / K : dose * Math.exp(-step / cool);
    if (dose >= 1) return t;
  }
  return null;
}

// ---------------------------------------------------------------- the fixtures

const value = (step, key, expected, relative, absolute = 0) => ({
  kind: "value",
  step,
  key,
  expected,
  relative,
  absolute,
});

/** A key of the exposed part or of a device, as the Engine's layout spells it. */
const key = (id, suffix) =>
  `${id}${["current", "voltage"].includes(suffix) ? ":" : "/"}${suffix}`;

const fixtures = [];

// 1. Earth-fault loops and the touch voltage (DC), with wrong variants.
const loops = [
  { name: "a", E: 100, Rs: 1, Rf: 10, Re: 9, limit: 50 },
  { name: "b", E: 230, Rs: 0.5, Rf: 100, Re: 20, limit: 50 },
  { name: "c", E: 24, Rs: 0.2, Rf: 5, Re: 2, limit: 12 },
  { name: "d", E: 400, Rs: 0.1, Rf: 0.001, Re: 0.5, limit: 50 },
];
loops.forEach((loop, index) => {
  const group = 1 + index;
  const { source, second, third } = ids(group);
  const build = (bonded, re) =>
    request(
      [
        dcSource(source, loop.E, loop.Rs),
        electrode(second, re, bonded ? [10, 0] : [20, 0]),
        frame(third, loop.limit),
      ],
      { faults: leak(group, source, third, loop.Rf) },
    );
  const correct = bondedLoop(loop);
  fixtures.push({
    id: `earth-loop-${loop.name}`,
    family: "earth-fault",
    variant: "correct",
    description: `DC ${loop.E} V, Rs ${loop.Rs}, leak ${loop.Rf}, electrode ${loop.Re} bonded to the frame`,
    request: build(true, loop.Re),
    checks: [
      value(25, key(source, "current"), -correct.current, 1e-6, 1e-9),
      value(25, key(third, "touch_voltage_rms"), correct.frame, 1e-6, 1e-6),
      value(25, key(third, "hazard"), correct.frame > loop.limit ? 1 : 0, 0),
      value(25, key(third, "valid"), 1, 0),
    ],
  });
  const open = unbondedLoop(loop);
  fixtures.push({
    id: `earth-loop-${loop.name}-unbonded`,
    family: "earth-fault",
    variant: "wrong",
    wrongOf: `earth-loop-${loop.name}`,
    description: "the bonding between the frame and the electrode is missing",
    request: build(false, loop.Re),
    checks: [
      value(25, key(third, "touch_voltage_rms"), open.frame, 1e-6, 1e-6),
      value(25, key(third, "hazard"), open.frame > loop.limit ? 1 : 0, 0),
      // The 1 GOhm exposed part next to ohm-scale branches limits the current to about 1e-10 A
      // in double precision (the solver conditioning noted for #197 and #202).
      value(25, key(source, "current"), -open.current, 0, 1e-10),
    ],
  });
  const worse = bondedLoop({ ...loop, Re: loop.Re * 10 });
  fixtures.push({
    id: `earth-loop-${loop.name}-poor-electrode`,
    family: "earth-fault",
    variant: "wrong",
    wrongOf: `earth-loop-${loop.name}`,
    description: "the electrode resistance is ten times too high",
    request: build(true, loop.Re * 10),
    checks: [
      value(25, key(third, "touch_voltage_rms"), worse.frame, 1e-6, 1e-6),
      value(25, key(source, "current"), -worse.current, 1e-6, 1e-9),
    ],
  });
});

// 2. The 50 Hz touch voltage (RMS within the Stage 9 measured-RMS tolerance).
{
  const group = 5;
  const { source, second, third } = ids(group);
  const E = 230;
  const Rs = 1;
  const Rf = 10;
  const Re = 9;
  const rms = (Re * E) / (Rs + Rf + Re);
  fixtures.push({
    id: "earth-loop-ac",
    family: "touch-voltage",
    variant: "correct",
    description:
      "230 V RMS, 50 Hz, leak 10 ohm, electrode 9 ohm bonded to the frame",
    request: request(
      [
        acSource(source, E, Rs),
        electrode(second, Re, [10, 0]),
        frame(third, 50),
      ],
      { faults: leak(group, source, third, Rf) },
    ),
    checks: [
      value(60, key(third, "touch_voltage_rms"), rms, 2e-3),
      value(60, key(third, "touch_voltage_peak"), sqrt2 * rms, 2e-3),
      value(60, key(third, "hazard"), 1, 0),
    ],
  });
  fixtures.push({
    id: "earth-loop-ac-no-earth",
    family: "touch-voltage",
    variant: "wrong",
    wrongOf: "earth-loop-ac",
    description:
      "no ground marker and no electrode: there is no earth node to read against",
    request: request([acSource(source, E, Rs), frame(third, 50)], {
      marker: false,
      faults: leak(group, source, third, Rf),
    }),
    checks: [
      value(60, key(third, "valid"), 0, 0),
      value(60, key(third, "hazard"), 0, 0),
    ],
  });
}

// 3. The residual-current device: a load whose whole current is a residual (it returns to the
// source, not through the device) and a balanced one.
const RCD = { rated: 0.03, T: 0.1, sense: 0.02 };
const rcdParameters = (extra = {}) => ({
  ratedResidualCurrentAmps: RCD.rated,
  tripTimeAtRatedSeconds: RCD.T,
  residualSenseSeconds: RCD.sense,
  closedContactResistanceOhms: 0.0001,
  initialClosed: 1,
  ...extra,
});
const rcdCircuit = (group, amps, balanced, parameters) => {
  const { source, second, third } = ids(group);
  const return2 = balanced ? [20, 0] : [2, 2];
  return request([
    dcSource(source, 100, 0),
    part(second, "resistor", { resistanceOhms: 100 / amps }, [
      ["pin1", [10, 0]],
      ["pin2", return2],
    ]),
    part(third, "rcd_two_pole", parameters, [
      ["l1", [0, 0]],
      ["t1", [10, 0]],
      ["l2", balanced ? [2, 2] : [30, 30]],
      ["t2", balanced ? [20, 0] : [31, 30]],
    ]),
  ]);
};
[1.05, 2, 5].forEach((ratio, index) => {
  const group = 6 + index;
  const { third } = ids(group);
  const expected = rcdTripTime(ratio, { tau: RCD.sense, T: RCD.T });
  fixtures.push({
    id: `rcd-trip-${ratio}`,
    family: "rcd",
    variant: "correct",
    description: `a residual of ${ratio} x the rated current trips after the law of the contract`,
    request: rcdCircuit(group, ratio * RCD.rated, false, rcdParameters()),
    checks: [
      {
        kind: "tripStep",
        key: key(third, "tripped"),
        expectedSeconds: expected,
        toleranceSeconds: Math.max(2e-3, 0.01 * expected),
        horizonSteps: 5000,
      },
    ],
  });
  // Wrong: a rated current one and a half times as high sees a smaller residual.
  const stiff = rcdTripTime(ratio / 1.5, { tau: RCD.sense, T: RCD.T });
  fixtures.push({
    id: `rcd-trip-${ratio}-rated-raised`,
    family: "rcd",
    variant: "wrong",
    wrongOf: `rcd-trip-${ratio}`,
    description:
      "the rated residual current is set to one and a half times the value of the correct device",
    request: rcdCircuit(
      group,
      ratio * RCD.rated,
      false,
      rcdParameters({ ratedResidualCurrentAmps: 1.5 * RCD.rated }),
    ),
    checks:
      stiff === null
        ? [
            {
              kind: "tripStep",
              key: key(third, "tripped"),
              expectedSeconds: null,
              toleranceSeconds: 0,
              horizonSteps: 5000,
            },
          ]
        : [
            {
              kind: "tripStep",
              key: key(third, "tripped"),
              expectedSeconds: stiff,
              toleranceSeconds: Math.max(2e-3, 0.01 * stiff),
              horizonSteps: 5000,
            },
          ],
  });
});
{
  const group = 9;
  const { third } = ids(group);
  fixtures.push({
    id: "rcd-below-rated",
    family: "rcd",
    variant: "correct",
    description: "half the rated residual current never trips the device",
    request: rcdCircuit(group, 0.5 * RCD.rated, false, rcdParameters()),
    checks: [
      {
        kind: "tripStep",
        key: key(third, "tripped"),
        expectedSeconds: null,
        toleranceSeconds: 0,
        horizonSteps: 3000,
      },
    ],
  });
  fixtures.push({
    id: "rcd-balanced",
    family: "rcd",
    variant: "correct",
    description:
      "a load that returns through the device has no residual and never trips it",
    request: rcdCircuit(10, 1, true, rcdParameters()),
    checks: [
      {
        kind: "tripStep",
        key: key(ids(10).third, "tripped"),
        expectedSeconds: null,
        toleranceSeconds: 0,
        horizonSteps: 3000,
      },
      value(500, key(ids(10).third, "residual_rms"), 0, 0, 1e-9),
    ],
  });
}

// 4. Breaker trip classes. K = 2 s keeps the thermal runs short.
const BREAKER = { rated: 10, tau: 0.02, K: 2, cool: 5 };
const breakerParameters = (curveClass, extra = {}) => ({
  closedContactResistanceOhms: 0.0001,
  instantaneousTripAmps: 100000,
  initialClosed: 1,
  curveClass,
  ratedCurrentAmps: BREAKER.rated,
  thermalSenseSeconds: BREAKER.tau,
  tripScaleSeconds: BREAKER.K,
  coolSeconds: BREAKER.cool,
  ...extra,
});
const breakerCircuit = (group, amps, parameters) => {
  const { source, second, third } = ids(group);
  return request([
    dcSource(source, 100, 0),
    part(second, "resistor", { resistanceOhms: 100 / amps }, [
      ["pin1", [10, 0]],
      ["pin2", [2, 2]],
    ]),
    part(third, "breaker_three_pole", parameters, [
      ["l1", [0, 0]],
      ["t1", [10, 0]],
      ["l2", [30, 30]],
      ["t2", [31, 30]],
      ["l3", [40, 40]],
      ["t3", [41, 40]],
    ]),
  ]);
};
const classes = [
  { class: 1, name: "B", multiple: 5 },
  { class: 2, name: "C", multiple: 10 },
  { class: 3, name: "D", multiple: 20 },
];
classes.forEach((c, index) => {
  const group = 11 + index;
  const { third } = ids(group);
  fixtures.push({
    id: `breaker-${c.name}-magnetic-above`,
    family: "breaker",
    variant: "correct",
    description: `class ${c.name}: 1.02 x ${c.multiple} x the rated current trips at the first sample`,
    request: breakerCircuit(
      group,
      1.02 * c.multiple * BREAKER.rated,
      breakerParameters(c.class),
    ),
    checks: [value(0, key(third, "tripped"), 1, 0)],
  });
  fixtures.push({
    id: `breaker-${c.name}-magnetic-below`,
    family: "breaker",
    variant: "correct",
    description: `class ${c.name}: 0.98 x ${c.multiple} x the rated current does not trip at once`,
    request: breakerCircuit(
      group,
      0.98 * c.multiple * BREAKER.rated,
      breakerParameters(c.class),
    ),
    checks: [value(0, key(third, "tripped"), 0, 0)],
  });
});
// A class swapped: a class D device where class B is needed does not trip at 6 x In at once.
fixtures.push({
  id: "breaker-B-magnetic-above-as-class-D",
  family: "breaker",
  variant: "wrong",
  wrongOf: "breaker-B-magnetic-above",
  description:
    "class D set on the breaker that should be class B: 1.02 x 5 x In is far below 20 x In",
  request: breakerCircuit(11, 1.02 * 5 * BREAKER.rated, breakerParameters(3)),
  checks: [value(0, key(ids(11).third, "tripped"), 0, 0)],
});
[
  { class: 3, name: "D", ratio: 5.5 },
  { class: 2, name: "C", ratio: 3 },
  { class: 1, name: "B", ratio: 2 },
  { class: 1, name: "B", ratio: 1.2 },
].forEach((c, index) => {
  const group = 14 + index;
  const { third } = ids(group);
  const expected = breakerThermalTime(c.ratio, BREAKER);
  fixtures.push({
    id: `breaker-${c.name}-thermal-${c.ratio}`,
    family: "breaker",
    variant: "correct",
    description: `class ${c.name} at ${c.ratio} x the rated current trips after the thermal law`,
    request: breakerCircuit(
      group,
      c.ratio * BREAKER.rated,
      breakerParameters(c.class),
    ),
    checks: [
      {
        kind: "tripStep",
        key: key(third, "tripped"),
        expectedSeconds: expected,
        toleranceSeconds: Math.max(2e-3, 0.01 * expected),
        horizonSteps: 20000,
      },
    ],
  });
});
fixtures.push({
  id: "breaker-thermal-rated-current-never-trips",
  family: "breaker",
  variant: "correct",
  description: "0.9 x the rated current never trips the thermal part",
  request: breakerCircuit(12, 0.9 * BREAKER.rated, breakerParameters(1)),
  checks: [
    {
      kind: "tripStep",
      key: key(ids(12).third, "tripped"),
      expectedSeconds: null,
      toleranceSeconds: 0,
      horizonSteps: 5000,
    },
  ],
});
{
  // Wrong: the rated current is half what it should be, so 2 x In is really 4 x In.
  const wrongTime = breakerThermalTime(4, BREAKER);
  fixtures.push({
    id: "breaker-B-thermal-2-rated-halved",
    family: "breaker",
    variant: "wrong",
    wrongOf: "breaker-B-thermal-2",
    description:
      "the rated current is set to half the value of the correct breaker",
    request: breakerCircuit(
      16,
      2 * BREAKER.rated,
      breakerParameters(1, { ratedCurrentAmps: BREAKER.rated / 2 }),
    ),
    checks: [
      {
        kind: "tripStep",
        key: key(ids(16).third, "tripped"),
        expectedSeconds: wrongTime,
        toleranceSeconds: Math.max(2e-3, 0.01 * wrongTime),
        horizonSteps: 20000,
      },
    ],
  });
}

// 5. Short circuits: a drawn 1 milliohm resistor across the source, bounded by the source's
// internal resistance, with and without a class C breaker in the loop.
{
  const group = 17;
  const { source, second, third } = ids(group);
  const E = 100;
  const Rs = 0.5;
  const short = 0.001;
  const expected = E / (Rs + short);
  const build = (breakerAmps) => {
    const parts = [
      dcSource(source, E, Rs),
      part(second, "resistor", { resistanceOhms: short }, [
        ["pin1", [10, 0]],
        ["pin2", [2, 2]],
      ]),
    ];
    if (breakerAmps === null) {
      // The short is wired straight to the source's positive pin.
      parts[1] = part(second, "resistor", { resistanceOhms: short }, [
        ["pin1", [0, 0]],
        ["pin2", [2, 2]],
      ]);
    } else {
      parts.push(
        part(
          third,
          "breaker_three_pole",
          breakerParameters(2, { initialClosed: 1 }),
          [
            ["l1", [0, 0]],
            ["t1", [10, 0]],
            ["l2", [30, 30]],
            ["t2", [31, 30]],
            ["l3", [40, 40]],
            ["t3", [41, 40]],
          ],
        ),
      );
    }
    return request(parts);
  };
  fixtures.push({
    id: "short-circuit-bounded-by-the-source",
    family: "short-circuit",
    variant: "correct",
    description:
      "a 1 mOhm short across a 100 V source with 0.5 ohm internal resistance",
    request: build(null),
    checks: [value(5, key(source, "current"), -expected, 1e-6, 1e-9)],
  });
  const throughBreaker = E / (Rs + short + 0.0001);
  fixtures.push({
    id: "short-circuit-cleared-by-class-C",
    family: "short-circuit",
    variant: "correct",
    description: `the same short through a class C breaker (threshold 100 A): ${throughBreaker.toFixed(1)} A trips it at the first sample`,
    request: build(1),
    checks: [value(0, key(third, "tripped"), throughBreaker >= 100 ? 1 : 0, 0)],
  });
  fixtures.push({
    id: "short-circuit-with-a-wrong-resistance",
    family: "short-circuit",
    variant: "wrong",
    wrongOf: "short-circuit-bounded-by-the-source",
    description: "the short is 10 mOhm instead of 1 mOhm",
    request: request([
      dcSource(source, E, Rs),
      part(second, "resistor", { resistanceOhms: 10 * short }, [
        ["pin1", [0, 0]],
        ["pin2", [2, 2]],
      ]),
    ]),
    checks: [
      value(5, key(source, "current"), -E / (Rs + 10 * short), 1e-6, 1e-9),
    ],
  });
}

// 6. The residual-current device on a 50 Hz residual: the detector filters the square of the
// sinusoid, so the oracle integrates the instantaneous waveform itself.
function rcdTripTimeAc(ratio, { tau, T }, horizon = 30) {
  const step = 1e-6;
  const omega = 2 * Math.PI * 50;
  let x = 0;
  let dose = 0;
  for (let t = step; t < horizon; t += step) {
    const i = (sqrt2 * ratio * Math.sin(omega * t)) ** 2;
    x += ((i - x) * step) / tau;
    const level = Math.sqrt(x);
    dose = level >= 1 ? dose + (step * level * level) / T : 0;
    if (dose >= 1) return t;
  }
  return null;
}
const rcdAcCircuit = (group, residualRms, rated = 1) => {
  const { source, second, third } = ids(group);
  return request([
    acSource(source, 230, 0.001),
    part(second, "resistor", { resistanceOhms: 230 / residualRms }, [
      ["pin1", [10, 0]],
      ["pin2", [2, 2]],
    ]),
    part(
      third,
      "rcd_two_pole",
      rcdParameters({ ratedResidualCurrentAmps: rated }),
      [
        ["l1", [0, 0]],
        ["t1", [10, 0]],
        ["l2", [30, 30]],
        ["t2", [31, 30]],
      ],
    ),
  ]);
};
// The rated residual current is 1 A here, so a residual of `ratio` amperes RMS is `ratio` x In.
[1.25, 2, 5].forEach((ratio, index) => {
  const group = 18 + index;
  const expected = rcdTripTimeAc(ratio, { tau: RCD.sense, T: RCD.T });
  fixtures.push({
    id: `rcd-trip-ac-${ratio}`,
    family: "rcd",
    variant: "correct",
    description: `a 50 Hz residual of ${ratio} x the rated current (RMS) trips after the law of the contract`,
    request: rcdAcCircuit(group, ratio),
    checks: [
      {
        kind: "tripStep",
        key: key(ids(group).third, "tripped"),
        expectedSeconds: expected,
        toleranceSeconds: Math.max(3e-3, 0.02 * expected),
        horizonSteps: 5000,
      },
    ],
  });
});
fixtures.push({
  id: "rcd-below-rated-ac",
  family: "rcd",
  variant: "correct",
  description:
    "a 50 Hz residual of half the rated current never trips the device",
  request: rcdAcCircuit(21, 0.5),
  checks: [
    {
      kind: "tripStep",
      key: key(ids(21).third, "tripped"),
      expectedSeconds: null,
      toleranceSeconds: 0,
      horizonSteps: 3000,
    },
  ],
});
{
  // Wrong: the same residual on a device rated at half the current (the largest rated
  // residual current is 1 A, so the correct AC device is rated at its maximum).
  const stiff = rcdTripTimeAc(2 / 0.5, { tau: RCD.sense, T: RCD.T });
  fixtures.push({
    id: "rcd-trip-ac-2-rated-halved",
    family: "rcd",
    variant: "wrong",
    wrongOf: "rcd-trip-ac-2",
    description:
      "the rated residual current is set to half the value of the correct device",
    request: rcdAcCircuit(19, 2, 0.5),
    checks: [
      {
        kind: "tripStep",
        key: key(ids(19).third, "tripped"),
        expectedSeconds: stiff,
        toleranceSeconds: stiff === null ? 0 : Math.max(3e-3, 0.02 * stiff),
        horizonSteps: 5000,
      },
    ],
  });
}

// 7. The short circuit cleared by a class 0 breaker whose instantaneous setting is below the
// prospective current: it trips at the first sample and the current is then zero.
{
  const E = 100;
  const Rs = 0.5;
  const short = 0.001;
  const classZero = (group, setting) => {
    const { source, second, third } = ids(group);
    return request([
      dcSource(source, E, Rs),
      part(second, "resistor", { resistanceOhms: short }, [
        ["pin1", [10, 0]],
        ["pin2", [2, 2]],
      ]),
      part(
        third,
        "breaker_three_pole",
        {
          closedContactResistanceOhms: 0.0001,
          instantaneousTripAmps: setting,
          initialClosed: 1,
        },
        [
          ["l1", [0, 0]],
          ["t1", [10, 0]],
          ["l2", [30, 30]],
          ["t2", [31, 30]],
          ["l3", [40, 40]],
          ["t3", [41, 40]],
        ],
      ),
    ]);
  };
  fixtures.push({
    id: "short-circuit-cleared-by-class-0",
    family: "short-circuit",
    variant: "correct",
    description:
      "the 1 mOhm short through a class 0 breaker set to 50 A, below the prospective current",
    request: classZero(22, 50),
    checks: [
      value(0, key(ids(22).third, "tripped"), 1, 0),
      value(5, key(ids(22).source, "current"), 0, 0, 1e-9),
    ],
  });
  fixtures.push({
    id: "short-circuit-class-0-set-too-high",
    family: "short-circuit",
    variant: "wrong",
    wrongOf: "short-circuit-cleared-by-class-0",
    description:
      "the instantaneous setting is 500 A, above the prospective current: nothing trips and the short carries it",
    request: classZero(27, 500),
    checks: [
      value(0, key(ids(27).third, "tripped"), 0, 0),
      value(
        5,
        key(ids(27).source, "current"),
        -E / (Rs + short + 0.0001),
        1e-6,
        1e-9,
      ),
    ],
  });
}

// 8. The three-phase prospective peak: a short between l1 and l2 of a purely resistive star
// source (neutral not connected), read by an AC ammeter in the loop.
const phasePeak = (group, resistance) => {
  const { source, second, third } = ids(group);
  return request(
    [
      part(
        source,
        "three_phase_source_star",
        {
          phaseVoltageRmsVolts: 230,
          frequencyHz: 50,
          phaseOffsetDegrees: 0,
          internalResistanceOhms: 0.5,
          reversedSequence: 0,
        },
        [
          ["l1", [0, 0]],
          ["l2", [3, 0]],
          ["l3", [4, 0]],
          ["n", [90, 90]],
        ],
      ),
      part(second, "resistor", { resistanceOhms: resistance }, [
        ["pin1", [0, 0]],
        ["pin2", [50, 0]],
      ]),
      part(
        third,
        "ac_ammeter",
        { burdenResistanceOhms: 0.0001, frequencyHz: 50 },
        [
          ["pin1", [50, 0]],
          ["pin2", [3, 0]],
        ],
      ),
    ],
    { marker: false },
  );
};
{
  const peak = (short) =>
    (sqrt2 * Math.sqrt(3) * 230) / (2 * 0.5 + short + 0.0001);
  fixtures.push({
    id: "short-circuit-three-phase-peak",
    family: "short-circuit",
    variant: "correct",
    description:
      "a 1 mOhm short between two phases of a resistive star source: peak sqrt(2) E_LL / (2 Rs + 1 mOhm + burden)",
    request: phasePeak(23, 0.001),
    checks: [value(60, key(ids(23).third, "peak_current"), peak(0.001), 5e-3)],
  });
  fixtures.push({
    id: "short-circuit-three-phase-peak-10-milliohm",
    family: "short-circuit",
    variant: "wrong",
    wrongOf: "short-circuit-three-phase-peak",
    description: "the short is 10 mOhm instead of 1 mOhm",
    request: phasePeak(24, 0.01),
    checks: [value(60, key(ids(24).third, "peak_current"), peak(0.01), 5e-3)],
  });
}

// 9. A TN-style earth fault on one phase of a star source feeding a balanced star load with the
// neutral wired back to the source: a leak on l1 to a frame bonded to the electrode.
{
  const E = 230;
  const Rs = 0.5;
  const Rl = 20;
  const Rf = 50;
  const Re = 30;
  const build = (group, bonded) => {
    const { source, second, third } = ids(group);
    const frameId = uuid(group, 4);
    return request(
      [
        part(
          source,
          "three_phase_source_star",
          {
            phaseVoltageRmsVolts: E,
            frequencyHz: 50,
            phaseOffsetDegrees: 0,
            internalResistanceOhms: Rs,
            reversedSequence: 0,
          },
          [
            ["l1", [0, 0]],
            ["l2", [3, 0]],
            ["l3", [4, 0]],
            ["n", [2, 2]],
          ],
        ),
        part(
          second,
          "three_phase_load_star",
          {
            phase1ResistanceOhms: Rl,
            phase2ResistanceOhms: Rl,
            phase3ResistanceOhms: Rl,
            phase1InductanceHenries: 0,
            phase2InductanceHenries: 0,
            phase3InductanceHenries: 0,
          },
          [
            ["l1", [0, 0]],
            ["l2", [3, 0]],
            ["l3", [4, 0]],
            ["n", [2, 2]],
          ],
        ),
        electrode(third, Re, bonded ? [10, 0] : [20, 0]),
        frame(frameId, 50),
      ],
      { faults: leak(group, source, frameId, Rf, "l1") },
    );
  };
  // Phase 1 carries the load and the leak from one node; the other phases are untouched.
  const node = (earthPath) => E / (1 + Rs * (1 / Rl + 1 / earthPath));
  const touchBonded = (Re * node(Rf + Re)) / (Rf + Re);
  fixtures.push({
    id: "earth-fault-three-phase-tn",
    family: "earth-fault",
    variant: "correct",
    description:
      "star source, balanced star load with the neutral at the source, a leak on l1 to a bonded frame",
    request: build(25, true),
    checks: [
      value(60, key(uuid(25, 4), "touch_voltage_rms"), touchBonded, 5e-3),
      value(60, key(uuid(25, 4), "hazard"), touchBonded > 50 ? 1 : 0, 0),
    ],
  });
  const touchUnbonded = node(Rf + 1e9) * (1e9 / (Rf + 1e9));
  fixtures.push({
    id: "earth-fault-three-phase-tn-unbonded",
    family: "earth-fault",
    variant: "wrong",
    wrongOf: "earth-fault-three-phase-tn",
    description:
      "the frame is not bonded to the electrode: it floats to the voltage of the leaking phase",
    request: build(26, false),
    checks: [
      value(60, key(uuid(26, 4), "touch_voltage_rms"), touchUnbonded, 5e-3),
      value(60, key(uuid(26, 4), "hazard"), 1, 0),
    ],
  });
}

// ------------------------------------------------------------------- assembling

const analytical = {
  sign: "A source's current is negative when it delivers current (current enters the first pin).",
  observation:
    "value checks read a layout key after `step` public 1 ms steps; tripStep checks step until the key is 1 and compare the time",
  fixtures,
};

const body = text(analytical);
const manifest = {
  generator: "node scripts/stage13-references.mjs --write",
  tools: {
    node: { generatedWith: "22.20.0", supported: ">=20" },
    dependencies:
      "none (IEEE-754 arithmetic with Math.exp, Math.sqrt and Math.sin only; closed forms and a 1 microsecond integration)",
  },
  analyticalSha256: digest(body),
  fixtureCount: fixtures.length,
  families: Object.fromEntries(
    [...new Set(fixtures.map((f) => f.family))].map((family) => [
      family,
      fixtures.filter((f) => f.family === family).length,
    ]),
  ),
  wrongVariants: fixtures.filter((f) => f.variant === "wrong").length,
};
const manifestBody = text(manifest);

// ------------------------------------------------------------------- consistency

const correct = new Map(
  fixtures.filter((f) => f.variant === "correct").map((f) => [f.id, f]),
);
const sameChecks = (a, b) =>
  JSON.stringify(a.checks) === JSON.stringify(b.checks);
for (const wrong of fixtures.filter((f) => f.variant === "wrong")) {
  const original = correct.get(wrong.wrongOf);
  assert.ok(original, `${wrong.id} names no correct fixture`);
  assert.notEqual(
    JSON.stringify(wrong.request),
    JSON.stringify(original.request),
    `${wrong.id} builds the same circuit as ${wrong.wrongOf}`,
  );
  assert.ok(
    !sameChecks(wrong, original),
    `${wrong.id} expects the same results`,
  );
}
assert.equal(
  new Set(fixtures.map((f) => f.id)).size,
  fixtures.length,
  "duplicate fixture IDs",
);

// ----------------------------------------------------------------------- the run

const args = process.argv.slice(2);
assert.ok(
  args.length === 1 && ["--check", "--write"].includes(args[0]),
  "Use --check or --write",
);
if (args[0] === "--write") {
  mkdirSync(directory, { recursive: true });
  writeFileSync(`${directory}analytical.json`, body);
  writeFileSync(`${directory}manifest.json`, manifestBody);
  console.log(`Wrote ${fixtures.length} Stage 13 reference circuits.`);
} else {
  assert.equal(
    readFileSync(`${directory}analytical.json`, "utf8"),
    body,
    "analytical.json is out of date: run --write",
  );
  assert.equal(
    readFileSync(`${directory}manifest.json`, "utf8"),
    manifestBody,
    "manifest.json is out of date: run --write",
  );
  console.log(
    `Verified ${fixtures.length} Stage 13 reference circuits (${manifest.wrongVariants} wrong variants).`,
  );
}
