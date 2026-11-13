# Stage 8 independent rail references

Issue #129 supplies shared, offline data for the contract's topology-only DC
rail feature. It does not implement rails or report a rail simulation as passed.
The inspection baseline is `f0737763f3881140d0a6d97a10b8beb95b7ed1e1`.
Contract #128 was human-merged in PR #136. This work follows that contract;
the older proposal-status prose is not an assertion of new owner approvals.

## Files and identity

- [Manifest](../../tests/golden/stage-8-manifest.json): provenance, conventions,
  numerical tolerances, commands and planned consumers.
- [Circuit pairs](../../tests/golden/stage-8-circuits.json): divider with a live
  R1 update, START/STOP seal-in and its unlatched wrong variant, RC hold/recharge,
  and two isolated supplies with ordinary and N/PE/L1/L2 labels.
- [Connectivity table](../../tests/golden/stage-8-connectivity.json): 27 explicit
  drawings with handwritten connected-pin partitions or expected error codes
  and required diagnostic references. Includes global-reference union, case
  sensitivity, rename, overlapping/touching/gapped rails, detached references,
  empty rails, invalid names and overflow-safe extent cases.

Each entry retains exactly the same component IDs, parameters and pin world
geometry across layouts. In the wire counterpart each rail is replaced with
one explicit ordinary segment, recorded by `railWireMap`; other wires and
markers do not change. Rail snapshots are schema v2. Wire snapshots are legacy
v1 to run on the pre-#130 Engine; consumers must use `decode_project_json` so
the same file migrates after #130. Connected groups and signal keys identify
component pins/quantities, never a derived node index. Two-terminal keys use
`<id>:voltage` / `<id>:current`; contactor coil keys use `<id>/coil/current`.

The Stage 1/6 drawn fixtures supply real symbol pin geometry. The small
connectivity probes use ordinary two-pin resistor geometry. Expected connected
groups are literal, independently reviewed truth-table entries: no geometry
algorithm in the generator calculates its own expected topology.

## Independent physical answers

The divider uses `I = 5 / (0.1 + R1 + 1000)` with R1 = 1000 then 3000 ohm.
Both resistor currents are I; the source's entering-pin1 current is -I.
Each resistor terminal voltage is its resistance times I; source terminal
voltage is `(R1 + 1000)*I`. KCL tolerance is 1 nA and summed terminal power
tolerance is 10 nW. Voltage/current tolerances remain 1 microvolt / 1 nA.

START/STOP reuses every accepted event and observation from the Stage 6
[seal-in reference](../../tests/golden/stage-6-start-stop-latching.expected.json).
Settled coil currents are `5/(100+.01+(.01*.1/(.01+.1)))` with START held,
`5/100.11` when sealed in, and `5/100.02` in the unlatched wrong variant while
START is held. States are exact and coil tolerance is 0.1 mA. At step 12 the
correct circuit remains picked while the wrong one is released with zero coil
current. Internal transition times are not invented: #131 compares paired
last-transition times at the contract's 1 ns equivalence bound.

The [RC reference](../../tests/golden/stage-6-button-capacitor-hold.expected.json)
uses `10*(1-exp(-activeSeconds/0.100001))`. Effective charge time is `step/1000`
before step 50, 0.049 s through step 99, then `(step-50)/1000` from step 100.
The generator and Rust offline check recompute every reused row directly.
Voltage tolerance stays 5 mV (initial zero 1 nV); open current stays 1 pA.

Independent supplies use ideal 5/10 V sources with 1 kohm loads. Require 5/10 V,
5/10 mA, two islands and two `NoGroundReference` warnings. Renaming labels to
N/PE/L1/L2 establishes no phase, earth or reference. Explicit markers on
separate returns intentionally join them in the shared geometry table.

Observation ordering is explicit: `afterEventsAtStep` reads after same-step
operator requests; divider rows without `afterEvent` precede the step-10 edit,
and `afterEvent: 0` reads after it. All times use the existing public 1 ms grid
and current-step recomputation semantics. Reference tolerance, integration
error and rail/wire equivalence bounds are distinct; none relaxes another.

No new ngspice execution/model is needed: rails add no equations. Existing
analytical Stage 1/6 answers are reused and rechecked, with no production
dependency, downloaded model or Engine-generated expected values.

## Reproduction and reuse

```text
node scripts/stage8-references.mjs --check
node scripts/stage8-references.mjs --write
cargo test -p open-circuit-engine --test stage_8_references
pnpm exec vitest run scripts/stage8-references.test.mjs
```

The default is check-only; only explicit `--write` replaces files. Use the
repository-pinned Node 24.19.0, standard library only. Generation has no Engine
imports/subprocesses; normal tests use checked-in files and stay offline.
Review changed geometry/oracles deliberately instead of replacing an answer
with current Engine output. The Vitest drift check runs in ordinary `pnpm test`.

The Rust tests verify paired input identity, execute ordinary-wire netlists
against all applicable handwritten groups (also reversing input order), and
run all six wire circuits through real compilation/operator/parameter updates
against numerical/state observations. Schema/name errors are independent
future rail-validation inputs, not errors a label-free wire conversion could
reproduce.

One pre-existing wire-baseline defect is recorded explicitly as
[issue #137](https://github.com/SWOFART/open-circuit/issues/137): an explicit
reference bypasses the pinless-island error. Its fixture retains the correct
`FloatingNode` expectation; `legacyWireLimitation` records that current wire
behavior separately. #131 must correct conductor validation and remove the
documented baseline limitation, never change the expected diagnostic.

#130 uses rail inputs for strict schema/validation tests. #131 loads the actual
rail side, checks all required diagnostic references/partitions, compares
mapped slots/states and transition times, and repeats/permutates runs without
introducing a second oracle. #132 replays the same requests through the bridge;
#133 reads the same geometry table for editor hints; #134 draws corresponding
requests through editor commands; #135 records native-desktop and candidate
three-platform evidence. Those results are not claimed by this reference task.
