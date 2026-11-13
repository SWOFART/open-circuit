// Deliberate, offline fixture generation. No Engine imports, subprocesses or solver output.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const golden = new URL("../tests/golden/", import.meta.url);
const read = (name) => JSON.parse(readFileSync(new URL(name, golden), "utf8"));
const id = (n) => `00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;
const point = (x, y) => ({ x, y });
const rail = (n, name, x, top, bottom) => ({
  id: id(n),
  name,
  start: point(x, top),
  end: point(x, bottom),
});
const wire = (n, points) => ({
  id: id(n),
  points: points.map(([x, y]) => point(x, y)),
});
const value = (component, suffix, unit, expected, toleranceAbs) => ({
  key: `${id(component)}${suffix.includes("/") ? "/" : ":"}${suffix}`,
  unit,
  expected,
  toleranceAbs,
  toleranceRel: 0,
});
const pin = (component, name) => `${id(component)}/${name}`;
const groups = (...items) =>
  items
    .map((g) => g.toSorted())
    .toSorted((a, b) =>
      a.join() < b.join() ? -1 : a.join() > b.join() ? 1 : 0,
    );

// These are literal circuit identities, not a connectivity algorithm.
const dividerGroups = groups(
  [pin(1, "pin1"), pin(2, "pin1")],
  [pin(2, "pin2"), pin(3, "pin1")],
  [pin(1, "pin2"), pin(3, "pin2")],
);
const latchGroups = (sealed) =>
  groups(
    [pin(1, "pin1"), pin(2, "pin1")],
    [pin(2, "pin2"), pin(3, "pin1"), ...(sealed ? [pin(4, "no_1")] : [])],
    [pin(3, "pin2"), pin(4, "a1"), ...(sealed ? [pin(4, "no_2")] : [])],
    [pin(1, "pin2"), pin(4, "a2")],
    [pin(4, "nc_1")],
    [pin(4, "nc_2")],
    ...(!sealed ? [[pin(4, "no_1")], [pin(4, "no_2")]] : []),
  );
const rcGroups = groups(
  [pin(1, "pin1"), pin(2, "pin1")],
  [pin(2, "pin2"), pin(3, "pin1")],
  [pin(3, "pin2"), pin(4, "pin1")],
  [pin(4, "pin2"), pin(1, "pin2")],
);

function paired(name, source, replacements, connectedPins) {
  const request = structuredClone(source);
  const rails = [];
  for (const [wireIndex, segmentIndex, name] of replacements) {
    const original = request.schematic.wires[wireIndex];
    const a = original.points[segmentIndex];
    const b = original.points[segmentIndex + 1];
    assert.equal(a.x, b.x);
    rails.push(
      rail(
        0x8000 + rails.length,
        name,
        a.x,
        Math.min(a.y, b.y),
        Math.max(a.y, b.y),
      ),
    );
    // Remove only a boundary segment of the original continuous path.
    assert(segmentIndex === 0 || segmentIndex === original.points.length - 2);
    original.points =
      segmentIndex === 0
        ? original.points.slice(1)
        : original.points.slice(0, -1);
    assert(original.points.length >= 2);
  }
  request.schematic.schemaVersion = 2;
  request.schematic.rails = rails;
  return makePair(name, request, connectedPins);
}

function makePair(name, request, connectedPins) {
  const railSchematic = request.schematic;
  const wireSchematic = structuredClone(railSchematic);
  wireSchematic.schemaVersion = 1; // Strict legacy decoder makes it executable before #130.
  delete wireSchematic.rails;
  for (const [index, r] of railSchematic.rails.entries()) {
    wireSchematic.wires.push({
      id: id(0x9000 + index),
      points: [r.start, r.end],
    });
  }
  return {
    id: name,
    railSchematic,
    wireSchematic,
    geometry: request.geometry,
    connectedPins,
    railWireMap: railSchematic.rails.map((r, i) => ({
      railId: r.id,
      wireId: id(0x9000 + i),
    })),
    events: [],
    observations: [],
    expectedWarningCodes: [],
  };
}

const divider = paired(
  "divider",
  {
    schematic: read("stage-1-divider.schematic.json"),
    geometry: read("stage-1-divider.geometry.json"),
  },
  [
    [0, 0, "L+"],
    [2, 0, "0V"],
  ],
  dividerGroups,
);
const dividerAnswer = (step, r1) => {
  const i = 5 / (0.1 + r1 + 1000);
  return {
    step,
    values: [
      value(1, "current", "amp", -i, 1e-9),
      value(1, "voltage", "volt", i * (r1 + 1000), 1e-6),
      value(2, "current", "amp", i, 1e-9),
      value(2, "voltage", "volt", i * r1, 1e-6),
      value(3, "current", "amp", i, 1e-9),
      value(3, "voltage", "volt", i * 1000, 1e-6),
    ],
    states: [],
  };
};
// Observation order explicitly distinguishes before/after a same-step live edit.
divider.observations = [
  dividerAnswer(0, 1000),
  dividerAnswer(10, 1000),
  { ...dividerAnswer(10, 3000), afterEvent: 0 },
  { ...dividerAnswer(20, 3000), afterEvent: 0 },
];
divider.events = [
  {
    kind: "parameter",
    newestStep: 10,
    componentId: id(2),
    parameterId: "resistanceOhms",
    value: 3000,
  },
];

const latchSource = read("stage-6-start-stop-latching.expected.json");
function latching(name, sourceFile, sealed) {
  const pair = paired(
    name,
    read(sourceFile),
    [
      [0, 0, "L+"],
      [3, 0, "0V"],
    ],
    latchGroups(sealed),
  );
  pair.events = latchSource.operatorEvents.map((e) => ({
    kind: "operator",
    newestStep: e.sentWhenNewestStep,
    effectiveFromStep: e.effectiveFromStep,
    componentId: id(e.component === "START" ? 3 : 2),
    position: e.position,
  }));
  const rows = sealed
    ? latchSource.expected
    : latchSource.wrongVariants[0].expected;
  pair.observations = rows.map((row) => ({
    step: row.step,
    afterEventsAtStep: true,
    values: [value(4, "coil/current", "amp", row.coilCurrentAmps, 1e-4)],
    states: [{ componentId: id(4), state: row.contactor }],
  }));
  return pair;
}
const latch = latching(
  "start-stop",
  "stage-6-drawn-latching.request.json",
  true,
);
const wrong = latching(
  "start-stop-no-seal-in",
  "stage-6-drawn-unlatched.request.json",
  false,
);
const rcSource = read("stage-6-button-capacitor-hold.expected.json");
const rc = paired(
  "rc-hold",
  read("stage-6-drawn-button.request.json"),
  [
    [0, 0, "L+"],
    [3, 0, "0V"],
  ],
  rcGroups,
);
rc.events = rcSource.operatorEvents.map((e) => ({
  kind: "operator",
  newestStep: e.sentWhenNewestStep,
  effectiveFromStep: e.effectiveFromStep,
  componentId: id(2),
  position: e.position,
}));
rc.observations = rcSource.expected.map((row) => ({
  step: row.step,
  afterEventsAtStep: true,
  values: [
    value(4, "voltage", "volt", row.capacitorVolts, row.toleranceVolts),
    ...(row.currentAmps === undefined
      ? []
      : [
          value(4, "current", "amp", row.currentAmps, row.currentToleranceAmps),
        ]),
  ],
  states: [
    {
      componentId: id(2),
      position: row.buttonPosition,
      conducting: row.buttonConducting,
    },
  ],
}));

function isolation() {
  const schematic = {
    schemaVersion: 2,
    components: [],
    wires: [],
    junctions: [],
    referenceMarkers: [],
    rails: [],
  };
  const geometry = { components: [] };
  for (const [offset, volts] of [
    [0, 5],
    [10, 10],
  ]) {
    for (const [n, typeId, y, parameters] of [
      [
        offset === 0 ? 1 : 3,
        "dc_voltage_source",
        0,
        { voltageVolts: volts, internalResistanceOhms: 0 },
      ],
      [offset === 0 ? 2 : 4, "resistor", 10, { resistanceOhms: 1000 }],
    ]) {
      schematic.components.push({
        id: id(n),
        typeId,
        position: point(offset, y),
        rotationDeg: 0,
        parameters,
      });
      geometry.components.push({
        componentId: id(n),
        pins: [
          { pinId: "pin1", position: point(offset - 1, y) },
          { pinId: "pin2", position: point(offset + 1, y) },
        ],
      });
    }
    schematic.rails.push(
      rail(0x8000 + schematic.rails.length, `P${offset}`, offset - 1, 0, 10),
      rail(0x8001 + schematic.rails.length, `R${offset}`, offset + 1, 0, 10),
    );
  }
  const pair = makePair(
    "two-islands",
    { schematic, geometry },
    groups(
      [pin(1, "pin1"), pin(2, "pin1")],
      [pin(1, "pin2"), pin(2, "pin2")],
      [pin(3, "pin1"), pin(4, "pin1")],
      [pin(3, "pin2"), pin(4, "pin2")],
    ),
  );
  pair.expectedIslandCount = 2;
  pair.expectedWarningCodes = ["NoGroundReference", "NoGroundReference"];
  pair.observations = [0, 10].map((step) => ({
    step,
    values: [
      value(2, "voltage", "volt", 5, 1e-6),
      value(2, "current", "amp", 0.005, 1e-9),
      value(4, "voltage", "volt", 10, 1e-6),
      value(4, "current", "amp", 0.01, 1e-9),
    ],
    states: [],
  }));
  return pair;
}
const isolated = isolation();
const namedIsolation = structuredClone(isolated);
namedIsolation.id = "two-islands-N-PE";
namedIsolation.railSchematic.rails.forEach(
  (r, i) => (r.name = ["N", "PE", "L1", "L2"][i]),
);

// Shared pure data for Rust and future editor-hint tests. Expected partitions are handwritten.
const base = {
  schemaVersion: 2,
  components: [
    {
      id: id(1),
      typeId: "resistor",
      position: point(-2, 6),
      rotationDeg: 90,
      parameters: { resistanceOhms: 1000 },
    },
    {
      id: id(2),
      typeId: "resistor",
      position: point(1, 0),
      rotationDeg: 0,
      parameters: { resistanceOhms: 1000 },
    },
  ],
  wires: [
    wire(10, [
      [-2, 5],
      [2, 5],
    ]),
  ],
  junctions: [],
  referenceMarkers: [],
  rails: [rail(0x8000, "L+", 0, 0, 10)],
};
const baseGeometry = {
  components: [
    {
      componentId: id(1),
      pins: [
        { pinId: "pin1", position: point(-2, 5) },
        { pinId: "pin2", position: point(-2, 7) },
      ],
    },
    {
      componentId: id(2),
      pins: [
        { pinId: "pin1", position: point(0, 0) },
        { pinId: "pin2", position: point(2, 0) },
      ],
    },
  ],
};
const separate = groups(
  [pin(1, "pin1")],
  [pin(1, "pin2")],
  [pin(2, "pin1")],
  [pin(2, "pin2")],
);
const joined = groups(
  [pin(1, "pin1"), pin(2, "pin1")],
  [pin(1, "pin2")],
  [pin(2, "pin2")],
);
const cases = [];
function topology(name, edit, expected = separate, errorCodes = []) {
  const request = {
    schematic: structuredClone(base),
    geometry: structuredClone(baseGeometry),
  };
  edit(request.schematic, request.geometry);
  const pair = makePair(name, request, expected);
  cases.push({ ...pair, errorCodes });
}
topology("interior-crossing", () => {});
topology(
  "crossing-junction",
  (s) => s.junctions.push({ id: id(20), position: point(0, 5) }),
  joined,
);
topology(
  "crossing-reference",
  (s) => s.referenceMarkers.push({ id: id(20), position: point(0, 5) }),
  joined,
);
topology(
  "wire-end-rail-interior",
  (s) =>
    (s.wires = [
      wire(10, [
        [-2, 5],
        [0, 5],
      ]),
    ]),
  joined,
);
topology(
  "wire-end-rail-end",
  (s) =>
    (s.wires = [
      wire(10, [
        [-2, 5],
        [-2, 0],
        [0, 0],
      ]),
    ]),
  joined,
);
topology("rail-end-wire-interior", (s) => (s.rails[0].end.y = 5), joined);
topology(
  "collinear-wire-overlap",
  (s) =>
    (s.wires = [
      wire(10, [
        [-2, 5],
        [-4, 5],
        [-4, 8],
        [0, 8],
        [0, 15],
      ]),
    ]),
  joined,
);
topology(
  "pin-on-rail-interior",
  (s, g) => {
    s.wires = [];
    s.components[0].position = point(-1, 5);
    s.components[0].rotationDeg = 0;
    g.components[0].pins = [
      { pinId: "pin1", position: point(-2, 5) },
      { pinId: "pin2", position: point(0, 5) },
    ];
  },
  groups([pin(1, "pin1")], [pin(1, "pin2"), pin(2, "pin1")], [pin(2, "pin2")]),
);
topology("rail-overlap", (s) => {
  s.wires = [];
  s.rails.push(rail(0x8001, "CONT", 0, 8, 15));
});
topology("rail-end-touch", (s) => {
  s.wires = [];
  s.rails.push(rail(0x8001, "CONT", 0, 10, 15));
});
topology(
  "rail-gap",
  (s) => {
    s.wires = [];
    s.rails.push(rail(0x8001, "CONT", 0, 11, 15));
  },
  [],
  ["FloatingNode"],
);
topology("dangling-extension", (s) => (s.wires = []));
topology("unique-rename", (s) => (s.rails[0].name = "PE"));
topology("case-sensitive-names", (s) => {
  s.wires = [];
  s.rails[0].name = "L1";
  s.rails.push(rail(0x8001, "l1", 0, 8, 15));
});
topology(
  "duplicate-name",
  (s) => s.rails.push(rail(0x8001, "L+", 0, 8, 15)),
  [],
  ["DuplicateRailName"],
);
topology(
  "invalid-name",
  (s) => (s.rails[0].name = " L+"),
  [],
  ["InvalidRailName"],
);
topology(
  "diagonal-rail",
  (s) => (s.rails[0].end.x = 1),
  [],
  ["InvalidGeometry"],
);
topology(
  "reversed-rail",
  (s) =>
    ([s.rails[0].start, s.rails[0].end] = [s.rails[0].end, s.rails[0].start]),
  [],
  ["InvalidGeometry"],
);
topology(
  "one-unit-rail",
  (s) => (s.rails[0].end.y = 1),
  [],
  ["InvalidGeometry"],
);
topology(
  "overflow-safe-extent",
  (s) => {
    s.rails[0].start.y = -2147483648;
    s.rails[0].end.y = 2147483647;
  },
  [],
  ["InvalidGeometry"],
);
topology(
  "detached-reference",
  (s) => s.referenceMarkers.push({ id: id(20), position: point(3, 5) }),
  [],
  ["DetachedReferenceMarker"],
);
topology("reference-on-rail", (s) =>
  s.referenceMarkers.push({ id: id(20), position: point(0, 2) }),
);
topology(
  "global-reference-union",
  (s) => {
    s.wires = [
      wire(10, [
        [-2, 5],
        [-4, 5],
      ]),
    ];
    s.rails.push(rail(0x8001, "RETURN", -4, 0, 10));
    s.referenceMarkers = [
      { id: id(20), position: point(0, 2) },
      { id: id(21), position: point(-4, 2) },
    ];
  },
  joined,
);
topology(
  "empty-rail",
  (s, g) => {
    s.components = [];
    s.wires = [];
    g.components = [];
  },
  [],
  ["FloatingNode"],
);
topology(
  "empty-referenced-rail",
  (s, g) => {
    s.components = [];
    s.wires = [];
    g.components = [];
    s.referenceMarkers = [{ id: id(20), position: point(0, 2) }];
  },
  [],
  ["FloatingNode"],
);

topology(
  "duplicate-disconnected-name",
  (s) => s.rails.push(rail(0x8001, "L+", 10, 0, 10)),
  [],
  ["DuplicateRailName"],
);
topology(
  "collinear-wire-end-touch",
  (s) =>
    (s.wires = [
      wire(10, [
        [-2, 5],
        [-4, 5],
        [-4, 15],
        [0, 15],
        [0, 10],
      ]),
    ]),
  joined,
);

for (const entry of cases) {
  const kind = entry.errorCodes[0];
  const ref = (r) => ({ kind: "rail", railId: r.id });
  entry.errorReferences = !kind
    ? []
    : kind === "DetachedReferenceMarker"
      ? [{ kind: "referenceMarker", referenceMarkerId: id(20) }]
      : kind === "DuplicateRailName"
        ? entry.railSchematic.rails.map(ref)
        : kind === "FloatingNode"
          ? [
              ref(entry.railSchematic.rails.at(-1)),
              ...entry.railSchematic.referenceMarkers.map((m) => ({
                kind: "referenceMarker",
                referenceMarkerId: m.id,
              })),
            ]
          : [ref(entry.railSchematic.rails[0])];
  const referenceId = (reference) =>
    reference.railId ?? reference.referenceMarkerId;
  entry.errorReferences.sort((a, b) =>
    referenceId(a) < referenceId(b)
      ? -1
      : referenceId(a) > referenceId(b)
        ? 1
        : 0,
  );
  if (entry.id === "empty-referenced-rail") {
    entry.legacyWireLimitation = {
      issue: 137,
      reason:
        "Existing wire netlist incorrectly accepts a referenced pinless island; retain FloatingNode oracle and fix conductor validation in #131.",
    };
  }
}

const circuits = [divider, latch, wrong, rc, isolated, namedIsolation];
const manifest = {
  manifestVersion: 1,
  issue: 129,
  contract: "docs/contracts/stage-8-supply-rails.md",
  baseline: "f0737763f3881140d0a6d97a10b8beb95b7ed1e1",
  generator: "node scripts/stage8-references.mjs --write",
  check: "node scripts/stage8-references.mjs --check",
  provenance:
    "Original project fixtures and direct Ohm/Kirchhoff/closed-form RC equations; no Engine-derived answers or added external models. Reuses Stage 1/6 known answers; new ngspice data is unnecessary for a topology-only feature.",
  runtime:
    "Node 24.19.0 (pinned .node-version), standard library only; routine tests offline",
  files: ["stage-8-circuits.json", "stage-8-connectivity.json"],
  equivalence: { absolute: 1e-9, relative: 1e-9, transitionTimeSeconds: 1e-9 },
  tolerances: { kclAmps: 1e-9, terminalPowerWatts: 1e-8 },
  conventions:
    "SI; current positive entering pin1 (source delivering is negative). Contactor coil current enters a1. Component IDs, pin IDs and signal keys match exactly across pairs; derived node/wire/rail IDs do not. Wire snapshots use schema v1 for pre-#130 executability; the project decoder migrates them to the current schema thereafter.",
  observationOrder:
    "afterEventsAtStep means operate before reading that sample. Divider observations without afterEvent precede the same-step parameter edit; afterEvent:0 follows its first edit. Event takes effect from (newestStep-1)*0.001 s (zero at step 0).",
  sources: [
    "stage-1-divider.expected.json",
    "stage-6-start-stop-latching.expected.json",
    "stage-6-button-capacitor-hold.expected.json",
  ],
  consumers: {
    references:
      "#129 validates data and wire-side netlists/physics only; it does not implement or certify rail solving",
    engine:
      "#130 decodes/validates rails; #131 consumes rail side plus partitions, mapped values/states and permutation/repeat checks",
    application:
      "#132 bridge and #133 editor-hint tests consume the same JSON; #134 draws these circuits with editor commands and matches component/pin keys; #135 records real desktop evidence",
  },
};

const near = (a, b, eps) => assert(Math.abs(a - b) <= eps, `${a} != ${b}`);
// Independent checks on reused reference answers, before generating any output.
const d = read("stage-1-divider.expected.json");
near(5 / 2000.1, d.equalResistors.sourceCurrentAmps, 1e-17);
near(5 / 4000.1, d.resistor1At3000Ohms.sourceCurrentAmps, 1e-17);
for (const row of latchSource.expected)
  near(
    row.coilCurrentAmps,
    [5, 8].includes(row.step)
      ? 5 / (100 + 0.01 + (0.01 * 0.1) / (0.01 + 0.1))
      : row.step === 12
        ? 5 / 100.11
        : 0,
    1e-16,
  );
near(
  latchSource.wrongVariants[0].expected[0].coilCurrentAmps,
  5 / 100.02,
  1e-16,
);
for (const row of rcSource.expected) {
  const active = row.step < 50 ? row.step : row.step < 100 ? 49 : row.step - 50;
  near(
    row.capacitorVolts,
    10 * (1 - Math.exp((-active * 0.001) / 0.100001)),
    1e-13,
  );
}
const outputs = {
  "stage-8-circuits.json": { circuits },
  "stage-8-connectivity.json": { cases },
  "stage-8-manifest.json": manifest,
};
const args = process.argv.slice(2);
assert(
  args.length <= 1 && args.every((a) => ["--check", "--write"].includes(a)),
  "Use --check (default) or explicit --write",
);
for (const [name, data] of Object.entries(outputs)) {
  const serialized = JSON.stringify(data, null, 2) + "\n";
  const url = new URL(name, golden);
  if (args.includes("--write")) writeFileSync(url, serialized);
  else
    assert.equal(
      readFileSync(url, "utf8"),
      serialized,
      `${fileURLToPath(url)} differs: inspect before deliberate --write`,
    );
}
console.log(
  `${args.includes("--write") ? "Wrote" : "Verified"} ${circuits.length} circuit pairs and ${cases.length} topology cases; independent formulas passed.`,
);
