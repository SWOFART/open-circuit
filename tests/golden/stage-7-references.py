#!/usr/bin/env python3
"""Stage 7 reference data: netlists, ngspice results, and their offline checks.

Standard library only, and independent of the Rust Engine. It has four jobs:

  --write-netlists        write tests/golden/stage-7-ngspice/*.cir from the case table below
  --check-netlists        fail if the checked-in netlists differ from the case table
  --analytical [--write]  print (or write) tests/golden/stage-7-analytical.expected.json, the
                          answers of the model equations of
                          docs/contracts/stage-7-nonlinear-circuits.md section 3, computed by
                          tests/golden/stage-7-reference.py
  --ngspice EXE [--write] run every netlist with the ngspice at EXE, compare the numbers with
                          the checked-in tests/golden/stage-7-ngspice.expected.json, and with
                          --write replace that file
  --check                 offline: verify the checked-in ngspice results against the analytical
                          answers (no ngspice needed). This is what a normal test run relies on.

Regeneration is deliberate: nothing is ever overwritten without --write, and the expected data
is never replaced by an Engine result. See docs/solver/stage-7-references.md.
"""

import argparse
import importlib.util
import json
import math
import os
import re
import subprocess
import sys
import tempfile
from decimal import Decimal as D
from pathlib import Path

HERE = Path(__file__).resolve().parent
NETLISTS = HERE / "stage-7-ngspice"
ANALYTICAL = HERE / "stage-7-analytical.expected.json"
MANIFEST = HERE / "stage-7-manifest.json"
NGSPICE_EXPECTED = HERE / "stage-7-ngspice.expected.json"

# The simulator and options the checked-in results were produced with (docs/solver/
# stage-7-references.md explains every one).
NGSPICE_VERSION = "ngspice-47"
OPTIONS = "temp=27 tnom=27 gmin=0 reltol=1e-9 vntol=1e-12 abstol=1e-15 itl1=500 itl2=500"
TRAN_OPTIONS = "temp=27 tnom=27 gmin=0 method=trap reltol=1e-6 vntol=1e-9 abstol=1e-15 itl4=200"
JUNCTION_RESISTANCE = "1e12"  # 1 pS across every junction: the contract's g_min

spec = importlib.util.spec_from_file_location("stage7_reference", HERE / "stage-7-reference.py")
ref = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ref)


def number(text):
    return float(D(text))


# --------------------------------------------------------------------------- the case table

DIODE = dict(i_s="1e-14", n="1")
BJT = dict(i_s="1e-14", bf="100", br="1")

CASES = []


def diode_case(case_id, description, source, resistance, i_s="1e-14", n="1", **extra):
    CASES.append(
        dict(
            id=case_id,
            kind="diode_dc",
            description=description,
            source=source,
            resistance=resistance,
            i_s=i_s,
            n=n,
            **extra,
        )
    )


def bjt_case(case_id, description, polarity, vcc, rc, vbb, rb, swapped=False, **model):
    CASES.append(
        dict(
            id=case_id,
            kind="bjt_dc",
            description=description,
            polarity=polarity,
            vcc=vcc,
            rc=rc,
            vbb=vbb,
            rb=rb,
            swapped=swapped,
            **{**BJT, **model},
        )
    )


diode_case("diode-forward", "Forward, resistor-limited: 5 V, 1 kOhm.", "5", "1000")
diode_case("diode-reverse", "Reverse: -5 V, 1 kOhm. Leakage is I_S plus the junction conductance.", "-5", "1000")
diode_case("diode-low-current", "Supported but hard: forward at about 0.2 uA (0.6 V, 1 MOhm).", "0.6", "1000000")
diode_case("diode-high-current", "Supported but hard: forward at about 0.4 A (5 V, 10 Ohm).", "5", "10")
diode_case(
    "diode-emission",
    "Non-default model: I_S = 1e-12 A, n = 1.8 (5 V, 1 kOhm).",
    "5",
    "1000",
    i_s="1e-12",
    n="1.8",
)
CASES.append(
    dict(
        id="diode-sweep",
        kind="diode_sweep",
        description="Polarity change: the source is swept from -5 V to 5 V in 1 V steps through 1 kOhm.",
        sources=[str(v) for v in range(-5, 6)],
        resistance="1000",
        **DIODE,
    )
)

bjt_case("npn-cutoff", "NPN cutoff: base source 0 V, 430 kOhm.", 1, "10", "1000", "0", "430000")
bjt_case("npn-active", "NPN forward active: base source 5 V, 430 kOhm.", 1, "10", "1000", "5", "430000")
bjt_case("npn-saturation", "NPN saturation: base source 5 V, 4.3 kOhm.", 1, "10", "1000", "5", "4300")
bjt_case(
    "npn-reverse-active",
    "Wrong wiring: the collector and emitter exchanged, so the device runs reverse active.",
    1,
    "10",
    "1000",
    "5",
    "430000",
    swapped=True,
)
bjt_case("pnp-cutoff", "PNP cutoff (the NPN case with every voltage negated).", -1, "-10", "1000", "0", "430000")
bjt_case("pnp-active", "PNP forward active (negated).", -1, "-10", "1000", "-5", "430000")
bjt_case("pnp-saturation", "PNP saturation (negated).", -1, "-10", "1000", "-5", "4300")
bjt_case(
    "npn-saturation-edge",
    "Supported but hard: NPN at the edge of saturation (base source 5 V, 43 kOhm; V_CE about 0.2 V).",
    1,
    "10",
    "1000",
    "5",
    "43000",
)
bjt_case(
    "npn-saturation-high-current",
    "Supported but hard: NPN saturated at about 100 mA (R_C = 100 Ohm, base source 5 V, 2.2 kOhm).",
    1,
    "10",
    "100",
    "5",
    "2200",
)
bjt_case(
    "npn-beta-variation",
    "Non-default model: I_S = 1e-15 A, beta_F = 30, beta_R = 5 (forward active, 430 kOhm).",
    1,
    "10",
    "1000",
    "5",
    "430000",
    i_s="1e-15",
    bf="30",
    br="5",
)

R_RC = "10000"
C_RC = "1e-6"
SWITCH_R = "0.01"
TRAN_TIMES = [0.054, 0.104, 0.204]
for case_id, reverse in (("rc-diode-charging", False), ("rc-diode-reversed", True)):
    CASES.append(
        dict(
            id=case_id,
            kind="rc_transient",
            description=(
                "Switched nonlinear RC with the diode reversed (anode on the capacitor side)."
                if reverse
                else "Switched nonlinear RC: 5 V, a switch (10 mOhm) closing at 4 ms, 10 kOhm, the diode, 1 uF from 0 V."
            ),
            source="5",
            resistance=R_RC,
            switch_resistance=SWITCH_R,
            capacitance=C_RC,
            close_time="0.004",
            times=[str(t) for t in TRAN_TIMES],
            reverse=reverse,
            **DIODE,
        )
    )


# --------------------------------------------------------------------------- netlists

def header(case):
    return f"* Stage 7 reference circuit: {case['id']}\n* {case['description']}\n"


def junction(name, a, b):
    return f"R{name} {a} {b} {JUNCTION_RESISTANCE}\n"


def netlist(case):
    kind = case["kind"]
    out = header(case)
    if kind in ("diode_dc", "diode_sweep"):
        model = f".model DM D(IS={case['i_s']} N={case['n']})\n"
        out += f".options {OPTIONS}\n"
        source = case["source"] if kind == "diode_dc" else case["sources"][0]
        out += f"VSRC s 0 {source}\nRS s a {case['resistance']}\nD1 a 0 DM\n"
        out += junction("GD", "a", "0") + model
        out += ".control\n"
        if kind == "diode_dc":
            out += "op\nwrdata " + case["id"] + ".out v(a) i(vsrc)\n"
        else:
            out += "dc VSRC -5 5 1\nwrdata " + case["id"] + ".out v(a) i(vsrc)\n"
        out += ".endc\n.end\n"
    elif kind == "bjt_dc":
        card = "NPN" if case["polarity"] > 0 else "PNP"
        out += f".options {OPTIONS}\n"
        out += f"VCC vcc 0 {case['vcc']}\nVBB vbb 0 {case['vbb']}\n"
        load = "e" if case["swapped"] else "c"
        # Zero-volt sources measure the current entering each terminal of the device.
        collector_node = "0" if case["swapped"] else "c"
        emitter_node = "e" if case["swapped"] else "0"
        out += f"RC vcc {load} {case['rc']}\nRB vbb b {case['rb']}\n"
        out += f"VSC {collector_node} tc 0\nVSB b tb 0\nVSE {emitter_node} te 0\n"
        out += f"Q1 tc tb te QM\n"
        out += junction("GBE", "tb", "te") + junction("GBC", "tb", "tc")
        out += f".model QM {card}(IS={case['i_s']} BF={case['bf']} BR={case['br']})\n"
        out += ".control\nop\n"
        out += f"wrdata {case['id']}.out v(b) v({load}) i(vsc) i(vsb) i(vse)\n"
        out += ".endc\n.end\n"
    elif kind == "rc_transient":
        out += f".options {TRAN_OPTIONS}\n"
        out += f".model SWM SW(VT=0.5 VH=0 RON={case['switch_resistance']} ROFF=1e15)\n"
        out += f".model DM D(IS={case['i_s']} N={case['n']})\n"
        out += "VSRC s 0 " + case["source"] + "\n"
        out += "VCTL ctl 0 PWL(0 0 " + case["close_time"] + " 0 0.004000001 1)\n"
        out += "S1 s m ctl 0 SWM\n"
        out += f"RS m d {case['resistance']}\n"
        if case["reverse"]:
            out += "D1 cap d DM\n" + junction("GD", "cap", "d")
        else:
            out += "D1 d cap DM\n" + junction("GD", "d", "cap")
        out += f"CC cap 0 {case['capacitance']} IC=0\n"
        out += ".control\ntran 1m 0.21 0 1u uic\nlinearize v(cap)\n"
        out += "wrdata " + case["id"] + ".out v(cap)\n.endc\n.end\n"
    else:
        raise ValueError(kind)
    return out


def write_netlists():
    NETLISTS.mkdir(exist_ok=True)
    for case in CASES:
        (NETLISTS / f"{case['id']}.cir").write_text(netlist(case), encoding="utf8", newline="\n")


def check_netlists():
    bad = []
    for case in CASES:
        path = NETLISTS / f"{case['id']}.cir"
        if not path.exists() or path.read_text(encoding="utf8") != netlist(case):
            bad.append(case["id"])
    extra = sorted(p.stem for p in NETLISTS.glob("*.cir") if p.stem not in {c["id"] for c in CASES})
    if bad or extra:
        sys.exit(f"netlists out of date: {bad}; unexpected files: {extra}")
    print(f"{len(CASES)} netlists match the case table")


# --------------------------------------------------------------------------- analytical answers

def fmt(x, digits=17):
    return float(f"{float(x):.{digits}g}")


def diode_point(source, resistance, i_s, n):
    v, i = ref.solve_diode_series(D(source), D(resistance), D(i_s), D(n))
    return dict(diodeVoltageVolts=fmt(v), diodeCurrentAmps=fmt(i))


def bjt_point(case):
    r = ref.solve_common_emitter(
        D(case["vcc"]),
        D(case["rc"]),
        D(case["vbb"]),
        D(case["rb"]),
        case["polarity"],
        case["swapped"],
        D(case["i_s"]),
        D(case["bf"]),
        D(case["br"]),
    )
    return dict(
        baseVoltageVolts=fmt(r["vb"]),
        loadNodeVoltageVolts=fmt(r["vc"]),
        collectorCurrentAmps=fmt(r["ic"]),
        baseCurrentAmps=fmt(r["ib"]),
        emitterCurrentAmps=fmt(r["ie"]),
    )


def rc_points(case):
    r = float(D(case["resistance"]) + D(case["switch_resistance"]))
    c = float(D(case["capacitance"]))
    current = ref.reversed_charging_current if case["reverse"] else ref.charging_current
    close = float(D(case["close_time"]))
    values = {}
    for t in case["times"]:
        elapsed = float(D(t)) - close
        coarse = ref.rk4(float(D(case["source"])), r, c, elapsed, 2e-5, current)
        fine = ref.rk4(float(D(case["source"])), r, c, elapsed, 1e-5, current)
        assert abs(coarse - fine) < 1e-9, (case["id"], t, coarse, fine)
        values[t] = fmt(fine, 15)
    return values


def case_quantities(case):
    kind = case["kind"]
    if kind == "diode_dc":
        return diode_point(case["source"], case["resistance"], case["i_s"], case["n"])
    if kind == "diode_sweep":
        return dict(
            points=[
                dict(sourceVolts=float(s), **diode_point(s, case["resistance"], case["i_s"], case["n"]))
                for s in case["sources"]
            ]
        )
    if kind == "bjt_dc":
        return bjt_point(case)
    if kind == "rc_transient":
        return dict(capacitorVoltsAtSeconds=rc_points(case))
    raise ValueError(kind)


def public(case):
    """The case as the manifest and the expected files record it (decimal strings as numbers)."""
    skip = {"id", "kind", "description"}
    return {k: v for k, v in case.items() if k not in skip}


def analytical():
    ref.use_constants(ref.BOLTZMANN, ref.ELEMENTARY_CHARGE)
    contract = {c["id"]: case_quantities(c) for c in CASES}
    ref.use_constants(ref.NGSPICE_BOLTZMANN, ref.NGSPICE_ELEMENTARY_CHARGE)
    aligned = {c["id"]: case_quantities(c) for c in CASES}
    ref.use_constants(ref.BOLTZMANN, ref.ELEMENTARY_CHARGE)
    return {
        "description": (
            "Answers of the model equations of docs/contracts/stage-7-nonlinear-circuits.md section 3, "
            "computed by tests/golden/stage-7-reference.py (60-digit Newton for direct current, "
            "Runge-Kutta checked at half the step for the transient). They are not Engine output "
            "and not ngspice output. `contract` uses the exact SI 2019 constants of section 3.1. "
            "`ngspiceConstants` runs the same equations with the CODATA 2014 values of k and q "
            "that ngspice 47 uses, to compare with ngspice almost to rounding."
        ),
        "cases": [
            {
                "id": c["id"],
                "kind": c["kind"],
                "description": c["description"],
                "circuit": public(c),
                "contract": contract[c["id"]],
                "ngspiceConstants": aligned[c["id"]],
            }
            for c in CASES
        ],
    }


# --------------------------------------------------------------------------- ngspice

def parse_columns(path, count):
    rows = []
    for line in path.read_text(encoding="utf8").splitlines():
        if not line.strip():
            continue
        values = [float(x) for x in line.split()]
        if len(values) != 2 * count:
            raise ValueError(f"{path.name}: expected {2 * count} columns, got {len(values)}")
        rows.append(values)
    return rows


def ngspice_quantities(case, directory):
    kind = case["kind"]
    out = directory / f"{case['id']}.out"
    if kind == "diode_dc":
        (row,) = parse_columns(out, 2)
        return dict(diodeVoltageVolts=row[1], diodeCurrentAmps=-row[3])
    if kind == "diode_sweep":
        rows = parse_columns(out, 2)
        return dict(
            points=[
                dict(sourceVolts=round(r[0], 9), diodeVoltageVolts=r[1], diodeCurrentAmps=-r[3])
                for r in rows
            ]
        )
    if kind == "bjt_dc":
        (row,) = parse_columns(out, 5)
        # Columns: v(b), v(load), i(vsc), i(vsb), i(vse); the zero-volt sources are wired so that
        # every current is positive entering the device terminal.
        return dict(
            baseVoltageVolts=row[1],
            loadNodeVoltageVolts=row[3],
            collectorCurrentAmps=row[5],
            baseCurrentAmps=row[7],
            emitterCurrentAmps=row[9],
        )
    if kind == "rc_transient":
        rows = parse_columns(out, 1)
        values = {}
        for t in case["times"]:
            target = float(D(t))
            row = min(rows, key=lambda r: abs(r[0] - target))
            assert abs(row[0] - target) < 1e-9, (case["id"], t, row[0])
            values[t] = row[1]
        return dict(capacitorVoltsAtSeconds=values)
    raise ValueError(kind)


def run_ngspice(exe):
    exe = str(Path(exe).resolve())
    version = subprocess.run([exe, "--version"], capture_output=True, text=True).stdout
    match = re.search(r"ngspice-(\d+(?:\.\d+)*)", version)
    if not match or f"ngspice-{match.group(1)}" != NGSPICE_VERSION:
        sys.exit(f"expected {NGSPICE_VERSION}, found: {version.strip().splitlines()[:3]}")
    results = {}
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        for case in CASES:
            source = NETLISTS / f"{case['id']}.cir"
            work = directory / source.name
            work.write_text(source.read_text(encoding="utf8"), encoding="utf8", newline="\n")
            done = subprocess.run(
                [exe, "-b", "-o", str(directory / (case["id"] + ".log")), str(work)],
                cwd=directory,
                capture_output=True,
                text=True,
                timeout=600,
            )
            if done.returncode != 0 or not (directory / f"{case['id']}.out").exists():
                sys.exit(f"ngspice failed on {case['id']}:\n{done.stdout}\n{done.stderr}")
            results[case["id"]] = ngspice_quantities(case, directory)
    return {
        "simulator": NGSPICE_VERSION,
        "options": OPTIONS,
        "transientOptions": TRAN_OPTIONS,
        "junctionConductance": f"an explicit {JUNCTION_RESISTANCE} Ohm across every junction; ngspice's own gmin is 0",
        "description": (
            "Numbers exactly as ngspice wrote them (wrdata, double precision), with the sign "
            "convention of the Open Circuit results: diode current entering the anode, transistor "
            "currents entering the terminals. Regenerate with tests/golden/stage-7-references.py "
            "--ngspice EXE --write."
        ),
        "cases": [{"id": c["id"], **results[c["id"]]} for c in CASES],
    }


# --------------------------------------------------------------------------- comparison

def flatten(quantities, prefix=""):
    flat = {}
    for key, value in quantities.items():
        if isinstance(value, dict):
            flat.update(flatten(value, prefix + key + "."))
        elif isinstance(value, list):
            for index, item in enumerate(value):
                flat.update(flatten(item, f"{prefix}{key}[{index}]."))
        else:
            flat[prefix + key] = value
    return flat


def differences(left, right):
    a, b = flatten(left), flatten(right)
    assert a.keys() == b.keys(), (sorted(a), sorted(b))
    return {k: (a[k], b[k]) for k in a if not k.endswith("sourceVolts")}


# Reference error: how far ngspice may be from the equations when both use the same constants and
# the same model. It is separate from the tolerance an Engine result is held to (the contract's
# fixture tolerances, an order of magnitude looser).
ALIGNED_VOLTS_ABS = 3e-8
ALIGNED_AMPS_REL = 1e-7
ALIGNED_AMPS_ABS = 1e-15
TRANSIENT_VOLTS_ABS = 1e-7
# ngspice uses CODATA 2014 constants, the contract the exact 2019 values: the largest effect on a
# result is a few times 1e-7 V, so ngspice and the contract's answers must agree within this.
CONTRACT_VOLTS_ABS = 2e-6
CONTRACT_AMPS_REL = 1e-5


def within(key, got, want, volts_abs, amps_rel, amps_abs):
    if "AtSeconds" in key:
        # The transient is compared in volts; with aligned constants it has its own bound.
        volts_abs = TRANSIENT_VOLTS_ABS if volts_abs == ALIGNED_VOLTS_ABS else volts_abs
        return abs(got - want) <= volts_abs
    if "Volts" in key:
        return abs(got - want) <= volts_abs
    return abs(got - want) <= amps_rel * max(abs(got), abs(want)) + amps_abs


def compare_checked_in(verbose=False):
    analytic = json.loads(ANALYTICAL.read_text(encoding="utf8"))
    ngspice = json.loads(NGSPICE_EXPECTED.read_text(encoding="utf8"))
    ids = [c["id"] for c in CASES]
    assert [c["id"] for c in analytic["cases"]] == ids
    assert [c["id"] for c in ngspice["cases"]] == ids
    worst = {"aligned": 0.0, "contract": 0.0}
    for a, n in zip(analytic["cases"], ngspice["cases"]):
        n = {k: v for k, v in n.items() if k != "id"}
        for label, key, volts, rel, absolute in (
            ("aligned", "ngspiceConstants", ALIGNED_VOLTS_ABS, ALIGNED_AMPS_REL, ALIGNED_AMPS_ABS),
            ("contract", "contract", CONTRACT_VOLTS_ABS, CONTRACT_AMPS_REL, ALIGNED_AMPS_ABS),
        ):
            for path, (got, want) in differences(n, a[key]).items():
                ok = within(path, got, want, volts, rel, absolute)
                if not ok:
                    sys.exit(f"{a['id']} {path}: ngspice {got!r} vs {label} {want!r}")
                worst[label] = max(worst[label], abs(got - want))
    print("ngspice agrees with the equations: aligned constants within 30 nV / 1e-7 relative,")
    print("contract constants within 2 uV / 1e-5 relative; largest absolute differences", worst)



# --------------------------------------------------------------------------- manifest

# What an Engine result is held to, per case. These are the fixture tolerances of
# docs/contracts/stage-7-nonlinear-circuits.md section 9 (stage-7-*.expected.json), extended to
# the added cases. They are solver accuracy: how far the Engine may be from the equations'
# answer. They are separate from the reference error (ngspice against the equations), which is
# an order of magnitude smaller and is documented under `referenceError`.
ENGINE_TOLERANCE = {
    "dc": {"voltsAbsolute": 1e-6, "ampsRelative": 1e-6, "ampsAbsoluteFloor": 1e-12},
    "leakage": {"voltsAbsolute": 1e-6, "ampsAbsolute": 1e-14},
    "cutoff": {"voltsAbsolute": 1e-6, "ampsAbsolute": 1e-13},
    "sweep": {"voltsAbsolute": 1e-6, "ampsRelative": 1e-6, "ampsAbsoluteFloor": 1e-14},
    "transient": {"voltsAbsolute": 0.005},
    "transientReversed": {"voltsAbsolute": 1e-5},
}
TOLERANCE_OF = {
    "diode-reverse": "leakage",
    "npn-cutoff": "cutoff",
    "pnp-cutoff": "cutoff",
    "diode-sweep": "sweep",
    "rc-diode-charging": "transient",
    "rc-diode-reversed": "transientReversed",
}
# The contract fixture file in which a case's answer is also recorded (the same numbers).
CONTRACT_FILE_OF = {
    "diode-forward": "stage-7-diode-dc.expected.json",
    "diode-reverse": "stage-7-diode-dc.expected.json",
    "npn-cutoff": "stage-7-bjt-dc.expected.json",
    "npn-active": "stage-7-bjt-dc.expected.json",
    "npn-saturation": "stage-7-bjt-dc.expected.json",
    "npn-reverse-active": "stage-7-bjt-dc.expected.json",
    "pnp-cutoff": "stage-7-bjt-dc.expected.json",
    "pnp-active": "stage-7-bjt-dc.expected.json",
    "pnp-saturation": "stage-7-bjt-dc.expected.json",
    "rc-diode-charging": "stage-7-diode-rc-transient.expected.json",
    "rc-diode-reversed": "stage-7-diode-rc-transient.expected.json",
}
ROLE_OF = {
    "npn-reverse-active": "wrongVariant",
    "rc-diode-reversed": "wrongVariant",
    "diode-low-current": "difficult",
    "diode-high-current": "difficult",
    "diode-emission": "difficult",
    "npn-saturation-edge": "difficult",
    "npn-saturation-high-current": "difficult",
    "npn-beta-variation": "difficult",
}


def manifest():
    fixtures = []
    for case in CASES:
        entry = {
            "id": case["id"],
            "kind": case["kind"],
            "role": ROLE_OF.get(case["id"], "canonical"),
            "description": case["description"],
            "netlist": f"stage-7-ngspice/{case['id']}.cir",
            "circuit": public(case),
            "engineTolerance": ENGINE_TOLERANCE[TOLERANCE_OF.get(case["id"], "dc")],
        }
        if case["id"] in CONTRACT_FILE_OF:
            entry["contractFixture"] = CONTRACT_FILE_OF[case["id"]]
        fixtures.append(entry)
    return {
        "manifestVersion": 1,
        "issue": "https://github.com/SWOFART/open-circuit/issues/109",
        "contract": "docs/contracts/stage-7-nonlinear-circuits.md",
        "documentation": "docs/solver/stage-7-references.md",
        "sources": {
            "analytical": {
                "tool": "tests/golden/stage-7-reference.py",
                "method": "60-digit decimal Newton on the model equations of the contract's section 3 (direct current); fourth-order Runge-Kutta at step h and h/2, required to agree to 1e-9 V (transient)",
                "uses": "the Python standard library only; no Engine, no simulator",
                "file": "stage-7-analytical.expected.json",
            },
            "ngspice": {
                "simulator": NGSPICE_VERSION,
                "release": "https://sourceforge.net/projects/ngspice/files/ng-spice-rework/47/ngspice-47_64.7z",
                "archiveSha256": "59225971bd68cdd1199443649aa4615a9e6d684933f205ab49006a3942518f5a",
                "license": "Modified BSD (ngspice COPYING); the simulator is not part of this repository and is not a dependency of any build or normal test",
                "manual": "ngspice-47-manual.pdf, distributed with the release",
                "file": "stage-7-ngspice.expected.json",
                "options": OPTIONS,
                "transientOptions": TRAN_OPTIONS,
            },
        },
        "alignment": {
            "temperature": "27 degrees Celsius (300.15 K) for the circuit and the model parameters, as in the contract's section 3.1",
            "modelLevel": "ngspice level-1 junction diode (D) and Gummel-Poon bipolar (NPN, PNP) with every parameter that the contract defers left at its default of zero or infinity (RS, CJO, TT, BV, VAF, IKF, ISE, ISC, RB, RC, RE, CJE, CJC, TF, TR): the diode law and the transport form of Ebers-Moll of the contract's sections 3.2 to 3.4",
            "junctionConductance": "The contract's g_min is a conductance directly across each junction. ngspice's own gmin is set to 0 and an explicit 1 pS resistor is placed across each junction in the netlist, because ngspice adds its gmin to the transport currents before they are divided by the betas, which is a different model (it gave a cutoff collector current of 2.0e-11 A where the contract's model gives 1.0e-11 A)",
            "thermalVoltage": "ngspice 47 derives V_T from the CODATA 2014 values k = 1.38064852e-23 J/K and q = 1.6021766208e-19 C; the contract uses the exact SI 2019 values. The analytical file therefore holds two answers per case: `contract` (what the Engine is held to) and `ngspiceConstants` (the same equations with ngspice's constants, to compare with ngspice almost to rounding). The constants change a result by at most 3.4e-7 V",
            "switch": "ngspice voltage-controlled switch with RON = 0.01 Ohm, ROFF = 1e15 Ohm and a 1 ns control ramp at 4 ms, for the Stage 6 switch of the contract's transient fixture (open: no conduction)",
            "initialConditions": "direct current: the operating point of each netlist (`op`, `dc`). Transient: the capacitor starts at 0 V (`IC=0` with `uic`), the switch open",
        },
        "conventions": {
            "units": "SI: volts, amperes, ohms, farads, seconds",
            "diode": "voltage = V(anode) - V(cathode); current positive entering the anode",
            "transistor": "terminal currents positive entering the collector, base, and emitter; `loadNodeVoltageVolts` is the voltage of the node the 1 kOhm collector resistor feeds (the collector, or the emitter when the pins are exchanged)",
            "ngspiceSigns": "wrdata columns are (scale, value) pairs; the source current of ngspice is negative when the source delivers, so the diode current is -i(vsrc); the zero-volt sources VSC, VSB, VSE are wired so that every current they report enters the device terminal",
            "timeAlignment": "the transient is sampled at 54, 104, and 204 ms, that is 50, 100, and 200 ms after the switch conducts (it is closed at 4 ms, the instant the Engine's step 4 ends); ngspice output is linearized to a 1 ms grid so the times coincide with the Engine's public steps",
        },
        "referenceError": {
            "meaning": "How far ngspice may be from the equations' answer. It certifies that the equations and the netlists describe the same model. It is not the Engine's tolerance.",
            "aligned": {"voltsAbsolute": ALIGNED_VOLTS_ABS, "transientVoltsAbsolute": TRANSIENT_VOLTS_ABS, "ampsRelative": ALIGNED_AMPS_REL, "ampsAbsolute": ALIGNED_AMPS_ABS, "basis": "ngspice uses reltol = 1e-9 on voltages up to 10 V, which allows 1e-8 V; the transient runs at reltol = 1e-6 and agrees much better"},
            "contract": {"voltsAbsolute": CONTRACT_VOLTS_ABS, "ampsRelative": CONTRACT_AMPS_REL, "ampsAbsolute": ALIGNED_AMPS_ABS, "basis": "the difference between ngspice's constants and the contract's"},
            "currentsNearZero": "a current is compared with a relative tolerance plus an absolute floor, so that a current that should be zero (the diode at 0 V) is not judged by a ratio",
        },
        "solverToleranceDiffersFromReferenceError": "An Engine result is compared with the `contract` answers of the analytical file using `engineTolerance`. ngspice is never the oracle for the Engine: it certifies the equations. Every `engineTolerance` is at least ten times the `referenceError.aligned` bound; the `contract` difference is the known effect of ngspice's constants and is not a tolerance.",
        "failureFixtures": [
            {
                "id": "diode-ideal-source",
                "file": "stage-7-diode-dc.expected.json",
                "expected": "ModelRangeExceeded, referencing the diode",
                "note": "an ideal 5 V source directly across a forward diode needs more than 80 thermal voltages; no ngspice comparison (ngspice has no such range rule)",
            },
            {
                "id": "bjt-ideal-source",
                "file": "stage-7-bjt-dc.expected.json",
                "expected": "ModelRangeExceeded, referencing the transistor",
                "note": "the common-emitter stage with the base resistor replaced by a wire",
            },
        ],
        "regeneration": {
            "analytical": "python tests/golden/stage-7-references.py --analytical --write",
            "netlists": "python tests/golden/stage-7-references.py --write-netlists",
            "ngspice": "python tests/golden/stage-7-references.py --ngspice <path to ngspice_con.exe or ngspice> --write",
            "offline": "python tests/golden/stage-7-references.py --check",
            "rule": "nothing is overwritten without --write, and expected data is never replaced by an Engine result",
        },
        "fixtures": fixtures,
    }


# --------------------------------------------------------------------------- main

def dump(path, data):
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf8", newline="\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--write-netlists", action="store_true")
    parser.add_argument("--check-netlists", action="store_true")
    parser.add_argument("--analytical", action="store_true")
    parser.add_argument("--manifest", action="store_true")
    parser.add_argument("--ngspice", metavar="EXE")
    parser.add_argument("--write", action="store_true", help="replace the checked-in expected file")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    if args.write_netlists:
        write_netlists()
    if args.check_netlists:
        check_netlists()
    if args.analytical:
        data = analytical()
        if args.write:
            dump(ANALYTICAL, data)
        else:
            current = json.loads(ANALYTICAL.read_text(encoding="utf8")) if ANALYTICAL.exists() else None
            print("unchanged" if current == data else "DIFFERS from the checked-in file (use --write)")
    if args.manifest:
        data = manifest()
        if args.write:
            dump(MANIFEST, data)
        else:
            current = json.loads(MANIFEST.read_text(encoding="utf8")) if MANIFEST.exists() else None
            print("unchanged" if current == data else "DIFFERS from the checked-in file (use --write)")
    if args.ngspice:
        data = run_ngspice(args.ngspice)
        current = json.loads(NGSPICE_EXPECTED.read_text(encoding="utf8")) if NGSPICE_EXPECTED.exists() else None
        if args.write:
            dump(NGSPICE_EXPECTED, data)
            print("wrote", NGSPICE_EXPECTED.name)
        elif current == data:
            print("ngspice results identical to the checked-in file")
        else:
            print("ngspice results DIFFER from the checked-in file (use --write to replace it)")
            sys.exit(1)
    if args.check:
        check_netlists()
        compare_checked_in()
        if json.loads(MANIFEST.read_text(encoding="utf8")) != manifest():
            sys.exit("stage-7-manifest.json differs from the case table (use --manifest --write)")
        if json.loads(ANALYTICAL.read_text(encoding="utf8")) != analytical():
            sys.exit("stage-7-analytical.expected.json differs from the equations (use --analytical --write)")
        print("manifest and analytical answers match the case table and the equations")


if __name__ == "__main__":
    main()
