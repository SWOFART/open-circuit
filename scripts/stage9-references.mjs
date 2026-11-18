// Deliberate Stage 9 reference generation. This file has no Engine imports.
// Closed-form solutions are the oracle; ngspice only checks the netlists.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
  mkdirSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const GOLDEN = join(ROOT, "tests", "golden");
const NETLISTS = join(GOLDEN, "stage-9-ngspice");
const MANIFEST_PATH = join(GOLDEN, "stage-9-manifest.json");
const ANALYTICAL_PATH = join(GOLDEN, "stage-9-analytical.expected.json");
const NGSPICE_PATH = join(GOLDEN, "stage-9-ngspice.expected.json");
const NGSPICE_VERSION = "ngspice-47";
const NGSPICE_SOURCE =
  "Observed from the checked-in netlists by ngspice-47; no Engine data is read.";
const NGSPICE_OPTIONS =
  "temp=27 tnom=27 gmin=0 method=trap reltol=1e-10 vntol=1e-12 abstol=1e-14 savecurrents";
const TRAN_STEP = 1e-6;
const WRDATA_STEP = 50e-6;
const BALANCE = { currentAbsAmps: 1e-9, powerAbsWatts: 1e-8 };
const NGSPICE_BALANCE = { currentAbsAmps: 1e-8, powerAbsWatts: 1e-3 };
const REFERENCE_ERROR = {
  voltageAbsVolts: 1e-4,
  currentAbsAmps: 1e-6,
  powerAbsWatts: 1e-4,
  powerFactorAbs: 1e-6,
  phaseAngleAbsDegrees: 1e-4,
  relative: 1e-6,
};
const PORTABLE_GENERATION_ERROR = { absolute: 1e-12, relative: 1e-12 };
const SQRT2 = Math.sqrt(2);
const DEG = Math.PI / 180;

const stable = (value) => {
  if (Array.isArray(value)) return value.map(stable);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value).map(([key, inner]) => [key, stable(inner)]),
    );
  }
  if (typeof value !== "number") return value;
  assert(Number.isFinite(value), `non-finite reference value: ${value}`);
  return value === 0 ? 0 : Number(value.toPrecision(14));
};
const json = (value) => `${JSON.stringify(stable(value), null, 2)}\n`;
const spice = (value) => {
  assert(Number.isFinite(value), `non-finite netlist value: ${value}`);
  return value === 0 ? "0" : value.toPrecision(16).replace(/e\+/, "e");
};
const deg = (value) => value * DEG;
const wrapDegrees = (value) => {
  let wrapped = ((((value + 180) % 360) + 360) % 360) - 180;
  if (Object.is(wrapped, -0)) wrapped = 0;
  return wrapped;
};
const sinWave = (peak, frequency, phaseDegrees, time) =>
  peak * Math.sin(2 * Math.PI * frequency * time + deg(phaseDegrees));

function branch(id, from, to, kind, parameters = {}) {
  return { id, from, to, kind, ...parameters };
}

function uniqueExpressions(specs) {
  return [...new Set(specs.map((spec) => spec.expression))];
}

function nodeVectors(nodes) {
  return nodes
    .filter((node) => node !== "0")
    .map((node) => ({ key: node, expression: `v(${node})`, kind: "node" }));
}

function sampleTimes(startupEnd, frequency, steadyStart) {
  const early = [1e-6];
  for (let ms = 1; ms <= startupEnd * 1000; ms += 1) early.push(ms / 1000);
  const steady = Array.from(
    { length: 21 },
    (_, index) => steadyStart + index / (20 * frequency),
  );
  return [...early, ...steady];
}

function singlePhaseCase({
  id,
  kind,
  frequencyHz,
  phaseOffsetDegrees,
  resistanceOhms,
  capacitanceFarads,
  inductanceHenries,
  highFrequency = false,
}) {
  const sourceRmsVolts = 10;
  const sourcePeakVolts = sourceRmsVolts * SQRT2;
  const sourceResistanceOhms = 0.1;
  const totalResistance = sourceResistanceOhms + resistanceOhms;
  const omega = 2 * Math.PI * frequencyHz;
  const phase = deg(phaseOffsetDegrees);
  let nodes;
  let branches;
  let nodeValues;
  let outputAmplitude;
  let outputPhaseDegrees;
  let phaseLagDegrees;
  let stopSeconds = 0.2 + 1 / frequencyHz;

  if (kind === "series_rc") {
    const tau = totalResistance * capacitanceFarads;
    const lag = Math.atan(omega * tau);
    outputAmplitude = sourcePeakVolts / Math.hypot(1, omega * tau);
    phaseLagDegrees = -lag / DEG;
    outputPhaseDegrees = wrapDegrees(phaseOffsetDegrees + phaseLagDegrees);
    const steadyAtStart = outputAmplitude * Math.sin(phase - lag);
    const initialCapacitorVolts = sourcePeakVolts * Math.sin(phase);
    const valueAt = (time) => {
      const source = sinWave(
        sourcePeakVolts,
        frequencyHz,
        phaseOffsetDegrees,
        time,
      );
      const steady = outputAmplitude * Math.sin(omega * time + phase - lag);
      const capacitor =
        steady +
        (initialCapacitorVolts - steadyAtStart) * Math.exp(-time / tau);
      const current = (source - capacitor) / totalResistance;
      const sourceTerminal = source - sourceResistanceOhms * current;
      return {
        nodes: { src: source, n1: sourceTerminal, cap: capacitor },
        currents: {
          V1: -current,
          Rsource: current,
          Rload: current,
          C1: current,
        },
      };
    };
    nodes = ["0", "src", "n1", "cap"];
    branches = [
      branch("V1", "src", "0", "voltageSource"),
      branch("Rsource", "src", "n1", "resistor", {
        resistanceOhms: sourceResistanceOhms,
      }),
      branch("Rload", "n1", "cap", "resistor", { resistanceOhms }),
      branch("C1", "cap", "0", "capacitor", { capacitanceFarads }),
    ];
    nodeValues = valueAt;
  } else if (kind === "series_rl") {
    const lag = Math.atan2(omega * inductanceHenries, totalResistance);
    outputAmplitude =
      sourcePeakVolts / Math.hypot(totalResistance, omega * inductanceHenries);
    phaseLagDegrees = -lag / DEG;
    outputPhaseDegrees = wrapDegrees(phaseOffsetDegrees + phaseLagDegrees);
    const steadyAtStart = outputAmplitude * Math.sin(phase - lag);
    const initialCurrent =
      (sourcePeakVolts * Math.sin(phase)) / totalResistance;
    const tau = inductanceHenries / totalResistance;
    const valueAt = (time) => {
      const source = sinWave(
        sourcePeakVolts,
        frequencyHz,
        phaseOffsetDegrees,
        time,
      );
      const steady = outputAmplitude * Math.sin(omega * time + phase - lag);
      const current =
        steady + (initialCurrent - steadyAtStart) * Math.exp(-time / tau);
      const inductorVolts = source - totalResistance * current;
      return {
        nodes: {
          src: source,
          n1: source - sourceResistanceOhms * current,
          n2: inductorVolts,
        },
        currents: {
          V1: -current,
          Rsource: current,
          Rload: current,
          L1: current,
        },
      };
    };
    nodes = ["0", "src", "n1", "n2"];
    branches = [
      branch("V1", "src", "0", "voltageSource"),
      branch("Rsource", "src", "n1", "resistor", {
        resistanceOhms: sourceResistanceOhms,
      }),
      branch("Rload", "n1", "n2", "resistor", { resistanceOhms }),
      branch("L1", "n2", "0", "inductor", { inductanceHenries }),
    ];
    nodeValues = valueAt;
  } else if (kind === "series_rlc") {
    const alpha = totalResistance / (2 * inductanceHenries);
    const omega0 = 1 / Math.sqrt(inductanceHenries * capacitanceFarads);
    assert(omega0 > alpha, `${id} must be underdamped for its closed form`);
    const omegaD = Math.sqrt(omega0 * omega0 - alpha * alpha);
    const realDenominator =
      1 - omega * omega * inductanceHenries * capacitanceFarads;
    const imaginaryDenominator = omega * totalResistance * capacitanceFarads;
    const denominatorMagnitude = Math.hypot(
      realDenominator,
      imaginaryDenominator,
    );
    const lag = Math.atan2(imaginaryDenominator, realDenominator);
    outputAmplitude = sourcePeakVolts / denominatorMagnitude;
    phaseLagDegrees = -lag / DEG;
    outputPhaseDegrees = wrapDegrees(phaseOffsetDegrees + phaseLagDegrees);
    const steadyAtStart = outputAmplitude * Math.sin(phase - lag);
    const steadyDerivativeAtStart =
      outputAmplitude * omega * Math.cos(phase - lag);
    const initialCapacitorVolts = sourcePeakVolts * Math.sin(phase);
    const naturalA = initialCapacitorVolts - steadyAtStart;
    const naturalB = (-steadyDerivativeAtStart + alpha * naturalA) / omegaD;
    const valueAt = (time) => {
      const source = sinWave(
        sourcePeakVolts,
        frequencyHz,
        phaseOffsetDegrees,
        time,
      );
      const angle = omega * time + phase - lag;
      const steady = outputAmplitude * Math.sin(angle);
      const steadyDerivative = outputAmplitude * omega * Math.cos(angle);
      const decay = Math.exp(-alpha * time);
      const naturalCos = Math.cos(omegaD * time);
      const naturalSin = Math.sin(omegaD * time);
      const natural = decay * (naturalA * naturalCos + naturalB * naturalSin);
      const naturalDerivative =
        decay *
        (-alpha * (naturalA * naturalCos + naturalB * naturalSin) +
          omegaD * (-naturalA * naturalSin + naturalB * naturalCos));
      const capacitor = steady + natural;
      const current =
        capacitanceFarads * (steadyDerivative + naturalDerivative);
      const inductorVolts = source - totalResistance * current - capacitor;
      return {
        nodes: {
          src: source,
          n1: source - sourceResistanceOhms * current,
          n2: capacitor + inductorVolts,
          n3: capacitor,
        },
        currents: {
          V1: -current,
          Rsource: current,
          Rload: current,
          L1: current,
          C1: current,
        },
      };
    };
    nodes = ["0", "src", "n1", "n2", "n3"];
    branches = [
      branch("V1", "src", "0", "voltageSource"),
      branch("Rsource", "src", "n1", "resistor", {
        resistanceOhms: sourceResistanceOhms,
      }),
      branch("Rload", "n1", "n2", "resistor", { resistanceOhms }),
      branch("L1", "n2", "n3", "inductor", { inductanceHenries }),
      branch("C1", "n3", "0", "capacitor", { capacitanceFarads }),
    ];
    nodeValues = valueAt;
  } else {
    throw new Error(`unsupported single-phase case ${kind}`);
  }

  const outputKey =
    kind === "series_rl" ? "Rload" : kind === "series_rc" ? "cap" : "n3";
  const outputUnit = kind === "series_rl" ? "amp" : "volt";
  const times = sampleTimes(0.04, frequencyHz, 0.2);
  const vectorSpecs = [
    ...nodeVectors(nodes),
    { key: "Rsource", expression: "@rsource[i]", kind: "current" },
    { key: "Rload", expression: "@rload[i]", kind: "current" },
    ...(kind === "series_rc"
      ? [{ key: "C1", expression: "@c1[i]", kind: "current" }]
      : []),
    ...(kind === "series_rl"
      ? [{ key: "L1", expression: "@l1[i]", kind: "current" }]
      : []),
    ...(kind === "series_rlc"
      ? [
          { key: "L1", expression: "@l1[i]", kind: "current" },
          { key: "C1", expression: "@c1[i]", kind: "current" },
        ]
      : []),
  ];
  vectorSpecs.push({
    key: "V1",
    expression: "@rsource[i]",
    kind: "current",
    sign: -1,
  });
  const sourceLine = `V1 src 0 SIN(0 ${spice(sourcePeakVolts)} ${spice(frequencyHz)} 0 0 ${spice(phaseOffsetDegrees)})`;
  const body = [
    sourceLine,
    `Rsource src n1 ${spice(sourceResistanceOhms)}`,
    kind === "series_rc"
      ? `Rload n1 cap ${spice(resistanceOhms)}`
      : kind === "series_rl"
        ? `Rload n1 n2 ${spice(resistanceOhms)}`
        : `Rload n1 n2 ${spice(resistanceOhms)}`,
    ...(kind === "series_rc"
      ? [
          `C1 cap 0 ${spice(capacitanceFarads)} IC=${spice(sourcePeakVolts * Math.sin(phase))}`,
        ]
      : []),
    ...(kind === "series_rl"
      ? [
          `L1 n2 0 ${spice(inductanceHenries)} IC=${spice((sourcePeakVolts * Math.sin(phase)) / totalResistance)}`,
        ]
      : []),
    ...(kind === "series_rlc"
      ? [
          `L1 n2 n3 ${spice(inductanceHenries)} IC=0`,
          `C1 n3 0 ${spice(capacitanceFarads)} IC=${spice(sourcePeakVolts * Math.sin(phase))}`,
        ]
      : []),
  ];
  const vectorExpressions = uniqueExpressions(vectorSpecs);
  const netlist = [
    `* Stage 9 reference circuit: ${id}`,
    `* ${kind}, ${frequencyHz} Hz, phase ${phaseOffsetDegrees} degrees; closed-form startup and steady state.`,
    ...body,
    `.options ${NGSPICE_OPTIONS}`,
    ".control",
    `tran ${spice(WRDATA_STEP)} ${spice(stopSeconds)} 0 ${spice(TRAN_STEP)} uic`,
    "set wr_singlescale",
    `wrdata ${id}.out ${vectorExpressions.join(" ")}`,
    ".endc",
    ".end",
    "",
  ].join("\n");
  const vectorMap = new Map(vectorSpecs.map((item) => [item.key, item]));
  const engineTolerance = {
    steadyStateAmplitudeRelative: highFrequency ? 1e-2 : 5e-3,
    steadyStatePhaseDegrees: 0.3,
    startupSignalAbsolute: stable(outputAmplitude * 1e-2),
    startupSignalUnit: outputUnit,
  };
  const summary = {
    sourceRmsVolts,
    sourcePhaseDegrees: phaseOffsetDegrees,
    outputSignal: outputKey,
    outputUnit,
    steadyStatePeak: outputAmplitude,
    steadyStatePhaseDegrees: outputPhaseDegrees,
    steadyStatePhaseLagDegrees: phaseLagDegrees,
    engineTolerance,
  };
  return {
    id,
    kind,
    role: "canonical",
    description: `${kind} on a 10 V RMS sinusoidal source at ${frequencyHz} Hz and ${phaseOffsetDegrees} degrees; startup begins at the source's operating-point value.`,
    parameters: {
      sourceRmsVolts,
      sourceFrequencyHz: frequencyHz,
      sourcePhaseOffsetDegrees: phaseOffsetDegrees,
      sourceInternalResistanceOhms: sourceResistanceOhms,
      resistanceOhms,
      ...(capacitanceFarads === undefined ? {} : { capacitanceFarads }),
      ...(inductanceHenries === undefined ? {} : { inductanceHenries }),
    },
    network: { referenceNode: "0", nodes, branches },
    vectorSpecs,
    vectorExpressions,
    netlist,
    times,
    summary,
    analyticalAt(time) {
      const state = nodeValues(time);
      return {
        nodeVoltagesVolts: state.nodes,
        branchCurrentsAmps: state.currents,
      };
    },
    observationFromNgspice(time, values) {
      const nodeValues = Object.fromEntries(
        nodes
          .filter((node) => node !== "0")
          .map((node) => [node, values.get(node)]),
      );
      nodeValues["0"] = 0;
      const branchCurrents = Object.fromEntries(
        branches.map((item) => {
          const spec = vectorMap.get(item.id);
          return [item.id, (spec.sign ?? 1) * values.get(spec.key)];
        }),
      );
      return {
        nodeVoltagesVolts: nodeValues,
        branchCurrentsAmps: branchCurrents,
      };
    },
  };
}

// Small complex-number helpers. Stage 9's three-phase references are closed-form
// sinusoidal steady-state solutions of a linear nodal system, independent of Engine.
const C = (re = 0, im = 0) => ({ re, im });
const cAdd = (a, b) => C(a.re + b.re, a.im + b.im);
const cSub = (a, b) => C(a.re - b.re, a.im - b.im);
const cMul = (a, b) => C(a.re * b.re - a.im * b.im, a.re * b.im + a.im * b.re);
const cScale = (a, scalar) => C(a.re * scalar, a.im * scalar);
const cDiv = (a, b) => {
  const denominator = b.re * b.re + b.im * b.im;
  return C(
    (a.re * b.re + a.im * b.im) / denominator,
    (a.im * b.re - a.re * b.im) / denominator,
  );
};
const cAbs = (a) => Math.hypot(a.re, a.im);
const cExp = (phase) => C(Math.cos(phase), Math.sin(phase));
const cSinValue = (phasor, omega, time) => cMul(phasor, cExp(omega * time)).im;
const cPolar = (peak, phaseRadians) => cScale(cExp(phaseRadians), peak);

function addAdmittance(matrix, rhs, index, from, to, admittance, emf = C()) {
  const a = from === "0" ? -1 : index.get(from);
  const b = to === "0" ? -1 : index.get(to);
  if (a >= 0) {
    matrix[a][a] = cAdd(matrix[a][a], admittance);
    if (b >= 0) matrix[a][b] = cSub(matrix[a][b], admittance);
    rhs[a] = cAdd(rhs[a], cMul(admittance, emf));
  }
  if (b >= 0) {
    matrix[b][b] = cAdd(matrix[b][b], admittance);
    if (a >= 0) matrix[b][a] = cSub(matrix[b][a], admittance);
    rhs[b] = cSub(rhs[b], cMul(admittance, emf));
  }
}

function solveComplex(matrix, rhs) {
  const n = rhs.length;
  const a = matrix.map((row, index) => [...row, rhs[index]]);
  for (let column = 0; column < n; column += 1) {
    let pivot = column;
    for (let row = column + 1; row < n; row += 1) {
      if (cAbs(a[row][column]) > cAbs(a[pivot][column])) pivot = row;
    }
    assert(cAbs(a[pivot][column]) > 1e-20, "singular phasor reference circuit");
    [a[column], a[pivot]] = [a[pivot], a[column]];
    const divisor = a[column][column];
    for (let cell = column; cell <= n; cell += 1) {
      a[column][cell] = cDiv(a[column][cell], divisor);
    }
    for (let row = 0; row < n; row += 1) {
      if (row === column) continue;
      const factor = a[row][column];
      for (let cell = column; cell <= n; cell += 1) {
        a[row][cell] = cSub(a[row][cell], cMul(factor, a[column][cell]));
      }
    }
  }
  return a.map((row) => row[n]);
}

function threePhaseCase({
  id,
  sourceKind,
  connection,
  sequenceReversed = false,
  neutralConnected = false,
  loadKind,
  loads,
  wrongVariantOf,
  description,
}) {
  const frequencyHz = 50;
  const phaseOffsetDegrees = 0;
  const sourceInternalResistanceOhms = 0.1;
  const loadNeutral = connection === "star" && neutralConnected ? "0" : "nload";
  const externalNodes =
    sourceKind === "star"
      ? [
          "l1",
          "l2",
          "l3",
          ...(connection === "star" && !neutralConnected ? ["nload"] : []),
        ]
      : ["l1", "l2"];
  const aliasNode = (node) =>
    sourceKind === "delta" && node === "l3" ? "0" : node;
  const physicalNodes = new Set(["0", ...externalNodes]);
  const omega = 2 * Math.PI * frequencyHz;
  const sourceRmsVolts = sourceKind === "star" ? 230 : 400;
  const sourcePeakVolts = sourceRmsVolts * SQRT2;
  const phaseRadians = deg(phaseOffsetDegrees);
  const sequenceDegrees = sequenceReversed ? [0, 120, -120] : [0, -120, 120];
  const sourceWindings = [];
  if (sourceKind === "star") {
    for (let phaseIndex = 0; phaseIndex < 3; phaseIndex += 1) {
      const line = `l${phaseIndex + 1}`;
      const internal = `src_${line}`;
      const sourceId = `V${phaseIndex + 1}`;
      const resistorId = `Rsource_${line}`;
      const emf = cPolar(
        sourcePeakVolts,
        phaseRadians + deg(sequenceDegrees[phaseIndex]),
      );
      sourceWindings.push({
        id: sourceId,
        resistorId,
        from: line,
        to: "0",
        internal,
        emf,
        resistance: sourceInternalResistanceOhms,
        phaseDegrees: sequenceDegrees[phaseIndex],
      });
    }
  } else {
    const windingPhases = sequenceReversed ? [0, 120, -120] : [0, -120, 120];
    for (const [index, pair] of [
      ["l1", "l2"],
      ["l2", "l3"],
      ["l3", "l1"],
    ].entries()) {
      const [from, to] = pair;
      const internal = `src_${from}_${to}`;
      const sourceId = `V${from}${to}`;
      const resistorId = `Rsource_${from}${to}`;
      const emf = cPolar(
        sourcePeakVolts,
        phaseRadians + deg(windingPhases[index]),
      );
      sourceWindings.push({
        id: sourceId,
        resistorId,
        from,
        to,
        internal,
        emf,
        resistance: sourceInternalResistanceOhms,
        phaseDegrees: windingPhases[index],
      });
    }
  }
  for (const winding of sourceWindings) {
    winding.fromNode = aliasNode(winding.from);
    winding.toNode = aliasNode(winding.to);
  }

  const loadEdges = loads.map((load, index) => {
    const from = aliasNode(connection === "star" ? `l${index + 1}` : load.from);
    const to = aliasNode(connection === "star" ? loadNeutral : load.to);
    return {
      id: load.id,
      from,
      to,
      resistanceOhms: load.resistanceOhms,
      inductanceHenries: load.inductanceHenries ?? 0,
      middle: load.inductanceHenries ? `mid_${load.id}` : null,
    };
  });

  const allBranches = [];
  const vectorSpecs = [];
  const lines = [];
  for (const winding of sourceWindings) {
    physicalNodes.add(winding.internal);
    allBranches.push(
      branch(
        winding.resistorId,
        winding.fromNode,
        winding.internal,
        "resistor",
        {
          resistanceOhms: sourceInternalResistanceOhms,
        },
      ),
      branch(winding.id, winding.internal, winding.toNode, "voltageSource"),
    );
    lines.push(
      `R${winding.resistorId.slice(1)} ${winding.fromNode} ${winding.internal} ${spice(sourceInternalResistanceOhms)}`,
      `V${winding.id.slice(1)} ${winding.internal} ${winding.toNode} SIN(0 ${spice(sourcePeakVolts)} ${spice(frequencyHz)} 0 0 ${spice(winding.phaseDegrees)})`,
    );
    vectorSpecs.push({
      key: winding.resistorId,
      expression: `@r${winding.resistorId.slice(1).toLowerCase()}[i]`,
      kind: "current",
    });
  }
  for (const edge of loadEdges) {
    if (edge.middle) physicalNodes.add(edge.middle);
    const resistorId = `R${edge.id}`;
    const inductorId = `L${edge.id}`;
    const resistorTo = edge.middle ?? edge.to;
    allBranches.push(
      branch(resistorId, edge.from, resistorTo, "resistor", {
        resistanceOhms: edge.resistanceOhms,
      }),
    );
    lines.push(
      `${resistorId} ${edge.from} ${resistorTo} ${spice(edge.resistanceOhms)}`,
    );
    vectorSpecs.push({
      key: resistorId,
      expression: `@${resistorId.toLowerCase()}[i]`,
      kind: "current",
    });
    if (edge.middle) {
      allBranches.push(
        branch(inductorId, edge.middle, edge.to, "inductor", {
          inductanceHenries: edge.inductanceHenries,
        }),
      );
      lines.push(
        `${inductorId} ${edge.middle} ${edge.to} ${spice(edge.inductanceHenries)}`,
      );
      vectorSpecs.push({
        key: inductorId,
        expression: `@${inductorId.toLowerCase()}[i]`,
        kind: "current",
      });
    }
  }

  const nodes = [...physicalNodes].sort((a, b) =>
    a === "0" ? -1 : b === "0" ? 1 : a.localeCompare(b),
  );
  vectorSpecs.unshift(...nodeVectors(nodes));
  for (const winding of sourceWindings) {
    vectorSpecs.push({
      key: winding.id,
      expression: `@r${winding.resistorId.slice(1).toLowerCase()}[i]`,
      kind: "current",
    });
  }
  const expressions = uniqueExpressions(vectorSpecs);

  const matrixNodes = [...externalNodes];
  const index = new Map(matrixNodes.map((node, position) => [node, position]));
  const matrix = matrixNodes.map(() => matrixNodes.map(() => C()));
  const rhs = matrixNodes.map(() => C());
  for (const winding of sourceWindings) {
    addAdmittance(
      matrix,
      rhs,
      index,
      winding.fromNode,
      winding.toNode,
      C(1 / winding.resistance),
      winding.emf,
    );
  }
  for (const edge of loadEdges) {
    const impedance = C(edge.resistanceOhms, omega * edge.inductanceHenries);
    addAdmittance(
      matrix,
      rhs,
      index,
      edge.from,
      edge.to,
      cDiv(C(1), impedance),
    );
  }
  const solution = solveComplex(matrix, rhs);
  const nodePhasors = new Map([["0", C()]]);
  matrixNodes.forEach((node, position) =>
    nodePhasors.set(node, solution[position]),
  );
  const branchPhasors = new Map();
  for (const winding of sourceWindings) {
    const current = cDiv(
      cSub(
        cSub(
          nodePhasors.get(winding.fromNode),
          nodePhasors.get(winding.toNode),
        ),
        winding.emf,
      ),
      C(winding.resistance),
    );
    nodePhasors.set(
      winding.internal,
      cAdd(nodePhasors.get(winding.toNode), winding.emf),
    );
    branchPhasors.set(winding.resistorId, current);
    branchPhasors.set(winding.id, current);
  }
  for (const edge of loadEdges) {
    const voltage = cSub(nodePhasors.get(edge.from), nodePhasors.get(edge.to));
    const current = cDiv(
      voltage,
      C(edge.resistanceOhms, omega * edge.inductanceHenries),
    );
    branchPhasors.set(`R${edge.id}`, current);
    if (edge.middle) {
      nodePhasors.set(
        edge.middle,
        cSub(nodePhasors.get(edge.from), cScale(current, edge.resistanceOhms)),
      );
      branchPhasors.set(`L${edge.id}`, current);
    }
  }

  const sampleStartSeconds = 0.1;
  const times = Array.from(
    { length: 21 },
    (_, index) => sampleStartSeconds + index / (20 * frequencyHz),
  );
  const timeState = (time) => ({
    nodes: Object.fromEntries(
      [...nodePhasors].map(([node, phasor]) => [
        node,
        cSinValue(phasor, omega, time),
      ]),
    ),
    currents: Object.fromEntries(
      [...branchPhasors].map(([id, phasor]) => [
        id,
        cSinValue(phasor, omega, time),
      ]),
    ),
  });

  const summary = {
    sourceKind,
    loadConnection: connection,
    loadKind,
    frequencyHz,
    sourceRmsVolts,
    sequence: sequenceReversed ? "reversed" : "positive",
    neutralConnected: connection === "star" ? neutralConnected : false,
    loadLineCurrentRmsAmps: Object.fromEntries(
      loadEdges.map((edge) => [
        edge.id,
        cAbs(branchPhasors.get(`R${edge.id}`)) / SQRT2,
      ]),
    ),
    loadNeutralVoltagePeak: nodePhasors.has("nload")
      ? cAbs(nodePhasors.get("nload"))
      : 0,
    engineTolerance: {
      steadyStateAmplitudeRelative: 5e-3,
      steadyStatePhaseDegrees: 0.3,
    },
  };

  const currentVectorByBranch = new Map();
  for (const winding of sourceWindings) {
    currentVectorByBranch.set(winding.resistorId, winding.resistorId);
    currentVectorByBranch.set(winding.id, winding.resistorId);
  }
  for (const edge of loadEdges) {
    currentVectorByBranch.set(`R${edge.id}`, `R${edge.id}`);
    if (edge.middle) currentVectorByBranch.set(`L${edge.id}`, `L${edge.id}`);
  }

  const stopSeconds = sampleStartSeconds + 1 / frequencyHz;
  const netlist = [
    `* Stage 9 reference circuit: ${id}`,
    `* ${description}`,
    ...lines,
    `.options ${NGSPICE_OPTIONS}`,
    ".control",
    `tran ${spice(WRDATA_STEP)} ${spice(stopSeconds)} 0 ${spice(TRAN_STEP)} uic`,
    "set wr_singlescale",
    `wrdata ${id}.out ${expressions.join(" ")}`,
    ".endc",
    ".end",
    "",
  ].join("\n");

  return {
    id,
    kind: `three_phase_${sourceKind}_${connection}_${loadKind}`,
    role: wrongVariantOf ? "wrongVariant" : "canonical",
    ...(wrongVariantOf ? { wrongVariantOf } : {}),
    description,
    parameters: {
      frequencyHz,
      phaseOffsetDegrees,
      sourceRmsVolts,
      sourceInternalResistanceOhms,
      neutralConnected: connection === "star" ? neutralConnected : false,
      sequenceReversed,
      loadConnection: connection,
      loads,
    },
    network: { referenceNode: "0", nodes, branches: allBranches },
    vectorSpecs,
    vectorExpressions: expressions,
    currentVectorByBranch,
    netlist,
    times,
    summary,
    analyticalAt(time) {
      const state = timeState(time);
      return {
        nodeVoltagesVolts: state.nodes,
        branchCurrentsAmps: state.currents,
      };
    },
    observationFromNgspice(_time, values) {
      const nodeValues = Object.fromEntries(
        nodes.map((node) => [node, node === "0" ? 0 : values.get(node)]),
      );
      const branchCurrents = {};
      for (const item of allBranches) {
        const vectorKey = currentVectorByBranch.get(item.id);
        branchCurrents[item.id] = values.get(vectorKey);
      }
      return {
        nodeVoltagesVolts: nodeValues,
        branchCurrentsAmps: branchCurrents,
      };
    },
  };
}

function wattmeterCase(reversed) {
  const id = reversed ? "wattmeter-rl-reversed-current" : "wattmeter-rl";
  const frequencyHz = 50;
  const omega = 2 * Math.PI * frequencyHz;
  const sourcePeakVolts = 10 * SQRT2;
  const sourcePhaseDegrees = 0;
  const sourceResistanceOhms = 0.1;
  const burdenResistanceOhms = 0.1;
  const loadResistanceOhms = 100;
  const inductanceHenries = 0.1;
  const inputResistanceOhms = 10_000_000;
  const loadImpedance = C(loadResistanceOhms, omega * inductanceHenries);
  const meterInputImpedance = C(inputResistanceOhms);
  const parallelLoad = cDiv(
    cMul(loadImpedance, meterInputImpedance),
    cAdd(loadImpedance, meterInputImpedance),
  );
  const sourcePhasor = cPolar(sourcePeakVolts, 0);
  const sourceCurrent = cDiv(
    sourcePhasor,
    cAdd(C(sourceResistanceOhms + burdenResistanceOhms), parallelLoad),
  );
  const voltagePhasor = cMul(sourceCurrent, parallelLoad);
  const loadCurrent = cDiv(voltagePhasor, loadImpedance);
  const inputCurrent = cDiv(voltagePhasor, meterInputImpedance);
  const phaseSign = reversed ? -1 : 1;
  const voltageRms = cScale(voltagePhasor, 1 / SQRT2);
  const currentRms = cScale(sourceCurrent, phaseSign / SQRT2);
  const apparentPower = cAbs(voltageRms) * cAbs(currentRms);
  const complexPower = cMul(voltageRms, C(currentRms.re, -currentRms.im));
  const realPower = complexPower.re;
  const reactivePower =
    Math.sign(complexPower.im) *
    Math.sqrt(
      Math.max(0, apparentPower * apparentPower - realPower * realPower),
    );
  const powerFactor = apparentPower === 0 ? 0 : realPower / apparentPower;
  const phaseAngleDegrees = Math.atan2(reactivePower, realPower) / DEG;
  const nodes = ["0", "src", "n1", "n2", "n3"];
  const branches = [
    branch("V1", "src", "0", "voltageSource"),
    branch("Rsource", "src", "n1", "resistor", {
      resistanceOhms: sourceResistanceOhms,
    }),
    branch("Rburden", "n1", "n2", "resistor", {
      resistanceOhms: burdenResistanceOhms,
    }),
    branch("Rload", "n2", "n3", "resistor", {
      resistanceOhms: loadResistanceOhms,
    }),
    branch("L1", "n3", "0", "inductor", { inductanceHenries }),
    branch("Rinput", "n2", "0", "resistor", {
      resistanceOhms: inputResistanceOhms,
    }),
  ];
  const branchPhasors = new Map([
    ["V1", cScale(sourceCurrent, -1)],
    ["Rsource", sourceCurrent],
    ["Rburden", sourceCurrent],
    ["Rload", loadCurrent],
    ["L1", loadCurrent],
    ["Rinput", inputCurrent],
  ]);
  const nodePhasors = new Map([
    ["0", C()],
    ["src", sourcePhasor],
    ["n1", cSub(sourcePhasor, cScale(sourceCurrent, sourceResistanceOhms))],
    ["n2", voltagePhasor],
    ["n3", cSub(voltagePhasor, cScale(loadCurrent, loadResistanceOhms))],
  ]);
  const measureEndSeconds = 0.3;
  const period = 1 / frequencyHz;
  const times = Array.from(
    { length: 201 },
    (_, index) => measureEndSeconds - period + (index * period) / 200,
  );
  const summary = {
    measurementWindowSeconds: period,
    measurementEndSeconds: measureEndSeconds,
    rmsVoltageVolts: cAbs(voltageRms),
    rmsCurrentAmps: cAbs(currentRms),
    realPowerWatts: realPower,
    reactivePowerVars: reactivePower,
    apparentPowerVoltAmps: apparentPower,
    powerFactor,
    phaseAngleDegrees,
    currentOrientation: reversed ? "reversed" : "loadConvention",
    engineTolerance: {
      measuredRmsMeanPeakPowerRelative: 2e-3,
      powerFactorAbsolute: 2e-3,
      phaseAngleDegrees: 0.3,
    },
  };
  const vectorSpecs = [
    ...nodeVectors(nodes),
    { key: "Rsource", expression: "@rsource[i]", kind: "current" },
    { key: "Rburden", expression: "@rburden[i]", kind: "current" },
    { key: "Rload", expression: "@rload[i]", kind: "current" },
    { key: "L1", expression: "@l1[i]", kind: "current" },
    { key: "Rinput", expression: "@rinput[i]", kind: "current" },
  ];
  vectorSpecs.push({
    key: "V1",
    expression: "@rsource[i]",
    kind: "current",
    sign: -1,
  });
  const vectorExpressions = uniqueExpressions(vectorSpecs);
  const netlist = [
    `* Stage 9 reference circuit: ${id}`,
    `* RL wattmeter, 50 Hz; current orientation ${reversed ? "reversed wrong variant" : "load convention"}.`,
    `V1 src 0 SIN(0 ${spice(sourcePeakVolts)} ${spice(frequencyHz)} 0 0 ${spice(sourcePhaseDegrees)})`,
    `Rsource src n1 ${spice(sourceResistanceOhms)}`,
    `Rburden n1 n2 ${spice(burdenResistanceOhms)}`,
    `Rload n2 n3 ${spice(loadResistanceOhms)}`,
    `L1 n3 0 ${spice(inductanceHenries)}`,
    `Rinput n2 0 ${spice(inputResistanceOhms)}`,
    `.options ${NGSPICE_OPTIONS}`,
    ".control",
    `tran ${spice(WRDATA_STEP)} ${spice(measureEndSeconds)} 0 ${spice(TRAN_STEP)} uic`,
    "set wr_singlescale",
    `wrdata ${id}.out ${vectorExpressions.join(" ")}`,
    ".endc",
    ".end",
    "",
  ].join("\n");
  const vectorMap = new Map(vectorSpecs.map((item) => [item.key, item]));
  return {
    id,
    kind: "wattmeter_rl_measurement",
    role: reversed ? "wrongVariant" : "canonical",
    ...(reversed ? { wrongVariantOf: "wattmeter-rl" } : {}),
    description: reversed
      ? "Wrong variant: the wattmeter's current-circuit reference is reversed; active and reactive power signs reverse."
      : "A wattmeter measures an RL load with its voltage circuit across the load and its current circuit in series.",
    parameters: {
      sourceRmsVolts: 10,
      frequencyHz,
      sourceInternalResistanceOhms: sourceResistanceOhms,
      burdenResistanceOhms,
      inputResistanceOhms,
      loadResistanceOhms,
      inductanceHenries,
    },
    network: { referenceNode: "0", nodes, branches },
    vectorSpecs,
    vectorExpressions,
    netlist,
    times,
    summary,
    analyticalAt(time) {
      const rotating = cExp(omega * time);
      return {
        nodeVoltagesVolts: Object.fromEntries(
          [...nodePhasors].map(([node, phasor]) => [
            node,
            cMul(phasor, rotating).im,
          ]),
        ),
        branchCurrentsAmps: Object.fromEntries(
          [...branchPhasors].map(([branchId, phasor]) => [
            branchId,
            cMul(phasor, rotating).im,
          ]),
        ),
      };
    },
    observationFromNgspice(_time, values) {
      const nodeVoltagesVolts = Object.fromEntries(
        nodes.map((node) => [node, node === "0" ? 0 : values.get(node)]),
      );
      const branchCurrentsAmps = Object.fromEntries(
        branches.map((item) => {
          const spec = vectorMap.get(item.id);
          return [item.id, (spec.sign ?? 1) * values.get(spec.key)];
        }),
      );
      return { nodeVoltagesVolts, branchCurrentsAmps };
    },
    summarizeNgspice(observations) {
      const usable = observations.slice(0, -1);
      const voltage = usable.map((sample) => sample.nodeVoltagesVolts.n2);
      const current = usable.map(
        (sample) => phaseSign * sample.branchCurrentsAmps.Rburden,
      );
      const rms = (values) =>
        Math.sqrt(
          values.reduce((sum, value) => sum + value * value, 0) / values.length,
        );
      const vRms = rms(voltage);
      const iRms = rms(current);
      let p = 0;
      let correlation = 0;
      const quarter = usable.length / 4;
      assert.equal(
        quarter,
        Math.floor(quarter),
        "measurement sample count needs four equal quarters",
      );
      for (let index = 0; index < usable.length; index += 1) {
        p += voltage[index] * current[index];
        correlation +=
          voltage[(index - quarter + usable.length) % usable.length] *
          current[index];
      }
      p /= usable.length;
      correlation /= usable.length;
      const s = vRms * iRms;
      const q = Math.sign(correlation) * Math.sqrt(Math.max(0, s * s - p * p));
      return {
        measurementWindowSeconds: period,
        measurementEndSeconds: measureEndSeconds,
        rmsVoltageVolts: vRms,
        rmsCurrentAmps: iRms,
        realPowerWatts: p,
        reactivePowerVars: q,
        apparentPowerVoltAmps: s,
        powerFactor: s === 0 ? 0 : p / s,
        phaseAngleDegrees: Math.atan2(q, p) / DEG,
        currentOrientation: reversed ? "reversed" : "loadConvention",
        engineTolerance: summary.engineTolerance,
      };
    },
  };
}

const load = (id, resistanceOhms, inductanceHenries = 0, from, to) => ({
  id,
  resistanceOhms,
  ...(inductanceHenries ? { inductanceHenries } : {}),
  ...(from ? { from } : {}),
  ...(to ? { to } : {}),
});

const CASES = [
  ...[50, 60].flatMap((frequencyHz) =>
    [0, 90].flatMap((phaseOffsetDegrees) => [
      singlePhaseCase({
        id: `rc-${frequencyHz}-phase-${phaseOffsetDegrees}`,
        kind: "series_rc",
        frequencyHz,
        phaseOffsetDegrees,
        resistanceOhms: 100,
        capacitanceFarads: 100e-6,
      }),
      singlePhaseCase({
        id: `rl-${frequencyHz}-phase-${phaseOffsetDegrees}`,
        kind: "series_rl",
        frequencyHz,
        phaseOffsetDegrees,
        resistanceOhms: 100,
        inductanceHenries: 0.1,
      }),
      singlePhaseCase({
        id: `rlc-${frequencyHz}-phase-${phaseOffsetDegrees}`,
        kind: "series_rlc",
        frequencyHz,
        phaseOffsetDegrees,
        resistanceOhms: 20,
        inductanceHenries: 0.1,
        capacitanceFarads: 100e-6,
      }),
    ]),
  ),
  singlePhaseCase({
    id: "rc-400-phase-0",
    kind: "series_rc",
    frequencyHz: 400,
    phaseOffsetDegrees: 0,
    resistanceOhms: 100,
    capacitanceFarads: 100e-6,
    highFrequency: true,
  }),
  wattmeterCase(false),
  wattmeterCase(true),
  ...[
    {
      id: "star-balanced-resistive-neutral",
      connection: "star",
      neutralConnected: true,
      loadKind: "balanced_resistive",
      loads: [load("phase1", 100), load("phase2", 100), load("phase3", 100)],
      description:
        "Balanced three-phase star resistors with the source neutral connected.",
    },
    {
      id: "star-balanced-resistive-floating-neutral",
      connection: "star",
      neutralConnected: false,
      loadKind: "balanced_resistive",
      loads: [load("phase1", 100), load("phase2", 100), load("phase3", 100)],
      description:
        "Balanced three-phase star resistors with a floating load neutral.",
    },
    {
      id: "star-balanced-rl-neutral",
      connection: "star",
      neutralConnected: true,
      loadKind: "balanced_rl",
      loads: [
        load("phase1", 100, 0.1),
        load("phase2", 100, 0.1),
        load("phase3", 100, 0.1),
      ],
      description:
        "Balanced three-phase star RL loads with the source neutral connected.",
    },
    {
      id: "star-balanced-rl-floating-neutral",
      connection: "star",
      neutralConnected: false,
      loadKind: "balanced_rl",
      loads: [
        load("phase1", 100, 0.1),
        load("phase2", 100, 0.1),
        load("phase3", 100, 0.1),
      ],
      description:
        "Balanced three-phase star RL loads with a floating load neutral.",
    },
    {
      id: "star-unbalanced-neutral",
      connection: "star",
      neutralConnected: true,
      loadKind: "unbalanced_resistive",
      loads: [load("phase1", 100), load("phase2", 220), load("phase3", 470)],
      description:
        "Unbalanced three-phase star resistors with the source neutral connected.",
    },
    {
      id: "star-unbalanced-no-neutral",
      connection: "star",
      neutralConnected: false,
      loadKind: "unbalanced_resistive",
      loads: [load("phase1", 100), load("phase2", 220), load("phase3", 470)],
      wrongVariantOf: "star-unbalanced-neutral",
      description:
        "Wrong variant: the unbalanced star load's neutral wire is missing, so its neutral point floats.",
    },
    {
      id: "star-unbalanced-reversed-sequence",
      connection: "star",
      neutralConnected: true,
      sequenceReversed: true,
      loadKind: "unbalanced_resistive",
      loads: [load("phase1", 100), load("phase2", 220), load("phase3", 470)],
      wrongVariantOf: "star-unbalanced-neutral",
      description:
        "Wrong variant: the three-phase source uses the reversed phase sequence.",
    },
  ].map((item) => threePhaseCase({ ...item, sourceKind: "star" })),
  ...[
    {
      id: "delta-load-on-star-source-balanced",
      sourceKind: "star",
      loads: [
        load("load12", 100, 0, "l1", "l2"),
        load("load23", 100, 0, "l2", "l3"),
        load("load31", 100, 0, "l3", "l1"),
      ],
      loadKind: "balanced_delta_resistive",
    },
    {
      id: "delta-load-on-star-source-unbalanced",
      sourceKind: "star",
      loads: [
        load("load12", 100, 0, "l1", "l2"),
        load("load23", 220, 0, "l2", "l3"),
        load("load31", 470, 0, "l3", "l1"),
      ],
      loadKind: "unbalanced_delta_resistive",
    },
    {
      id: "delta-load-on-delta-source-balanced",
      sourceKind: "delta",
      loads: [
        load("load12", 100, 0, "l1", "l2"),
        load("load23", 100, 0, "l2", "l3"),
        load("load31", 100, 0, "l3", "l1"),
      ],
      loadKind: "balanced_delta_resistive",
    },
    {
      id: "delta-load-on-delta-source-unbalanced",
      sourceKind: "delta",
      loads: [
        load("load12", 100, 0, "l1", "l2"),
        load("load23", 220, 0, "l2", "l3"),
        load("load31", 470, 0, "l3", "l1"),
      ],
      loadKind: "unbalanced_delta_resistive",
    },
    {
      id: "delta-wrong-connection",
      sourceKind: "delta",
      loads: [
        load("load12", 100, 0, "l1", "l2"),
        load("load23", 470, 0, "l2", "l3"),
        load("load31", 220, 0, "l3", "l1"),
      ],
      loadKind: "wrong_delta_connection",
      wrongVariantOf: "delta-load-on-delta-source-unbalanced",
      description:
        "Wrong variant: the 220 ohm and 470 ohm delta branches are interchanged.",
    },
  ].map((item) =>
    threePhaseCase({
      ...item,
      connection: "delta",
      neutralConnected: false,
      description:
        item.description ??
        `${item.loadKind} delta load on the ${item.sourceKind} three-phase source.`,
    }),
  ),
];

function validateObservation(network, observation, label, balance = BALANCE) {
  const nodeVoltages = observation.nodeVoltagesVolts;
  const branchCurrents = observation.branchCurrentsAmps;
  const kcl = Object.fromEntries(network.nodes.map((node) => [node, 0]));
  let power = 0;
  for (const item of network.branches) {
    const current = branchCurrents[item.id];
    assert(Number.isFinite(current), `${label}: missing current ${item.id}`);
    assert(
      Number.isFinite(nodeVoltages[item.from]),
      `${label}: missing node ${item.from}`,
    );
    assert(
      Number.isFinite(nodeVoltages[item.to]),
      `${label}: missing node ${item.to}`,
    );
    kcl[item.from] += current;
    kcl[item.to] -= current;
    power += (nodeVoltages[item.from] - nodeVoltages[item.to]) * current;
  }
  const maximumCurrent = Math.max(
    1,
    ...Object.values(branchCurrents).map(Math.abs),
  );
  for (const [node, residual] of Object.entries(kcl)) {
    assert(
      Math.abs(residual) <= balance.currentAbsAmps + maximumCurrent * 1e-12,
      `${label}: KCL at ${node} is ${residual} A`,
    );
  }
  assert(
    Math.abs(power) <=
      balance.powerAbsWatts + maximumCurrent * maximumCurrent * 1e-12,
    `${label}: power residual is ${power} W`,
  );
}

function makeAnalyticalFile() {
  return {
    schemaVersion: 1,
    generator: "node scripts/stage9-references.mjs --analytical --write",
    source:
      "Contract equations and closed-form linear circuit solutions; no Engine or simulator output.",
    cases: CASES.map((fixture) => ({
      id: fixture.id,
      summary: fixture.summary,
      observations: fixture.times.map((timeSeconds) => {
        const observation = fixture.analyticalAt(timeSeconds);
        return {
          timeSeconds,
          ...observation,
          nodeVoltagesVolts: { 0: 0, ...observation.nodeVoltagesVolts },
        };
      }),
    })),
  };
}

function manifest() {
  return {
    manifestVersion: 1,
    issue: "https://github.com/SWOFART/open-circuit/issues/165",
    parent: "https://github.com/SWOFART/open-circuit/issues/154",
    contract: "docs/contracts/stage-9-three-phase-foundation.md",
    documentation: "docs/solver/stage-9-references.md",
    sources: {
      analytical: {
        tool: "scripts/stage9-references.mjs",
        method:
          "Closed-form first-order RC and RL startup responses, the underdamped RLC response, sinusoidal steady-state phasors, and a complex nodal solve for the three-phase linear loads. Uses Node's built-in Math only; no Engine, simulator, or package.",
        file: "stage-9-analytical.expected.json",
      },
      ngspice: {
        simulator: NGSPICE_VERSION,
        release:
          "https://sourceforge.net/projects/ngspice/files/ng-spice-rework/47/ngspice-47_64.7z",
        archiveSha256:
          "59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a",
        license:
          "Modified BSD (ngspice COPYING); ngspice is not stored in this repository and is not a build or normal-test dependency.",
        options: NGSPICE_OPTIONS,
        transient: `tran ${spice(WRDATA_STEP)} <stop> 0 ${spice(TRAN_STEP)} uic; passive currents are recorded with savecurrents`,
        file: "stage-9-ngspice.expected.json",
      },
    },
    signAndTimeConventions: {
      source:
        "SIN(0, sqrt(2) * Vrms, frequencyHz, 0, 0, phaseOffsetDegrees); pin1 or the first winding terminal is positive.",
      branchCurrent:
        "Positive from the first node named in each netlist element to the second.",
      threePhase:
        "Positive sequence is 0, -120, +120 degrees; reversed sequence swaps the second and third phases.",
      startup:
        "The RC, RL, and RLC cases set C/L initial states to the Stage 9 operating point at e(0) and retain the start-up transient.",
      threePhaseObservations:
        "The three-phase cases are observed after 0.1 s, at 21 evenly spaced times over one 50 Hz period; RL startup has decayed before the window.",
    },
    tolerances: {
      engine: {
        steadyStateAmplitudeRelative50And60Hz: 5e-3,
        steadyStatePhaseDegrees50And60Hz: 0.3,
        startupSignalAbsoluteFraction: 1e-2,
        measuredRmsMeanPeakPowerRelative: 2e-3,
        powerFactorAbsolute: 2e-3,
        measuredPhaseAngleDegrees: 0.3,
        measuredFrequencyRelative: 1e-3,
        steadyStateAmplitudeRelative400Hz: 1e-2,
        balanceCurrentAbsoluteAmps: BALANCE.currentAbsAmps,
        balancePowerAbsoluteWatts: BALANCE.powerAbsWatts,
      },
      ngspiceAgainstAnalytical: REFERENCE_ERROR,
      storedAnswerBalance: BALANCE,
      ngspiceBalance: NGSPICE_BALANCE,
      basis:
        "The contract estimates about 0.8% worst-case amplitude error at 400 Hz. Issue #165 fixes a 1e-2 relative tolerance, leaving a small margin; the 50/60 Hz tolerances remain the contract's values.",
    },
    regeneration: {
      netlists: "node scripts/stage9-references.mjs --write-netlists",
      analytical: "node scripts/stage9-references.mjs --analytical --write",
      ngspice:
        "node scripts/stage9-references.mjs --ngspice <path to ngspice-47 console executable> --write",
      offline: "node scripts/stage9-references.mjs --check",
      rule: "Expected values are generated only from the closed-form references. The simulator validates those values and never replaces them; nothing is overwritten without --write.",
    },
    referenceError: REFERENCE_ERROR,
    balanceChecks: BALANCE,
    fixtures: CASES.map((fixture) => ({
      id: fixture.id,
      kind: fixture.kind,
      role: fixture.role,
      ...(fixture.wrongVariantOf
        ? { wrongVariantOf: fixture.wrongVariantOf }
        : {}),
      description: fixture.description,
      parameters: fixture.parameters,
      netlist: `stage-9-ngspice/${fixture.id}.cir`,
      observationCount: fixture.times.length,
      observationTimesSeconds: fixture.times,
      ...(fixture.summary.engineTolerance
        ? { engineTolerance: fixture.summary.engineTolerance }
        : {}),
      ...(fixture.role === "wrongVariant"
        ? { distinctFrom: fixture.wrongVariantOf }
        : {}),
    })),
  };
}

function parseOutput(filePath, vectorExpressions) {
  const lines = readFileSync(filePath, "utf8").trim().split(/\r?\n/);
  assert(lines.length > 1, `${filePath}: ngspice produced no samples`);
  const rows = lines.map((line) =>
    line.trim().split(/\s+/).map(Number.parseFloat),
  );
  for (const [index, row] of rows.entries()) {
    assert.equal(
      row.length,
      vectorExpressions.length + 1,
      `${filePath}: row ${index} column count`,
    );
    assert(
      row.every(Number.isFinite),
      `${filePath}: row ${index} contains a non-number`,
    );
  }
  return rows;
}

function interpolate(rows, time) {
  let low = 0;
  let high = rows.length - 1;
  const endTolerance = 1e-12 * Math.max(1, Math.abs(time));
  assert(
    time >= rows[0][0] - endTolerance && time <= rows[high][0] + endTolerance,
    `time ${time} is outside ngspice output`,
  );
  time = Math.max(rows[0][0], Math.min(rows[high][0], time));
  while (high - low > 1) {
    const middle = (low + high) >> 1;
    if (rows[middle][0] <= time) low = middle;
    else high = middle;
  }
  if (rows[low][0] === time) return rows[low];
  const fraction = (time - rows[low][0]) / (rows[high][0] - rows[low][0]);
  return rows[low].map((value, index) =>
    index === 0 ? time : value + (rows[high][index] - value) * fraction,
  );
}

function runNgspice(executable) {
  const version = execFileSync(executable, ["--version"], { encoding: "utf8" });
  assert(
    version.includes(NGSPICE_VERSION),
    `expected ${NGSPICE_VERSION}; found ${version.trim()}`,
  );
  const cases = [];
  for (const fixture of CASES) {
    const tempRoot = mkdtempSync(join(tmpdir(), "open-circuit-stage9-"));
    try {
      writeFileSync(
        join(tempRoot, `${fixture.id}.cir`),
        fixture.netlist,
        "utf8",
      );
      const output = execFileSync(executable, ["-b", `${fixture.id}.cir`], {
        cwd: tempRoot,
        encoding: "utf8",
        maxBuffer: 4 * 1024 * 1024,
      });
      assert.match(
        output,
        /No\. of Data Rows\s*:/,
        `${fixture.id}: ngspice did not complete`,
      );
      const rows = parseOutput(
        join(tempRoot, `${fixture.id}.out`),
        fixture.vectorExpressions,
      );
      const vectorIndex = new Map(
        fixture.vectorExpressions.map((expression, index) => [
          expression,
          index + 1,
        ]),
      );
      const observations = fixture.times.map((timeSeconds) => {
        const row = interpolate(rows, timeSeconds);
        const values = new Map(
          fixture.vectorSpecs.map((spec) => [
            spec.key,
            row[vectorIndex.get(spec.expression)],
          ]),
        );
        return {
          timeSeconds,
          ...fixture.observationFromNgspice(timeSeconds, values),
        };
      });
      cases.push({
        id: fixture.id,
        ...(fixture.summarizeNgspice
          ? { summary: fixture.summarizeNgspice(observations) }
          : { summary: fixture.summary }),
        observations,
      });
    } finally {
      rmSync(tempRoot, { recursive: true, force: true });
    }
  }
  return {
    schemaVersion: 1,
    simulator: NGSPICE_VERSION,
    source: NGSPICE_SOURCE,
    cases,
  };
}

function byId(file, label) {
  const cases = file?.cases;
  assert(Array.isArray(cases), `${label}: cases array missing`);
  const entries = new Map(cases.map((entry) => [entry.id, entry]));
  assert.equal(entries.size, cases.length, `${label}: duplicate case IDs`);
  return entries;
}

function assertClose(actual, expected, absolute, relative, what) {
  assert(
    Number.isFinite(actual) && Number.isFinite(expected),
    `${what}: non-finite`,
  );
  const bound =
    absolute + relative * Math.max(Math.abs(actual), Math.abs(expected));
  assert(
    Math.abs(actual - expected) <= bound,
    `${what}: ${actual} vs ${expected}, bound ${bound}`,
  );
}

function assertGeneratedData(actual, expected, label = "generated data") {
  if (typeof expected === "number") {
    assert.equal(typeof actual, "number", `${label}: expected a number`);
    assertClose(
      actual,
      expected,
      PORTABLE_GENERATION_ERROR.absolute,
      PORTABLE_GENERATION_ERROR.relative,
      label,
    );
    return;
  }
  if (Array.isArray(expected)) {
    assert(Array.isArray(actual), `${label}: expected an array`);
    assert.equal(actual.length, expected.length, `${label}: array length`);
    expected.forEach((item, index) =>
      assertGeneratedData(actual[index], item, `${label}[${index}]`),
    );
    return;
  }
  if (expected !== null && typeof expected === "object") {
    assert(
      actual !== null && typeof actual === "object" && !Array.isArray(actual),
      `${label}: expected an object`,
    );
    const expectedKeys = Object.keys(expected).sort();
    assert.deepEqual(
      Object.keys(actual).sort(),
      expectedKeys,
      `${label}: keys`,
    );
    for (const key of expectedKeys) {
      assertGeneratedData(actual[key], expected[key], `${label}.${key}`);
    }
    return;
  }
  assert.equal(actual, expected, label);
}

function assertKeys(value, expectedKeys, label) {
  assert(
    value !== null && typeof value === "object" && !Array.isArray(value),
    `${label}: expected an object`,
  );
  assert.deepEqual(
    Object.keys(value).sort(),
    [...expectedKeys].sort(),
    `${label}: keys`,
  );
}

function validateWrongVariants(analyticalCases) {
  const cases = byId(analyticalCases, "analytical");
  for (const fixture of CASES.filter((item) => item.role === "wrongVariant")) {
    const correct = cases.get(fixture.wrongVariantOf);
    const wrong = cases.get(fixture.id);
    assert(correct && wrong, `${fixture.id}: wrong-variant pair missing`);
    if (fixture.id === "wattmeter-rl-reversed-current") {
      assert(
        wrong.summary.realPowerWatts < 0 && correct.summary.realPowerWatts > 0,
      );
      continue;
    }
    let distinct = false;
    const count = Math.min(
      correct.observations.length,
      wrong.observations.length,
    );
    for (let index = 0; index < count && !distinct; index += 1) {
      const a = correct.observations[index];
      const b = wrong.observations[index];
      for (const key of Object.keys(a.nodeVoltagesVolts)) {
        if (
          Math.abs(a.nodeVoltagesVolts[key] - b.nodeVoltagesVolts[key]) > 1e-3
        )
          distinct = true;
      }
      for (const key of Object.keys(a.branchCurrentsAmps)) {
        if (
          Math.abs(a.branchCurrentsAmps[key] - b.branchCurrentsAmps[key]) > 1e-5
        )
          distinct = true;
      }
    }
    assert(
      distinct,
      `${fixture.id}: wrong variant did not differ from ${fixture.wrongVariantOf}`,
    );
  }
}

function verifyData(analytical, ngspice) {
  const manifestData = manifest();
  const analyticalById = byId(analytical, "analytical");
  const ngspiceById = byId(ngspice, "ngspice");
  const expectedIds = CASES.map((fixture) => fixture.id).sort();
  assert.deepEqual([...analyticalById.keys()].sort(), expectedIds);
  assert.deepEqual([...ngspiceById.keys()].sort(), expectedIds);
  assertKeys(
    ngspice,
    ["schemaVersion", "simulator", "source", "cases"],
    "ngspice file",
  );
  assert.equal(ngspice.schemaVersion, 1, "ngspice schema version");
  assert.equal(ngspice.simulator, NGSPICE_VERSION, "ngspice simulator version");
  assert.equal(ngspice.source, NGSPICE_SOURCE, "ngspice source description");
  validateWrongVariants(analytical);

  for (const fixture of CASES) {
    const analyticCase = analyticalById.get(fixture.id);
    const spiceCase = ngspiceById.get(fixture.id);
    assertKeys(
      spiceCase,
      ["id", "summary", "observations"],
      `${fixture.id}/ngspice case`,
    );
    assert.equal(spiceCase.id, fixture.id, `${fixture.id}: ngspice case id`);
    assertKeys(
      spiceCase.summary,
      Object.keys(analyticCase.summary),
      `${fixture.id}/ngspice summary`,
    );
    assert.equal(
      analyticCase.observations.length,
      fixture.times.length,
      `${fixture.id}: analytical count`,
    );
    assert.equal(
      spiceCase.observations.length,
      fixture.times.length,
      `${fixture.id}: ngspice count`,
    );
    for (const [sourceName, rows] of [
      ["analytical", analyticCase.observations],
      ["ngspice", spiceCase.observations],
    ]) {
      for (const [index, observation] of rows.entries()) {
        assertKeys(
          observation,
          ["timeSeconds", "nodeVoltagesVolts", "branchCurrentsAmps"],
          `${fixture.id}/${sourceName}/${index}`,
        );
        assertKeys(
          observation.nodeVoltagesVolts,
          fixture.network.nodes,
          `${fixture.id}/${sourceName}/${index}/nodeVoltagesVolts`,
        );
        assertKeys(
          observation.branchCurrentsAmps,
          fixture.network.branches.map((item) => item.id),
          `${fixture.id}/${sourceName}/${index}/branchCurrentsAmps`,
        );
        assertClose(
          observation.timeSeconds,
          fixture.times[index],
          1e-14,
          0,
          `${fixture.id}/${sourceName}: time`,
        );
        validateObservation(
          fixture.network,
          observation,
          `${fixture.id}/${sourceName}/${index}`,
          sourceName === "ngspice" ? NGSPICE_BALANCE : BALANCE,
        );
      }
    }
    const expected = new Map(
      analyticCase.observations.map((row) => [row.timeSeconds, row]),
    );
    for (const observation of spiceCase.observations) {
      const exact = expected.get(observation.timeSeconds);
      assert(exact, `${fixture.id}: ngspice time lacks analytical answer`);
      for (const item of fixture.network.branches) {
        const expectedCurrent = exact.branchCurrentsAmps[item.id];
        const actualCurrent = observation.branchCurrentsAmps[item.id];
        assertClose(
          actualCurrent,
          expectedCurrent,
          REFERENCE_ERROR.currentAbsAmps,
          REFERENCE_ERROR.relative,
          `${fixture.id}/${observation.timeSeconds}/${item.id} current`,
        );
        const expectedVoltage =
          exact.nodeVoltagesVolts[item.from] - exact.nodeVoltagesVolts[item.to];
        const actualVoltage =
          observation.nodeVoltagesVolts[item.from] -
          observation.nodeVoltagesVolts[item.to];
        assertClose(
          actualVoltage,
          expectedVoltage,
          REFERENCE_ERROR.voltageAbsVolts,
          REFERENCE_ERROR.relative,
          `${fixture.id}/${observation.timeSeconds}/${item.id} voltage`,
        );
      }
    }
    const meterSummaryError = new Map([
      ["rmsVoltageVolts", REFERENCE_ERROR.voltageAbsVolts],
      ["rmsCurrentAmps", REFERENCE_ERROR.currentAbsAmps],
      ["realPowerWatts", REFERENCE_ERROR.powerAbsWatts],
      ["reactivePowerVars", REFERENCE_ERROR.powerAbsWatts],
      ["apparentPowerVoltAmps", REFERENCE_ERROR.powerAbsWatts],
      ["powerFactor", REFERENCE_ERROR.powerFactorAbs],
      ["phaseAngleDegrees", REFERENCE_ERROR.phaseAngleAbsDegrees],
    ]);
    for (const [key, expectedValue] of Object.entries(analyticCase.summary)) {
      const absolute = fixture.summarizeNgspice
        ? meterSummaryError.get(key)
        : undefined;
      if (absolute === undefined) {
        assertGeneratedData(
          spiceCase.summary[key],
          expectedValue,
          `${fixture.id}/summary.${key}`,
        );
      } else {
        assertClose(
          spiceCase.summary[key],
          expectedValue,
          absolute,
          REFERENCE_ERROR.relative,
          `${fixture.id}/${key}`,
        );
      }
    }
  }
  const fixtures = manifestData.fixtures;
  assert(fixtures.some((fixture) => fixture.role === "canonical"));
  assert(fixtures.some((fixture) => fixture.role === "wrongVariant"));
  for (const wanted of [
    "series_rc",
    "series_rl",
    "series_rlc",
    "three_phase_star_star_balanced_resistive",
    "three_phase_delta_delta_unbalanced_delta_resistive",
  ]) {
    assert(
      CASES.some((fixture) => fixture.kind === wanted),
      `missing ${wanted}`,
    );
  }
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeOrCheck(path, contents, shouldWrite) {
  if (shouldWrite) {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, json(contents), "utf8");
    return;
  }
  assertGeneratedData(
    readJson(path),
    stable(contents),
    `${path} is stale; regenerate deliberately`,
  );
}

function writeNetlists() {
  mkdirSync(NETLISTS, { recursive: true });
  for (const fixture of CASES) {
    writeFileSync(join(NETLISTS, `${fixture.id}.cir`), fixture.netlist, "utf8");
  }
}

function checkNetlists() {
  const expected = CASES.map((fixture) => `${fixture.id}.cir`).sort();
  const actual = readdirSync(NETLISTS)
    .filter((name) => name.endsWith(".cir"))
    .sort();
  assert.deepEqual(
    actual,
    expected,
    "stage-9 netlist set is stale or has unexpected files",
  );
  for (const fixture of CASES) {
    assert.equal(
      readFileSync(join(NETLISTS, `${fixture.id}.cir`), "utf8"),
      fixture.netlist,
      `${fixture.id}: netlist differs from its case table`,
    );
  }
}

function check() {
  const checkedManifest = readJson(MANIFEST_PATH);
  const analytical = readJson(ANALYTICAL_PATH);
  const ngspice = readJson(NGSPICE_PATH);
  assertGeneratedData(
    checkedManifest,
    stable(manifest()),
    "Stage 9 manifest is stale",
  );
  assertGeneratedData(
    analytical,
    stable(makeAnalyticalFile()),
    "Stage 9 analytical references are stale",
  );
  checkNetlists();
  verifyData(analytical, ngspice);
  console.log(
    `Verified ${CASES.length} Stage 9 circuits, ${CASES.reduce((sum, item) => sum + item.times.length, 0)} stored observations, KCL, power balance, and wrong variants`,
  );
}

function usage() {
  return [
    "Usage:",
    "  node scripts/stage9-references.mjs --write-netlists",
    "  node scripts/stage9-references.mjs --manifest --write",
    "  node scripts/stage9-references.mjs --analytical [--write]",
    "  node scripts/stage9-references.mjs --ngspice <ngspice-47 executable> [--write]",
    "  node scripts/stage9-references.mjs --check",
  ].join("\n");
}

function main() {
  const args = process.argv.slice(2);
  if (args.length === 0) throw new Error(usage());
  const write = args.includes("--write");
  if (args.includes("--write-netlists")) {
    writeNetlists();
    console.log(`Wrote ${CASES.length} Stage 9 ngspice netlists`);
    return;
  }
  if (args.includes("--manifest")) {
    writeOrCheck(MANIFEST_PATH, manifest(), write);
    console.log(`${write ? "Wrote" : "Verified"} Stage 9 manifest`);
    return;
  }
  if (args.includes("--analytical")) {
    const expected = makeAnalyticalFile();
    if (!write) process.stdout.write(json(expected));
    else {
      writeOrCheck(ANALYTICAL_PATH, expected, true);
      console.log("Wrote closed-form Stage 9 answers");
    }
    return;
  }
  if (args.includes("--ngspice")) {
    const index = args.indexOf("--ngspice");
    const executable = args[index + 1];
    assert(executable && !executable.startsWith("--"), usage());
    assert(
      args.filter((arg) => arg === "--ngspice").length === 1,
      "use one ngspice executable per run",
    );
    checkNetlists();
    const result = runNgspice(executable);
    const analytical = makeAnalyticalFile();
    verifyData(analytical, result);
    writeOrCheck(NGSPICE_PATH, result, write);
    console.log(
      `${write ? "Wrote" : "Verified"} ${CASES.length} ngspice-47 circuit results`,
    );
    return;
  }
  if (args.includes("--check")) {
    assert.equal(
      args.length,
      1,
      "--check cannot be combined with other actions",
    );
    check();
    return;
  }
  throw new Error(usage());
}

main();
