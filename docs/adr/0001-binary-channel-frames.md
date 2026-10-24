# ADR 0001: Deliver simulation frames over a binary Tauri channel

**Status:** Proposed for Stage 1. It becomes accepted when the pull request is merged after
review by both the Engine and Application workstreams, as the contract requires for a
change to the frame layout.
**Issue:** #8. **Contract:** `docs/contracts/stage-1.md` section 8.

## Context

`ARCHITECTURE.md` section 4 asks the bridge to send simulation frames to the UI as binary
`f64` arrays, with no JSON per frame, and to verify in the prototype how well binary
transfer through the Tauri 2 `Channel` works in the chosen version. The fallback it names is
a command that returns `Vec<u8>` / `ArrayBuffer`, polled on a timer.

The bridge has to keep the UI from blocking physics, send only the latest snapshot, cap
delivery at 60 frames per second, and never let a slow webview make the backend queue frames.

## Decision

Frames are sent as `InvokeResponseBody::Raw` messages on a `Channel<InvokeResponseBody>`
that the UI passes to `start_simulation`. The webview receives each as an `ArrayBuffer`. The
UI sends `acknowledge_frame` after processing a frame, and the backend stops sending while
two frames are unacknowledged. The fallback is not needed.

The frame envelope, commands, and delivery rules are in the contract, section 8.

## What the source and the prototype showed

Version checked: the `tauri` crate 2.12.5 with `@tauri-apps/api` 2.12.4 (the version in
`pnpm-lock.yaml`), on Windows 11 with WebView2 155.0.4283.62.

- `tauri::ipc::Channel<InvokeResponseBody>` accepts `InvokeResponseBody::Raw(Vec<u8>)`, and
  the JavaScript `Channel` hands the message to `onmessage` as an `ArrayBuffer`. The
  JavaScript side also keeps messages in order using an index, so frames cannot be
  delivered out of order.
- Tauri chooses the path by size. A raw message under 1024 bytes is written into the page
  as `new Uint8Array([...]).buffer` through the webview's own script evaluation, so it is
  not subject to the page's content security policy. A larger one is fetched over the `ipc:`
  protocol, which the existing `connect-src` already allows. Both paths deliver an
  `ArrayBuffer`. A Stage 1 frame is under 1024 bytes, so it takes the first path. This is
  still binary at the API level and carries no JSON, but the bytes cross the boundary
  as a decimal array literal, which costs more than a raw transfer would. At 60 frames per
  second and about 100 bytes per frame this is negligible; a layout with thousands of
  values would take the fetch path instead.
- No capability or permission was added. The commands are application commands, and the
  window's capability file still lists no permissions.

The prototype evidence from the real desktop executable is at the end of this file.

## Alternatives considered

- **Polling a command that returns bytes.** The architecture's fallback. It works, but the
  UI would have to poll on a timer and the backend could not tell when a frame was consumed.
  Not needed, since the channel works.
- **JSON frames.** Rejected by the architecture: no JSON per frame.
- **One JSON event per frame.** Same objection, plus the cost of string encoding every value.

## Consequences

- Backpressure is part of the design: without acknowledgements a webview could be sent
  frames faster than it reads them. The cost is one small command per frame. At most two
  frames are outstanding; because an unacknowledged frame expires after one second, a webview
  that never acknowledges still receives about two frames per second of roughly 100 bytes
  each. That is bounded, not queued, but it is not zero.
- Frames and command replies are not ordered relative to each other, so the UI applies the
  contract's acceptance rule to every frame and handles a frame that overtakes a reply.
- A frame format change is a contract change and needs a new magic value.
- `u64` counters arrive as JSON numbers in the JSON results and as true 64-bit integers in
  frames; the TypeScript adapter normalizes them to `bigint`.

## Prototype evidence

Recorded on 2026-10-24 with the real application: Tauri 2.12.5, Windows 11 (10.0.26100),
WebView2 155.0.4283.62, release build, strict CSP unchanged. The run was repeated on the
final code of the change that introduced this ADR, with the same results.

The window was driven through the WebView2 remote-debugging port, which was enabled only in
a separate local build outside the repository (a `TAURI_CONFIG` override adding
`additionalBrowserArgs`; the application code is identical). The UI was read and edited
through its own DOM, so every number below went Rust engine, simulation thread, binary
channel, `ArrayBuffer`, decoder, UI. The readings were shown with six significant digits
when this was recorded; the application now shows seven (contract section 6.1), and
`docs/acceptance/stage-1-divider.md` records the same scenario at that precision.

| Check | Result |
|---|---|
| Initial readings, `R1 = R2 = 1000 Ω`, `Rs = 0.1 Ω`, `Vs = 5 V` | `R1` and `R2` voltage `2.49988 V`, series current `2.49988 mA`, source terminal voltage `4.99975 V`, source current `-2.49988 mA` (discharging, negative into `pin1` as the contract says) |
| Simulation clock against the wall clock | 2.561 s then 4.569 s over a 2.0 s wait: 1.0x real time |
| UI updates from frames | 121 text updates in 2.0 s: about 60 per second, at the cap |
| `R1` changed to `3k` in the input | `R1` `3.74991 V`, `R2` `1.24997 V`, current `1.24997 mA`, as in `tests/golden/stage-1-divider.expected.json` |
| `R1` changed back to `1k` | the first readings were restored |
| Invalid `R2` (`0`) | refused in the UI with "outside the allowed range"; readings and the clock were unaffected, nothing reached the engine |
| Closing the window | the process exited by itself within the 8 s allowed; no process was left running |

The automated checks cover what a single run cannot: the byte-exact frame format against a
golden frame, the 60 Hz cap and fixed 1 ms step with a fake clock, the slow-consumer bound
(an unacknowledged consumer receives exactly two frames), the lost-acknowledgement timeout,
stream release on stop, on a new compile, and on shutdown, and the TypeScript adapter
against a stand-in for the Tauri API.

Not verified: Linux and macOS. CI builds the application there but does not run the window,
and the WebKit-based webviews on those systems have not been observed. The behaviour of the
two delivery paths for payloads of 1024 bytes and more (fetch over `ipc:`) was read in the
Tauri source but not exercised, because no Stage 1 frame is that large.
