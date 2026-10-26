# ADR 0002: Couple the PMDC motor implicitly and carry device state in OCF2 frames

**Status:** Draft for review by both Engine and Application workstreams.
**Issue:** #63.
**Contract:** [`docs/contracts/stage-4-components.md`](../contracts/stage-4-components.md).

## Context

Stage 4 needs a motor model whose armature current and rotor speed exchange energy, plus runtime relay/fuse states visible to the application. `ARCHITECTURE.md` previously described coupling motor speed through the previous step; the document also called out oscillation from that explicit coupling. The existing OCF1 frame contains only numeric values and metadata, so it cannot represent relay/fuse states without changing the frame contract.

## Decision

- Solve the PMDC armature-current and rotor-speed equations implicitly and together with the circuit at every internal step. Use the shared SI torque/back-EMF constant and the trapezoidal rule; the next-step electrical equation uses next-step speed, and the next-step mechanical equation uses next-step current.
- Use OCF2 for Stage 4 live frames. Keep the contiguous analog `f64` payload and append a bounded, fixed-width discrete state table for relays and fuses to the same binary frame. All state records share the frame's run, revision, layout, step, time, and sequence metadata.
- Keep `crates/engine` transport-independent. Engine owns the typed DTO and state table; `crates/app` encodes/decodes and transports the frame; TypeScript consumes generated types and displays the result.
- Preserve the version 1 schematic schema. Runtime state is never serialized into a project.

## Alternatives considered

- **Previous-step explicit motor coupling:** rejected because the architecture already identifies oscillation, and the motor's two-way energy transfer should not use a stale state.
- **A JSON state event alongside OCF1 frames:** rejected because separate streams can be observed at different frame sequences. OCF2 associates analog values and discrete state atomically, retains existing latest-frame coalescing/backpressure, and still avoids JSON per frame.
- **Encode relay/fuse states as numeric analog slots:** rejected because a discrete enum is not a physical `f64` measurement and would make layout units/sign conventions misleading.
- **Increase the project schema version:** not needed; the component additions are additive and runtime state remains outside the saved schematic.

## Consequences

- Engine DTOs, TypeScript declarations, the app frame codec, and golden-frame validation must change together when Stage 4 is implemented.
- OCF2 is a new frame magic; readers must not interpret OCF1 bytes as OCF2.
- Relay/fuse state accompanies numeric results in each frame, while motor angular velocity is an ordinary analog result slot.
- All newly specified models still require independent known-answer tests before implementation is accepted.
