// The Stage 10 oracles have no dependency on the application or the Engine.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const directory = fileURLToPath(
  new URL("../tests/golden/stage-10/", import.meta.url),
);
const version = "ngspice-47";
const archiveHash =
  "59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a";
const options =
  "method=trap reltol=1e-9 vntol=1e-10 abstol=1e-12 temp=27 tnom=27";
const spiceStep = 10e-6;
const phaseDegrees = [0, -120, 120];
const tau = 2 * Math.PI;
const sqrt2 = Math.sqrt(2);
const zero = [0, 0];

const digest = (value) => createHash("sha256").update(value).digest("hex");
const json = (value) => `${JSON.stringify(value, null, 2)}\n`;
const add = ([a, b], [c, d]) => [a + c, b + d];
const sub = ([a, b], [c, d]) => [a - c, b - d];
const mul = ([a, b], [c, d]) => [a * c - b * d, a * d + b * c];
const div = ([a, b], [c, d]) => [
  (a * c + b * d) / (c * c + d * d),
  (b * c - a * d) / (c * c + d * d),
];
const sinusoid = ([re, im], angle) =>
  sqrt2 * (re * Math.sin(angle) + im * Math.cos(angle));
const phasePhasors = (volts) =>
  phaseDegrees.map((deg) => {
    const angle = (deg * tau) / 360;
    return [volts * Math.cos(angle), volts * Math.sin(angle)];
  });
const phaseSources = (volts, hz) =>
  phaseDegrees.map(
    (phase, i) =>
      `vline${i} line${i} 0 ac ${volts} ${phase} sin(0 ${volts * sqrt2} ${hz} 0 0 ${phase})`,
  );
const samples = (start, end, stride = 5) =>
  Array.from(
    { length: Math.floor((end - start) / stride) + 1 },
    (_, i) => (start + i * stride) / 1000,
  );

function deck(fixture) {
  const { id, vectors, stop, body, acHz, initialConditions = false } = fixture;
  const analysis = acHz
    ? `.ac lin 1 ${acHz} ${acHz}`
    : `.tran ${spiceStep} ${stop} 0 ${spiceStep}${initialConditions ? " uic" : ""}`;
  return [
    `Open Circuit Stage 10 reference: ${id}`,
    `.options ${options}`,
    ...body,
    analysis,
    ".control",
    "set noaskquit",
    "set numdgt=16",
    "set wr_singlescale",
    "run",
    `wrdata ${id}.out ${vectors.map(({ expression }) => expression).join(" ")}`,
    "quit",
    ".endc",
    ".end",
    "",
  ].join("\n");
}

function resultRows(fixture, oracle) {
  return fixture.times.map((time) =>
    [time, ...oracle(time)].map((value) => {
      assert(Number.isFinite(value), `${fixture.id}: nonfinite reference`);
      return Number(value.toPrecision(13));
    }),
  );
}

function measure(trace, time) {
  let left = 0;
  let right = trace.length - 1;
  while (right - left > 1) {
    const middle = (left + right) >>> 1;
    if (trace[middle][0] < time) left = middle;
    else right = middle;
  }
  const [a, ...low] = trace[left];
  const [b, ...high] = trace[right];
  assert(
    time >= a - spiceStep && time <= b + spiceStep,
    `sample ${time} is outside trace`,
  );
  return low.map(
    (value, i) => value + ((high[i] - value) * (time - a)) / (b - a),
  );
}

function runSpice(executable, fixtures) {
  const banner = execFileSync(executable, ["--version"], { encoding: "utf8" });
  assert(banner.includes(`${version} : Circuit`), `expected ${version}`);
  const temporary = mkdtempSync(join(tmpdir(), "open-circuit-stage10-"));
  try {
    return Object.fromEntries(
      fixtures.map((fixture) => {
        const { id, vectors, acHz } = fixture;
        writeFileSync(join(temporary, `${id}.cir`), deck(fixture));
        const log = execFileSync(executable, ["-b", `${id}.cir`], {
          cwd: temporary,
          stdio: "pipe",
          timeout: 60_000,
          encoding: "utf8",
        });
        const file = readFileSync(join(temporary, `${id}.out`), "utf8");
        const trace = file
          .trim()
          .split(/\r?\n/)
          .map((row) => row.trim().split(/\s+/).map(Number));
        if (acHz) {
          assert(
            trace.length === 1 && trace[0].length === 1 + 2 * vectors.length,
            id,
          );
          const phasors = vectors.map((_, i) => [
            trace[0][1 + 2 * i],
            trace[0][2 + 2 * i],
          ]);
          return [
            id,
            resultRows(fixture, (time) =>
              phasors.map((p) => sinusoid(p, tau * acHz * time)),
            ),
          ];
        }
        assert(
          trace.length > 2 &&
            trace.every(
              (row) =>
                row.length === vectors.length + 1 && row.every(Number.isFinite),
            ),
          id,
        );
        assert(
          trace.at(-1)[0] >= fixture.times.at(-1),
          `${id}: incomplete transient\n${log.slice(-2000)}`,
        );
        return [id, resultRows(fixture, (time) => measure(trace, time))];
      }),
    );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

function load({ id, hz, delta, floating, inductive }) {
  const resistance = [10, 16, 24];
  const inductance = inductive ? [0.005, 0.009, 0.015] : [0, 0, 0];
  const branches = phaseDegrees.map((_, i) => {
    const from = `line${i}`;
    const to = delta ? `line${(i + 1) % 3}` : floating ? "neutral" : "0";
    return {
      from,
      to,
      elements: [
        `vload${i} ${from} sense${i} 0`,
        inductive
          ? `rload${i} sense${i} coil${i} ${resistance[i]}\nlcoil${i} coil${i} ${to} ${inductance[i]}`
          : `rload${i} sense${i} ${to} ${resistance[i]}`,
      ],
    };
  });
  const source = phasePhasors(48);
  const impedance = resistance.map((r, i) => [r, tau * hz * inductance[i]]);
  const neutral = floating
    ? div(
        source.map((v, i) => div(v, impedance[i])).reduce(add),
        impedance.map((z) => div([1, 0], z)).reduce(add),
      )
    : zero;
  const voltage = source.map((v, i) =>
    sub(v, delta ? source[(i + 1) % 3] : neutral),
  );
  const current = voltage.map((v, i) => div(v, impedance[i]));
  return {
    id,
    acHz: hz,
    kind: delta ? "three_phase_load_delta" : "three_phase_load_star",
    inputs: {
      hz,
      delta,
      floating,
      inductive,
      resistance,
      inductance,
      phaseRmsVolts: 48,
    },
    body: [
      ...phaseSources(48, hz),
      ...branches.flatMap(({ elements }) => elements),
    ],
    vectors: branches.flatMap(({ from, to }, i) => [
      {
        key: `branch${i + 1}/voltage`,
        expression: to === "0" ? `v(${from})` : `v(${from},${to})`,
        tolerance: 0.01,
      },
      {
        key: `branch${i + 1}/current`,
        expression: `i(vload${i})`,
        tolerance: 0.001,
      },
    ]),
    times: samples(100, 195),
    stop: 0.2,
    analytical: (time) =>
      voltage.flatMap((v, i) =>
        [v, current[i]].map((x) => sinusoid(x, tau * hz * time)),
      ),
  };
}

function transformer({ id, hz, loaded }) {
  const resistance = [10, 16, 24];
  const inductance = [0.005, 0.009, 0.015];
  const turnsRatio = 2;
  const lp = 0.054,
    ls = 0.0185,
    mutual = 0.025;
  const w = tau * hz;
  const zp = [2, w * lp],
    zs = [1, w * ls],
    zm = [0, w * mutual];
  const oracles = phasePhasors(48).map((voltage, i) => {
    if (!loaded) {
      const ip = div(voltage, zp);
      return [ip, zero, mul(zm, ip)];
    }
    const loadZ = [resistance[i], w * inductance[i]];
    const totalZ = add(zs, loadZ);
    const ip = div(voltage, sub(zp, div(mul(zm, zm), totalZ)));
    const is = div(sub(zero, mul(zm, ip)), totalZ);
    return [ip, is, sub(zero, mul(loadZ, is))];
  });
  const body = [...phaseSources(48, hz)];
  for (let i = 0; i < 3; i++) {
    body.push(
      `vp${i} line${i} probe${i} 0`,
      `rp${i} probe${i} primary${i} 2`,
      `lp${i} primary${i} 0 ${lp}`,
      `ls${i} secondary${i} 0 ${ls}`,
      `kbank${i} lp${i} ls${i} ${mutual / Math.sqrt(lp * ls)}`,
      `vsec${i} output${i} sensedsec${i} 0`,
      `rs${i} sensedsec${i} secondary${i} 1`,
    );
    if (loaded)
      body.push(
        `rload${i} output${i} load${i} ${resistance[i]}`,
        `lload${i} load${i} 0 ${inductance[i]}`,
      );
    else body.push(`rprobe${i} output${i} 0 1e9`);
  }
  return {
    id,
    acHz: hz,
    kind: "transformer_three_phase",
    inputs: {
      hz,
      loaded,
      turnsRatio,
      lp,
      ls,
      mutual,
      primaryResistance: 2,
      secondaryResistance: 1,
      resistance,
      inductance,
    },
    body,
    vectors: phaseDegrees.flatMap((_, i) => [
      {
        key: `primary${i + 1}/current`,
        expression: `i(vp${i})`,
        tolerance: 0.001,
      },
      {
        key: `secondary${i + 1}/current`,
        expression: `i(vsec${i})`,
        tolerance: 0.001,
      },
      {
        key: `secondary${i + 1}/voltage`,
        expression: `v(output${i})`,
        tolerance: 0.01,
      },
    ]),
    times: samples(400, 495),
    stop: 0.5,
    analytical: (time) =>
      oracles.flat().map((value) => sinusoid(value, w * time)),
  };
}

function contactor(hz) {
  const id = `contactor-${hz}`;
  const resistance = 10,
    inductance = 0.01,
    senseSeconds = 0.02;
  const high = 10,
    low = 1,
    switchTime = 0.3,
    endTime = 0.5;
  const omega = tau * hz;
  const impedance = Math.hypot(resistance, omega * inductance);
  const lag = Math.atan2(omega * inductance, resistance);
  const forcing = (rms, t) =>
    ((sqrt2 * rms) / impedance) * Math.sin(omega * t - lag);
  const coil = (t) =>
    t <= switchTime
      ? forcing(high, t) -
        forcing(high, 0) * Math.exp((-t * resistance) / inductance)
      : forcing(low, t) +
        (forcing(high - low, switchTime) -
          forcing(high, 0) *
            Math.exp((-switchTime * resistance) / inductance)) *
          Math.exp((-(t - switchTime) * resistance) / inductance);
  const pickup = 0.7,
    dropout = 0.4;
  const decay = Math.exp(-spiceStep / senseSeconds);
  let detector = 0,
    picked = false;
  const trace = [];
  for (let i = 1; i <= Math.round(endTime / spiceStep); i++) {
    detector =
      decay * detector + (1 - decay) * coil((i - 0.5) * spiceStep) ** 2;
    if (!picked && detector >= pickup ** 2) picked = true;
    else if (picked && detector <= dropout ** 2) picked = false;
    if (i % 100 === 0)
      trace[i / 100] = [coil(i * spiceStep), detector, Number(picked)];
  }
  return {
    id,
    kind: "contactor_three_pole",
    stop: endTime,
    initialConditions: true,
    inputs: {
      hz,
      resistance,
      inductance,
      senseSeconds,
      pickup,
      dropout,
      high,
      low,
      switchTime,
    },
    body: [
      `bsource source 0 v=ternary_fcn(time<${switchTime},${high * sqrt2},${low * sqrt2})*sin(${omega}*time)`,
      `rcoil source probe ${resistance}`,
      "vcoil probe winding 0",
      `lcoil winding 0 ${inductance} ic=0`,
      "bfilter 0 detector i=i(vcoil)*i(vcoil)",
      "csense detector 0 0.02 ic=0",
      "rsense detector 0 1",
      "vflag supply 0 1",
      "rflag supply output 1",
      "sflag output 0 detector 0 pickup",
      ".model pickup sw(ron=1e-6 roff=1e9 vt=0.325 vh=0.165)",
      "bstate picked 0 v=1-v(output)",
    ],
    vectors: [
      { key: "coil/current", expression: "i(vcoil)", tolerance: 0.002 },
      {
        key: "coil/current_squared",
        expression: "v(detector)",
        tolerance: 0.002,
      },
      { key: "picked", expression: "v(picked)", tolerance: 0.0001 },
    ],
    times: samples(5, 495),
    analytical: (time) => trace[Math.round(time * 1000)],
  };
}

function overload(multiple, ac = false) {
  const id = ac
    ? "overload-ac-50"
    : `overload-dc-${multiple.toString().replace(".", "p")}`;
  const stop = 0.25,
    rated = 10,
    senseSeconds = 0.02,
    tripScale = 0.1,
    cool = 0.1;
  const sources = ac
    ? phaseSources(rated * multiple, 50)
    : [2, -rated * multiple, 3].map(
        (amps, i) => `vline${i} line${i} 0 dc ${amps}`,
      );
  const phaseAt = (t) =>
    ac
      ? phasePhasors(rated * multiple).map((value) =>
          sinusoid(value, tau * 50 * t),
        )
      : [2, -rated * multiple, 3];
  const decay = Math.exp(-spiceStep / senseSeconds);
  let q = 0,
    dose = 0;
  const trace = [];
  for (let i = 1; i <= Math.round(stop / spiceStep); i++) {
    const x = Math.max(
      ...phaseAt((i - 0.5) * spiceStep).map((phase) => (phase / rated) ** 2),
    );
    q = decay * q + (1 - decay) * x;
    dose =
      q > 1
        ? Math.min(1, dose + (spiceStep * (q - 1)) / tripScale)
        : dose * Math.exp(-spiceStep / cool);
    if (i % 100 === 0) trace[i / 100] = [q, dose, Number(dose === 1)];
  }
  const squares = phaseDegrees.map((_, i) => `(i(vline${i})/${rated})^2`);
  return {
    id,
    kind: "thermal_overload_three_pole",
    stop,
    initialConditions: true,
    inputs: { multiple, ac, rated, senseSeconds, tripScale, cool },
    body: [
      ...sources,
      ...sources.map((_, i) => `rtest${i} line${i} 0 1`),
      `bfilter 0 q i=max(max(${squares[0]},${squares[1]}),${squares[2]})`,
      "csense q 0 0.02 ic=0",
      "rsense q 0 1",
      "bdose 0 accum i=ternary_fcn(v(q)>1,(v(q)-1)/0.1,-v(accum)/0.1)",
      "cdose accum 0 1 ic=0",
      "bclip dose 0 v=min(v(accum),1)",
      "btrip tripped 0 v=ternary_fcn(v(accum)>=1,1,0)",
    ],
    vectors: [
      { key: "normalized_detector", expression: "v(q)", tolerance: 0.002 },
      { key: "thermal_dose", expression: "v(dose)", tolerance: 0.002 },
      { key: "tripped", expression: "v(tripped)", tolerance: 0.001 },
    ],
    times: samples(5, 245),
    analytical: (time) => trace[Math.round(time * 1000)],
  };
}

function breaker() {
  return {
    id: "breaker-trip-reset",
    kind: "breaker_three_pole",
    stop: 0.21,
    initialConditions: true,
    inputs: {
      thresholdAmps: 2,
      closedResistance: 0.01,
      loadResistance: 1,
      closeAt: 0.02,
      overloadAt: 0.05,
      restoreAt: 0.08,
      resetAt: 0.12,
      recloseAt: 0.16,
    },
    body: [
      "vfeed supply 0 pwl(0 1 0.0498 1 0.05 4 0.0798 4 0.08 1)",
      "vcmd cmd 0 pwl(0 0 0.0198 0 0.02 1 0.1198 1 0.12 0 0.1598 0 0.16 1)",
      "btripgate gate 0 v=(1+tanh(30*(v(supply)/1.01-2)))/2",
      "bmemory 0 latch i=(v(cmd)*v(gate)*(1-v(latch))-(1-v(cmd))*v(latch))/5e-5",
      "clatch latch 0 1 ic=0",
      "bpole supply probe i=v(supply)*v(cmd)*(1-v(latch))/1.01",
      "vpole probe 0 0",
    ],
    vectors: [
      { key: "pole1/current", expression: "i(vpole)", tolerance: 0.001 },
      { key: "tripped", expression: "v(latch)", tolerance: 0.001 },
    ],
    times: samples(2, 202),
    analytical: (t) => [
      (t >= 0.02 && t < 0.05) || t >= 0.16 ? 1 / 1.01 : 0,
      Number(t >= 0.05 && t < 0.12),
    ],
  };
}

const motorInputs = {
  voltageRmsVolts: 48,
  frequencyHz: 50,
  statorResistance: 1,
  rotorResistance: 1,
  statorLeakage: 0.005,
  rotorLeakage: 0.005,
  magnetizingInductance: 0.1,
  polePairs: 2,
  inertia: 0.002,
  friction: 0.01,
  loadTorque: 0.1,
  returnResistance: 10,
  starSeriesResistance: 0.015,
  starContactorResistance: 0.01,
  deltaSeriesResistance: 0.055,
};

const control = {
  voltage: 24,
  switchResistance: 0.01,
  contactResistance: 0.01,
  coilResistance: 10,
  coilInductance: 1e-5,
  shuntResistance: 100,
  senseSeconds: 0.005,
  pickup: 1,
  dropout: 0.8,
};

function contactDelay(engage) {
  const series = 2 * control.switchResistance;
  const parallel =
    (series * control.shuntResistance) / (series + control.shuntResistance);
  const steady =
    (control.voltage * control.shuntResistance) /
    (series + control.shuntResistance) /
    (parallel + control.coilResistance);
  const decay = Math.exp(-1e-7 / control.senseSeconds);
  const resistance =
    control.coilResistance + (engage ? parallel : control.shuntResistance);
  let squared = engage ? 0 : steady ** 2;
  for (let tick = 0; tick < 200_000; tick++) {
    const current =
      steady *
      Math.exp((-(tick + 0.5) * 1e-7 * resistance) / control.coilInductance);
    const held = engage ? steady - current : current;
    squared = decay * squared + (1 - decay) * held ** 2;
    if (
      engage ? squared >= control.pickup ** 2 : squared <= control.dropout ** 2
    )
      return (tick + 1) * 1e-7;
  }
  throw new Error("contactor did not change state");
}

function starterTimeline(starter) {
  if (starter === "direct-on-line")
    return [
      { time: 0, position: "open" },
      { time: 0.02 + contactDelay(true), position: "star" },
    ];
  if (starter === "star-delta")
    return [
      { time: 0, position: "open" },
      { time: 0.02 + contactDelay(true), position: "star" },
      { time: 0.11 + contactDelay(false), position: "open" },
      { time: 0.14 + contactDelay(true), position: "delta" },
    ];
  return [{ time: 0, position: "star" }];
}

function motorReference(timeline, { reversed, locked, starter, step }) {
  const {
    voltageRmsVolts: vrms,
    frequencyHz: hz,
    statorResistance: rs,
    rotorResistance: rr,
    statorLeakage: lls,
    rotorLeakage: llr,
    magnetizingInductance: lm,
    polePairs,
    inertia,
    friction,
    loadTorque,
    returnResistance,
    starSeriesResistance,
    starContactorResistance,
    deltaSeriesResistance,
  } = motorInputs;
  const ls = lls + lm,
    lr = llr + lm;
  const det = ls * lr - lm ** 2;
  const shifts = reversed ? [0, 120, -120] : phaseDegrees;
  const currents = ([a, b]) => [
    a,
    (-a + Math.sqrt(3) * b) / 2,
    (-a - Math.sqrt(3) * b) / 2,
  ];
  const torque = ([sa, sb, ra, rb]) =>
    1.5 * polePairs * lm * (ra * sb - rb * sa);
  function windingVoltages(time, state, position) {
    const lines = shifts.map(
      (phase) => vrms * sqrt2 * Math.sin(tau * (hz * time + phase / 360)),
    );
    return currents(state).map((amps, i) => {
      if (position === "open") return -returnResistance * amps;
      const isDelta = position === "delta";
      const source = lines[i] - (isDelta ? lines[(i + 1) % 3] : 0);
      if (!starter) return source;
      const series = isDelta
        ? deltaSeriesResistance
        : starSeriesResistance +
          (starter === "star-delta" ? starContactorResistance : 0);
      return (source - series * amps) / (1 + series / returnResistance);
    });
  }
  function derivative(time, state, position) {
    const [sa, sb, ra, rb, speed] = state;
    const branch = windingVoltages(time, state, position);
    const statorRate = [
      branch[0] - rs * sa,
      (branch[1] - branch[2]) / Math.sqrt(3) - rs * sb,
    ];
    const rotorRate = [
      -rr * ra - polePairs * speed * (lm * sb + lr * rb),
      -rr * rb + polePairs * speed * (lm * sa + lr * ra),
    ];
    return [
      (lr * statorRate[0] - lm * rotorRate[0]) / det,
      (lr * statorRate[1] - lm * rotorRate[1]) / det,
      (ls * rotorRate[0] - lm * statorRate[0]) / det,
      (ls * rotorRate[1] - lm * statorRate[1]) / det,
      locked ? 0 : (torque(state) - friction * speed - loadTorque) / inertia,
    ];
  }
  let state = [0, 0, 0, 0, 0],
    schedule = 0;
  const points = [];
  const move = (s, d, h) => s.map((value, i) => value + h * d[i]);
  for (let i = 0; i < Math.round(0.25 / step); i++) {
    const time = i * step;
    if (time + 1e-12 >= (timeline[schedule + 1]?.time ?? Infinity)) schedule++;
    const { position } = timeline[schedule];
    const a = derivative(time, state, position);
    const b = derivative(time + step / 2, move(state, a, step / 2), position);
    const c = derivative(time + step / 2, move(state, b, step / 2), position);
    const d = derivative(time + step, move(state, c, step), position);
    state = state.map(
      (value, j) => value + (step * (a[j] + 2 * b[j] + 2 * c[j] + d[j])) / 6,
    );
    if ((i + 1) % Math.round(0.001 / step) === 0) {
      const windingCurrents = currents(state);
      const voltages = windingVoltages(time + step, state, position);
      const branchCurrents = windingCurrents.map(
        (current, j) => current + voltages[j] / returnResistance,
      );
      const supplied = branchCurrents.map((current, j) =>
        position === "open"
          ? 0
          : position === "delta"
            ? current - branchCurrents[(j + 2) % 3]
            : current,
      );
      const sourceOrder = reversed ? [0, 2, 1] : [0, 1, 2];
      points[Math.round((i + 1) * step * 1000)] = [
        ...windingCurrents,
        state[4],
        torque(state),
        ...(starter ? sourceOrder.map((index) => -supplied[index]) : []),
      ];
    }
  }
  return (time) => points[Math.round(time * 1000)];
}

function motor({ id, starter = null, locked = false, reversed = false }) {
  const p = motorInputs;
  const phases = reversed ? [0, 120, -120] : phaseDegrees;
  const timeline = starterTimeline(starter);
  function control(position) {
    const level = ({ position: actual }) => Number(position === actual);
    const points = [[0, level(timeline[0])]];
    for (let i = 1; i < timeline.length; i++) {
      const t = timeline[i].time;
      points.push([t - 2e-6, level(timeline[i - 1])], [t, level(timeline[i])]);
    }
    return `pwl(${points.flat().join(" ")})`;
  }
  const ls = p.statorLeakage + p.magnetizingInductance;
  const lr = p.rotorLeakage + p.magnetizingInductance;
  const lm = p.magnetizingInductance;
  const determinant = ls * lr - lm * lm;
  const body = phases.map(
    (phase, i) =>
      `vline${i} l${i} 0 sin(0 ${p.voltageRmsVolts * sqrt2} ${p.frequencyHz} 0 0 ${phase})`,
  );
  body.push(
    `vstar star 0 ${control("star")}`,
    `vdelta delta 0 ${control("delta")}`,
    `bsa sa 0 v=(${lr}*v(fsa)-${lm}*v(fra))/${determinant}`,
    `bsb sb 0 v=(${lr}*v(fsb)-${lm}*v(frb))/${determinant}`,
    `bra ra 0 v=(${ls}*v(fra)-${lm}*v(fsa))/${determinant}`,
    `brb rb 0 v=(${ls}*v(frb)-${lm}*v(fsb))/${determinant}`,
    "bcu cu 0 v=v(sa)",
    `bcv cv 0 v=(-v(sa)+${Math.sqrt(3)}*v(sb))/2`,
    `bcw cw 0 v=(-v(sa)-${Math.sqrt(3)}*v(sb))/2`,
  );
  for (const [i, phase] of ["u", "v", "w"].entries()) {
    const current = `v(c${phase})`;
    const line = `v(l${i})`;
    const lineToLine = `v(l${i},l${(i + 1) % 3})`;
    const drive = (source, series) =>
      `(${source}-${series}*${current})/${1 + series / p.returnResistance}`;
    const v = !starter
      ? line
      : `v(star)*(${drive(line, p.starSeriesResistance + (starter === "star-delta" ? p.starContactorResistance : 0))})+v(delta)*(${drive(lineToLine, p.deltaSeriesResistance)})-(1-v(star)-v(delta))*${p.returnResistance}*${current}`;
    body.push(`bv${phase} v${phase} 0 v=${v}`);
  }
  if (starter) {
    const branch = ["u", "v", "w"].map(
      (phase) => `(v(c${phase})+v(v${phase})/${p.returnResistance})`,
    );
    for (const [index, current] of branch.entries()) {
      body.push(
        `bload${index} l${index} 0 i=v(star)*${current}+v(delta)*(${current}-${branch[(index + 2) % 3]})`,
      );
    }
  }
  body.push(
    `bfsalpha 0 fsa i=v(vu)-${p.statorResistance}*v(sa)`,
    `bfsbeta 0 fsb i=(v(vv)-v(vw))/${Math.sqrt(3)}-${p.statorResistance}*v(sb)`,
    `bfralpha 0 fra i=-${p.rotorResistance}*v(ra)-${p.polePairs}*v(speed)*v(frb)`,
    `bfrbeta 0 frb i=-${p.rotorResistance}*v(rb)+${p.polePairs}*v(speed)*v(fra)`,
    ...["fsa", "fsb", "fra", "frb"].map(
      (state) => `c${state} ${state} 0 1 ic=0`,
    ),
    `btorque torque 0 v=${1.5 * p.polePairs * lm}*(v(ra)*v(sb)-v(rb)*v(sa))`,
    ...(locked
      ? ["vspeed speed 0 0"]
      : [
          `bshaft 0 speed i=(v(torque)-${p.friction}*v(speed)-${p.loadTorque})/${p.inertia}`,
          "cspeed speed 0 1 ic=0",
        ]),
  );
  return {
    id,
    kind: starter ?? "induction_motor_three_phase",
    stop: 0.25,
    initialConditions: true,
    inputs: { ...p, locked, reversed, timeline },
    body,
    vectors: [
      ...["u", "v", "w"].map((phase) => ({
        key: `${phase}/current`,
        expression: `v(c${phase})`,
        tolerance: 0.08,
      })),
      { key: "angular_velocity", expression: "v(speed)", tolerance: 0.25 },
      {
        key: "electromagnetic_torque",
        expression: "v(torque)",
        tolerance: 0.1,
      },
      ...(starter
        ? (reversed ? [0, 2, 1] : [0, 1, 2]).map((phase, index) => ({
            key: `source/l${index + 1}/current`,
            expression: `i(vline${phase})`,
            tolerance: 0.12,
          }))
        : []),
    ],
    times: samples(3, 243, 3).filter((time) =>
      timeline.every((event) => Math.abs(time - event.time) >= 0.001),
    ),
    analytical: motorReference(timeline, {
      reversed,
      locked,
      starter,
      step: 2e-6,
    }),
  };
}

function starterDrawing(starter, reversed) {
  const p = motorInputs;
  const components = [];
  const geometry = [];
  const nets = new Map();
  const wires = [];
  const stableId = (n) =>
    `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
  const wire = (...points) =>
    wires.push({ id: stableId(100 + wires.length), points });
  let nextPin = 0;
  const addPart = (typeId, parameters, pins) => {
    const id = stableId(components.length + 1);
    components.push({
      id,
      typeId,
      parameters,
      rotationDeg: 0,
      position: { x: nextPin * 2, y: 0 },
    });
    geometry.push({
      componentId: id,
      pins: Object.entries(pins).map(([pinId, node]) => {
        const position = { x: ++nextPin * 2, y: 4 };
        if (!nets.has(node)) nets.set(node, []);
        nets.get(node).push(position);
        return { pinId, position };
      }),
    });
    return id;
  };
  const terminals = (line, load, aux = {}) => ({
    l1: line[0],
    t1: load[0],
    l2: line[1],
    t2: load[1],
    l3: line[2],
    t3: load[2],
    no_1: "ground",
    no_2: "ground",
    nc_1: "ground",
    nc_2: "ground",
    ...aux,
  });
  const controlShunt = (left, right, ohms) =>
    addPart("resistor", { resistanceOhms: ohms }, { pin1: left, pin2: right });
  const switchPart = (from, to) =>
    addPart(
      "switch_spst",
      {
        closedResistanceOhms: control.switchResistance,
        initialClosed: 0,
      },
      { pin1: from, pin2: to },
    );
  const coil = (line, load, node, aux) => {
    const id = addPart(
      "contactor_three_pole",
      {
        coilResistanceOhms: control.coilResistance,
        coilInductanceHenries: control.coilInductance,
        closedContactResistanceOhms: control.contactResistance,
        pickupCurrentAmps: control.pickup,
        dropoutCurrentAmps: control.dropout,
        coilSenseSeconds: control.senseSeconds,
      },
      terminals(line, load, { a1: node, a2: "ground", ...aux }),
    );
    controlShunt(node, "ground", control.shuntResistance);
    return id;
  };
  const mains = ["line1", "line2", "line3"];
  const sensed = ["sense1", "sense2", "sense3"];
  const starts = ["u1", "v1", "w1"];
  const ends =
    starter === "star-delta" ? ["u2", "v2", "w2"] : Array(3).fill("neutral");
  const source = addPart(
    "three_phase_source_star",
    {
      phaseVoltageRmsVolts: p.voltageRmsVolts,
      frequencyHz: p.frequencyHz,
      phaseOffsetDegrees: 0,
      reversedSequence: 0,
      internalResistanceOhms: 0,
    },
    {
      l1: mains[0],
      l2: mains[reversed ? 2 : 1],
      l3: mains[reversed ? 1 : 2],
      n: "ground",
    },
  );
  addPart(
    "dc_voltage_source",
    { voltageVolts: control.voltage, internalResistanceOhms: 0 },
    { pin1: "control", pin2: "ground" },
  );
  const overload = addPart(
    "thermal_overload_three_pole",
    {
      closedContactResistanceOhms: control.contactResistance,
      senseResistanceOhms: 0.005,
      ratedCurrentAmps: 100,
      thermalSenseSeconds: 0.02,
      tripScaleSeconds: 0.1,
      coolSeconds: 0.1,
      resetDose: 0.25,
    },
    terminals(mains, sensed, { nc_1: "control", nc_2: "main-enable" }),
  );
  const mainSwitch = switchPart("main-enable", "main-coil");
  const main = coil(sensed, starts, "main-coil");
  const operations = [
    { step: 20, componentId: mainSwitch, position: "closed" },
  ];
  const states = { main, overload };
  if (starter === "star-delta") {
    const starSwitch = switchPart("star-enable", "star-coil");
    const deltaSwitch = switchPart("delta-enable", "delta-coil");
    states.star = coil(ends, Array(3).fill("neutral"), "star-coil", {
      nc_1: "control",
      nc_2: "delta-enable",
    });
    states.delta = coil(ends, [starts[1], starts[2], starts[0]], "delta-coil", {
      nc_1: "control",
      nc_2: "star-enable",
    });
    operations.push(
      { step: 20, componentId: starSwitch, position: "closed" },
      { step: 90, componentId: deltaSwitch, position: "closed" },
      { step: 100, componentId: deltaSwitch, position: "open" },
      { step: 110, componentId: starSwitch, position: "open" },
      { step: 140, componentId: deltaSwitch, position: "closed" },
    );
  }
  const motor = addPart(
    "induction_motor_three_phase",
    {
      statorResistanceOhms: p.statorResistance,
      rotorResistanceOhms: p.rotorResistance,
      statorLeakageHenries: p.statorLeakage,
      rotorLeakageHenries: p.rotorLeakage,
      magnetizingHenries: p.magnetizingInductance,
      polePairs: p.polePairs,
      rotorInertiaKilogramMetersSquared: p.inertia,
      viscousFrictionNewtonMeterSecondsPerRadian: p.friction,
      loadTorqueNewtonMeters: p.loadTorque,
    },
    {
      u1: starts[0],
      u2: ends[0],
      v1: starts[1],
      v2: ends[1],
      w1: starts[2],
      w2: ends[2],
    },
  );
  for (let i = 0; i < 3; i++)
    controlShunt(starts[i], ends[i], p.returnResistance);
  // Route a distinct horizontal wire for each net. Stubs cross other
  // lanes only at interior points, so the crossings do not form junctions.
  let ground;
  for (const [index, [node, pins]] of [...nets].entries()) {
    const y = 8 + index * 2;
    const left = { x: pins[0].x - 1, y };
    wire(left, { x: pins.at(-1).x + 1, y });
    for (const { x, y: pinY } of pins) wire({ x, y: pinY }, { x, y });
    if (node === "ground") ground = left;
  }
  return {
    request: {
      documentRevision: "start",
      schematic: {
        schemaVersion: 2,
        components,
        wires,
        junctions: [],
        rails: [],
        referenceMarkers: [
          {
            id: "00000000-0000-4000-8000-000000000999",
            position: ground,
          },
        ],
      },
      geometry: { components: geometry },
    },
    motor,
    source,
    states,
    // Stage 6 operator requests recompute the newest public interval.
    operations: operations.map((operation) => ({
      ...operation,
      step: operation.step + 1,
    })),
  };
}

const fixtures = [
  ...[50, 60].map(contactor),
  breaker(),
  ...[1, 1.5, 2, 3].map((multiple) => overload(multiple)),
  overload(1.5, true),
  motor({ id: "induction-blocked", locked: true }),
  motor({ id: "induction-run-up" }),
  motor({ id: "direct-on-line", starter: "direct-on-line" }),
  motor({
    id: "direct-on-line-swapped",
    starter: "direct-on-line",
    reversed: true,
  }),
  motor({ id: "star-delta", starter: "star-delta" }),
  motor({ id: "star-delta-swapped", starter: "star-delta", reversed: true }),
  ...[50, 60].flatMap((hz) => [
    load({
      id: `star-linked-${hz}`,
      hz,
      delta: false,
      floating: false,
      inductive: true,
    }),
    load({
      id: `star-floating-${hz}`,
      hz,
      delta: false,
      floating: true,
      inductive: true,
    }),
    load({
      id: `delta-resistive-${hz}`,
      hz,
      delta: true,
      floating: false,
      inductive: false,
    }),
    load({
      id: `delta-inductive-${hz}`,
      hz,
      delta: true,
      floating: false,
      inductive: true,
    }),
    transformer({ id: `transformer-open-${hz}`, hz, loaded: false }),
    transformer({ id: `transformer-loaded-${hz}`, hz, loaded: true }),
  ]),
];

function verify(analytical, observed) {
  assert.deepEqual(
    Object.keys(observed),
    fixtures.map(({ id }) => id),
  );
  for (const { id, vectors, times } of fixtures) {
    assert.equal(observed[id].length, times.length, id);
    for (const [i, row] of observed[id].entries()) {
      assert.equal(row.length, vectors.length + 1, id);
      assert.equal(row[0], analytical[id][i][0], id);
      for (const [j, { key, tolerance }] of vectors.entries()) {
        assert(
          Math.abs(row[j + 1] - analytical[id][i][j + 1]) <= tolerance,
          `${id} ${key} at ${row[0]} s: ngspice ${row[j + 1]} vs analytical ${analytical[id][i][j + 1]}`,
        );
      }
    }
  }
}

const args = process.argv.slice(2);
const write = args[0] === "--write";
const executable =
  args.length === 3 && args[1] === "--ngspice" ? args[2] : undefined;
assert(
  (args[0] === "--check" && args.length === 1) ||
    (executable && ["--write", "--check"].includes(args[0])),
  "Use --check or --write --ngspice <ngspice-47 console executable>",
);
const analytical = Object.fromEntries(
  fixtures.map((fixture) => [
    fixture.id,
    resultRows(fixture, fixture.analytical),
  ]),
);
const drawings = Object.fromEntries(
  fixtures
    .filter(
      ({ kind, inputs }) =>
        inputs.timeline && kind !== "induction_motor_three_phase",
    )
    .map(({ id, kind, inputs }) => [id, starterDrawing(kind, inputs.reversed)]),
);
const observed = write
  ? runSpice(executable, fixtures)
  : JSON.parse(readFileSync(join(directory, "ngspice.json"), "utf8"));
verify(analytical, observed);
if (!write && executable) {
  const fresh = runSpice(executable, fixtures);
  verify(analytical, fresh);
  for (const { id, vectors } of fixtures) {
    for (const [i, row] of observed[id].entries()) {
      for (const [j, { tolerance }] of vectors.entries()) {
        assert(
          Math.abs(fresh[id][i][j + 1] - row[j + 1]) <= tolerance / 10,
          `${id}: ngspice replay changed`,
        );
      }
    }
  }
}
const manifest = {
  generator: "node scripts/stage10-references.mjs --write --ngspice <path>",
  ngspice: {
    version,
    archiveSha256: archiveHash,
    options,
    maxStepSeconds: spiceStep,
  },
  sign: "Voltage first minus second terminal. Positive current enters the first terminal.",
  observationFormat: "[timeSeconds, ...vectors]",
  ngspiceDataSha256: digest(json(observed)),
  fixtures: fixtures.map((fixture) => {
    const { id, kind, inputs, vectors, times, acHz, stop } = fixture;
    return {
      id,
      kind,
      inputs,
      vectors,
      times,
      analysis: acHz
        ? { kind: "ac", frequencyHz: acHz }
        : { kind: "transient", stopSeconds: stop },
      netlistSha256: digest(deck(fixture)),
    };
  }),
};
if (write) {
  mkdirSync(join(directory, "netlists"), { recursive: true });
  for (const fixture of fixtures)
    writeFileSync(
      join(directory, "netlists", `${fixture.id}.cir`),
      deck(fixture),
    );
  writeFileSync(join(directory, "manifest.json"), json(manifest));
  writeFileSync(join(directory, "analytical.json"), json(analytical));
  writeFileSync(join(directory, "ngspice.json"), json(observed));
  writeFileSync(join(directory, "starters.json"), json(drawings));
} else {
  for (const fixture of fixtures)
    assert.equal(
      readFileSync(
        join(directory, "netlists", `${fixture.id}.cir`),
        "utf8",
      ).replaceAll("\r\n", "\n"),
      deck(fixture),
    );
  assert.deepEqual(
    JSON.parse(readFileSync(join(directory, "manifest.json"), "utf8")),
    manifest,
  );
  const stored = JSON.parse(
    readFileSync(join(directory, "analytical.json"), "utf8"),
  );
  assert.deepEqual(
    JSON.parse(readFileSync(join(directory, "starters.json"), "utf8")),
    drawings,
  );
  for (const { id } of fixtures) {
    assert.equal(stored[id].length, analytical[id].length, id);
    for (const [i, row] of stored[id].entries()) {
      assert.equal(row.length, analytical[id][i].length, id);
      for (const [j, value] of row.entries()) {
        assert(
          Math.abs(value - analytical[id][i][j]) <=
            1e-11 * Math.max(Math.abs(value), 1),
          id,
        );
      }
    }
  }
}
console.log(
  `${write ? "Wrote" : "Verified"} ${fixtures.length} Stage 10 reference circuits.`,
);
