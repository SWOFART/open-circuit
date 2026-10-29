# Stage 6 Contract: Switches, Push Buttons, and Contactors (Control Circuits)

**Status:** Draft for review by the Engine and Application workstreams (issue #95).
**Scope:** A latching switch, normally open and normally closed push buttons, and a contactor with a coil and two isolated auxiliary contacts; the **operator state** that the user changes while a simulation runs; its persistence; the symbols and the interaction that operate it. This document extends, but does not replace, [`ARCHITECTURE.md`](../../ARCHITECTURE.md), [stage-1.md](stage-1.md), [stage-2-editor.md](stage-2-editor.md), [stage-3-dynamics.md](stage-3-dynamics.md), [stage-4-components.md](stage-4-components.md), and [stage-5-assignments.md](stage-5-assignments.md). Where it states a rule for an earlier topic, section 1.2 lists the earlier text it amends. The models are recorded in [../physics/models.md](../physics/models.md).

All quantities are finite SI values. The models are small and generic: they are not manufacturer parts and say nothing about ratings, endurance, or safety.

## 1. Boundaries and compatibility

### 1.1 What Stage 6 adds

- Three **operator-controlled contacts**: a latching switch (`switch_spst`), a normally open push button (`push_button_no`), and a normally closed push button (`push_button_nc`). Each is a two-terminal contact with a small closed resistance and no conduction when open.
- A **contactor** (`contactor`): a coil on pins `a1` and `a2` and two galvanically isolated auxiliary contacts, one normally open and one normally closed, on the Stage 4 relay rules.
- **Operator state**: the position of a switch or a push button, which the user changes while a run is going and which the Engine applies deterministically (section 4).

The layers keep their Stage 1 roles:

| Layer | Owns |
|---|---|
| `crates/engine` | The device models, operator state and how a change takes effect, the discrete states and their reporting, diagnostics, and the result keys. Transport-independent: no Tauri, file, or UI code. |
| `crates/app` | The command that carries a position change, its stale-run and revision rules, the frame format and its delivery. It decides nothing about what conducts. |
| `src/` | Symbols, pin geometry, the pointer and keyboard interaction that produce a request, and the presentation of what the Engine reports. It never decides whether a contact conducts. |

### 1.2 Earlier text this contract amends

| Earlier text | Amendment |
|---|---|
| stage-1.md section 4 catalog: units and numeric domains | One catalog unit is added, `dimensionless`, and one numeric domain, `zero_or_one` (the value 0 or 1). Nothing else in the catalog changes. |
| stage-3-dynamics.md section 3.4, the table of operations on a run | A row is added: `set_operator_state` (section 4.2). The other rows are unchanged. |
| stage-4-components.md section 4, event processing | An operator change that alters the circuit is processed like a relay contact transition: events are settled at that time, and the first internal step that advances model time uses backward Euler once (section 4.3). |
| stage-4-components.md section 5, the state record table of OCF2 | Three state kinds are added, and a second flag bit (section 5.2). The magic stays `OCF2`: the application and its interface are built and shipped together, and no frame is stored. A reader that does not know a kind rejects the frame, as stage-4-components.md already requires. |
| stage-2-editor.md and stage-5-assignments.md: the saved project | Unchanged in kind. A project is still a version 1 schematic, with the new types and one optional parameter added (section 4.5). It never contains operator state, a position reached while running, or a result. |

### 1.3 Out of scope

Supply rails and cabinet-style drawing (Stage 8), diodes, transistors, and extended models (Stage 7), AC/phasor analysis and multi-pole power contacts, overload relays and motor starters, contact bounce, arcing, make and break delays, welding, wear, time relays, and the use of the new devices in assignments (checks, faults, and diagnoses for them need their own contract change). Editable part names are a separate editor feature.

## 2. Catalog entries

The Engine catalog is authoritative. Type IDs, pin IDs, and parameter IDs are stable serialized strings. Parameter IDs use the existing camelCase format.

| Type ID | Ordered pin IDs and roles | Parameters (SI; domain; default) |
|---|---|---|
| `switch_spst` | `pin1`, `pin2` (unpolarized) | `closedResistanceOhms` (> 0 Ω; 0.01 Ω); `initialClosed` (optional, `dimensionless`, `zero_or_one`, an initial condition; omitted means open) |
| `push_button_no` | `pin1`, `pin2` (unpolarized) | `closedResistanceOhms` (> 0 Ω; 0.01 Ω) |
| `push_button_nc` | `pin1`, `pin2` (unpolarized) | `closedResistanceOhms` (> 0 Ω; 0.01 Ω) |
| `contactor` | `a1` (coil, positive reference), `a2` (coil, negative reference), `no_1`, `no_2` (normally open contact, unpolarized), `nc_1`, `nc_2` (normally closed contact, unpolarized) | `coilResistanceOhms` (> 0 Ω; 100 Ω), `coilInductanceHenries` (> 0 H; 0.01 H), `pickupCurrentAmps` (> 0 A; 0.04 A), `dropoutCurrentAmps` (≥ 0 A; 0.02 A), `closedContactResistanceOhms` (> 0 Ω; 0.1 Ω) |

Defaults are reproducible educational values, not ratings or manufacturer specifications. The contactor additionally requires `dropoutCurrentAmps < pickupCurrentAmps`, as the relay does, checked at compile and update boundaries. `initialClosed` is an initial condition: changing, setting, or clearing it needs a compile (Stage 3 section 3.4). A push button has no initial position: it is released at the start of every run.

`dimensionless` and `zero_or_one` exist so that the catalog describes the one two-valued parameter; the interface shows a parameter with the domain `zero_or_one` as a choice (open or closed), not as a number field. A value other than 0 or 1 is `InvalidParameter`.

## 3. Component models

### 3.1 Switch and push buttons

A contact is in one of two **positions**:

| Type | Positions | Conducts when |
|---|---|---|
| `switch_spst` | `open`, `closed` | `closed` |
| `push_button_no` | `released`, `pressed` | `pressed` |
| `push_button_nc` | `released`, `pressed` | `released` |

When it conducts, the branch is a resistor of `closedResistanceOhms`: `i = (V(pin1) - V(pin2)) / R`, current positive entering `pin1`. When it does not conduct, the branch is removed from the circuit: no current flows, and its pins keep a place in the netlist, as an open part does in Stage 5 (stage-5-assignments.md section 5.2). The voltage reported for an open contact is `V(pin1) - V(pin2)` of the nodes it would connect when both lie in one solved island, and `0` otherwise, exactly as for an open fault there. Leakage, contact capacitance, rated current and voltage, and any dependence on time are omitted.

The **position** is operator state, an input of the run: the Engine does not derive it, and no solver result changes it. The initial position of a switch is `initialClosed` (open when omitted); a push button starts `released`. Which position conducts is the Engine's knowledge: the interface reads `conducting` from the result (section 5.2), and never infers it from a type or a position.

### 3.2 Contactor

The coil is the relay's series resistance and inductance (stage-4-components.md section 3.1), between `a1` and `a2`: `v_coil = V(a1) - V(a2)`, `i_coil` positive entering `a1`, thresholds on `abs(i_coil)`, with the relay's states `released` and `picked`, its hysteresis (`released → picked` when `abs(i_coil) >= pickupCurrentAmps`, `picked → released` when `abs(i_coil) <= dropoutCurrentAmps`), and its initial settling at time zero.

The two contacts are **independent and isolated** from the coil and from each other. In `released` the `nc_1`–`nc_2` branch has `closedContactResistanceOhms` and the `no_1`–`no_2` branch is open; in `picked` the reverse. Transitions are atomic at their event time, and contact travel, bounce, arcing, and delays are not modeled. A contactor has no power poles in this stage: its contacts are rated by nothing, and the model is the same whether they switch a control circuit or a load.

Unlike the relay's `common`, no pin is shared between the two contacts, so a contactor can switch two separate circuits. Event processing, the same-time re-solve limit, `EventChatter`, and the determinism rules are those of stage-4-components.md section 4, with the contactor counted among the stateful components.

## 4. Operator state

### 4.1 What it is

Operator state is the position of each switch and push button in the active run. It is:

- **not a schematic parameter** (the saved `initialClosed` is the initial position only, section 4.5);
- **not part of the document history**: changing it never adds an undo entry, never marks the document as modified, and is not undone by undo;
- **owned by the run**: a successful compile starts every position at its initial position (open or `initialClosed` for a switch, released for a push button), and nothing carries over from an earlier run.

A run is therefore a deterministic function of the compile request and the ordered list of the operator requests it accepted, each with the public step that was the newest when it was received (the simulation thread serializes requests, so that step is also the one it is applied to). The same requests applied at the same steps give bit-identical results.

### 4.2 The request

The application carries a position change with one command, `set_operator_state`, which follows `set_parameter` (stage-3-dynamics.md section 3.4, stage-1.md section 5):

```text
SetOperatorStateRequest {
  runId, expectedDocumentRevision, documentRevision,
  componentId, position: open | closed | released | pressed
}
SetOperatorStateResult =
    success { snapshot, internalSubsteps, diagnostics }
  | failure { diagnostics }
```

`expectedDocumentRevision` must equal the run's current revision, and `documentRevision` is the new revision the run adopts. The revision is an opaque token compared for equality: an operator change is not a document edit, but it uses the same token so that a frame computed before the change can never be shown after it. The application generates a new token for every request, as it does for a parameter update.

The Engine refuses the request, leaving the run, its positions, and its revision unchanged, with:

| Cause | Diagnostic |
|---|---|
| `runId` is not the active run | `StaleRun` |
| `expectedDocumentRevision` is not the run's revision | `StaleDocumentRevision` |
| `componentId` is not a switch or a push button of the run's circuit, or `position` is not a position of its type (`closed` for a push button, `pressed` for a switch) | `InvalidParameter`, referencing the component |
| The change leaves discrete events that cannot settle at that time, or makes a device state non-finite | `EventChatter` or `NonFiniteState`, referencing the components, as in a compile. The run, the positions, and the revision stay as they were (the change is all or nothing, like a parameter update) |

A request for the position the device already has succeeds without a change: the run adopts the new revision, nothing is recomputed, and no transition time is recorded. Requests are applied in the order the simulation thread receives them, and the application never merges or drops two requests for one device (a press followed by a release is two requests). If both arrive while the same step is the newest, the release is applied from the same start state with the same newest step and the press has no effect on any sample; the interface therefore does not send a release for a button it has not pressed, and a click that is shorter than one public step may leave no trace.

### 4.3 When a change takes effect

Exactly as a live parameter update (stage-3-dynamics.md section 3.4): if the latest snapshot is step `N`, the change takes effect from the **start of the current public step**, the span from `N - 1` to `N`. The Engine keeps the state at the end of step `N - 1`, applies the new position, settles discrete events at that time as stage-4-components.md section 4 says (a relay or contactor may change state at once), computes step `N` again from it, and chooses the number of internal substeps again. The first internal step that advances model time after a change of the circuit uses backward Euler once. This is a rule for operator changes that the Engine implements in Stage 6; today's `set_parameter` does not apply it, and the implementation issues must not read "exactly as a live parameter update" as covering it. At step 0 there is no earlier state, and the initialization of stage-3-dynamics.md section 3.2 is run again with the new position.

The reply and the next frame carry the new revision, the same `simulationStep`, and the recomputed values. Earlier steps never change, and the recomputed step replaces the sample of that step in the trace history (stage-3-dynamics.md section 7.5). The position is part of the recomputed snapshot, with the model time of the change recorded as the device's `lastTransitionTimeSeconds` (the time at which the new position conducts: `(N - 1) × 0.001 s`, and `0` at step 0, where there is no earlier step).

A contact always has a finite closed resistance, so a change of position cannot by itself create a conflict between ideal sources and cannot make a circuit singular in the Stage 1 sense; closing a switch across an ideal source is a large but finite current, and is accepted. At step 0 the re-initialization can still fail as a compile can, and a change at any step can be refused for the discrete-state reasons in the table above. A circuit in which a contactor opens the very contact that feeds its own coil has no stable state, and the contract defines no result for it: the Engine's behavior there (settling in one state at a time-zero solve, or switching between states within a public step) is not a model to rely on, and no fixture uses it. In every refusal the run is unchanged and the interface restores the position it last read from a frame.

### 4.4 Operator state, compile, and the document

| Operation | Effect on operator state |
|---|---|
| Successful compile | A new run: every position at its initial position, a new trace history, no earlier request applied. |
| Failed compile | The previous run and its positions are untouched. |
| `set_parameter` | The run's positions are unchanged. A changed `closedResistanceOhms` takes effect as any live parameter does, and the position stays. |
| `set_parameter` of `initialClosed` | Refused with `InvalidParameter`; the application compiles instead, which restarts the run at the new initial position. |
| Any schematic edit that compiles | A new run, so every position returns to its initial position. This includes moving a part, because any edit other than a live parameter update compiles. |
| Undo and redo | They change the document and therefore compile; they never replay an operator change. |
| New, Open, Open Example | A new document and a new run. |

The application shows the position it was last told by the Engine, and nothing else. It never keeps its own copy that it could show for a run it does not belong to.

### 4.5 Persistence

The project file stays a version 1 schematic (stage-2-editor.md section 2.2). Stage 6 changes it only by allowing the four new component types, and the optional `initialClosed` parameter of a switch. An older reader rejects a file that uses a new type with its existing "unknown component type" diagnostic and never reads it as something else; reviewers should confirm this choice (see "Points for review").

A project file never contains: the position reached while running, whether a button is held, a contact's conduction, a contactor's state or last transition, or any result or trace. A switch drawn closed is saved as `initialClosed: 1`; a switch drawn open has no `initialClosed`. A push button has no saved position at all.

## 5. Results, discrete state, and frame format

### 5.1 Numeric results

A switch and a push button expose the standard two slots, `<componentId>:voltage` then `<componentId>:current`, with the pin-1/pin-2 sign convention. An open contact reports current `0`. The contactor exposes six slots in catalog port order, with the keys and sign conventions below; the first listed pin is the positive reference and the current is positive entering it:

| Key | Quantity | Reference pins |
|---|---|---|
| `<componentId>/coil/voltage`, `<componentId>/coil/current` | voltage, current | `a1`, `a2` |
| `<componentId>/no/voltage`, `<componentId>/no/current` | voltage, current | `no_1`, `no_2` |
| `<componentId>/nc/voltage`, `<componentId>/nc/current` | voltage, current | `nc_1`, `nc_2` |

A result layout never contains a discrete state as a floating-point value.

### 5.2 Discrete state

The snapshot's typed state records (stage-4-components.md section 5) gain three kinds, in the same canonical component-ID order as the relay and fuse records:

```text
SwitchState     { componentId, position: open | closed,       conducting, lastTransitionTimeSeconds? }
PushButtonState { componentId, position: released | pressed,  conducting, lastTransitionTimeSeconds? }
ContactorState  { componentId, state: released | picked,                  lastTransitionTimeSeconds? }
```

`conducting` is the Engine's statement of whether the contact is closed. `lastTransitionTimeSeconds` is absent until the first change of the position (a transition from an operator request that changed it), and otherwise the model time of the latest one; a contactor's is the exact event time, as for a relay.

In the binary state record of OCF2 (32 bytes, as before), the `kind` codes are `1` relay, `2` fuse, `3` switch, `4` push button, `5` contactor. `value` is `0` for open, released, or intact and `1` for closed, pressed, picked, or tripped. `flags` bit 0 means a transition time is present, and bit 1 (new) means `conducting`; bit 1 is defined only for kinds `3` and `4` and must be `0` for every other kind. All other flag bits, nonzero reserved bytes, an unknown kind or value, and the other conditions of stage-4-components.md section 5 reject the whole frame. A reader also verifies that each record references exactly one stateful component of the active schematic of the matching kind and that none is missing.

[`tests/golden/stage-6-frame.json`](../../tests/golden/stage-6-frame.json) is an independently encoded OCF2 example with two numeric values and one record of each new kind. The codec implementation must validate its bytes exactly.

Compile and `set_operator_state` results carry the same typed records as their snapshot, and the TypeScript types are generated from Engine-owned Rust types.

### 5.3 Traces

Trace history stays limited to numeric result-layout slots (section 5.1). The position of a device is not traced; the effect of a change appears in the numeric slots, replacing the sample of the step it was applied in (section 4.3).

## 6. Symbols, labels, and interaction

### 6.1 Symbols

Symbols follow the electrical-schematic convention (graphical symbols as in IEC 60617): a switch is a contact with a lever, a push button a contact with a push actuator, normally open and normally closed drawn differently, and the contactor a coil box with pins A1 at the top and A2 at the bottom beside two contact symbols marked with the terminal pairs conventionally used for auxiliary contacts, 13–14 (normally open) and 21–22 (normally closed). The coil and the contacts of one contactor are one component, drawn together, with a dashed mechanical link between them.

The pin positions in the symbol's local grid (x right, y down, rotated by the Stage 1 rules) are the only geometry the Engine needs:

| Type | Local pin positions |
|---|---|
| `switch_spst`, `push_button_no`, `push_button_nc` | `pin1` (-1, 0), `pin2` (1, 0), as the other two-terminal parts |
| `contactor` | `a1` (-2, -2), `a2` (-2, 2), `no_1` (2, -2), `no_2` (2, 2), `nc_1` (5, -2), `nc_2` (5, 2) |

The symbol shows the position and the state the Engine reported, so that a closed contact, a pressed button, and a picked contactor can be told apart on the drawing. Before the first result, and when a compile fails, the symbols are drawn at rest.

### 6.2 Part names

The editor's label prefixes, which are editor state and not saved, are `S` for switches and push buttons and `Q` for contactors (`K` stays for relays), so that a contactor reads `Q1` and a push button `S1`; the dash of the drawing convention (`-Q1`) is not part of the editor's label.

### 6.3 Operation

The interface sends requests; it does not decide what conducts. With the select tool, whenever the Engine holds a run for the document (the live stream may be paused):

- **Switch:** a click on the lever toggles the position, by requesting the other position.
- **Push button:** pressing the pointer on the actuator requests `pressed`, and releasing the pointer, leaving the actuator while pressed, cancelling the pointer, losing focus, and pressing Escape while it is pressed each request `released`, so a button never stays pressed after the user has let go.
- **Keyboard:** a focused device is operated with Space or Enter (a switch toggles on key down; a push button is pressed on key down and released on key up or when focus is lost; the repeat events of a held key are ignored, so a held key neither toggles a switch again nor presses a button again). Every device has an accessible name (its part name) and its position and state in words.
- The lever or actuator is the operating area; the rest of the symbol selects and moves the part as before, and a drag that starts on the body never operates it.
- Requests for one device are sent in order: a release that follows a press waits behind it and is never dropped. A refused or failed request restores the displayed position to the one the Engine last reported, and says why in words; the document stays editable. When the outcome of a request is unknown (no reply, or a transport failure), the interface does not guess: every frame carries the state of every device (section 5.2), so the next frame of the run shows the true positions, and no compile is needed (a compile would reset every position).

A change of the circuit that starts a new run (section 4.4) shows every device at its initial position, because the Engine's first snapshot of the new run says so.

## 7. Diagnostics

No diagnostic code is added. The codes in section 4.2 keep their earlier meanings. `InvalidParameter` is used for a parameter outside its domain (including `initialClosed` other than 0 or 1), the contactor's cross-parameter rule, and a request that names something that cannot be operated or a position its type does not have. `EventChatter` and `NonFiniteState` apply to the contactor as to the relay.

### 7.1 Message keys

The Engine sends stable keys and codes, and the interface owns the prose (as in the earlier contracts). The new catalog entries carry these keys, in the existing `component.<type>.<name>` form:

| Type | `displayNameKey` | `descriptionKey` of its parameters |
|---|---|---|
| `switch_spst` | `component.switchSpst.name` | `component.switchSpst.closedResistance`, `component.switchSpst.initialClosed` |
| `push_button_no` | `component.pushButtonNo.name` | `component.pushButtonNo.closedResistance` |
| `push_button_nc` | `component.pushButtonNc.name` | `component.pushButtonNc.closedResistance` |
| `contactor` | `component.contactor.name` | `component.contactor.coilResistance`, `.coilInductance`, `.pickupCurrent`, `.dropoutCurrent`, `.closedContactResistance` |

Diagnostics keep their `diagnostic.<code>` keys. The positions and states in a result (`open`, `closed`, `released`, `pressed`, `picked`) and `conducting` are stable values, not keys: the interface turns them into words, including the words "open" and "closed" for the choice that a `zero_or_one` parameter is shown as, the accessible position and state of each device, and the sentence that says why a request was refused.

### 7.2 Generated types and the catalog

The following are Engine-owned Rust types, generated for TypeScript with the existing generator (`src/generated/**` is never edited by hand): the additions to `ComponentTypeId`, `PinId`, and `ParameterId`; `Unit` with `dimensionless`; `NumericDomain` with `zero_or_one`; `OperatorPosition` (`open`, `closed`, `released`, `pressed`); `SetOperatorStateRequest` and `SetOperatorStateResult`; and the `switch`, `pushButton`, and `contactor` variants of `DeviceState` with their `SwitchPosition`, `PushButtonPosition`, and contactor state values. The catalog entries of the four new types list no fault kinds: `faultKinds` is empty, and a fault on one of them is an `InvalidFault` (stage-5-assignments.md section 5.2) until a later contract adds some. The live numeric parameters of the new types, which follow the `set_parameter` rules of stage-3-dynamics.md section 3.4, are `closedResistanceOhms` and `closedContactResistanceOhms`, `coilResistanceOhms`, `coilInductanceHenries`, `pickupCurrentAmps`, and `dropoutCurrentAmps` (as for the relay); `initialClosed` is not live.

## 8. Canonical known-answer fixtures

Expected data is kept separately from the schematic inputs. The values are direct circuit equations and closed-form solutions, not Engine output. A fixture lists, for each operator request, **the newest step when it was sent** and the step from which it takes effect (section 4.3), and the expected sample of each step shown.

| Fixture | Circuit and operator events | Expected check |
|---|---|---|
| [Switch in a divider](../../tests/golden/stage-6-switch-divider.expected.json) | An ideal 10 V source, a switch (0.01 Ω closed), and two 1 kΩ resistors in series; the switch starts open; it is closed when step 3 is the newest and opened again when step 6 is. | Open: 0 V and 0 A. Closed: 4.999975000125 V across the second resistor and 4.999975000125 mA. The sample of the step in which a request was sent already shows its effect. |
| [Normally closed push button holding a capacitor](../../tests/golden/stage-6-button-capacitor-hold.expected.json) | An ideal 10 V source, a normally closed button (0.01 Ω), 1 kΩ, and a 100 µF capacitor from 0 V. The button is pressed when step 50 is the newest and released when step 100 is. | The capacitor follows `10 (1 − e^(−t/τ))` with `τ = 0.100001 s`, holds the value at 49 ms with no current while the button is open, and charges on from it after the release (closed-form values in the file, tolerance 5 mV). |
| [Start/stop circuit latching through a contactor](../../tests/golden/stage-6-start-stop-latching.expected.json) | An ideal 5 V source, a normally closed STOP button, the parallel pair of a normally open START button and the contactor's NO contact, and the contactor coil to ground. START is pressed at step 5 and released at 10, STOP pressed at 15 and released at 20. | Released with no current until START; picked with 49.9905 mA while START is held, picked with 49.9451 mA after it is released (held through the contact), released with 0 A after STOP, and still off afterwards. One wired-wrong variant, run with the same operator events: with no seal-in contact wired, the circuit picks while START is held and releases as soon as it is released. Tolerance 0.1 mA. (Wiring the contactor's NC contact in parallel with START has no stable state, section 4.3, and is not a fixture.) |

The relay-style timing of the contactor in the third fixture is covered by the equations: the coil's `L/R` time constant is 0.1 ms, far inside one public step, so each listed sample is the settled value, and a coil current above the pickup threshold picks the contactor within the step.

## 9. Layer boundaries and verification

- The Engine validates the parameters, derives connectivity, solves the equations, owns operator state, positions, and discrete states, creates the result layout and the frames, and emits structured diagnostics.
- The application owns `set_operator_state`, its stale-run and revision handling, and the frame delivery. It calculates nothing about a device.
- TypeScript owns the symbols, the pin geometry, the pointer and keyboard interaction, and the presentation of what the Engine reports. It uses the generated types.
- Executable checks come with the implementation issues: the Engine's device tests (#96, #97), the bridge tests (#98), the interface tests (#99), and the end-to-end acceptance (#100).
- A change to a field, a position, a limit, a model equation, a default, a state or frame code, a fixture value, or the interaction rules needs both workstreams' review and an update to this contract and the golden files. A change to an architectural decision also updates ARCHITECTURE.md or adds an ADR.

## 10. Primary references

- IEC 60617, graphical symbols for diagrams: the switch, push button, contact, and coil symbols (the standard itself is not reproduced here).
- The IEC 60947 series (low-voltage switchgear and controlgear) and the related European standards for terminal marking: the 13–14 and 21–22 convention for auxiliary contacts (named here as the convention only; the standards are not reproduced).
- IEC 60947-4-1, contactors and motor-starters (electromechanical): the contactor as a switching device with a coil and contacts, for the terms used above.
- The relay and event rules of [stage-4-components.md](stage-4-components.md) and its references.

These references support the vocabulary and the symbols. They do not set the generic defaults in this contract.

## Points for review

These are the choices a reviewer is asked to confirm or change:

1. **Operator state is not a parameter and not in the history.** A position is an input of a run, reset by every compile, and never an undo entry. The alternative, a position stored in the document and edited like a parameter, would recompile on every click and make a held button an edit.
2. **Only the initial position of a switch is saved,** as the optional parameter `initialClosed`. A push button saves nothing. Saving the running position was rejected because a project would then reopen in a state the user did not draw.
3. **The revision token is reused.** An operator change takes a new revision token, as a parameter update does, so no frame computed before it can be shown after it. The alternative, a separate operator counter, would add a second stale-frame rule for no gain.
4. **One command for both kinds,** with position words per type, instead of a boolean "closed". A boolean would make the interface decide what "pressed" means for a normally closed button; the Engine reports `conducting` instead.
5. **A refused change is an ordinary refusal.** A change the Engine cannot settle (`EventChatter`, `NonFiniteState`) fails like the same circuit would at compile: the run goes on and the interface restores the displayed position. The alternative, accepting it and stopping the run, as a chatter during a run does, would turn a student's wiring mistake into a lost run. The cases that are certain today are a stale run or revision and a request that names something that cannot be operated; a circuit that settles only by luck (a coil fed through its own normally closed contact) is left undefined, section 4.3.
6. **A press is always followed by a release request.** The interface releases a button on pointer up, leave, cancel, blur, and Escape; the Engine has no timeout. A release lost in transport would leave a button pressed until the next compile, so the application queues the release behind the press and never merges the two.
7. **The contactor has two isolated auxiliary contacts and no power poles.** It is the relay model with isolated contacts, enough for control circuits. Multi-pole power switching belongs with AC and the supply rails of Stage 8.
8. **New state kinds in OCF2.** The magic stays `OCF2` because the application and the interface ship together and frames are never stored. A new magic would be needed only if a reader of an older version could meet a newer frame.
9. **A new unit and a new numeric domain** (`dimensionless`, `zero_or_one`) for the one two-valued parameter. The alternative, a boolean parameter type, would change the numeric-only catalog and the file format for every part.
