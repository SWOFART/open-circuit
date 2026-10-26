# Stage 4 Contract: Switching, Protection, Motors, and Meters

**Status:** Draft. Requires approval from both the Engine and Application workstreams.
**Scope:** Generic educational models for one SPDT relay, an I²t fuse, a permanent-magnet DC motor, and resistive voltmeter/ammeter models.
**Architecture:** Pure-Rust Engine owns physics and runtime state; the app transports Engine-owned DTOs; the UI presents them. See [`ARCHITECTURE.md`](../../ARCHITECTURE.md).

This contract extends the Stage 1 schematic/catalog/result contracts and the Stage 3 transient solver contract. It chooses deliberately small models; none represents every characteristic of a real component or a manufacturer's part. All quantities are finite SI values. Numeric display prefixes do not change Engine values.

## 1. Shared rules

- The saved project remains a version 1 schematic. The new component types and numeric parameters are additive. Runtime relay/fuse state, fuse I²t accumulation, motor speed after time zero, node values, and result frames are never written into the schematic.
- A successful compile starts a new run at simulation time zero. Relay state starts released, fuse state starts intact with zero accumulated I²t, and motor speed starts from its configured initial value. No state transfers across compiles.
- The public solver step remains 1 ms. The deterministic internal-substep rule and the 60 Hz maximum UI delivery rate remain separate as defined in the Stage 1 and Stage 3 contracts. The motor's coupled dynamic modes contribute to the existing rate bound.
- The passive sign convention remains `voltage = V(pin1) - V(pin2)`, `current` positive entering `pin1`, and `power = voltage × current`. Relay ports define their own named pin pairs below. Positive motor current and voltage produce positive shaft torque and positive rotation.
- NaN, infinities, null numeric values, unknown parameters, and values outside the stated domains are rejected with `InvalidParameter`. Cross-parameter constraints are checked at compile and update boundaries.
- Catalog defaults are reproducible educational defaults, not ratings, safety advice, or manufacturer specifications. A real part profile requires a named part and its exact datasheet conditions.

## 2. Catalog entries and parameter domains

The Engine catalog is authoritative. Type IDs and pin IDs are stable serialized strings. Parameter IDs use the existing camelCase format. The defaults below are intentionally simple values for the first implementation.

| Type ID | Ordered pin IDs and roles | Parameters (SI; domain; default) |
|---|---|---|
| `relay_spdt` | `coil_positive` (positive reference), `coil_negative` (negative reference), `common`, `normally_open`, `normally_closed` | `coilResistanceOhms` (> 0 Ω; 100 Ω), `coilInductanceHenries` (> 0 H; 0.01 H), `pickupCurrentAmps` (> 0 A; 0.04 A), `dropoutCurrentAmps` (≥ 0 A; 0.02 A), `closedContactResistanceOhms` (> 0 Ω; 0.1 Ω) |
| `fuse` | `pin1`, `pin2` (unpolarized) | `intactResistanceOhms` (> 0 Ω; 0.01 Ω), `tripI2tAmpereSquaredSeconds` (> 0 A²·s; 0.004 A²·s) |
| `pmdc_motor` | `pin1` (positive), `pin2` (negative) | `armatureResistanceOhms` (> 0 Ω; 2 Ω), `armatureInductanceHenries` (> 0 H; 0.1 H), `motorConstantNewtonMetersPerAmp` (> 0 N·m/A; 0.1 N·m/A), `rotorInertiaKilogramMetersSquared` (> 0 kg·m²; 0.01 kg·m²), `viscousFrictionNewtonMeterSecondsPerRadian` (≥ 0 N·m·s/rad; 0.01 N·m·s/rad), `loadTorqueNewtonMeters` (finite signed N·m; 0 N·m), `initialAngularVelocityRadiansPerSecond` (optional finite signed rad/s; omitted means 0), `initialArmatureCurrentAmps` (optional finite signed A; omitted means DC initialization at the configured initial speed) |
| `voltmeter` | `pin1`, `pin2` (unpolarized) | `inputResistanceOhms` (> 0 Ω; 10 MΩ) |
| `ammeter` | `pin1`, `pin2` (unpolarized) | `burdenResistanceOhms` (> 0 Ω; 0.1 Ω) |

The catalog unit IDs added for these entries are `ampere_squared_second`, `kilogram_meter_squared`, `newton_meter`, `newton_meter_per_amp`, `newton_meter_second_per_radian`, and `radian_per_second`. `newton_meter_per_amp` is numerically equal to the PMDC back-EMF constant unit `volt_second_per_radian` in SI; only the former is used for the shared motor constant. The serialized motor result quantity is `angularVelocity` with unit `radian_per_second`.

Relay validation additionally requires `dropoutCurrentAmps < pickupCurrentAmps`. All parameters marked as initial conditions are schematic inputs and require a compile when changed; live numeric parameters follow Stage 3's `set_parameter` rules.

## 3. Component models

### 3.1 SPDT relay

The coil is a series resistance and inductance:

```text
v_coil = coilResistanceOhms × i_coil
       + coilInductanceHenries × d(i_coil)/dt
```

`v_coil = V(coil_positive) - V(coil_negative)` and `i_coil` is positive entering `coil_positive`. Coil polarity does not affect actuation: thresholds use `abs(i_coil)`. A suppressor diode, coil temperature, magnetic saturation, motion, and manufacturer-specific operate/release delays are omitted.

The two contact branches are `common`–`normally_closed` and `common`–`normally_open`. In the `released` state the NC branch has `closedContactResistanceOhms` and the NO branch is open. In the `picked` state the NO branch has that resistance and the NC branch is open. The branches are mutually exclusive. A transition is atomic at its event time; contact travel, arcing, bounce, leakage, and a finite break interval are not modeled.

The relay is non-latching. A new run starts `released`. After a solved operating point or transient internal step:

- `released → picked` when `abs(i_coil) >= pickupCurrentAmps`;
- `picked → released` when `abs(i_coil) <= dropoutCurrentAmps`;
- otherwise, retain the existing state.

The strict threshold gap provides hysteresis. The Engine checks at internal-step boundaries and applies a transition at the first boundary where its condition is true. At initialization it starts released, solves, evaluates the threshold, and repeats at time zero until stable before returning the initial snapshot. Real relay operate and release values are part-specific; Omron's technical explanation defines separate must-operate and must-release values and reports different catalog values by relay model [Omron relay technical explanation](https://www.ia.omron.com/data_pdf/guide/36/generalrelay_tg_e_10_6.pdf).

### 3.2 I²t fuse

While intact, the fuse is a resistor of `intactResistanceOhms`. Its runtime thermal proxy is the accumulated squared-current integral:

```text
I2t(0) = 0
I2t(n+1) = I2t(n) + h/2 × (i(n)² + i(n+1)²)
```

`i` is the signed pin current; squaring makes polarity irrelevant. Only completed, accepted internal intervals contribute. At the first internal-step boundary where the accumulator is greater than or equal to `tripI2tAmpereSquaredSeconds`, the fuse changes from `intact` to `tripped` and its branch becomes an ideal open circuit at that same model time. The reported accumulator is not clamped, so a trip may overshoot the threshold by one internal interval. The threshold is a simplified model parameter, not a rated current, a product time-current curve, or a claim about total clearing I²t. Arc energy and opening delay are omitted. Manufacturer guides distinguish melting from clearing I²t and explain why time-current curves depend on the fuse design and test conditions [Littelfuse Fuseology Design Guide](https://www.littelfuse.com/assetdocs/fuseology-selection-guide?assetguid=d812dff2-1c47-4dc3-bce7-07a4001ddc32).

The fuse is one-shot within a run. It does not cool or reset, including after current falls to zero. A new successful compile starts it intact with a zero accumulator. A tripped fuse is a normal device state, not a compile error or diagnostic. The Engine continues solving; the result reports `state = tripped`, zero fuse current, and the solved terminal voltage across the open fuse.

### 3.3 Permanent-magnet DC motor

The motor is a single-shaft, linear PMDC model. `pin1` is the positive armature terminal. With current positive into `pin1`, positive current develops positive torque and positive angular velocity:

```text
v = R × i + L × di/dt + k × ω
J × dω/dt = k × i - b × ω + τ_external
```

Here `R` and `L` are armature resistance and inductance, `k` is the shared torque/back-EMF constant, `ω` is angular velocity, `J` is rotor inertia, `b` is viscous friction, and `τ_external` is signed torque applied to the shaft (negative torque opposes positive rotation). In SI, the shared `k` gives reciprocal electromechanical power coupling. `τ_external` is constant during a run except for a live parameter update. No time-varying load profile, Coulomb/static friction, speed limit, saturation, temperature, gearbox, commutator ripple, or position state is modeled. `loadTorqueNewtonMeters` is an applied shaft torque, not a universal load profile.

Angular velocity defaults to zero. If `initialArmatureCurrentAmps` is absent, initialize it from the DC solve with the configured initial speed held fixed; if present, it is the signed current at time zero and participates in the Stage 3 consistency check. Motor state is runtime data after time zero and is not saved.

For each internal step of length `h`, the Engine solves the circuit MNA unknowns, `i(n+1)`, and `ω(n+1)` together using these trapezoidal companion equations:

```text
L × (i(n+1) - i(n)) / h
  = 1/2 × [(v(n) - R×i(n) - k×ω(n))
        + (v(n+1) - R×i(n+1) - k×ω(n+1))]

J × (ω(n+1) - ω(n)) / h
  = 1/2 × [(k×i(n) - b×ω(n) + τ_external)
        + (k×i(n+1) - b×ω(n+1) + τ_external)]
```

In particular, the next-step back EMF uses `ω(n+1)` and next-step torque uses `i(n+1)`; coupling through only the previous state is not allowed. This implicit coupled solve uses the Stage 3 deterministic substep rule, which must include the motor's linear dynamic modes in its rate bound. After an event topology change, the first advancing internal step uses the existing one-step backward-Euler event recovery rule.

MIT's DC motor notes derive the armature voltage, torque, and back-EMF equations and identify the shared constant in SI [MIT OpenCourseWare, Introduction to Robotics, Chapter 2](https://ocw.mit.edu/courses/2-12-introduction-to-robotics-fall-2005/9158f91fc935aae58345446fc110f6e7_chapter2.pdf).

### 3.4 Voltmeter and ammeter

Both meters are passive two-terminal resistive components, and both affect the circuit they measure:

- The voltmeter is a resistance `inputResistanceOhms` across `pin1` and `pin2`. Its reading is `V(pin1) - V(pin2)`; its input current is reported using the normal pin-1 sign convention. It is not an infinite-impedance probe.
- The ammeter is a series resistance `burdenResistanceOhms`. Its reading is the current entering `pin1`; its burden voltage is `V(pin1) - V(pin2)`. It is not an ideal zero-resistance probe.

There is no range switching, accuracy/quantization model, AC response, autoranging, lead resistance, or manufacturer-specific profile. The finite defaults are generic teaching values. Fluke manuals provide model- and range-specific examples (including 10 MΩ nominal voltmeter input impedance and current-dependent burden voltage), which support modeling loading rather than treating meters as ideal [Fluke 78 manual](https://assets.fluke.com/manuals/78______umeng0100.pdf), [Fluke 87 manual](https://assets.fluke.com/manuals/87______umeng0800.pdf).

## 4. Event processing and diagnostics

For each internal step, solve the electrical/mechanical system first. Then evaluate fuse integrals and relay thresholds from that accepted pre-transition solution. Gather all transitions, sort them by canonical component UUID bytes, and apply them together at the same simulation time. Rebuild and solve at that same time, then repeat event evaluation until no state changes. This makes simultaneous events independent of schematic order and avoids applying one device's event against another device's stale solution.

The Engine allows at most 16 state-changing same-time re-solves after the initial solve; if a seventeenth would be needed, it fails. It also detects a repeated discrete-state vector. If either limit is hit, emit the structured error diagnostic `EventChatter` referencing the stateful components involved, do not publish a partial frame for the failed public step, retain the last committed snapshot, and stop that run. A successful compile that cannot settle initial events fails with `EventChatter`. Recompiling after a schematic edit starts a fresh run.

After any relay contact transition or fuse opening, the first internal step that advances model time uses backward Euler once. Same-time re-solves do not consume this recovery step. Fuse tripping itself is reported through the typed device state and zero-current numeric result, not `EventChatter` or a warning.

Existing structured diagnostics keep their Stage 1/3 meanings. Stage 4 adds:

| Code | Severity | Meaning | References |
|---|---|---|---|
| `EventChatter` | error | Relay/event state vector repeats or fails to settle within the deterministic same-time retry limit. The current run stops at the last committed public step. | The relay/fuse component IDs participating in the unresolved event set. |
| `NonFiniteState` | error | A device-state calculation (including the fuse I²t accumulator or motor state) becomes non-finite from otherwise accepted inputs. The current run stops at the last committed public step. | The component whose state cannot be represented as a finite `f64`. |

Invalid domains and the relay cross-parameter constraint use `InvalidParameter`. A motor may start from a non-equilibrium initial speed/current and then accelerate; this is valid, not a diagnostic.

## 5. Results, discrete state, and frame format

The analog result layout remains numeric and deterministic. Two-terminal components continue to expose `voltage` then `current`; a fuse adds result quantity `accumulatedI2t` in unit `ampere_squared_second`, and the motor adds quantity `angularVelocity` in unit `radian_per_second`. Their stable keys are `<componentId>/accumulated_i2t` and `<componentId>/angular_velocity`. Relay keys are `<componentId>/coil/voltage`, `<componentId>/coil/current`, `<componentId>/normally_open/voltage`, `<componentId>/normally_open/current`, `<componentId>/normally_closed/voltage`, and `<componentId>/normally_closed/current`. The relay therefore exposes six slots in catalog port order: coil voltage/current, normally-open branch voltage/current, then normally-closed branch voltage/current. For each named relay port, voltage is the first listed pin potential minus the second, and branch current is positive entering the first listed pin (`coil_positive`, `common`, or `common` respectively). A fuse and each meter use the standard pin-1/pin-2 sign convention. The motor speed slot uses the result sign-convention value `positiveRotationWithPositivePin1Current`. Quantity and sign-convention values follow the existing camelCase serde format; unit IDs follow the existing snake_case format. A result layout never includes discrete states as floating-point values.

Each successful compile/result snapshot also contains the typed state records for every relay and fuse, ordered by canonical component UUID bytes. The serialized state values are the exact lowercase strings shown here:

```text
RelayState { componentId, state: released | picked, lastTransitionTimeSeconds? }
FuseState  { componentId, state: intact | tripped, lastTransitionTimeSeconds? }
```

The transition time is absent before the first transition; otherwise it is the exact model time of the event. Motor speed is the `angular_velocity` analog result slot, not a discrete state. These fields are runtime results, never schematic content.

Stage 4 uses frame magic `OCF2`, because adding discrete state records changes the Stage 1 `OCF1` frame envelope. The Stage 4 app emits OCF2 for every live frame, including frames with no relay/fuse records; it does not reinterpret OCF1 as OCF2. The fixed header retains Stage 1 fields and order through `valueCount`, then adds `stateCount` before `revisionLength`:

| Offset | Size | Field |
|---|---:|---|
| 0 | 4 | Magic `OCF2` |
| 4 | 4 | `layoutVersion`, `u32` |
| 8 | 8 | `frameSequence`, `u64` |
| 16 | 8 | `simulationStep`, `u64` |
| 24 | 8 | `simulationTimeSeconds`, `f64` |
| 32 | 16 | `runId`, UUID raw bytes |
| 48 | 4 | `valueCount`, `u32` |
| 52 | 4 | `stateCount`, `u32` |
| 56 | 4 | `revisionLength`, `u32` UTF-8 bytes |
| 60 | n | `documentRevision`, UTF-8 |
| next | p | Zero padding to the next 8-byte boundary |
| next | 8 × `valueCount` | Numeric `f64` payload in layout offset order |
| next | 32 × `stateCount` | Discrete state records in component UUID order |

All integers and floats are little-endian. Each 32-byte state record contains `componentId` (16 raw UUID bytes), `kind` (`1` relay, `2` fuse), `value` (`0` released/intact, `1` picked/tripped), `flags` (`u16`, bit 0 means a transition time is present), four zero reserved bytes, and `lastTransitionTimeSeconds` (`f64`; zero when bit 0 is clear). A reader also verifies that each state record references exactly one stateful component in the active schematic and that no such component is missing. Unknown kind/value codes, unknown flag bits, nonzero reserved bytes, duplicate or unsorted component IDs, non-finite or future transition times, nonzero padding, truncated data, invalid UTF-8, revision strings over 256 bytes, or size/count overflow reject the whole frame. A frame also rejects non-finite analog values and non-finite simulation time, as in Stage 1. `stateCount` is at most 32,768; `valueCount` retains the Stage 1 maximum of 65,536. The exact length is the padded header plus `8 × valueCount + 32 × stateCount` bytes.

`runId`, document revision, layout version, and frame sequence are checked using the Stage 1 stale-frame rules. The state table belongs to that exact frame. Existing latest-snapshot delivery, the 60 Hz cap, acknowledgements, and no-queue behavior apply to the entire OCF2 message, so numeric values and device states cannot be delivered from different frames. Trace history remains limited to numeric result-layout slots (including motor angular velocity and fuse accumulated I²t); this contract does not add discrete-state traces or event-history storage.

[`tests/golden/stage-4-frame.json`](../../tests/golden/stage-4-frame.json) is an independently encoded OCF2 example with two numeric values and one relay state record. The app codec implementation must validate its bytes exactly.

Compile and parameter-update JSON results expose the same typed device-state records as the associated snapshot. The TypeScript DTOs are generated from Engine-owned Rust types; `crates/engine` stays transport-independent and does not import Tauri or UI code.

## 6. Canonical known-answer fixtures

Expected data is kept separately from schematic inputs. The fixture conditions below are the normative circuit definitions; matching expected values and tolerances are in `tests/golden/`.

| Fixture | Circuit and event sequence | Expected check |
|---|---|---|
| [SPDT self-latching relay](../../tests/golden/stage-4-relay-self-latching.expected.json) | An ideal 5 V source drives the relay coil (`R=100 Ω`, `L=10 mH`) and a 1 Ω start-path resistor to ground. The NO contact connects coil return to ground when picked. NC is left open. With `I_pickup=40 mA`, initialization starts released and picks at time zero. At 2 ms set the start-path resistance to `1e12 Ω`; the NO contact keeps the relay picked. At 5 ms it remains picked with approximately 49.95005 mA coil current. At 6 ms set the source to 0 V; by 7 ms the relay is released. | Exact states at the listed times; coil-current tolerance `0.1 mA` while held. This checks initial transition, holding path, drop-out, deterministic runtime state, and reset-on-compile semantics. |
| [I²t fuse](../../tests/golden/stage-4-fuse-i2t.expected.json) | Ideal 2.02 V source, 0.01 Ω intact fuse, and 1 Ω load in series. Fuse threshold is 0.004 A²·s. The steady intact current is 2 A. | It trips at 1 ms with accumulated I²t = 0.004 A²·s. At 2 ms it remains tripped, carries 0 A, and has 2.02 V across its open terminals. |
| [PMDC startup](../../tests/golden/stage-4-pmdc-motor.expected.json) | Ideal 10 V source; `R=2 Ω`, `L=0.1 H`, `k=0.1 N·m/A`, `J=0.01 kg·m²`, `b=0.01 N·m·s/rad`, external torque 0, and both initial states zero. | Closed-form solution of the coupled linear ODE at 8 s: `ω=33.33349259 rad/s`, `i=3.33332471 A`; tolerances are 0.01 rad/s and 0.001 A. This is an independent analytical reference, not an Engine-generated golden. |
| [Meter loading](../../tests/golden/stage-4-meters-loading.expected.json) | Voltmeter: 10 V Thevenin source with 1 MΩ internal resistance and a 10 MΩ meter across its output. Ammeter: ideal 10 V source, 10 Ω load, and 1 Ω ammeter resistance in series. | Voltmeter reads 9.09090909 V and draws 0.909091 µA. Ammeter reads 0.90909091 A and has 0.90909091 V burden. The values demonstrate loading and follow independent divider/Ohm's-law calculations. |

For the PMDC fixture, with state vector `x = [i, ω]ᵀ`, the equations reduce to `x' = A x + c`, where `A = [[-20, -1], [10, -1]]` and `c = [100, 0]ᵀ`. Its eigenvalues are `(-21 ± √321)/2`; the expected values at 8 s are from the closed-form solution with `x(0) = [0, 0]ᵀ`. The fixture values must be calculated independently from these equations, not copied from simulator output. The other fixtures use direct network equations and the deterministic event rules above. Their values are not manufacturer profiles.

## 7. Layer boundaries and review

- The Engine validates catalog parameters, derives connectivity, solves the equations, owns fuse/relay/motor runtime state, creates result layouts and OCF2 frames, and emits structured diagnostics.
- The Application owns the simulation thread, frame scheduling, binary Channel, acknowledgements, and transport lifecycle. It does not calculate device equations.
- TypeScript owns symbols, pin geometry, interaction, and presentation. It uses generated Engine types and displays state/result fields; it does not derive connectivity or physics.
- The component catalog is the single source of stable type/pin/parameter/unit IDs. The frame layout is the single source of analog numeric slots; OCF2 state records are a typed side table.
- Review is required from both workstreams. A change to the equations, domains/defaults, signs, state transitions, error semantics, frame format, or fixture values updates this contract and the associated golden reference before implementation is accepted.

## 8. Primary references

- Omron, [Technical Explanation for General-purpose Relays](https://www.ia.omron.com/data_pdf/guide/36/generalrelay_tg_e_10_6.pdf): relay mechanisms and separate, part-specific operate/release criteria.
- Littelfuse, [Fuseology Design Guide](https://www.littelfuse.com/assetdocs/fuseology-selection-guide?assetguid=d812dff2-1c47-4dc3-bce7-07a4001ddc32): melting versus clearing I²t and fuse-specific time-current behavior.
- MIT OpenCourseWare, [Introduction to Robotics, Chapter 2: Actuators and Drive Systems](https://ocw.mit.edu/courses/2-12-introduction-to-robotics-fall-2005/9158f91fc935aae58345446fc110f6e7_chapter2.pdf): armature circuit, torque, and back-EMF relations.
- Fluke, [78 Automotive Meter manual](https://assets.fluke.com/manuals/78______umeng0100.pdf) and [87 Multimeter manual](https://assets.fluke.com/manuals/87______umeng0800.pdf): examples of finite voltage input impedance and current burden.

These references support the physical relationships and explain why real-part characteristics vary. They do not set the generic defaults in this contract.
