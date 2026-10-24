# ARCHITECTURE.md

An open-source educational electrical circuit simulator (a FluidSIM alternative).
Stack: **Rust (physics) + TypeScript/React/PixiJS (editor) + Tauri 2 (desktop)**.
Repository: **a single monorepo** (Cargo workspace + pnpm).

Document status: living. Any change to an architectural decision is recorded
here (or in `docs/adr/`) first, then in code.

---

## 0. Core principles

1. **Physics lives in one place (`crates/engine`) and is verified independently of the UI.**
   Code is partly written by AI, so physics bugs look plausible.
   Tests must catch them, not eyeballs.
2. **One source of truth for every fact.** A device's electrical behavior: Rust.
   A device's drawing: TS. File format: the drawn schematic plus a version, never computed values.
3. **Every physical model is accepted only together with a test against a known answer.**
4. **Layer boundaries are strict.** `engine` knows nothing about Tauri or the UI.
5. **Simple first.** Fixed time step, dense matrices, simple wire routing.
   We add complexity only when it is measured and needed.

---

## 1. Three layers

```
┌─────────────────────────── Tauri application ──────────────────────────┐
│  UI (TypeScript)                  Bridge (crates/app)    Engine (Rust) │
│  ┌──────────────┐  commands       ┌────────────┐        ┌───────────┐  │
│  │ React panels │ ─────────────▶  │ Tauri cmds │ ─────▶ │  engine   │  │
│  │ EditorCore   │                 │ sim thread │        │ (pure     │  │
│  │ (PixiJS)     │ ◀───────────────│ Channel    │ ◀───── │  Rust)    │  │
│  └──────────────┘  frames ~60 Hz  └────────────┘        └───────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

| Decision | Why | Cost |
|---|---|---|
| `engine` does not depend on Tauri/UI | `cargo test` in seconds, CLI, CI, optional WASM later | A thin adapter layer (`app`) |
| UI stores the schematic as a drawing, Rust treats it as a circuit | Connectivity detection is part of the physics; a bug here yields wrong voltages with no error message | Rust receives raw components and wires |
| Monorepo | A data-format change touches Rust and TS at once: one commit, one CI run | Requires boundary discipline (see section 9) |

---

## 2. Repository structure

```
circuit-sim/
├── Cargo.toml                  ← workspace
├── package.json                ← pnpm workspace
├── pnpm-workspace.yaml
├── rust-toolchain.toml         ← pinned Rust version
├── .github/
│   ├── workflows/ci.yml
│   ├── CODEOWNERS
│   └── pull_request_template.md
├── crates/
│   ├── engine/                 ← physics (pure Rust, no Tauri)
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── schematic.rs    ← input format: components + wires
│   │   │   ├── netlist.rs      ← connectivity, nodes, per-wire currents
│   │   │   ├── devices/        ← one file per device type
│   │   │   │   ├── mod.rs      ← Device trait, catalog
│   │   │   │   ├── resistor.rs
│   │   │   │   ├── source.rs
│   │   │   │   ├── lamp.rs
│   │   │   │   ├── switch.rs
│   │   │   │   ├── relay.rs
│   │   │   │   └── motor.rs
│   │   │   ├── mna.rs          ← builds the G·x = b system
│   │   │   ├── solver.rs       ← LU behind a trait (replaceable)
│   │   │   ├── transient.rs    ← time stepping, integration
│   │   │   ├── newton.rs       ← iterations for nonlinear devices
│   │   │   ├── events.rs       ← relays, switches, burn-outs
│   │   │   ├── diagnostics.rs  ← short circuit, floating node, etc.
│   │   │   └── snapshot.rs     ← per-step result + frame layout map
│   │   └── tests/              ← integration + proptest
│   ├── engine-cli/             ← headless runner, SPICE export
│   └── app/                    ← Tauri: commands, sim thread, Channel
├── src/                        ← frontend
│   ├── editor/                 ← EditorCore (no React), tools
│   ├── symbols/                ← IEC 60617 symbols (SVG) + pin positions
│   ├── ui/                     ← React: panels, inspector, palette
│   ├── store/                  ← UI state
│   └── generated/              ← types generated from Rust (DO NOT edit by hand)
├── tests/golden/               ← reference circuits (.json) + ngspice results
├── docs/
│   ├── adr/                    ← architecture decision records
│   ├── contracts/              ← versioned cross-layer contracts
│   │   └── stage-1.md
│   └── devices.md              ← how to add a new device
├── ARCHITECTURE.md
├── CONTRIBUTING.md
└── LICENSE                     ← GPL-3.0-or-later
```

**Dependency rule (enforced in CI):**

```
src/ (TS)  ──▶  crates/app  ──▶  crates/engine
                                       ▲
                 crates/engine-cli ────┘
```

- `engine` imports nothing from `app`, `engine-cli`, or `src/`.
- `src/` talks to Rust only through generated types and Tauri commands.
- `src/editor/` does not import React.

---

## 3. Engine

### 3.1 Data and behavior are separate

```
ComponentSpec (enum, serializable)  ──compile──▶  Box<dyn Device>
 Resistor { ohms }                                 stamp(), update_state()
 Lamp { volts, watts }
 Relay { coil_ohms, pull_in, drop_out, ... }
```

- The schematic file stores only `ComponentSpec`: simple and versionable.
- Behavior is created when the schematic is "compiled", so models can change without breaking old files.
- A `Device` trait instead of one giant `match`: **one device = one file**, the solver is never edited.

### 3.2 Method: modified nodal analysis (MNA)

Ohm's law and Kirchhoff's laws are assembled into `G·x = b`. Each device "stamps"
its contribution into the matrix. This is the method used by SPICE and Falstad, so
results can be compared against ngspice. There are no separate formulas for
"series/parallel sections": they break on bridges and relays.

### 3.3 One mode: transient analysis

There is no separate DC/AC mode. DC is simply a source that does not change.
Initial state: operating point, then time steps. Lamps warm up, relays click,
motors spin up. A phasor/AC-analysis mode can be added separately later.

### 3.4 Time and integration

- C and L: **companion models** (resistor + source), trapezoidal rule.
- After an event (relay, switch), take one **backward Euler** step: otherwise the trapezoidal rule produces spurious ringing.
- Start with a fixed physics step of **1 ms** (`timeStepSeconds = 0.001`), with sub-steps only if needed. "Slow down 100x" is the ratio of model time to wall-clock time, not a different step.

### 3.5 Nonlinear elements

Newton–Raphson inside each step: limit voltage jumps across junctions, with a fallback
(gradually added conductance to ground) if convergence fails.

### 3.6 Events: relays, burn-out, fuses

Non-smooth behavior lives in state machines around the solver:

1. step solved → each device gets `update_state(solution, dt)`;
2. a relay compares coil current against thresholds (hysteresis) and switches its contacts;
3. a lamp accumulates heat, a fuse integrates ∫I²dt;
4. a state change → rebuild the matrix and repeat the step at the same point in time (with a retry limit against endless chattering).

### 3.7 Motor

An electromechanical pair. Angular velocity is an additional unknown **inside** the
equation system: coupling through the previous step explicitly oscillates.

### 3.8 Unrealistic circuits and diagnostics

- Sources have a default internal resistance (e.g. 0.1 Ω): a short circuit produces a large current, not `NaN`. More physical, and it is exactly what students should learn.
- Structured diagnostics: `ShortCircuit`, `FloatingNode`, `NoGroundReference` (ground is added automatically). The UI highlights the offending parts.

### 3.9 Wire currents (for animation)

The solver knows device currents but not the current of an individual wire segment.
`netlist.rs` builds a spanning tree per node and distributes pin currents using
Kirchhoff's current law. This is engine math, not UI. The UI only draws moving dots.

### 3.10 Units

SI everywhere in `f64` (V, A, Ω, F, H, s). Prefixes (kΩ, µF) only in UI input/display.

---

## 4. Rust ↔ TS bridge

```
UI ──commands──▶ [queue] ──▶ simulation thread (owns Engine)
                                   │ ~16 ms
                                   ▼
                        latest-snapshot buffer
                                   │
        Tauri Channel (binary) ────┘──▶ UI (Float64Array)
```

- Simulation runs in a **separate thread**; the UI never blocks physics.
- Only the **latest** snapshot is kept: a lagging UI skips frames instead of building a queue.
- UI delivery is throttled independently to at most **60 frames per wall-clock second** (nominally 16.67 ms). It never changes the 1 ms physics step; a frame may contain a later simulation step than the previous frame.
- A frame is a binary `f64` array with a fixed layout. When a schematic is compiled, the UI receives a "component → offset" map once. No JSON per frame.
- Rust → TS types are generated automatically (`tauri-specta` or `ts-rs`). CI checks that `src/generated/` is up to date.
- `set_param(id, value)`: a lightweight command that only rebuilds stamps.
- Topology change: fast recompile with **state transfer** (capacitor charge, relay position) keyed by component ID.

> **Verify in the prototype (step 1):** how well binary transfer through the Tauri 2 `Channel`
> works in the chosen version. Fallback: a shared buffer via a command returning `Vec<u8>`/`ArrayBuffer` on a timer.

---

## 5. Frontend

- **`EditorCore`** is a plain TS class using PixiJS, with no React. React handles panels only. Reasons: performance with thousands of objects, and editor logic can be tested in Vitest without a browser.
- **Immutable document, history via `immer`** (patches and inverse patches): undo/redo works automatically for any change, with no hand-written do/undo pairs.
- **Tools are finite state machines** (select, wire, place, pan) with explicit states. "Comfortable drawing mechanics" is hundreds of small behaviors; without state machines they become a tangle of flags.
- **Symbols:** IEC 60617 (DIN), with a toggle for ANSI. They live in `src/symbols/`.
- **Wires:** orthogonal routing (L/Z routes), junctions created automatically, "rubber-band" wires while dragging. Spatial index via `rbush`. A* routing is deferred.
- **Device catalog** comes from Rust at startup; the property inspector is generated from it automatically (units, ranges, defaults).

---

## 6. File format

JSON with `schemaVersion` and migration functions (`migrations/` in `engine`).
Only what was drawn is stored: components, parameters, wires, junction/reference markers,
and positions.
**Nodes, currents, and simulation state are not saved**; they are computed.
Export to a SPICE netlist (for ngspice and interchange).

The Stage 1 schematic, connectivity, Engine API, diagnostics, and result-layout
contract is specified in [`docs/contracts/stage-1.md`](docs/contracts/stage-1.md).
That document elaborates this architecture for the first end-to-end milestone;
it must not override the layer boundaries or physics decisions above.

---

## 7. Testing

| Level | Where | What it checks |
|---|---|---|
| Unit | `crates/engine` | Ohm's law, divider, bridge, RC time constant |
| Property-based (`proptest`) | `crates/engine/tests` | Sum of currents at a node = 0; power delivered = power consumed |
| Golden | `tests/golden/` | ngspice results on the same circuits, tolerance set in the file |
| Editor | `src/**/*.test.ts` (Vitest) | Tool state machines, undo/redo, snapping |
| Contract | `crates/app` + `src/` | Every device type in Rust has a symbol with the same pin count |
| Build | CI | Windows, Linux, macOS |

**Rule:** a model without a known-answer test is not accepted in a PR.

---

## 8. CI (minimum to start)

One workflow `ci.yml`, matrix `windows-latest`, `ubuntu-latest`, `macos-latest`:

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `pnpm install --frozen-lockfile`
5. `pnpm lint` and `pnpm typecheck`
6. `pnpm test` (Vitest)
7. Check that `src/generated/` is current (regenerate and `git diff --exit-code`)
8. Tauri build (no publishing)

Also: a dependency-rule check (`engine` must not pull in Tauri), e.g. via `cargo tree`.

---

## 9. Working in one repository

### 9.1 Branches and commits

- `main` is always green and protected: merge only via PR with passing CI.
- Branches: `feat/…`, `fix/…`, `docs/…`, `refactor/…`.
- Short-lived branches, small PRs. One PR = one idea.
- Conventional Commits with a scope:
  `feat(engine): add RC companion model`, `fix(editor): snap wire end to pin`,
  `docs(arch): clarify event loop`.
- Scopes: `engine`, `app`, `editor`, `ui`, `symbols`, `bridge`, `format`, `ci`, `docs`.

### 9.2 Ownership (`CODEOWNERS`)

```
/crates/engine/      @physics-owner
/crates/app/         @bridge-owner
/src/editor/         @editor-owner
/src/symbols/        @editor-owner
/tests/golden/       @physics-owner
/ARCHITECTURE.md     @all-reviewers
```

Changes to `ARCHITECTURE.md`, the file format, and the public `engine` interface require owner review.

### 9.3 What counts as a breaking change

A **contract** changes, and the PR must say so explicitly, if it touches:

- the public `engine` API (the `Device` trait, `Snapshot`, commands);
- the frame format or layout map;
- the schematic file format (`schemaVersion`);
- the device catalog (type ids, parameter names).

For contracts, a single PR updates Rust, generated types, TS consumers, migrations, and tests.

### 9.4 ADRs

Major decisions are recorded in `docs/adr/NNNN-title.md`:
context → decision → alternatives → consequences. Short format, one page.

### 9.5 How to add a new device (checklist)

1. `crates/engine/src/devices/<name>.rs`: a `ComponentSpec` variant, a `Device` implementation (`stamp`, `update_state`), and parameter descriptions (units, range, default).
2. Register it in the catalog (`devices/mod.rs`).
3. A known-answer test in `crates/engine` (plus a golden reference in `tests/golden/` where applicable).
4. A symbol in `src/symbols/<name>.svg` with pin positions.
5. Regenerate types so the contract test passes.
6. Update `docs/devices.md`.

The solver and editor are **not edited** in the process. This is the test that the architecture holds.

### 9.6 Working with AI-generated code

- Every generated piece of physics comes with a test; nothing is committed without green tests.
- The PR author is responsible for understanding the model (formulas go in a comment on `stamp`).
- Larger models (relay, motor) are first described briefly (equations, assumptions) in a module comment or ADR, then implemented.

### 9.7 License

GPL-3.0-or-later. It allows porting ideas from GPL projects (e.g. Falstad CircuitJS) and prevents closed forks. Dependencies are checked for license compatibility.

---

## 10. Deliberately deferred

- Sparse matrices (the solver sits behind a `Solver` trait).
- WASM/web version (the architecture allows it, but it is an extra build for now).
- Adaptive time step.
- AC/phasor analysis.
- Assignment mode, fault finding, AI explanations, collaboration, electro-pneumatics.
- Smart A* wire routing.

---

## 11. Known risks

1. **WebView on Linux (WebKitGTK):** canvas/WebGL can be slower. Check in the first week; keep a Canvas2D renderer as a fallback.
2. **Relay and motor physics:** models are tuned iteratively; golden tests are mandatory.
3. **Binary Tauri → UI channel:** verify in the prototype (see section 4).
4. **Project scope:** the main risk. Follow the plan below strictly.

---

## 12. Plan and definition of done

| # | Stage | Done when |
|---|---|---|
| 1 | End-to-end skeleton: battery + 2 resistors, UI shows voltage from Rust | Changing a resistor in the UI changes the number; Ohm's-law test is green; CI is green on 3 OSes |
| 2 | Editor: grid, parts, wires, undo/redo, saving | A circuit can be drawn, saved, and reopened; contract tests are green |
| 3 | Dynamics: C, L, oscilloscope | The RC curve matches ngspice within tolerance |
| 4 | Relay, motor, fuse, measuring instruments | A relay self-latching circuit works; voltmeter and ammeter show correct values |
| 5 | Assignments and fault finding | A teacher creates an assignment, a student completes it and gets checked |
| 6 | Diodes, transistors, extended models | Golden tests on nonlinear circuits pass |
