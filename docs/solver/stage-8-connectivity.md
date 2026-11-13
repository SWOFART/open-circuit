# Stage 8 DC rail connectivity

Issue #131 implements the Engine portion of the
[published DC rail contract](../contracts/stage-8-supply-rails.md), on top of
#129's independent references and #130's validated project schema. Canvas
rendering/authoring and bridge acceptance remain their separate Stage 8 issues.

## One conductor assembly

`netlist.rs` represents every ordinary wire segment and each complete rail as
an exact-grid segment tagged with its drawn kind and UUID. All segments of a
wire polyline are joined first. The same closed-interval and endpoint tests
then handle wire/wire, wire/rail and rail/rail contacts. An interior perpendicular
crossing stays separate until a pin, endpoint, junction or reference marker
joins it. Point membership uses coordinate comparisons, never subtraction in
`i32`, pixel tolerances or iteration over grid units.

Names are validated once as drawing labels; they never enter union operations
or the solver. A rail is not a source, resistor, device or reference marker.
After geometric union, all explicit reference markers deliberately join their
nodes globally, as before. Otherwise each electrical island retains its local
automatic reference and `NoGroundReference` warning. `N` and `PE` have no special
meaning. Existing sources, MNA equations, internal substeps, nonlinear solving,
integration and event rules are unchanged.

Count and checked contact-work limits are validated before segment/contact
assembly. A rail is one segment even when its extent is one million grid units.
Runtime `NetNode` records sorted `pins`, `wire_ids` and `rail_ids`. Nodes with
pins keep their previous smallest-pin ordering; pinless nodes use their smallest
tagged conductor identity, wire before rail. No node index or mapping is saved.

## Drawable failures and runtime results

Pinless conductor islands fail with `FloatingNode` before numerical reference
selection, including an island with an explicit reference. This fixes the
ordinary-wire defect [#137](https://github.com/SWOFART/open-circuit/issues/137)
as well as the rail case. A floating diagnostic retains every member wire/rail
and attached junction/reference marker. Marker membership is collected during
the existing attachment pass and resolved after union; it is not reconstructed
by scanning geometry again for each island.

Source diagnostics retain their original component/pin references and add
the rails on the affected source's terminal nodes. References sort by canonical
UUID, with kind and pin ID as tie breakers, and are deduplicated. Invalid input
cannot produce a partial new run: failed compilation preserves the previous
snapshot and trace history atomically.

Successful recompilation, including a unique rail rename, creates a fresh run
at step zero with freshly initialized device state and trace history. Numeric
parameters and START/STOP operators retain their existing live-update behavior.
Rails add no layout slot, state record, trace quantity or per-segment current.
OCF2 and OCT1 formats are unchanged.

## Verification

Run:

```text
node scripts/stage8-references.mjs --check
cargo test -p open-circuit-engine --test stage_8_references --test stage_8_schema
pnpm test
```

The reference integration suite now executes all 27 handwritten rail geometry
cases, checks exact connected-pin partitions and required diagnostic references,
and repeats compilation with reversed components, wires, rails, markers and
pin geometry. It checks member mapping against `railWireMap`: each drawn rail
corresponds to one ordinary wire segment with a different UUID in the reference
drawing. The pin identities and electrical islands match; derived node indices
are not external identities. Translation checks repeat valid cases near both
signed-32-bit coordinate limits.

All six pairs run through the real Engine: DC divider/live R1 update, correct
START/STOP seal-in, unlatched wrong wiring, RC hold/recharge, and two isolated
supplies with ordinary and N/PE labels. Each run independently meets the
original physical observations and warnings. Paired component slots satisfy
`abs(a-b) <= 1e-9 + 1e-9*abs(b)`, discrete states match exactly and paired
last-transition times differ by at most 1 ns. Repeated/permuted rail runs have
identical numeric/state output in one environment. Divider checks retain the
1 nA KCL and 10 nW terminal-power bounds.

Trace checks cover every public sample and current-step replacement after
operator/parameter events, with no invented rail signals. Additional tests
exercise source short/contradictory ideal-source diagnostics with rail members,
atomic failed compilation, successful rename/reset and stale-run refusal.
The unchanged Stage 1–7 suites remain the regression evidence for existing
physics, nonlinear convergence/rollback, integration and event semantics.
