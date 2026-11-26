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
  new URL("../tests/golden/stage-14/", import.meta.url),
);
const version = "ngspice-47";
const options = "reltol=1e-9 abstol=1e-13 vntol=1e-10 temp=27 tnom=27";
const archiveSha256 =
  "59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a";
const fixtures = [];
const round = (value) => Number(value.toPrecision(12));
const json = (value) => JSON.stringify(value, null, 2) + "\n";

function add(id, device, body, results, state, step = null) {
  fixtures.push({ id, device, body, results, state, step });
}

// Each probe is a zero-volt source oriented into the first branch terminal.
function resistive(id, device, volts, ohms, state, extra = {}) {
  const current = ohms === null ? 0 : volts / ohms;
  add(
    id,
    device,
    [
      "Vin input 0 " + volts,
      "Vsense input p 0",
      ...(ohms === null ? [] : ["Rpart p 0 " + ohms]),
    ],
    {
      voltageVolts: ["v(p)", volts],
      currentAmps: ["i(vsense)", current],
      ...extra,
    },
    state,
  );
}

for (const [device, positions] of [
  [
    "limit_switch",
    [
      ["released", false],
      ["pressed", true],
    ],
  ],
  [
    "emergency_stop",
    [
      ["armed", true],
      ["latched", false],
    ],
  ],
  [
    "key_switch",
    [
      ["off", false],
      ["on", true],
    ],
  ],
]) {
  for (const [position, closed] of positions) {
    resistive(device + "-" + position, device, 24, closed ? 0.01 : null, {
      position,
    });
  }
}

for (const [index, position] of ["left", "center", "right"].entries()) {
  const body = ["Vin com 0 24"];
  const results = {};
  for (const branch of [1, 2]) {
    const closed = index === (branch - 1) * 2;
    const current = closed ? 24 / 120.01 : 0;
    body.push(
      "Vprobe" + branch + " com feed" + branch + " 0",
      "Rload" + branch + " out" + branch + " 0 120",
    );
    if (closed)
      body.push(
        "Rcontact" + branch + " feed" + branch + " out" + branch + " 0.01",
      );
    results["throw" + branch + "VoltageVolts"] = [
      "v(com,out" + branch + ")",
      24 - 120 * current,
    ];
    results["throw" + branch + "CurrentAmps"] = [
      "i(vprobe" + branch + ")",
      current,
    ];
  }
  add("selector-" + position, "selector_switch", body, results, { position });
}

// The contact decks check the electrical endpoints. Delay is an exact
// public-step oracle derived from the contract's edge/deadline rule.
for (const mode of ["on", "off"]) {
  for (const [id, edge, delayMs, elapsed, cancelled] of [
    ["zero", 0, 0, 0, false],
    ["pending", 2, 3, 2, false],
    ["deadline", 2, 3, 3, false],
    ["fractional-pending", 2, 3.5, 3, false],
    ["fractional-deadline", 2, 3.5, 4, false],
    ["cancel-before", 2, 3, 2, true],
    ["cancel-at-deadline", 2, 3, 3, true],
  ]) {
    const energized = (mode === "on") !== cancelled;
    const timing = !cancelled && elapsed < Math.ceil(delayMs);
    const outputOn = mode === "on" ? energized && !timing : energized || timing;
    const volts = energized ? 24 : 0;
    const body = [
      "Vcoil coil 0 " + volts,
      "Vcoilprobe coil cp 0",
      "Rcoil cp 0 240",
      "Vin supply 0 24",
    ];
    const results = {
      coilVoltageVolts: ["v(cp)", volts],
      coilCurrentAmps: ["i(vcoilprobe)", volts / 240],
    };
    for (const [branch, closed] of [
      ["no", outputOn],
      ["nc", !outputOn],
    ]) {
      const current = closed ? 24 / 120.01 : 0;
      body.push(
        "V" + branch + " supply " + branch + "p 0",
        "Rload" + branch + " " + branch + "n 0 120",
      );
      if (closed)
        body.push("R" + branch + " " + branch + "p " + branch + "n 0.01");
      results[branch + "VoltageVolts"] = [
        "v(" + branch + "p," + branch + "n)",
        24 - 120 * current,
      ];
      results[branch + "CurrentAmps"] = ["i(v" + branch + ")", current];
    }
    add(
      "relay-" + mode + "-" + id,
      "relay_" + mode + "_delay",
      body,
      results,
      {
        energized,
        outputOn,
        timing,
        timerElapsedSeconds: timing ? elapsed / 1000 : 0,
        edgeStep: edge,
        delayMs,
        oppositeEdgeAtSample: cancelled,
      },
      edge + elapsed,
    );
  }
}

for (const volts of [3, 6, 12, 24]) {
  resistive(
    "lamp-" + volts,
    "lamp",
    volts,
    120,
    { lit: volts ** 2 / 120 >= 0.25 },
    {
      powerWatts: ["v(p)*i(vsense)", volts ** 2 / 120],
    },
  );
  resistive("indicator-" + volts, "indicator", -volts, 2400, {
    lit: volts >= 12,
  });
  resistive("buzzer-" + volts, "buzzer", volts, 240, { active: volts >= 6 });
}

for (const device of ["solenoid", "valve"]) {
  for (const volts of [0, 4.8, 9.6, 24]) {
    const energized = volts >= 9.6;
    resistive(
      device + "-dc-" + volts.toString().replace(".", "_"),
      device,
      volts,
      24,
      device === "valve"
        ? { position: energized ? "open" : "closed" }
        : { energized },
    );
  }
  const times = [0.001, 0.002, 0.003, 0.005, 0.02];
  for (const time of times) {
    const current = 1 - Math.exp((-24 * time) / 0.05);
    const energized = current >= 0.4;
    add(
      device + "-rise-" + Math.round(time * 1000),
      device,
      [
        "Vin input 0 24",
        "Vsense input p 0",
        "Rcoil p mid 24",
        "Lcoil mid 0 0.05 ic=0",
      ],
      { voltageVolts: ["v(p)", 24], currentAmps: ["i(vsense)", current] },
      device === "valve"
        ? { position: energized ? "open" : "closed" }
        : { energized },
      Math.round(time * 1000),
    );
    fixtures.at(-1).time = time;
  }
}

const thermalVolts = 0.02586492578632875;
for (const [device, ideality, voltages] of [
  ["led", 2, [-2, 1.25, 1.5]],
  ["zener_diode", 1, [-6, -5.1, -4, 0.7]],
]) {
  for (const [index, volts] of voltages.entries()) {
    const breakdown = device === "zener_diode";
    const current =
      1e-14 * Math.expm1(volts / (ideality * thermalVolts)) +
      (breakdown ? Math.min(0, (volts + 5.1) / 5) : 0);
    add(
      device + "-" + index,
      device,
      [
        "Vin input 0 " + volts,
        "Vsense input p 0",
        "Djunction p 0 Dref",
        ".model Dref D(Is=1e-14 N=" + ideality + ")",
        ...(breakdown ? ["Bbreak p 0 I=min(0,(v(p)+5.1)/5)"] : []),
      ],
      { voltageVolts: ["v(p)", volts], currentAmps: ["i(vsense)", current] },
      breakdown ? { inBreakdown: volts < -5.1 } : { lit: current >= 0.001 },
    );
  }
}

for (const [id, gate, drain] of [
  ["off", 1, 2],
  ["linear", 5, 1],
  ["saturated", 5, 4],
  ["reverse", 4, -1],
]) {
  const overdrive = Math.max(0, gate - Math.min(drain, 0) - 2);
  const channel = Math.min(Math.abs(drain), overdrive);
  const current = Math.sign(drain) * 0.1 * channel * (overdrive - channel / 2);
  add(
    "mosfet-" + id,
    "mosfet_n",
    [
      "Vgate gate 0 " + gate,
      "Vdrain input 0 " + drain,
      "Vsense input d 0",
      ...(drain < 0
        ? ["Mref 0 gate d d Ref W=1u L=1u"]
        : ["Mref d gate 0 0 Ref W=1u L=1u"]),
      ".model Ref NMOS(Level=1 Vto=2 Kp=0.1 Is=1e-30)",
    ],
    {
      gateSourceVoltageVolts: ["v(gate)", gate],
      drainSourceVoltageVolts: ["v(d)", drain],
      drainCurrentAmps: ["i(vsense)", current],
    },
    { conducting: overdrive > 0 },
  );
}

for (const [id, input] of [
  ["follower", 3],
  ["low", -15],
  ["high", 15],
]) {
  const loadedGain = 1000 / 1.01;
  const linear = (input * loadedGain) / (1 + loadedGain);
  const volts = Math.max(-12 / 1.01, Math.min(linear, 12 / 1.01));
  add(
    "amplifier-" + id,
    "operational_amplifier",
    [
      "Vin plus 0 " + input,
      "Bdrive drive 0 V=max(-12,min(12,1000*v(plus,out)))",
      "Rout drive feed 1",
      "Vsense out feed 0",
      "Rload out 0 100",
    ],
    {
      differentialVoltageVolts: ["v(plus,out)", input - volts],
      outputVoltageVolts: ["v(out)", volts],
      outputCurrentAmps: ["i(vsense)", -volts / 100],
    },
    {
      state: { follower: "linear", low: "clippedLow", high: "clippedHigh" }[id],
    },
  );
}

for (const [input, time] of [
  [23, 0.001],
  [24, 0.001],
  [24, 0.005],
  [24, 0.01],
  [24, 0.02],
]) {
  const enabled = input >= 24;
  const body = ["Vin input 0 " + input, "Rin input 0 10000"];
  const results = {
    inputVoltageVolts: ["v(input)", input],
    inputCurrentAmps: ["-i(vin)", input / 10000],
  };
  for (const [index, pole] of ["u", "v", "w"].entries()) {
    const phase = [0, -120, 120][index];
    const amplitude = 24 * Math.SQRT2;
    const volts = enabled
      ? (amplitude * Math.sin(2 * Math.PI * (50 * time + phase / 360)) * 24) /
        25
      : 0;
    body.push(
      "V" +
        pole +
        " " +
        pole +
        "source 0 sin(0 " +
        amplitude +
        " 50 0 0 " +
        phase +
        ")",
      "Vprobe" + pole + " " + pole + "out " + pole + "feed 0",
      "Rload" + pole + " " + pole + "out 0 24",
    );
    if (enabled)
      body.push("Rseries" + pole + " " + pole + "source " + pole + "feed 1");
    results[pole + "VoltageVolts"] = ["v(" + pole + "out)", volts];
    results[pole + "CurrentAmps"] = ["i(vprobe" + pole + ")", -volts / 24];
  }
  add(
    "converter-" + input + "-" + Math.round(time * 1000),
    "frequency_converter",
    body,
    results,
    { enabled },
    Math.round(time * 1000),
  );
  fixtures.at(-1).time = time;
}

function deck(fixture) {
  return [
    "Open Circuit Stage 14: " + fixture.id,
    ".options " + options,
    ...fixture.body,
    fixture.time
      ? ".tran 1u " +
        fixture.time +
        " 0 1u" +
        (fixture.id.includes("-rise-") ? " uic" : "")
      : ".op",
    ".control",
    "set noaskquit",
    "set numdgt=16",
    "set wr_singlescale",
    "run",
    ...Object.values(fixture.results).map(
      ([expression], index) => "let sample" + index + " = " + expression,
    ),
    "wrdata values.out " +
      Object.keys(fixture.results)
        .map((_, index) => "sample" + index)
        .join(" "),
    "quit",
    ".endc",
    ".end",
    "",
  ].join("\n");
}

function spiceResults(executable, decks) {
  const banner = execFileSync(executable, ["--version"], { encoding: "utf8" });
  assert(banner.includes(version + " : Circuit"), "Expected " + version);
  const temporary = mkdtempSync(join(tmpdir(), "open-circuit-stage14-"));
  try {
    return Object.fromEntries(
      fixtures.map((fixture) => {
        writeFileSync(join(temporary, "case.cir"), decks[fixture.id]);
        execFileSync(executable, ["-n", "-b", "case.cir"], {
          cwd: temporary,
          timeout: 30_000,
        });
        const output = join(temporary, "values.out");
        const lines = readFileSync(output, "utf8").trim().split(/\r?\n/);
        rmSync(output);
        const [scale, ...values] = lines.at(-1).trim().split(/\s+/).map(Number);
        const keys = Object.keys(fixture.results);
        assert(
          values.length === keys.length && values.every(Number.isFinite),
          fixture.id,
        );
        if (fixture.time !== undefined)
          assert(Math.abs(scale - fixture.time) < 1e-12, fixture.id);
        return [
          fixture.id,
          Object.fromEntries(
            keys.map((key, index) => [key, round(values[index])]),
          ),
        ];
      }),
    );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

const [mode, flag, executable, ...extra] = process.argv.slice(2);
assert(
  extra.length === 0 &&
    ((mode === "--check" && flag === undefined) ||
      (["--write", "--check"].includes(mode) &&
        flag === "--ngspice" &&
        executable)),
  "Use --check or (--check|--write) --ngspice <ngspice-47 console executable>",
);
const decks = Object.fromEntries(
  fixtures.map((fixture) => [fixture.id, deck(fixture)]),
);
const saved =
  mode === "--check"
    ? JSON.parse(readFileSync(join(directory, "references.json"), "utf8"))
    : null;
const observed = executable ? spiceResults(executable, decks) : null;
const manifest = {
  ngspice: { version, archiveSha256, options },
  node: readFileSync(
    new URL("../.node-version", import.meta.url),
    "utf8",
  ).trim(),
  cases: Object.fromEntries(
    fixtures.map(({ id, device, results, state, step }) => {
      const analytical = Object.fromEntries(
        Object.entries(results).map(([key, [, value]]) => [key, round(value)]),
      );
      const ngspice = saved ? saved.cases[id].ngspice : observed[id];
      assert.deepEqual(Object.keys(ngspice), Object.keys(analytical), id);
      for (const [key, value] of Object.entries(analytical)) {
        assert(
          Number.isFinite(ngspice[key]) &&
            Math.abs(ngspice[key] - value) <= 1e-8 + 1e-3 * Math.abs(value),
          id +
            "/" +
            key +
            ": ngspice " +
            ngspice[key] +
            " vs analytical " +
            value,
        );
        if (saved && observed) {
          assert(
            Math.abs(observed[id][key] - ngspice[key]) <=
              1e-10 + 1e-6 * Math.abs(value),
            id + "/" + key + ": ngspice replay changed",
          );
        }
      }
      return [
        id,
        {
          device,
          step,
          state,
          analytical,
          ngspice,
          netlistSha256: createHash("sha256").update(decks[id]).digest("hex"),
        },
      ];
    }),
  ),
};
assert(
  process.versions.node === manifest.node,
  "Use the repository's pinned Node.js",
);
if (mode === "--write") mkdirSync(directory, { recursive: true });
for (const [file, contents] of [
  ["references.json", json(manifest)],
  ...Object.entries(decks).map(([id, contents]) => [id + ".cir", contents]),
]) {
  const path = join(directory, file);
  if (mode === "--write") writeFileSync(path, contents);
  else
    assert.equal(
      readFileSync(path, "utf8").replace(/\r\n/g, "\n"),
      contents,
      file,
    );
}
console.log(
  "Verified " +
    fixtures.length +
    " Stage 14 reference cases" +
    (observed ? " with ngspice replay." : "."),
);
