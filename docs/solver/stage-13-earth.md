# Stage 13 solver notes: the earth node and earth faults

Contract: [`docs/contracts/stage-13-protection-and-safety.md`](../contracts/stage-13-protection-and-safety.md)
sections 2, 3, 7.1 and 8. This file records how issue #197 implements them; the contract decides
what they mean.

## The earth node

`netlist::derive_netlist` gives a drawing that has an `earth_electrode` one extra conductor
entity, the earth node. It is joined to every ground marker's conductor (the markers already
denote one reference) and becomes a node of the netlist (`Netlist::earth_node`). Each electrode
is a resistor from its `pe` node to that node (`DeviceModel::Resistor`), and it also joins its
island to the earth node, so every island that has an electrode shares the earth return. The
earth node is in the island's explicit-reference set: that island's reference is the earth node
and no automatic reference (and no `NoGroundReference` warning) is chosen. A drawing without an
electrode has no earth entity in any node, so every earlier circuit is solved as before.

## The exposed part

`exposed_part` is a resistor of 1 GΩ (`EXPOSED_PART_RESISTANCE_OHMS`) from its `ep` node to the
reference node of its island, so a frame that no wire or leak reaches is still a defined net, and
an unbonded frame floats to the voltage the circuit gives it. When its net is the reference (a
lone exposed part) both terminals are one node and the stamp is a no-op. It has no result slots
yet; the touch voltage and the hazard flag come with issue #201.

## The `leak` fault

`FaultOverlay` turns a `leak` into a resistor appended to the effective schematic, with the fault's
ID, and appends its pin geometry to the effective `GeometryEnvelope` (`FaultOverlay::geometry`):
pin 1 at the position of the leaking pin, pin 2 at the position of the frame's `ep` pin. Pins at
one point are one conductor, so the resistor joins the two nets exactly as a drawn resistor would,
and nothing in the netlist or the solver knows about faults. The resistor has no entry in the
result layout (the layout is built from the request's schematic), so no result slot appears for it.
Every solve after a compile uses the overlay's geometry; the request's geometry stays the base for
rebuilding the overlay after a live parameter update.

## Warnings

At the end of `derive_netlist`, for each exposed part: `NoEarthPath` when its island's reference is
not explicit (no electrode and no ground marker in the island), and `ExposedPartNotBonded` when its
net holds no electrode pin. Both are warnings and never stop a run.

## Touch voltage and the hazard flag (issue #201)

The exposed part's device (`ExposedPartDevice`, `DeviceModel::ExposedPart`) is the 1 GΩ resistor to
the reference of its island, and its result carries the voltage across it (the voltage of the frame
against that reference) and an `earth_joined` marker (the island's reference is explicit: an
electrode or a ground marker). The measurement update (`measurements.rs`, shared with the Stage 9
AC meters) treats the part as a voltage meter with its own `frequencyHz` window: over one period
it writes `touch_voltage_rms` and `touch_voltage_peak` from the internal-substep samples, and
`hazard` as the plain comparison `rms > touchVoltageLimitVolts`, then `valid`. Before the first
full window, and whenever the island is not joined to the earth node, all four are written as 0
(not valid, never hazardous). A new frequency starts a new window (`measurement_ids_invalidated_by_
frequency_change`). The layout has these four slots only, in this order; there is no instantaneous
voltage or current slot.
