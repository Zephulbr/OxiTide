# Plan 7.3 — ASIO output

Spec: `docs/superpowers/specs/2026-10-11-oxitide-track7-asio-output-design.md`. Branch: `track7-asio`.

## Tasks

1. **ABI** — `crates/oxitide-audio/src/asio_sys.rs`: `#[repr(C)]` IASIO vtable, `ASIOBufferInfo`,
   `ASIOChannelInfo`, `ASIOCallbacks`, error/selector/sample-type constants, `CoCreateInstance`
   with the CLSID as IID, registry enumeration of `HKLM\SOFTWARE\ASIO`. `windows` features:
   `Win32_System_Registry`, `Win32_UI_WindowsAndMessaging`.
2. **Pure helpers** (`asio_output.rs`, unit-tested): `asio_device_name` id parsing, `AsioFormat`
   (type → bytes/bits/float, encode one word), whole-frame `render_frames`, `to_word`
   conversion with software volume, `is_bit_perfect`.
3. **`AsioOutput`** — owner thread (STA, message pump), driver load/init/rate/channels/type/
   buffers/start, global render state with in-flight counter, `asioMessage` reset handling,
   flush request/ack, status, teardown. `#[ignore]` hardware test.
4. **Routing** — `output::is_asio_device`; `enumerate_output_devices` appends ASIO drivers;
   `RoutedOutput` (WASAPI ↔ ASIO, `exclusive_fallback` on ASIO open failure); `player::real_output`
   uses it.
5. **Engine** — effective mode `Exclusive` for `asio:` devices (worker); unit test.
6. **UI** — Settings: copy under the bit-perfect toggle when an ASIO device is selected.
7. **Docs** — `THIRD_PARTY_NOTICES.md` (Steinberg GPLv3 option + trademark), `AGENTS.md`,
   roadmap revision line, spike note status, smoke checklist `plans/notes/2026-10-11-asio-smoke.md`,
   `ui/design` untouched (no visual change beyond copy).
8. **Gates** — `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
   `npm --prefix ui run check`, `npm --prefix ui test`.
