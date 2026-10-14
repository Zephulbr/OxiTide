# Track 7 — ASIO output (design)

- **Date:** 2026-10-11
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 7, "ASIO output (maybe)");
  `plans/notes/2026-10-01-asio-spike.md` (feasibility, risks, proposed shape)
- **Process note:** the owner called ASIO on 2026-10-11 ("implement the ASIO output as per the
  docs"). This spec follows the spike's proposed shape with one deviation (T7A-D1).

## 1. Goal

Play through ASIO drivers on Windows next to the existing WASAPI backend, so interfaces whose
WDM driver cannot run WASAPI exclusive cleanly get a vendor-native, sample-exact path.

## 2. Scope

**In:** driver enumeration, an `AsioOutput` backend, routing between WASAPI and ASIO by device
id, exclusive-fallback semantics, status/bit-perfect reporting, Settings copy, notices, docs.

**Out:** opening the driver's control panel from the app, buffer-size selection (the driver's
preferred size is used), resampling (an unsupported rate fails or falls back, T7A-D5),
multichannel/DSD, hot-reconfiguration without a pause (T7A-D6), 32-bit Windows builds.

## 3. Decisions

- **T7A-D1 — hand-written IASIO binding instead of `asio-sys`.** `asio-sys` needs LLVM/libclang
  (bindgen) and downloads the SDK at build time; the dev machine has neither. The IASIO COM
  vtable is 21 methods after IUnknown and a handful of `#[repr(C)]` structs, so `asio_sys.rs` declares them by
  hand and loads drivers with `CoCreateInstance` (the driver's CLSID doubles as the IID, as in
  the SDK's loader). No new dependency, so no feature gate: normal `cargo test`/`clippy`
  compiles and tests the code. Compiled for 64-bit Windows only (x64/ARM64 have one calling
  convention; 32-bit `thiscall` is out of scope). The interface is Steinberg's; we take the
  GPLv3 option of the dual-licensed SDK (AGPL-compatible) and record that in
  `THIRD_PARTY_NOTICES.md`.
- **T7A-D2 — device identity.** ASIO devices enumerate from `HKLM\SOFTWARE\ASIO` (registry
  only; no driver is loaded at enumeration, PM-D1) with id `asio:<registry key>` and name
  `<description> (ASIO)`, appended after the WASAPI endpoints. `audio.output_device` stays a
  free-form string, so there is no config schema change.
- **T7A-D3 — routing.** `real_output()` on Windows returns `RoutedOutput`, which owns a
  `WasapiOutput` and an `AsioOutput` and dispatches every trait call to the one the last `open`
  selected (`asio:` prefix → ASIO). Switching backends stops the other one first.
- **T7A-D4 — ASIO is always exclusive.** The engine treats an `asio:` device as
  `OutputMode::Exclusive` regardless of the bit-perfect toggle, so 16/24-bit sources arrive as
  left-aligned `i32`. Status is `Exclusive`; `bit_perfect` is true only when the input is
  integer, the driver's sample type is integer with at least the source's bits, and volume is
  100% (ASIO has no endpoint volume; software volume clears it with the WASAPI notice text).
  Float drivers are never reported bit-perfect.
- **T7A-D5 — open failures follow `exclusive_fallback`.** If the driver cannot load, lacks the
  channels, rejects the sample rate, or uses an unsupported sample type: `shared` opens WASAPI
  shared on the system default with the notice "ASIO unavailable (<reason>); using shared
  output"; `warn_only` stops with that reason. Most ASIO drivers are single-client, so a busy
  driver lands here too.
- **T7A-D6 — driver resets are device loss.** `kAsioResetRequest` (control-panel change,
  unplug), a buffer-size change, or an external sample-rate change marks the output lost:
  the next write fails, the engine pauses with the notice, and play reopens the driver (the
  existing device-loss path). A stalled driver (no callbacks) trips the same 2 s ring timeout
  as WASAPI.
- **T7A-D7 — threading.** One owner thread per open (`oxitide-asio`, COM STA, pumps window
  messages) loads, configures, starts, stops, and releases the driver; `init` gets the desktop
  window handle (as PortAudio does). The driver's buffer-switch callback has no user pointer,
  so the render state is published through a process-global pointer with an in-flight counter
  so teardown never frees it under a running callback. Only one ASIO driver is open at a time.
- **T7A-D8 — render path.** `write` converts to one 32-bit word per sample (left-aligned `i32`
  for integer drivers, `f32` bits for float drivers) and pushes it into an `rtrb` ring like
  WASAPI. The callback consumes whole frames only (no channel slip on underrun), de-interleaves
  into the driver's buffers in its native format (Int16/24/32 LSB, Int32 LSB16/18/20/24,
  Float32/64 LSB; MSB and DSD types are rejected at open), fills underruns with silence, and
  calls `outputReady` when the driver supports it. A flush (seek/fade-pause) is a request the
  callback serves by draining the ring.

## 4. Risks accepted (from the spike)

In-process drivers can crash or hang the app; single-client conflicts; no hotplug events
beyond reset requests; volume below 100% is software and not bit-perfect; the same interface
can appear twice (WDM endpoint and ASIO driver). The Settings copy states the volume and
always-exclusive behaviour.

## 5. Testing

Unit tests: id parsing, sample-type mapping and encoding for every supported type, whole-frame
de-interleave with underrun, word conversion (`f32`/`i32`, software volume), bit-perfect rule,
engine effective mode for `asio:` ids. Hardware: an `#[ignore]` test that opens the first
registered driver, and a human smoke checklist
(`plans/notes/2026-10-11-asio-smoke.md`) on real ASIO drivers.

## 6. Completion gate

`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run
check`, `npm --prefix ui test` green; owner smoke checklist passed on at least one ASIO driver.
