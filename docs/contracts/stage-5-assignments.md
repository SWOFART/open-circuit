# Stage 5 Contract: Assignments, Faults, and Checks

**Status:** Draft for review by the Engine and Application workstreams (issue #79).
**Scope:** An assignment file, the checks that judge a student's circuit, the faults that make a circuit misbehave, and the fault-finding task. This document extends, but does not replace, [`ARCHITECTURE.md`](../../ARCHITECTURE.md), [stage-1.md](stage-1.md), [stage-2-editor.md](stage-2-editor.md), [stage-3-dynamics.md](stage-3-dynamics.md), and [stage-4-components.md](stage-4-components.md). Where it states a rule for an earlier topic, section 1.2 lists the earlier text it amends. The fault models and the fixture references are in [../physics/models.md](../physics/models.md).

## 1. Boundaries and compatibility

### 1.1 What Stage 5 adds

- An **assignment file**: a teacher's task (text, a starting circuit, what is locked, what is checked, optionally faults), separate from the saved schematic.
- **Checks**: criteria the Engine evaluates on a student's circuit, with a structured per-criterion result.
- **Faults**: an overlay the Engine applies when it compiles a circuit, so that a correct circuit misbehaves without its schematic changing.
- A **fault-finding task**: the student investigates a faulty circuit and submits a diagnosis, which the Engine compares with the faults.

The layers keep their Stage 1 roles:

| Layer | Owns |
|---|---|
| `crates/engine` | The assignment's validation rules and types, fault validation and application, the evaluation of checks and diagnoses, and their structured results and diagnostics. Transport-independent: no Tauri, file, or UI code. |
| `crates/app` | Native file dialogs and file access for assignment files, the commands that carry requests and results, running a check away from the live simulation, and the stale-result rules. It evaluates nothing. |
| `src/` | Authoring and student interfaces, editor restrictions, and presentation of Engine results. It validates no electrical rule and compares no measured value. |

### 1.2 Earlier text this contract amends

| Earlier text | Amendment |
|---|---|
| ARCHITECTURE.md section 10: "Assignment mode, fault finding, AI explanations, collaboration, electro-pneumatics" are deliberately deferred | Assignment mode and fault finding move into scope with this contract; the rest stay deferred. ARCHITECTURE.md is updated in the same change. |
| stage-1.md section 4, **Compile**: the request is a schematic, pin positions, and a document revision | The request gains an optional `faults` list (section 5.3). An absent list is an empty list, so every earlier request keeps its meaning. |
| stage-1.md section 2 and stage-2-editor.md section 2.2: the saved project is the user-drawn schematic | Unchanged. A project file never contains faults, check results, a diagnosis, or any assignment data (section 2.6). |
| stage-1.md section 4 diagnostics table | `InvalidFault` is added (section 7). Existing codes keep their meaning. |
| stage-1.md section 4, **Catalog**: component types, pins, parameters, units, defaults, domains | Each entry gains `faultKinds` (section 6). Nothing else in the catalog changes. |

### 1.3 Out of scope

User accounts, servers, networking, LMS integration, grade books, analytics, plagiarism detection, sending or collecting submissions, AI-generated hints or explanations, collaboration, new device models, faults of relays, fuses, motors, and meters beyond parameter faults (section 5.2), contact faults, time-varying faults, and AC/phasor or semiconductor circuits.

## 2. The assignment file

### 2.1 Format

An assignment is one UTF-8 JSON document, saved with the extension `.ocassignment`. Field names are camelCase, `schemaVersion` is `1`, and unknown fields are rejected. A project is a different file (`.ocircuit`) and a project reader never reads an assignment.

| Field | Type | Rule |
|---|---|---|
| `schemaVersion` | integer | Exactly `1`. |
| `id` | stable ID | A UUID that identifies the assignment across edits and copies. |
| `title` | string | 1 to 120 characters, no control characters. |
| `instructions` | string | 0 to 8000 characters. Plain text: it is displayed as text and never interpreted as markup. Line breaks are allowed. |
| `task` | object | `{ "kind": "build" }` or `{ "kind": "findFault", "reveal": "never" \| "afterCheck" }`. `reveal` is required for `findFault` and absent for `build` (sections 2.2 and 5.7). |
| `startingCircuit` | schematic | A Stage 1 version 1 schematic. Required for `findFault`; optional (default: empty) for `build`. |
| `restrictions` | object | Section 3. Optional; the defaults are `allowedComponentTypes: null` and `lockedComponents: []`. |
| `checks` | list | The criteria of section 4. At most 64. Required (possibly empty) for `build`; absent for `findFault`. |
| `faults` | list | The faults of section 5. At most 8. Required and not empty for `findFault`; absent for `build`. |
| `reference` | schematic | The teacher's solution, used while authoring (section 2.3). Optional. |

### 2.2 Task kinds

- **`build`**: the student edits a copy of the starting circuit, and the Engine checks the result against `checks`.
- **`findFault`**: the Engine compiles the starting circuit with the `faults` applied. The student investigates with the circuit's meters and the oscilloscope and submits a diagnosis, which the Engine compares with `faults` (section 5.7).

### 2.3 The reference and the student's copy

`reference` is the teacher's solution. A teacher's file keeps it so that the authoring workflow can show that the assignment is passable: the Engine's check of `reference` against its own `checks` is expected to be `passed`, and the application shows the result when the teacher saves. A file whose reference does not pass is still a valid file.

The application exports a **student's copy**: the same file without `reference`. A student's copy is a complete assignment; nothing else is removed.

### 2.4 What the file cannot protect

The file is plain data on the student's computer. Expected values and tolerances in `checks` and the `faults` of a `findFault` task are in it, readable by anyone who opens it in a text editor, and a modified file can claim anything. Stage 5 is a classroom aid, not secure examination software, and the product says so. The contract's protections are limited to these:

- in a fault-finding task the application does not display the faults, and shows the per-entry detail of a diagnosis only as the task's `reveal` policy allows (section 5.7);
- `reference` is left out of a student's copy;
- the `locked` check (section 4.8) fails when locked parts are changed, even if the editor's restrictions were bypassed.

Two more limits are part of the design and not defects. The Engine's answer is an oracle: a student who may check as often as they like can learn what a correct answer is, in a build task from the pass and fail of each criterion (whose expected values are in the file anyway), and in a fault-finding task by submitting one diagnosis entry at a time (section 5.7). The contract keeps no attempt counter, so it cannot prevent this. And `allowedComponentTypes` is enforced by the editor only; the Engine does not check which part types a student used.

### 2.5 Validation, limits, and versioning

- The Engine validates an assignment without solving any circuit, so a file that describes a faulty or incomplete circuit still opens. It rejects, with a file error (section 7.2) that names the field by its JSON pointer: malformed JSON, a missing, malformed, or unsupported `schemaVersion` (never read as version 1), unknown fields, invalid or duplicate stable IDs (the components, wires, junctions, and markers of one schematic are unique among themselves as for projects; `startingCircuit` and `reference` are separate schematics and `reference` keeps the IDs of the starting circuit's parts, which is how the `locked` check and the reference check find them; criterion and fault IDs are unique among themselves and differ from every ID in `startingCircuit` and in `reference`), values outside a limit, a `startingCircuit` or `reference` that is not a valid version 1 schematic, and references that do not resolve (a criterion or fault that names a component that is not in the starting circuit, a `lockedComponents` entry that is not in it, or an unknown component type ID).
- Limits: the file is at most 4 MiB; `checks` at most 64; `faults` at most 8; `allowedComponentTypes` at most the number of catalog types; every `id` is a stable ID; every number is finite.
- Any later schema version needs an explicit, deterministic, sequential, tested migration, as for projects (stage-2-editor.md section 5).
- Native file access, atomic save, and leaving the open assignment untouched when a load or save fails belong to `crates/app`, on the same rules as stage-2-editor.md section 5.

### 2.6 What is never stored

Neither an assignment file nor a project file produced while working on one contains: the state the Engine computes (node values, device states, traces), a check result or diagnosis, a student's name or any other personal data, the application's window or editor state, or the time of any attempt. A student's work is saved as an ordinary project (`.ocircuit`); it holds no link to the assignment, and the application may check any open document against the assignment that is open.

## 3. The starting circuit, locked parts, and restrictions

`startingCircuit` is a schematic, so its components, wires, and markers keep their stable IDs. When a student opens the assignment the application creates a new, unsaved document from it with those IDs; the starting circuit itself is never edited in place.

```json
"restrictions": {
  "allowedComponentTypes": ["resistor", "capacitor"],
  "lockedComponents": ["00000000-0000-4000-8000-000000000001"]
}
```

- `allowedComponentTypes`: `null` (every catalog type) or a list of catalog type IDs. The palette offers only these types. Parts already in the starting circuit stay, whatever their type.
- `lockedComponents`: IDs of components in the starting circuit. A locked component cannot be deleted, and its type and every parameter value cannot be edited. Moving or rotating it and wiring to its pins are allowed.

The editor and the document store enforce both rules; the Engine's `locked` check (section 4.8) verifies the locked parts independently.

## 4. Checks

### 4.1 Criteria

A `checks` entry is a criterion. Every criterion has a unique stable `id` and an optional `label` of at most 120 characters, and exactly one `kind`:

| Kind | Fields | Passes when |
|---|---|---|
| `value` | `signal`, `atSeconds`, `expected`, `toleranceAbs`, optional `toleranceRel` | The signal's value at that public step is within the tolerance of `expected`. |
| `range` | `signal`, `fromSeconds`, `toSeconds`, `min`, `max` | At every public step from `fromSeconds` to `toSeconds`, inclusive, `min <= value <= max`. |
| `deviceState` | `componentId`, `state`, `atSeconds` | The Engine-reported state of the relay or fuse at that step equals `state`. |
| `componentCount` | `typeId`, `min`, optional `max` | The number of components of that type in the student's schematic is at least `min` and, when `max` is present, at most `max`. |

A **signal** is `{ "key": <layout slot key>, "unit": <catalog unit ID> }`. The key is exactly the key the Engine puts in its result layout (stage-1.md section 5, stage-3-dynamics.md section 7.1, stage-4-components.md section 5), so it names the component and the quantity, including the relay's six ports, a fuse's `accumulated_i2t`, and a motor's `angular_velocity`. File validation checks that the key is `<componentId>` followed by a separator and a suffix that belongs to the slot list of that component's type (which the Engine derives from the type alone), that the component is in the starting circuit, and that `unit` is the catalog unit of that slot; it solves nothing. At check time the key must exist in the student's layout; otherwise the verdict is `notEvaluated` (the student removed the part).

### 4.2 Time

Times are simulation seconds from the start of a new run, as in stage-3-dynamics.md section 3.1. Each time is a non-negative multiple of the public step (0.001 s), at most 60 s; with `n = round(t / 0.001)`, a value is a multiple when `abs(t / 0.001 - n) <= 1e-9`, and any other value is a file error. A criterion reads the state at the public step `round(t / 0.001)`, where step 0 is the initialized state. `fromSeconds <= toSeconds`.

### 4.3 Tolerance

A `value` criterion passes when `abs(measured - expected) <= toleranceAbs + toleranceRel * abs(expected)`. `toleranceAbs` is a finite number `>= 0`, `toleranceRel` (default 0) is a finite number `>= 0`, and `expected` is finite. A `range` criterion has no tolerance: its bounds are exact and inclusive, with `min <= max`.

### 4.4 Device states

`state` is one of the exact strings of stage-4-components.md section 5: `released` or `picked` for a relay, `intact` or `tripped` for a fuse. A `deviceState` criterion naming a component of another type is a file error.

### 4.5 Component counts

A `componentCount` criterion reads only the student's schematic: no circuit is compiled for it. `min` is an integer from 0 to 1000; `max`, when present, is an integer from `min` to 1000. Locked components count.

### 4.6 What the Engine does

A check takes the assignment's `checks`, `restrictions`, and `startingCircuit`, the student's schematic with the pin geometry the editor derives, and nothing else. The Engine:

1. evaluates the `locked` criterion (section 4.8) and every `componentCount` criterion on the schematic;
2. compiles the student's schematic, with no faults, in a fresh run that is independent of the run the user is watching. If an error-severity diagnostic exists, every criterion that needs a run has the verdict `notEvaluated`, and the diagnostics are returned;
3. otherwise advances that run through the public steps up to the latest step any criterion needs, and evaluates each criterion at its steps, in file order.

If the run stops early (`EventChatter` or `NonFiniteState`, stage-4-components.md section 4), criteria that need a later step are `notEvaluated` and the diagnostic is returned. The result depends only on the assignment and the schematic: not on the frame rate, the wall-clock, a trace history, or earlier checks. A check never starts a live stream and never changes the live run. Its cost is bounded by 64 criteria and 60 s of model time (60 000 public steps).

### 4.7 The result

```json
{
  "outcome": "passed" | "partial" | "failed" | "notEvaluated",
  "counts": { "pass": 2, "fail": 1, "notEvaluated": 0 },
  "criteria": [ { "id": "...", "kind": "value", "verdict": "pass" | "fail" | "notEvaluated",
                  "measured": 2.498126405, "expected": 2.5, "toleranceAbs": 0.025, "toleranceRel": 0.0,
                  "unit": "volt", "atSeconds": 0.0, "messageKey": "check.value.pass", "references": [ ... ] } ],
  "locked": { "verdict": "pass" | "fail", "messageKey": "check.locked.pass", "references": [ ... ] },
  "outcomeKey": "check.outcome.passed",
  "diagnostics": [ ... ]
}
```

- `criteria` has one entry per authored criterion, in file order, and repeats the criterion's own numbers so that a result can be shown without the file:

  | Kind | Fields besides `id`, `kind`, `verdict`, `messageKey`, `references` |
  |---|---|
  | `value` | `unit`, `atSeconds`, `measured`, `expected`, `toleranceAbs`, `toleranceRel` |
  | `range` | `unit`, `fromSeconds`, `toSeconds`, `atSeconds` and `measured` (below), `min`, `max` |
  | `deviceState` | `atSeconds`, `measured` (the state), `expected` (the state) |
  | `componentCount` | `measured` (the count), `min`, `max` (absent when unbounded) |

  `measured` is the value found and is absent when the verdict is `notEvaluated`: a JSON number for `value` and `range` (for a `range`, the value with the smallest margin to a bound, negative when outside, and `atSeconds` is its time; on a tie, the earliest step), an integer for `componentCount`, and a string (the state) for `deviceState`; the `kind` says which. Every number is finite.
- `references` are the Stage 1 diagnostic references (component IDs) that the verdict concerns.
- `messageKey` is `check.<kind>.<verdict>` (`check.value.pass`, `check.range.fail`, `check.deviceState.notEvaluated`, ...); `locked.messageKey` is `check.locked.<verdict>` and `outcomeKey` is `check.outcome.<outcome>`. User-facing prose lives in the UI.
- `outcome`, in this order of precedence: `failed` when `locked` fails (even if the circuit does not compile); otherwise `notEvaluated` when the circuit did not compile; otherwise `failed` when no authored criterion passes, `passed` when every authored criterion passes, and `partial` in every other case. A task with no authored criteria has the outcome `passed` when `locked` passes.
- There is no score, weight, or grade: only the counts and the outcome. Anything more is a later contract.

### 4.8 The `locked` criterion

Every check evaluates, without being authored, that each `lockedComponents` entry is present in the student's schematic with the type and all parameter values it has in the starting circuit. Its result is reported separately, as `locked`. A failure is shown to the student in words; it is not a compile diagnostic.

## 5. Faults

### 5.1 The fault list

A fault is `{ "id": <stable ID>, "kind": "open" | "short" | "parameter", "componentId": <stable ID>, "parameterId"?: <ParameterId>, "value"?: <number> }`. `parameterId` and `value` are required for `parameter` and absent otherwise. At most one fault applies to a component.

### 5.2 What each kind means

All faults are static for a run.

| Kind | Meaning |
|---|---|
| `open` | The component's electrical branch is removed: no current flows through it. Its result slots stay in the layout: current is `0` and voltage is `V(pin1) - V(pin2)` of the nodes it would connect, so a meter across a faulted part shows what a real meter would. |
| `short` | The component's branch is replaced by a fixed resistance of `0.001 Ohm` (1 mOhm), the same constant for every part, not a parameter. Its voltage and current slots report that resistance's values. |
| `parameter` | The parameter named by `parameterId` has the value `value` in place of the schematic's, which must be inside the catalog domain for that parameter (and satisfy the relay's cross-parameter rule). A parameter that is an initial condition is not allowed. |

The removed part's own state does not exist while it is faulted: an open or shorted capacitor or inductor has no initial condition (the Stage 3 initialization, stage-3-dynamics.md section 3.2, runs on the faulted network), and its slots show no stored energy.

Supported combinations in Stage 5:

| Component | `open` | `short` | `parameter` |
|---|---|---|---|
| `resistor`, `capacitor`, `inductor` | yes | yes | yes |
| `dc_voltage_source` | yes (no source, so no EMF and no current) | no | yes |
| `relay_spdt`, `fuse`, `pmdc_motor`, `voltmeter`, `ammeter` | no | no | yes |

A fault on a combination marked "no" is `InvalidFault`. A valid fault can still make the network unsolvable (an `open` on a source, or on the only connection of a node): the compile then fails with the existing Stage 1 and Stage 3 diagnostics (`FloatingNode`, `SingularSystem`, `InconsistentState`, ...) unchanged, and an open part whose two pins end up in different electrical islands (the part was the only link between them) reports voltage `0`, because the Stage 1 local references of two islands cannot be compared; the usual `NoGroundReference` warnings still apply. File validation does not solve anything, so it cannot catch this; the authoring workflow compiles the starting circuit with the faults and shows the result, and rejects a task whose faulted starting circuit does not compile. Faults of the relay's contacts (welded, stuck open), a blown fuse as a fault, and faults of motors and meters are deferred (points for review).

### 5.3 The overlay

Faults are an input to compile, never part of the schematic:

- The compile request has an optional `faults` list (section 1.2), validated with the rest of the request. A successful compile starts a new run as always; the faults are part of that run, and a recompile without them starts an unfaulted run.
- The Engine validates every fault before it solves: the component exists in the schematic, the kind is supported for its type (section 5.2), no component has two faults, no two faults share an ID, and a `parameter` fault names an existing, non-initial-condition parameter with a value inside its domain. Any violation is the error `InvalidFault` with a reference to the component, and compilation has no usable result.
- The saved project is never changed, and faults are never added to it (section 2.6).

### 5.4 Faults and live updates

A fault's `parameter` override wins over the schematic's value. A live `set_parameter` of an overridden parameter (stage-3-dynamics.md section 3.4) is accepted by the Engine and does not change any result, because the overriding value is not a live parameter; the application treats the schematic's value as unchanged for display. A live update of any other parameter works as for an unfaulted run.

### 5.5 Results

The result layout of a faulted run is the layout of the unfaulted schematic: the same slots in the same order, so the oscilloscope selection survives, with the values of section 5.2. Discrete states (stage-4-components.md section 5) are the states of the faulted network.

### 5.6 Faults in a build task

A `build` task has no faults. The check of section 4.6 compiles without faults.

### 5.7 The fault-finding task

- The application compiles the student's current document, which starts as a copy of `startingCircuit`, with the assignment's `faults` for every run the student sees. Nothing in the editor, the inspector, or a message names a fault.
- **Every faulted component must be in `lockedComponents`** (a file error `TaskMismatch` otherwise). A student therefore cannot delete a faulted part, change its type, or edit its parameters, so the faults cannot be probed or dodged by editing the part, and a fault can never refer to a component that is missing from the document. The student may still move parts, rewire, and add the part types `allowedComponentTypes` offers (normally meters); a student who bypasses the editor's restrictions with a modified file can see the faults anyway (section 2.4).
- The student's diagnosis is a list of at most 8 entries `{ "componentId": <stable ID>, "kind": "open" | "short" | "parameter", "parameterId"?: <ParameterId> }`; `parameterId` is required for `parameter` and absent otherwise. It carries no value.
- The Engine validates the diagnosis first and returns file-style errors (section 7.2, with a JSON pointer into the diagnosis) for more than 8 entries, a `componentId` that is not a component of the starting circuit, a missing or unneeded `parameterId`, or a `parameterId` that is not a parameter of that component's type. It then compares the diagnosis with `faults` and returns, with no circuit solved:
  - per submitted entry, a verdict: `correct` (a fault with the same component, kind, and, for `parameter`, parameter ID), `wrongKind` (a fault on that component of another kind, or of kind `parameter` on another parameter), or `incorrect` (no fault on that component);
  - per fault, a verdict: `found` (some entry is `correct` for it) or `missed`;
  - the outcome: `passed` when every fault is `found` and no entry is `incorrect` or `wrongKind`; `failed` when no entry is `correct` or `wrongKind`; `partial` otherwise. (Duplicate entries for one fault count once.)
- A diagnosis result has message keys `diagnosis.entry.<verdict>`, `diagnosis.fault.<verdict>`, and `diagnosis.outcome.<outcome>`.
- `task.reveal` governs what the application shows of the result. With `afterCheck` it shows each entry's and each fault's verdict and may then show the faults; with `never` it shows only the outcome and never the faults. The Engine's result always carries the full detail, so this is a presentation policy of the local application, not a protection. Repeated submissions are allowed and the contract keeps no attempt counter, so a student can find the faults by submitting one entry at a time; this is the oracle of section 2.4, accepted as a limit.
- A diagnostic about a faulted run (for example `FloatingNode` for a fault that is valid but makes the network unsolvable) references components as usual and can therefore point at the faulted part; the authoring workflow rejects such tasks (section 5.2), so only the student's own edits can cause it.

## 6. The application's duties

- Open, save, and export assignment files through typed commands; validate through the Engine; keep the open assignment and the open document unchanged when an operation fails.
- Run a check on its own Engine instance, away from the simulation thread, so a check never delays the live frames. A check request carries the document revision it was made for. A result is shown only while its revision is the document's current one; any other result is discarded without an error, as a stale frame is (stage-1.md section 5).
- Keep an authoring document (a teacher's) and a student's document separate from the assignment itself: editing a circuit never edits the assignment, and the assignment is saved only by an explicit save.
- Enforce the restrictions of section 3 in the editor and document store, and show the `locked` result in words when it fails.

The Engine's operations, as semantics rather than command spellings, are: **validate an assignment** (text to an assignment, or the errors of section 7.2); **compile** (stage-1.md section 4, with the optional `faults`); **check** (assignment, student's schematic with its pin geometry, and document revision, to the result of section 4.7); and **check a diagnosis** (assignment and diagnosis, to the result of section 5.7). The app carries each of them as a typed command and adds file open, save, and export of assignments.

The catalog entry of every component type gains one additive field, `faultKinds`: the subset of `open`, `short`, and `parameter` that the type supports (section 5.2), so the authoring interface reads the support table from the Engine instead of keeping a copy. The parameters a `parameter` fault may name are the entry's parameters that are not initial conditions. The Engine types (assignment, criteria, faults, diagnosis, requests, results, and file errors) are generated for TypeScript from Rust with the existing generator, and `src/generated/**` is never edited by hand.

## 7. Diagnostics and errors

### 7.1 Circuit diagnostics

Stage 5 adds one diagnostic code to the Stage 1 structure (a stable code, a severity, a localization key, and typed references), localization key `diagnostic.invalidFault`:

| Code | Severity | Meaning | References |
|---|---|---|---|
| `InvalidFault` | error | A fault names a missing component, a combination that is not supported (section 5.2), a component that already has a fault, a fault ID that is used twice, or a parameter that is missing, an initial condition, or outside its domain. | The component. |

### 7.2 Assignment file errors

A file that cannot be accepted is reported with one error per problem, each `{ "code": <code>, "path": <JSON pointer>, "messageKey": "assignment.error.<code>" }`, in document order, at most 32 of them:

| Code | Meaning |
|---|---|
| `Malformed` | Not JSON, or not an object. |
| `UnsupportedVersion` | `schemaVersion` is missing, malformed, or not `1`. |
| `UnknownField` | A field this version does not define. |
| `InvalidField` | A value of the wrong type or outside its domain or limit (including non-finite numbers and times that are not multiples of the public step). |
| `DuplicateId` | A stable ID is used twice within its space (section 2.5): two criteria, two faults, a criterion or fault that reuses an ID of the starting circuit or of the reference, or a repeated ID inside one schematic. |
| `UnknownReference` | A criterion, fault, or restriction names a component, component type, or signal that does not exist where it must (starting circuit, catalog). |
| `TaskMismatch` | The fields do not match `task.kind` (section 2.1). |
| `TooLarge` | The file exceeds a size or count limit. |

## 8. Canonical fixtures

The normative circuits and expected outcomes are in `tests/golden/stage-5-*.expected.json`; schematics and assignment files for them are added with the Engine and bridge issues under `tests/golden/stage-5-*`. All reference values come from direct circuit equations, not from Engine output.

| Fixture | Circuit and attempts | Expected |
|---|---|---|
| [Divider to specification](../../tests/golden/stage-5-divider-build.expected.json) (`build`; `value`, `range`, `componentCount`, `locked`) | Locked ideal 10 V source V1 and locked 10 MOhm voltmeter VM1 from the output node to ground; the student adds resistors. Criteria: reading 2.5 V +/- 0.025 V at 0 s, the reading in [2.4, 2.6] V from 0 to 10 ms, exactly two resistors. | Attempts: 30 kOhm over 10 kOhm reads 2.498126405 V and `passed`; the same voltage with three resistors is `partial`; two 10 kOhm resistors read 4.997501249 V and are `partial`; no resistors read 0 V and are `failed`; a changed locked source is `failed`; a wire across the source is `notEvaluated` with `ShortCircuit`, while the two schematic criteria are still evaluated. |
| [Fuse trips](../../tests/golden/stage-5-fuse-state.expected.json) (`build`; `deviceState`) | Locked ideal 2.02 V source and locked fuse (catalog defaults); the student adds a load. Criterion: the fuse is `tripped` at 2 ms. | A 1 Ohm load trips the fuse at 1 ms (`passed`); a 10 Ohm load carries 0.2018 A and leaves it `intact` at 2 ms (`failed`). |
| [Fault finding](../../tests/golden/stage-5-fault-finding.expected.json) (`findFault`; faults and diagnosis) | A 10 V ideal source, R1 = R2 = 1 kOhm in series, and a 10 MOhm voltmeter across R2, all four parts locked; the fault is R2 `open`. | The voltmeter reads 4.99975001 V healthy and 9.99900010 V with the fault; the file also gives the readings under three wrong hypotheses. Diagnoses: R2 open is `passed`; R2 short is `partial` (`wrongKind`); R2 open plus R1 short is `partial`; R1 open is `failed`; an empty list is `failed`. |

The fixture files name criteria, components, signals, and faults by short readable names instead of UUIDs and layout keys; real files use stable IDs. Tolerances are the ones in the files (`1e-9` V for readings, `1e-12` A for currents). The fault resistance for `short` is the contract's 1 mOhm.

## 9. Verification and change control

- Executable checks come with the implementation issues: the Engine's fault tests (#80), the check evaluation tests (#81), the file and command tests (#82), the interface tests (#83, #84), and the end-to-end acceptance (#85).
- A change to a field, a limit, a criterion's semantics, a verdict, a fault's meaning, a diagnostic, or a fixture value needs both workstreams' review and an update to this contract and the golden files. A change to an architectural decision also updates ARCHITECTURE.md or adds an ADR.

## Points for review

These are the choices a reviewer is asked to confirm or change:

1. **Checks name components by stable ID in the starting circuit.** A build task is judged by what locked meters and parts show, plus component counts, not by naming parts the student adds (their IDs are new). The alternative, matching a student's parts by role or label, would need a role field in the schematic.
2. **A local file protects nothing** (section 2.4). A fault-finding task and its expected values are readable in the file. The alternative (encoding them) hides nothing from a determined student and adds a format; this contract states the limit instead.
3. **No score or weights.** Only `passed`, `partial`, `failed`, and `notEvaluated`, with counts. A grade needs a decision about weighting that the stage does not need.
4. **Faults are an overlay on `compile`.** An optional field of the compile request, never saved, instead of a separate command or a schematic extension. It keeps the saved project at version 1.
5. **Fault coverage is deliberately small:** open, short, and parameter faults on passive parts and the source, and parameter faults on the Stage 4 devices. A blown fuse, a stuck relay contact, and faults of motors and meters are deferred.
6. **`short` is 1 mOhm** for every part. A shorted ideal source is not allowed: the Stage 1 `ShortCircuit` already covers a student's wire across a source.
7. **A check runs on its own Engine instance** and may take up to 60 s of model time. If it must be faster, the limit is the thing to lower, not the determinism.
8. **Locked parts may be moved and rotated**; only their existence, type, and parameters are fixed.
9. **Reveal is a UI policy** (`never`, `afterCheck`) for fault-finding tasks only, with no attempt counter, so no attempt state exists to store and the Engine's answer is an oracle (sections 2.4 and 5.7).
10. **Every faulted component must be locked** in a fault-finding task (section 5.7). It closes the obvious ways to probe a fault by editing the part, at the price that a teacher cannot let students replace the faulty part; repairing a circuit is not part of Stage 5.
11. **The `locked` result outranks the criteria and a failed compile**: a student who changes a locked part gets `failed` however well the rest passes.
