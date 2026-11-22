# Stage 13 Contract: Protection and Safety

**Issue:** [#196](https://github.com/SWOFART/open-circuit/issues/196). **Parent:** [#158](https://github.com/SWOFART/open-circuit/issues/158). **Status:** proposal for the approval of both workstreams and the project owner. Implementation (#197 to #206) follows acceptance of this contract.

**Scope:** Protective earth (PE) as a real conductor, earth faults, the residual-current device (RCD), breaker trip curves, short-circuit current, touch voltage and the hazard result, with their diagnostics, parameters and tolerances. It amends the earlier contracts where it says so and nothing else.

> **No installation standard is claimed.** The devices, thresholds, curves and limits of this stage describe a *simulation for teaching*. A circuit that behaves well here is not thereby safe, compliant with IEC 60364, IEC 60898, IEC 61008/61009, VDE, GOST or any other rule, or fit for a real installation, and a trip time, a curve class, a 30 mA rating or a 50 V limit named below is a model parameter, not a statement about a product. The application says so wherever it shows a hazard result (section 7.4). Arc flash, lightning, detailed soil, cable and busbar models, and manufacturer data are out of scope.

## 1. Boundary and compatibility

Everything here uses the Stage 9 time-domain engine and the fixed 1 ms public step, bounded deterministic internal substeps, trapezoidal integration, backward Euler after events, and the event rules of Stages 3, 6 and 10. Rust owns connectivity, equations, states, measurements and diagnostics; Tauri carries generated DTOs; the editor draws pins and shows results.

| Earlier text | What changes |
|---|---|
| stage-8-supply-rails.md section 2 and stage-9: N and PE are labels; a rail named `PE` is ordinary text | **Unchanged for rails.** A rail named `PE` is still only a label and creates no earth connection. PE gets its meaning from the devices of section 2, never from a name. |
| ARCHITECTURE.md Stage 9 note: "PE stays a label until Stage 13 gives it a safety meaning" | Section 2 gives it one: a *PE net* is a conductor net that contains an earth electrode (section 2.2). ARCHITECTURE.md records this decision with this contract. |
| stage-1.md section 4, reference: the reference is a ground marker, or an automatic one in each island | A project reference also exists when the drawing has an earth electrode and no marker (section 2.1). Drawings without an electrode keep the earlier behaviour exactly. |
| stage-5-assignments.md section 5: fault kinds `open`, `short` and `parameter` | A fourth kind, `leak`, is added (section 3), with the new fields `pinId` and `frameId` (the fault entry still refuses unknown fields), and `value` also required for `leak`. An assignment file's references are checked when it loads, as component references are: the part, the pin and the frame (an `exposed_part` of the starting circuit) must resolve. How a diagnosis names a `leak` is defined by #205. Existing kinds and files keep their meaning. |
| stage-10 section 3, **Breaker**: instantaneous trip only | Optional trip classes are added (section 5). With the new parameter at its default the Stage 10 behaviour is unchanged to the bit. |
| stage-9 and stage-10 assignment and fault exclusion | Lifted for the devices of this stage and, for `leak` only, for the Stage 9 and 10 devices listed in section 3.2. Everything else stays excluded. |

**Project format.** The schematic version stays 3: this stage adds component types and parameters and no schematic field. An application that does not know a type refuses the file through its existing unknown-type refusal; there is no downgrade. Runtime state and derived results are never saved.

## 2. Protective earth

### 2.1 The earth node

All ground markers already denote one reference (stage-1.md). That reference is also the **earth node**, `0 V`. A drawing that has an earth electrode (section 2.2) and no ground marker still has the earth node: the electrode's far end is it. The earth node is Engine-derived and never saved. An island that contains an electrode, or a ground marker, takes the earth node as its reference in place of an automatic one, which is how islands earthed in different places share the earth return. A drawing with neither has **no earth node**: it keeps the existing automatic-reference behaviour and its `NoGroundReference` warning, and an exposed part in it reports a touch voltage that is not valid (section 7.2); so does an exposed part in an island that is not joined to the earth node.

### 2.2 The earth electrode and the PE net

`earth_electrode` is a one-pin device (`pe`). It is a resistor `earthResistanceOhms` from `pe` to the earth node, in every operating state, and it carries no operator state. Several electrodes in one project are in parallel. A **PE net** is any conductor net that contains the `pe` pin of an electrode; the PE conductor of a drawing is the set of wires and rails of its PE nets. Bonding a motor frame, an exposed part or a transformer's earthed point to PE is drawing a wire to a PE net: no part needs a "frame pin" (the Stage 9 and 10 pin sets do not change).

The electrode joins electrical islands through the earth node as physics says: two sub-circuits earthed at different places share the earth node and the earth-return path. Island and reference rules otherwise stay as they are.

### 2.3 What PE does not do

PE carries no current in normal operation because nothing drives it. A source's internal resistance, a load, a fault (section 3) or a drawn wire gives it current. There is no coloured-conductor convention, no TN/TT/IT classification and no check that PE is bonded where a standard would require it: the hazard result of section 7 is the only judgement.

## 3. Earth faults

### 3.1 The `leak` fault

A fault of kind `leak` is an insulation failure: it joins one named pin of one component to a **frame**, the `ep` pin of a named exposed part (section 7.1), through a resistance. It is an overlay applied at compile, like the other faults (stage-5-assignments.md section 5): it is never in the schematic and never in a project file. The frame is the conductor a person could touch; whether it is bonded to PE is what the drawing decides, with a wire, so one fault covers both the bonded frame (the current flows back through PE and the electrode) and the unbonded one (the frame floats up to the live voltage).

The fault entry gains two optional fields, `pinId` and `frameId` (both required for `leak`, refused for the other kinds), and `value`, today required for a `parameter` fault, is also required for `leak` as the leakage resistance in ohms.

| Field | Domain |
|---|---|
| `kind` | `leak`; the catalog lists it in each part's `faultKinds` where section 3.2 allows it |
| `pinId` | a pin of the faulted component, from the catalog |
| `frameId` | the stable ID of an `exposed_part` of the schematic |
| `value` | 0.001 Ω to 1e9 Ω; 0.001 Ω is the same bolted-fault floor as the `short` fault (`SHORT_FAULT_RESISTANCE_OHMS`) |

The fault adds the branch `value` between the pin's net and the net of the frame's `ep` pin, and nothing else; it never changes a parameter, the schematic or any state, and it joins the two nets into one island as any branch does. An unknown part, pin or frame, a frame that is not an exposed part, a kind not listed for the part, a missing field or a value outside the domain is `InvalidFault`. The Stage 5 limits stay: at most one fault per component and at most eight faults per task, so one part takes one leak.

### 3.2 Which parts accept it

`leak` is allowed on every pin of these types: `resistor`, `inductor`, `capacitor`, `dc_voltage_source`, `ac_voltage_source`, `three_phase_source_star`, `three_phase_source_delta`, `three_phase_load_star`, `three_phase_load_delta`, `transformer_three_phase`, `induction_motor_three_phase`, `pmdc_motor`, `contactor_three_pole`, `breaker_three_pole` and `thermal_overload_three_pole`. It is not allowed on the earth electrode, the exposed part or a meter. The drawing itself can also make an earth fault, with a wire or a resistor from a live net to the frame; both are the same physics.

### 3.3 The fault loop

Nothing new is solved: the loop is the ordinary circuit, source → live conductor → fault resistance → frame → PE conductor (the bonding wire) → electrode → earth node → the source's earthed point or its earth return. With the frame bonded the touch voltage is the drop across the electrode, `Re·I`; with it unbonded the frame sits at the voltage of the leaking pin. Whether the fault trips a protective device is decided by the device models of sections 4 and 5, from the currents that flow.

## 4. The residual-current device

### 4.1 Types and pins

| Type ID | Ordered groups |
|---|---|
| `rcd_two_pole` | pole1 (l1, t1), pole2 (l2, t2) |
| `rcd_four_pole` | pole1 (l1, t1), pole2 (l2, t2), pole3 (l3, t3), pole4 (l4, t4) |

A group is the named ordered pair of pins of a pole, current entering its first pin and voltage measured first minus second, as in Stage 10. The poles are series paths of `closedContactResistanceOhms` when closed and open circuits when open. An RCD has one linked state, switched atomically like the Stage 10 breaker.

### 4.2 The residual current and the trip

The **residual current** is the instantaneous sum of the currents of all poles, `iΔ(t) = Σ iₖ(t)` (each entering its first pin). In a healthy circuit it is zero to the solver's tolerance; a current that leaves through an earth fault does not come back through the device and appears as `iΔ`.

The Engine takes a mean-square detector exactly as the Stage 10 coil and overload detectors do, `τ·dx/dt = iΔ² − x` with `τ = residualSenseSeconds` (default one 50 Hz period), and the detector RMS `I = √x`. For a sinusoidal residual the detector output ripples at twice the supply frequency, by about ±4 % at the default `τ` and a 50 Hz supply, so the AC fixtures use at least `1.25 I_n`; a shorter `τ` ripples more and can make the device chatter around `I_n` as its dose resets and restarts, which is the model's honest behaviour and not a defect. With `I_n = ratedResidualCurrentAmps` and `T = tripTimeAtRatedSeconds`:

- **No-trip zone.** For `I < I_n` the trip dose `d` is set to 0 and nothing accumulates (an RCD has no memory in this model). The detector is first order, so a constant residual of exactly `I_n` only approaches `I_n` and never reaches it: **a residual trips the device only when it is above `I_n`** (the reference fixtures use at least `1.05 I_n` for DC), and a residual of exactly `I_n` is not a case the model promises anything for.
- **Trip.** For `I ≥ I_n` the dose accumulates `d ← min(1, d + h·(I/I_n)²/T)`, so at a constant `I` the trip time is `T·(I_n/I)²` (`T` at `I_n`, a quarter of it at `2 I_n`, 1/25 at `5 I_n`), plus the detector's start-up from rest. At `d = 1` the device latches: all poles open together.
- **Open and reset.** An explicit open command opens the poles and clears the latch, the dose and the detector state (`x = 0`); a close is refused while latched. Closing on a residual current always succeeds: the detector and the dose start from rest, so the device trips only after the detector start-up and about `T·(I_n/I)²` (unlike the instantaneous breaker, it never trips in the settled reply of the close), and the latch then refuses a further close. There is no test button in this stage (section 12).

The curve is a teaching characteristic. In particular the contract does **not** claim the IEC test points (the no-trip at half the rated current, the 300 ms, 150 ms and 40 ms limits): a user who sets `T` to 0.3 s gets a trip at the rated current after 0.3 s and nothing more.

### 4.3 Results

`<id>/tripped` (0 or 1), `<id>/residual_rms` (the detector RMS, amperes) and `<id>/trip_dose` (0 to 1), in that order after the pole currents of the device, using the Stage 10 key and slot rules. The results are Engine state of the run and are never saved.

## 5. Breaker trip classes

### 5.1 The new parameters

The three-pole breaker keeps `instantaneousTripAmps` (live, and the only trip threshold while the class is 0) and gains `curveClass` (an optional choice, 0 to 3; absent means 0) and, for the classes 1 to 3, `ratedCurrentAmps` and the thermal parameters of section 9. With **`curveClass` 0** the Stage 10 model applies unchanged, `ratedCurrentAmps` and the thermal parameters are ignored, and every earlier file and fixture gives the same answers. With 1, 2 or 3 the breaker is a **thermal-magnetic** device:

| `curveClass` | Name | Magnetic threshold |
|---|---|---|
| 1 | B | `5 · ratedCurrentAmps` |
| 2 | C | `10 · ratedCurrentAmps` |
| 3 | D | `20 · ratedCurrentAmps` |

- **Magnetic part.** As in Stage 10, if the absolute instantaneous current of any pole reaches the threshold the breaker trips at the first sampled crossing and all poles open together; `instantaneousTripAmps` is ignored for the classes 1 to 3.
- **Thermal part.** The same first-order detector and dose model as the Stage 10 overload relay, per pole maximum, with `ratedCurrentAmps` as the rated current and `thermalSenseSeconds`, `tripScaleSeconds` and `coolSeconds` unchanged in meaning: at a constant `q = (I/I_n)² > 1` the trip time from a cold start is `K/(q−1)`. At `d = 1` it latches and opens. The cooling continues while latched and the close is refused **only while latched**, as for the Stage 10 breaker; there is no dose gate on closing (that belongs to the overload relay's reset), and a breaker of class 0 never refuses a close for any reason of dose.
- An explicit open command clears the *latch* and keeps the dose, so closing at once into an overload trips again soon; the dose cools while the poles are open.

The multipliers are model values, chosen so that a breaker of the class certainly trips there (decision 4 of section 13); they claim no match with any manufacturer's or standard's curve.

### 5.2 Results

The result layout is built at compile for each component: a breaker of class 1 to 3 has `<id>/thermal_dose` after the existing `<id>/tripped`, and a breaker of class 0 has exactly its Stage 10 slots, so no earlier layout changes.

## 6. Short circuit

No new solver, device or limit is added. A short circuit is a drawn wire or resistor (for example 1 mΩ) between two conductors, or the existing `short` fault of a resistor, inductor or capacitor, and its current is whatever the circuit's resistances allow: the **source's internal resistance** (and the winding and contact resistances) bound it, as Ohm's law says. A short across an ideal source (zero internal resistance and zero loop resistance) stays what it is today: an invalid circuit refused with the existing diagnostics (`ShortCircuit` for the Stage 9 sources, the singular-system diagnostics otherwise), not an infinite current.

The magnetic part of the breaker (section 5.1) and the fuse clear a short circuit; nothing here computes a **prospective short-circuit current** at a point of the drawing (that is an analysis of the circuit with the source shorted, deferred to section 12). The oscilloscope and the ammeters show the peak. The interface may show a protection device's trip as it shows any state, but it never computes "breaking capacity", "let-through energy" or "selectivity": those words have no meaning in this model.

## 7. Touch voltage and the hazard result

### 7.1 The exposed part

`exposed_part` is a one-pin measuring device (`ep`). It is a fixed resistance of **1 GΩ** from `ep` to the reference of its island, in every state, so that its net is never floating, it draws a negligible current, and an unbonded frame is well defined: it floats to the voltage the drawing gives it through a leak or a wire, within `(Rs + Rf)/1 GΩ` relative of the voltage of the leaking pin. It reads the voltage of its net against the earth node, valid only when its island is joined to the earth node (through an electrode or a ground marker of the island). The user places it on a conductor that a person could touch (a motor frame, a housing), normally bonded to PE. A lone exposed part is an island of its own with its own automatic reference and no earth path; its net is that reference, so the 1 GΩ stamp is a no-op there.

### 7.2 Definitions

With `v(t)` the voltage of the net of `ep` minus the earth node, over the Stage 9 window (one period `1/f`, `f = frequencyHz`, the substep samples, section 5.1 of stage-9-three-phase-foundation.md):

- the **touch voltage** `U_T` is the RMS of `v` over the window (for a constant voltage this is its absolute value);
- the **peak touch voltage** is `max |v|` over the window;
- the part is **hazardous** (`hazard = 1`) when `U_T > touchVoltageLimitVolts`, and `hazard = 0` otherwise; a reading that is not yet valid (before the first full window, or because the drawing has no earth node, section 2.1) is reported not valid by the Stage 9 validity rule and is never hazardous.

The limit is a parameter of the device, default 50 V, with no connection to any standard's limit curve or to the time a voltage lasts.

### 7.3 Results

`<id>/touch_voltage_rms`, `<id>/touch_voltage_peak` and `<id>/hazard` (0 or 1), in that order, followed by one `<id>/valid` slot for all three, placed as the Stage 9 measuring devices place theirs (stage-9-three-phase-foundation.md section 6.2). The exposed part has no instantaneous voltage or current slot. The values are Engine measurements; the interface only formats them and colours the part.

### 7.4 What the interface says

Wherever the application shows a touch voltage or a hazard it also shows that the value is a simulation for teaching and not a safety statement (the wording is a message key, decision 6 of section 13). A hazard is advice: it never stops a run, an export or a save.

## 8. Diagnostics and message keys

Diagnostics are compile-time findings of the Engine, listed with each compile result. What happens while a run goes on (a latch, a touch voltage above its limit) is a **result**: the state and hazard slots of sections 4.3, 5.2 and 7.3, which the interface reads and shows; no run-time diagnostic is added.

New codes, each with a message key `diagnostic.<camelCase>` and a text in English in the interface (the language is still decided separately), all warnings:

| Code | When |
|---|---|
| `ExposedPartNotBonded` | an exposed part's net is not joined by wires and rails alone to a PE net (a net that contains an electrode pin); a frame wired only to a ground marker, with no electrode, therefore warns too |
| `NoEarthPath` | an exposed part's island is not joined to the earth node (there is no earth node at all, or the island has no electrode and no ground marker), so its touch voltage is not valid |

Existing codes cover the rest: `InvalidParameter` for a wrong parameter or relationship (`curveClass` outside 0 to 3, a rated current beyond the domain), `InvalidFault` for a wrong `leak`, `NonFiniteState`, `NonConvergence` and `EventChatter` as in Stage 10. A warning never stops a run. The new codes enter the Rust `DiagnosticCode` and the generated TypeScript types are regenerated, never edited.

## 9. Parameters

All scalars are finite, ranges are inclusive and SI in DTOs. Bounds are set in the Rust catalog; the inspector and the Engine enforce them.

| Device | Parameters | Bounds and defaults |
|---|---|---|
| `earth_electrode` | `earthResistanceOhms` | 0.001 to 1,000,000 Ω, default 10 Ω |
| `exposed_part` | `frequencyHz`, `touchVoltageLimitVolts` | 1 to 400 Hz, 50 Hz; 1 to 1,000 V, 50 V |
| `rcd_two_pole`, `rcd_four_pole` | `ratedResidualCurrentAmps`, `tripTimeAtRatedSeconds`, `residualSenseSeconds` | 0.003 to 1 A, 0.03 A; 0.01 to 2 s, 0.1 s; 0.001 to 0.2 s, 0.02 s |
| `rcd_two_pole`, `rcd_four_pole` | `closedContactResistanceOhms`, `initialClosed` | 0.0001 to 10 Ω, 0.01 Ω; optional 0 or 1, default 0 |
| `breaker_three_pole` | `curveClass` | optional integer 0 to 3, default 0 |
| `breaker_three_pole` | `ratedCurrentAmps`, `thermalSenseSeconds`, `tripScaleSeconds`, `coolSeconds` | as the overload relay of Stage 10 (0.1 to 10,000 A, 10 A; 0.005 to 2 s, 0.02 s; 0.01 to 10,000 s, 10 s; 0.01 to 10,000 s, 30 s), used only when `curveClass` is not 0 |

Live edits: the exposed part's limit and frequency, the RCD's rated residual current and trip time, and the breaker's `instantaneousTripAmps` (as in Stage 10) are live; everything else (the electrode's resistance, the class, the breaker's rated current and thermal parameters, the sense times, the contact resistance and the initial state) goes through a compile, as the Stage 10 classification does, device by device. The Engine refuses a live update of a compile-only input.

## 10. Results, order and numerical behaviour

- **No new frame kind.** The results use the existing frame, slot and trace envelopes; the order of the new keys is the order of sections 4.3, 5.2 and 7.3, after the existing keys of the device. Catalog order appends `earth_electrode`, `exposed_part`, `rcd_two_pole`, `rcd_four_pole` after the Stage 10 types.
- **Atomicity.** Contact states of an RCD or a class breaker are linked and settle through the existing event loop in component-ID order; same-time solves never advance the detector or the dose; every published snapshot holds a settled set of poles.
- **Faults.** A `leak` is a stamped resistor; it never creates a derived state, and the same drawing and fault list give bit-identical replays in one environment.
- **Determinism.** As in every earlier stage: identical inputs and events give bit-identical results in one environment; across operating systems the tolerances of section 11 apply.

## 11. Known answers and tolerances

[#202](https://github.com/SWOFART/open-circuit/issues/202) pins independent analytical results; an ngspice reference is added only where one can be built without reproducing the Engine's formula. Each record keeps tool version, hash, options, signs, inputs and the observation steps.

| Fixture | Acceptance |
|---|---|
| Earth fault loop: a DC source `E` with internal resistance `Rs` and no other load, its negative terminal at the reference, a `leak` of `Rf` from its positive pin to an exposed part whose frame is bonded by a wire to an electrode `Re` | the leakage current `E/(Rs + Rf + Re)` within `1e-9 A` absolute plus `1e-6` of the value; the frame voltage `Re·I` likewise, for `Re ≤ 100 Ω` and `Rs + Rf ≤ 1 kΩ` (larger resistances widen the tolerance by their ratio to the exposed part's 1 GΩ); with the bonding wire removed the frame sits at `E·(1 − (Rs + Rf)/1 GΩ)` and the current is `E/(Rs + Rf + 1 GΩ)`, about `E/1 GΩ`, within `1e-12 A` absolute plus `1e-6` of the value |
| Touch voltage of the same fixture on the `exposed_part` (DC, and the same with a 50 Hz source, RMS) | `U_T` within `1e-6 V` plus `1e-6` of the value after one full window; `hazard` exact on both sides of the limit with a 1 % margin; not valid when the drawing has no earth node |
| RCD with a balanced load (no residual) and with a leakage of `0.5`, `1.05`, `2` and `5` times `I_n` (DC), and of `0.5`, `1.25`, `2` and `5` times (50 Hz RMS) | balanced: `tripped = 0` and `\|iΔ\| ≤ 1e-9 A`; `I < I_n`: never trips; above the margins: trip time within `max(2 ms, 1 %)` of the independent waveform, which accumulates the dose `(I/I_n)²/T` from the same first-order detector (about `T·(I_n/I)²` plus the detector start-up) |
| RCD close on a residual current, trip, open, close while latched | the close succeeds; the trip comes after the delay of the row above, never in the settled reply of the close; a close while latched is refused; the open clears the latch, the dose and the detector state exactly |
| Breaker classes B, C, D at 0.9, 1.2, 2, 5.5, 10.5 and 21 times the rated current | no magnetic trip below the threshold; magnetic trip at the first sampled crossing above it (use at least a 1 % margin); thermal trip time within `max(2 ms, 1 %)` of `K(1−d)/(q−1)` plus the detector start-up; close refused only while latched; class 0 identical to the Stage 10 fixtures |
| Three-phase TN-style fault: a three-phase source, a star load, a `leak` on one phase to a bonded frame | line currents, the earth-fault current and the frame voltage by the independent analytical solution within `1e-6` relative, the neutral and the earth return as the resistances say |
| Prospective peak: a drawn 1 mΩ resistor between `l1` and `l2` of a purely resistive three-phase **star** source (no inductance, `n` not connected, internal resistance `Rs` in each winding), nothing else in the loop | the line-to-line RMS voltage `E_LL = √3·E` (with `E` the phase RMS) over the loop resistance `2·Rs + 1 mΩ` gives the peak current `√2·E_LL/(2·Rs + 1 mΩ)` within `1e-6` relative |
| Clearing: the same circuit with pole 1 of a class-0 breaker (`closedContactResistanceOhms` 0.0001 Ω) in the `l1` line and `instantaneousTripAmps` below the prospective peak | the breaker trips at the first sampled crossing of its threshold, all poles open together, and the current is then zero; the peak of the first row is checked on the run without the breaker, since a tripped run never reaches it |

Existing Stage 1 to 12 tolerances and fixtures stay unchanged and must pass.

## 12. Deferred

Prospective short-circuit analysis, breaking capacity, selectivity and cascading; the RCD test button and its types A, AC and B waveform behaviour; time-delayed (S-type) RCDs; the IEC test tables; TN/TT/IT classification and automatic checks; cable and busbar impedance, voltage drop, earth-loop impedance by length; arc flash and lightning; fuse curves (the Stage 4 fuse keeps its model); a touch-voltage curve over time; any claim of compliance with an installation standard. Each needs its own contract and none may be implied by the interface.

## 13. Decisions that need approval

1. **PE as a device, not a name** (section 2): an `earth_electrode` resistor to the earth node (the ground-marker reference), PE nets by containing an electrode, rails named `PE` staying labels. The alternative (a role flag on a rail) would change the project format and was not chosen.
2. **A `leak` fault with a `pinId` and a `frameId`** (section 3) to inject an insulation failure to a named exposed part without changing the schematic, rather than a new drawn part or a leak straight to the earth node (which would give the frame no voltage).
3. **The RCD model** (section 4): sum of pole currents, a mean-square detector, a no-trip zone below `I_n`, a trip time `T·(I_n/I)²`, no IEC test points, and two types of two and four poles.
4. **The breaker classes** (section 5): B, C, D as 5, 10 and 20 times the rated current and the Stage 10 dose model for the thermal part, behind an optional `curveClass` whose default keeps the Stage 10 breaker unchanged.
5. **The exposed part and the hazard** (section 7): the Stage 9 window, a 50 V default limit, a plain RMS comparison without a time curve.
6. **The wording of the teaching disclaimer** (sections 7.4 and 8) and the English interface for now; the language and the symbol standard are still decided separately.
7. **Scope of `leak`**: the parts of section 3.2, and the Stage 5 limit of one fault per component.
8. **The earth node without a ground marker** (section 2.1), an amendment of the reference rule of stage-1.md section 4 that applies only to drawings with an electrode, and the two compile-time warnings of section 8 (no run-time diagnostic).
9. **The exposed part's fixed 1 GΩ resistor** to the reference of its island (section 7.1), which keeps an unbonded frame from being a floating net; the alternative is to make an isolated exposed part an error.

## 14. Verification and ownership

- **Contract** (#196): both workstreams review this document; the decisions of section 13 are answered or recorded as deferred; ARCHITECTURE.md records the PE decision with this contract.
- **Engine** (#197 to #201), with the solver conditioning of the 1 GΩ exposed-part resistor next to 1 mΩ branches checked at #197 and #202: the earth electrode and `leak`, the RCD, the breaker classes (with the class-0 regression), and the exposed part with its hazard result, each against the fixtures of section 11.
- **References** (#202): reproducible fixtures with their regeneration commands.
- **Bridge and interface** (#203, #204): the new types, parameters, faults and results through the bridge; symbols (the RCD and the earth symbol from the owner's pictures, placeholders until then), inputs, the hazard colouring and the teaching disclaimer.
- **Assignments** (#205): three-phase fault-finding tasks use the `leak` fault and the diagnosis entry it needs.
- **Acceptance** (#206): the earth fault trips a protective device as the references say, on three platforms, with the full check list and the earlier stages intact.
