#!/usr/bin/env python3
"""Independent reference answers for the Stage 7 fixtures.

Standard library only. It does not use the Engine, a circuit simulator, or any numerical
package. Direct-current answers are root finds on the model equations of
docs/contracts/stage-7-nonlinear-circuits.md section 3, done by Newton's method in 60-digit
decimal arithmetic and checked by their residual in the same arithmetic. The transient answer
integrates the circuit's differential equation with a classical fourth-order Runge-Kutta rule
and is checked against the same rule at half the step.

Run:  python tests/golden/stage-7-reference.py
It prints the numbers that tests/golden/stage-7-*.expected.json record. Regeneration is
deliberate: change the circuits here and in the contract together, run this, and copy the
numbers into the expected files in the same change.
"""

from decimal import Decimal as D, getcontext
import json
import math

getcontext().prec = 60

# Section 3.1: SI 2019 exact constants and the fixed temperature of 27 degrees Celsius.
BOLTZMANN = D("1.380649e-23")
ELEMENTARY_CHARGE = D("1.602176634e-19")
TEMPERATURE = D("300.15")
VT = BOLTZMANN * TEMPERATURE / ELEMENTARY_CHARGE
GMIN = D("1e-12")

DEFAULT_IS = D("1e-14")
DEFAULT_N = D(1)
DEFAULT_BF = D(100)
DEFAULT_BR = D(1)


def diode(v, i_s=DEFAULT_IS, n=DEFAULT_N):
    """Current entering the anode and its derivative (section 3.2)."""
    x = v / (n * VT)
    e = x.exp()
    return i_s * (e - 1) + GMIN * v, i_s * e / (n * VT) + GMIN


def npn(vbe, vbc, i_s=DEFAULT_IS, bf=DEFAULT_BF, br=DEFAULT_BR):
    """Terminal currents entering collector, base, emitter, and the 2x2 Jacobian of
    (i_c, i_b) with respect to (vbe, vbc) (section 3.3)."""
    ef = (vbe / VT).exp()
    er = (vbc / VT).exp()
    i_f = i_s * (ef - 1)
    i_r = i_s * (er - 1)
    gf = i_s * ef / VT
    gr = i_s * er / VT
    i_c = i_f - i_r * (1 + 1 / br) - GMIN * vbc
    i_b = i_f / bf + i_r / br + GMIN * (vbe + vbc)
    jac = (
        (gf, -(1 + 1 / br) * gr - GMIN),
        (gf / bf + GMIN, gr / br + GMIN),
    )
    return i_c, i_b, -(i_c + i_b), jac


def solve_diode_series(v_source, resistance):
    """V - R i = v_D with i the diode current (series circuit)."""
    v = D("0.6") if v_source > 0 else v_source
    for _ in range(200):
        i, g = diode(v)
        f = (v_source - v) / resistance - i
        step = f / (g + 1 / resistance)
        v += step
        if abs(step) < D("1e-50"):
            break
    i, _ = diode(v)
    residual = abs((v_source - v) / resistance - i)
    assert residual < D("1e-40"), residual
    return v, i


def device_currents(vb, vc, polarity, swapped):
    """Terminal currents entering (collector, base, emitter) of an NPN (polarity +1) or PNP
    (polarity -1) whose base is at `vb`. In the normal wiring the collector is at `vc` and
    the emitter at ground; swapped, the emitter is at `vc` and the collector at ground."""
    v_c, v_e = (D(0), vc) if swapped else (vc, D(0))
    # The PNP is the NPN with every voltage and current negated (section 3.4).
    i_c, i_b, i_e, _ = npn(polarity * (vb - v_e), polarity * (vb - v_c))
    return polarity * i_c, polarity * i_b, polarity * i_e


def solve_common_emitter(v_cc, r_c, v_bb, r_b, polarity, swapped=False):
    """Common-emitter stage: R_C from the supply to the load node, R_B from the base source
    to the base. The unknowns are the base and load-node voltages; the equations are the
    current balance at each, and Newton's method uses a central-difference Jacobian in
    60-digit arithmetic (only the final residual matters, and it is checked)."""

    def residual(vb, vc):
        i_c, i_b, i_e = device_currents(vb, vc, polarity, swapped)
        i_load = i_e if swapped else i_c
        return (v_bb - vb) / r_b - i_b, (v_cc - vc) / r_c - i_load

    vb, vc = D(0), D(0)
    h = D("1e-30")
    for _ in range(1000):
        f_b, f_c = residual(vb, vc)
        # Jacobian columns by central differences.
        fb1, fc1 = residual(vb + h, vc)
        fb0, fc0 = residual(vb - h, vc)
        fb3, fc3 = residual(vb, vc + h)
        fb2, fc2 = residual(vb, vc - h)
        a, c_ = (fb1 - fb0) / (2 * h), (fc1 - fc0) / (2 * h)
        b, d = (fb3 - fb2) / (2 * h), (fc3 - fc2) / (2 * h)
        det = a * d - b * c_
        dvb = (-f_b * d + f_c * b) / det
        dvc = (-f_c * a + f_b * c_) / det
        # A damped step keeps the exponentials in range from the first guess.
        scale = min(D(1), D("0.2") / max(abs(dvb), abs(dvc), D("1e-60")))
        vb, vc = vb + scale * dvb, vc + scale * dvc
        if scale == 1 and max(abs(dvb), abs(dvc)) < D("1e-45"):
            break
    f_b, f_c = residual(vb, vc)
    assert abs(f_b) < D("1e-35") and abs(f_c) < D("1e-35"), (f_b, f_c)
    i_c, i_b, i_e = device_currents(vb, vc, polarity, swapped)
    return {"vb": vb, "vc": vc, "ic": i_c, "ib": i_b, "ie": i_e}


def fmt(x, digits=15):
    return float(f"{float(x):.{digits}g}")


# ---------------------------------------------------------------- transient (float64)

VT_F = float(VT)
GMIN_F = float(GMIN)
IS_F = float(DEFAULT_IS)


def diode_f(v):
    x = v / VT_F
    e = math.exp(x)
    return IS_F * (e - 1.0) + GMIN_F * v, IS_F * e / VT_F + GMIN_F


def charging_current(v_source, v_cap, r):
    """Current in a series R and diode from the source into the capacitor."""
    lo = -v_source - 1.0 if v_source < 0 else -1.0
    # Solve (V - v_cap - v_d) / R = i_d(v_d) for v_d with Newton, damped from a safe start.
    v_d = 0.5 if v_source - v_cap > 0.5 else (v_source - v_cap) / 2.0
    for _ in range(100):
        i, g = diode_f(v_d)
        f = (v_source - v_cap - v_d) / r - i
        step = f / (g + 1.0 / r)
        v_d += step
        if abs(step) < 1e-15:
            break
    return diode_f(v_d)[0]


def reversed_charging_current(v_source, v_cap, r):
    """Current into the capacitor when the diode's anode is on the capacitor side: with
    v_D = v_cap - v_R and v_R = V - I R, the current is I = -i_d(v_D)."""
    u = 0.0
    for _ in range(100):
        i, g = diode_f(u)
        f = -i - (u - v_cap + v_source) / r
        step = f / (g + 1.0 / r)
        u += step
        if abs(step) < 1e-15:
            break
    return -diode_f(u)[0]


def rk4(v_source, r, c, t_end, h, current=charging_current):
    v = 0.0
    n = round(t_end / h)
    for _ in range(n):
        k1 = current(v_source, v, r) / c
        k2 = current(v_source, v + 0.5 * h * k1, r) / c
        k3 = current(v_source, v + 0.5 * h * k2, r) / c
        k4 = current(v_source, v + h * k3, r) / c
        v += h * (k1 + 2 * k2 + 2 * k3 + k4) / 6.0
    return v


def main():
    out = {"thermalVoltageVolts": fmt(VT, 17)}

    forward = solve_diode_series(D(5), D(1000))
    reverse = solve_diode_series(D(-5), D(1000))
    out["diodeForward"] = {"vd": fmt(forward[0]), "i": fmt(forward[1])}
    out["diodeReverse"] = {"vd": fmt(reverse[0], 17), "i": fmt(reverse[1], 17)}

    cases = {}
    for name, polarity, v_bb, r_b, swapped in [
        ("npnCutoff", 1, D(0), D(430000), False),
        ("npnActive", 1, D(5), D(430000), False),
        ("npnSaturation", 1, D(5), D(4300), False),
        ("npnReverseActive", 1, D(5), D(430000), True),
        ("pnpCutoff", -1, D(0), D(430000), False),
        ("pnpActive", -1, D(-5), D(430000), False),
        ("pnpSaturation", -1, D(-5), D(4300), False),
    ]:
        v_cc = D(10) * polarity
        r = solve_common_emitter(v_cc, D(1000), v_bb, r_b, polarity, swapped)
        cases[name] = {key: fmt(value, 17) for key, value in r.items()}
    out["commonEmitter"] = cases

    # Switched nonlinear RC: 5 V, a switch (10 mOhm closed), 10 kOhm, diode, 1 microfarad, the
    # capacitor starting at 0 V.
    r, c = 10_000.01, 1e-6  # the closed switch adds its 10 milliohm
    transient = {}
    for t in (0.05, 0.1, 0.2):
        coarse = rk4(5.0, r, c, t, 2e-5)
        fine = rk4(5.0, r, c, t, 1e-5)
        assert abs(coarse - fine) < 1e-9, (t, coarse, fine)
        transient[str(t)] = fmt(fine, 15)
    out["diodeCharging"] = transient
    reverse_c = {}
    for t in (0.05, 0.1, 0.2):
        reverse_c[str(t)] = fmt(
            rk4(5.0, r, c, t, 1e-5, reversed_charging_current), 12
        )
    out["diodeChargingReversed"] = reverse_c

    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
