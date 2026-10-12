# ASIO spike findings (2026-10-01)

> **Status (2026-10-11):** built as Plan 7.3 — see
> `specs/2026-10-11-oxitide-track7-asio-output-design.md`. It deviates from this note by
> hand-writing the IASIO binding instead of using `asio-sys` (no LLVM, no feature gate).

Feature-parity assessment (not committed). Question: do we support ASIO output, and what
would it take? Owner decision: park it as a **maybe down the line** Track 7 candidate —
nice to have, not necessary now. Read-only spike; no probe code, no dependency added.

## Current state

- Output is WASAPI-only. `AudioOutput` is backend-agnostic (`crates/oxitide-audio/src/output.rs:89`);
  the one concrete impl is `WasapiOutput` (`wasapi_output.rs`), shared + exclusive/bit-perfect.
- The backend is chosen once at startup (`crates/oxitide-app/src/player.rs:73`), but `Engine`
  only reopens when `AudioOutputSpec` changes (`engine.rs:1111`), so same-format gapless
  tracks do not reopen the device.
- ASIO drivers are invisible today: `enumerate_output_devices()` lists WDM render endpoints,
  and ASIO devices are not WASAPI endpoints.

## Feasibility (2026-10-01)

- `asio-sys` 0.4.0 exposes a managed API (`Asio`, `Driver`): `driver_names()`,
  `load_driver()`, `channels()`, `output_data_type()`, `buffersize_range()`,
  `can_sample_rate()`/`set_sample_rate()`, `prepare_output_stream()`, `start`/`stop`/`destroy`,
  `add_callback` (driver-thread buffer callback), `add_event_callback`. Direct use fits the
  trait better than pulling in cpal, which duplicates a whole audio stack.
- Proposed shape if ever built: a new `AsioOutput` (~500–700 lines incl. tests, mirroring
  `wasapi_output.rs`) plus a small routing `AudioOutput` dispatching on a device-id prefix
  (e.g. `asio:<driver>`); callback pulls from the existing `rtrb` ring; seek/flush = drain
  ring + fill silence; volume is software-only; `OutputStatus` bit-perfect = volume == 1.0.
  `output_device` is already a free-form string (`config.rs:24`), so no config schema break.
- Enumeration perf (PM-D1): list registry names only; loading a driver to inspect channels /
  rates must happen on selection, not at startup.
- Effort estimate: happy path ~3–5 focused days; ~1.5–3 weeks to harden against real driver
  quirks. Integration testing needs ASIO4ALL or real hardware (human checklist); only pure
  helpers (id parsing, format conversion, buffer-size choice) are unit-testable.

## Licensing / build

- Steinberg dual-licensed the ASIO SDK in Oct 2025 (proprietary or GPLv3). GPLv3 is
  compatible with this repo's AGPL-3.0-or-later; add SDK notices when distributing.
- `asio-sys` is Apache-2.0 but its build needs LLVM/libclang (bindgen) and auto-downloads the
  ASIO SDK. Feature-gate the dependency (`--features asio`) so normal `cargo test`/`clippy`
  stays toolchain-free; packaging turns it on.

## Risks (ranked)

1. ASIO drivers are in-process DLLs — a buggy driver can crash/hang the whole app
   (WASAPI is isolated in `audiodg.exe`). `asio-sys`/cpal's ASIO path is also less
   battle-tested than WASAPI.
2. Volume regression: ASIO has no endpoint/hardware volume, so any volume < 100% is software
   attenuation and breaks bit-perfect — unlike WASAPI exclusive, which uses hardware endpoint
   volume (`wasapi_output.rs:217`) and stays bit-perfect.
3. Most ASIO drivers are single-client: a running DAW / other ASIO host causes open failures.
4. No hotplug/removal notifications; device changes need manual rescan and different engine
   handling.
5. Per-driver quirks: buffer sizes, sample types (Int32/Int24/Float32), rates changeable only
   in the vendor control panel; reopen across format changes can click/pop.
6. UI duplication/confusion (same interface appears as WASAPI endpoint + ASIO driver); the
   "bit-perfect" claim becomes unverifiable (drivers may process internally; ASIO4ALL wraps WDM).

## Benefits

- Rescues hardware where WASAPI exclusive fails or is capped: vendor WDM drivers that reject
  the track's rate currently force a resampled shared fallback or an error. This is the one
  concrete gap ASIO closes.
- Direct hardware clock control (set rate before creating buffers) instead of endpoint
  negotiation; no Windows mixer/APO/endpoint-volume dependency.
- Vendor-native path for pro interfaces; feature parity with Roon/Foobar/MusicBee.
- Low latency and multichannel are irrelevant for stereo TIDAL playback; for PCM
  16/44.1–24/192, a healthy WASAPI exclusive setup already yields bit-perfect, so the delta is
  compatibility and control, not fidelity.

## Recommendation

Do not build now. Revisit if an affected interface appears (WASAPI exclusive cannot drive it
cleanly) or the owner wants feature parity; then run the normal spec → plan cycle (device
identity and output-status semantics change, so architectural).

## Sources

- `asio-sys` 0.4.0 docs: <https://docs.rs/asio-sys/0.4.0/asio_sys/bindings/index.html>
- Steinberg ASIO licensing: <https://www.steinberg.net/developers/asiosdk-open/>
- cpal ASIO reference backend: <https://github.com/RustAudio/cpal/blob/master/src/host/asio/stream.rs>
