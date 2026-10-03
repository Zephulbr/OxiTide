# OxiTide — Track 8 Linux Support

- **Status:** Proposed — spec written 2026-10-03; plans pending
- **Date:** 2026-10-03
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 8; §3, §5);
  `plans/notes/2026-10-01-linux-support-spike.md` (read-only findings);
  `2026-09-21-oxitide-design.md`; `2026-09-23-oxitide-plan3-bitperfect-gapless-design.md`
- **Process note:** scope, decisions, and architecture were brainstormed live with the
  owner (2026-10-03) while Track 9 UI work continued. This document is the standalone
  source of truth for Track 8.

---

## 1. Purpose & Outcome

OxiTide is Windows-first; Track 8 makes it a real Linux client on modern Arch- and
Debian-based distros **without weakening the Windows product**. The spike proved the tree
already compiles to a silent non-Windows build (`real_output()` returns `NullOutput` at
`crates/oxitide-app/src/player.rs:70`); Track 8 replaces the stubs with working platform
implementations.

**Outcome:** on a bare-metal Linux host, OxiTide logs in (persistently where a Secret
Service exists), plays audio in the existing Shared/Exclusive model — direct `hw:` for
bit-perfect, server `default` for shared — exposes MPRIS media controls, registers
`tidal://`, respects the Wayland geometry limits, and ships as deb + AppImage with an AUR
recipe. Windows behavior and gates are unchanged.

**Success =** every WSL2-verifiable item passes in the WSL2 Ubuntu 22.04 environment; the
recorded bare-metal smoke checklist (§5.3) passes on real hardware; Windows
`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
`npm --prefix ui run check`, and `npm --prefix ui test` stay green throughout; Track 8 is
closed only through the completion gate in §5.4.

## 2. Scope

**In scope:**

- ALSA output backend with full parity: Exclusive (`hw:`), Shared (`default`),
  `ExclusiveFallback`, device enumeration, hardware/software volume, xrun recovery.
- Keyring: target-specific features, Secret Service probe, session-only fallback, UI
  notice.
- MPRIS media controls (play/pause/next/previous/stop, metadata, position/seek, Raise).
- Deep-link `.desktop` registration for `tidal://`; Windows `scheme.rs` untouched.
- Wayland geometry limitations documented; WebKitGTK CSS pass for the 4.1 floor.
- Packaging: `tauri.linux.conf.json` (deb + AppImage), in-repo AUR `PKGBUILD`, Linux
  build doc.
- `/proc`-based process metrics in `perf.rs` and a documented perf-harness exemption.
- Linux smoke checklists and the deferred bare-metal checklist.

**Out of scope:**

- CI automation, signing, auto-update (repo has none; Windows packaging is unsigned too).
- rpm and Flatpak (deferred; deb/AppImage/AUR cover the target distro list).
- Tray, global hotkeys, AppIndicator (Track 10; they ride Track 8's seams later).
- DSP/EQ, casting, Atmos, 360 RA (Track 7), downloads (PM-D2), i18n.
- macOS and any non-Linux non-Windows target.
- Visible Windows-side changes: all Linux additions are cfg-gated.

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| T8-D1 | **Full output parity via the `alsa` crate, direct device model.** Exclusive opens `hw:` (bit-perfect), Shared opens `default` (PipeWire/Pulse), `ExclusiveFallback` reopens shared on `EBUSY`/unsupported formats. | Matches the existing `AudioOutput`/`OutputMode` contract and the app's bit-perfect promise; `cpal` hides exclusive semantics, PipeWire-native breaks the PulseAudio/22.04 baseline and cannot do direct-`hw:` bit-perfect. |
| T8-D2 | **Session-only keyring fallback with an explanatory notice.** When no Secret Service is present, tokens live in memory only; Settings/Account and login explain why persistence is off and link to gnome-keyring/KWallet sources — no in-app tutorial. | Owner call; avoids plaintext or weak-crypto secrets on disk. |
| T8-D3 | **Perf: port `/proc` metrics; document a Windows-only harness exemption.** Memory/creation-time budgets stay measurable on Linux; the scripted WebView2 CDP scenarios are explicitly exempt. | Owner call; full harness port (spike: ~2-3 weeks) is deferred, but budgets must not silently disappear. |
| T8-D4 | **One spec for the whole track, staged plans (8.1–8.5).** | Owner call; mirrors the Track 3 pattern. |
| T8-D5 | **WSL2 Ubuntu 22.04 is the build base; bare-metal smoke is deferred but gates track closure.** | No Linux hardware available now; Tauri cannot cross-compile; Ubuntu 22.04 is the WebKitGTK 4.1 floor and AppImage's oldest-base guidance. |
| T8-D6 | **deb + AppImage artifacts, in-repo AUR `PKGBUILD`; rpm/Flatpak deferred.** | Two package families cover the target distro list; AUR verification needs an Arch environment (deferred). |
| T8-D7 | **MPRIS via `mpris-server` (zbus), reusing the neutral `plan()`/`SessionSender` seam** in `crates/oxitide-app/src/media_controls.rs`. | Least new code; the neutral seam exists precisely for this. |
| T8-D8 | **Windows-first guardrails hold.** Linux code is `#[cfg(target_os = "linux")]`; shared-code changes are limited to neutral seams; every plan keeps Windows gates green. | Avoids regressions and keeps merges parallel-safe with Track 9 UI work. |

## 4. Architecture

### 4.1 Audio output (ALSA)

**Placement.** New `crates/oxitide-audio/src/alsa_output.rs` behind
`#[cfg(target_os = "linux")]`; `alsa` added only under
`[target.'cfg(target_os = "linux")'.dependencies]` in `crates/oxitide-audio/Cargo.toml`.
`real_output()` (`player.rs:70`) gains a `target_os = "linux"` arm;
`enumerate_output_devices()` (`output.rs:211`) returns real devices on Linux. The Windows
`WasapiOutput` path is untouched.

**Mode mapping** (existing `OutputMode` / `ExclusiveFallback` semantics):

- `Exclusive` → open the selected device, else `hw:0,0`, directly. Formats come from the
  existing `exclusive_formats()` chain (`output.rs:64`, 32-bit store, valid bits
  16/24/32): negotiate `S32_LE` first; fall back to `S24_3LE`/`S16_LE` with explicit
  packing where the candidate's `store_bits`/`valid_bits` allow.
- `Shared` → open `default`; the plug plugin routes to PipeWire or PulseAudio.
- `ExclusiveFallback::Shared` → on `EBUSY`/unsupported format, reopen shared and set
  `OutputStatus.notice`; `WarnOnly` surfaces the failure as a notice/pause using the
  existing engine path (`engine.rs:2593-2669`).

**Testable seam (key design choice).** Pure logic is platform-neutral and unit-tested on
Windows, keeping `cargo test --workspace` green without Linux hardware:

- candidate → ALSA format/packing mapping (including S24_3/S16 sample packing),
- device-hint parsing, filtering, dedup, and default selection,
- mixer raw value ↔ percent ↔ `f32` volume mapping.

Only `open`/`write`/`flush`/`stop` I/O is cfg-gated. Linux-only integration tests use
ALSA's built-in **`null` PCM** in WSL2: real open, hw-params, write, and xrun-recovery
paths with no hardware required.

**Runtime behavior.** Blocking interleaved writes from the engine worker thread;
`-EPIPE` (xrun) → `snd_pcm_prepare` + retry; `-ESTRPIPE` → resume; partial writes loop;
`flush` discards ring and device buffered frames and restarts on the already-open handle
(drop + prepare) so a seek does not release and re-lock an exclusive DAC; `stop` drops and
closes. Volume prefers a card mixer
element (Master/PCM) when present and writable; otherwise software scaling with the
existing status semantics (`bit_perfect = false` and the "volume applied in software"
notice, `engine.rs:2593-2619`). Status follows the existing `OutputStatus` contract.

**Enumeration.** `alsa::device_name::HintIter` filtered to output PCMs; expose `default`
(`is_default = true`), `sysdefault`, and per-card `hw:`/`plughw:` entries with friendly
descriptions; IDs are the ALSA device names (stored as strings in config, as Windows
device IDs already are).

**Known limits (accepted).** No hotplug events (enumeration is on-demand); PipeWire may
resample shared streams to the graph rate (the same conceptual gap as WASAPI shared).

### 4.2 Keyring

Both `crates/oxitide-tidal/Cargo.toml:15` and `crates/oxitide-app/Cargo.toml:26`
currently hardcode `keyring = { version = "3", features = ["windows-native"] }`; they
become target-specific dependencies:

- Windows: `windows-native` (unchanged behavior).
- Linux: `sync-secret-service` + `crypto-rust` (Secret Service with a pure-Rust crypto
  backend; keyring v3 silently falls back to its mock store when no platform feature
  applies, so the feature selection is correctness-critical).

**Availability probe.** At startup, probe for a usable Secret Service with a sentinel
write/read through the configured store; any failure selects an in-memory store instead
(the test-facing `MemoryStore` pattern already exists). The probe result is exposed as
status (command/DTO) so the UI can render the notice.

**Fallback behavior (T8-D2).** Session-only tokens; login still works and the app is fully
usable, but nothing persists across restarts. The notice appears in Settings/Account and
at login; it states the cause (no Secret Service detected), the consequence (login will
not persist), and links to gnome-keyring/KWallet sources. Token values are never logged
(`TokenSet`'s redacting `Debug` stays authoritative).

### 4.3 MPRIS media controls

New `crates/oxitide-app/src/media_controls_linux.rs` using `mpris-server` (zbus-based),
added as a Linux target dependency. It reuses the platform-neutral
`plan()`/`SessionSender`/`ControlCommand` seam (`media_controls.rs:87`, `:49`); `spawn()`
takes `enabled` and no HWND. Dispatch in `media_controls.rs` becomes
`windows → media_controls_win`, `linux → media_controls_linux`, other → existing stub.

Mapping: metadata (title, artist, album, cover via the existing `cover_url`), playback
status, position + `Seeked`, and Play/Pause/Next/Previous/Stop; "Raise" focuses the Tauri
window. Hardware media keys ride the desktop's MPRIS consumer on Linux. The Settings copy
"Windows media controls" (`ui/src/views/Settings.svelte:651`) becomes platform-neutral.

### 4.4 Deep links and `.desktop`

`tauri-plugin-deep-link` supports Linux and `register_all()` is already called
(`main.rs:251`); the Linux `.desktop` entry carries
`MimeType=x-scheme-handler/tidal;` (via the bundle's desktop template/config as Tauri
requires). The Windows registry snapshot in `scheme.rs` stays Windows-only. Registration
verification in WSLg is partial and listed in the smoke checklist.

### 4.5 Wayland window geometry

Wayland forbids client-side positioning: mini-player placement and saved-position restore
degrade; size and the rest keep working. Documented limitation — no code change unless a
small guard is needed to avoid a visibly broken restore.

### 4.6 WebKit CSS pass

Phase 2, **gated on the Track 9 UI freeze**: verify `@property`, `backdrop-filter`, and
`color-mix` behavior against the WebKitGTK 4.1 floor in `ui/src/styles/` and adjust only
where the floor breaks. WSLg renders WebKitGTK for checks; the oldest-base check is on
the deferred list.

### 4.7 Packaging and build environment

**Build base.** WSL2 **Ubuntu 22.04 LTS** is the pinned Linux build environment (glibc
floor; AppImage guidance is to build on the oldest supported base). A new
`docs/linux-build.md` records: WSL install, apt dependencies (`build-essential`,
`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libasound2-dev`, `librsvg2-dev`,
`libasound2-plugins`, `patchelf`, `file`, and Tauri's remaining prerequisites), Rust/Node
per repo pins, WSLg notes (GUI smoke; shared audio via PulseAudio), and the build
commands.

**Bundle config.** A platform config `tauri.linux.conf.json` overrides bundle settings:
targets `deb` + `appimage`, the `.desktop` entry with the scheme handler, Audio category,
AGPL-3.0-or-later license, and the long-description "for Windows" copy fixed. Nothing in
the Windows bundle config changes.

**AUR.** A source `PKGBUILD` lives in-repo (e.g. `packaging/aur/PKGBUILD`) against Arch's
`webkit2gtk-4.1`. Its install verification is deferred to an Arch environment (§9).

**Artifacts.** Produced only on Linux, under `target/release/bundle/`; commands documented
in `docs/linux-build.md`. Unsigned, like the Windows installers.

### 4.8 Performance metrics and harness

`perf.rs`'s Windows-only process metrics (`perf.rs:329`, `:487`) gain Linux
implementations so budgets stay measurable:

- process creation time from `/proc/<pid>/stat` starttime (parsing after the last `)` to
  survive `comm` containing spaces),
- tree memory by walking `/proc/<pid>/task/*/children` and reading `smaps_rollup`/`statm`.

Parsing is pure and unit-tested with fixture strings. The scripted S1–S10 CDP scenarios
(`scripts/perf/run-scenario.ps1`, `drive.mjs`) are documented as a Windows-only
exemption; S4/S8/S9/S10 were always human-in-the-loop. The exemption is recorded in
`scripts/perf/` docs and AGENTS.md with this spec as the authority.

## 5. Testing & Verification

### 5.1 Gates

- Windows (unchanged): `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui run check`, `npm --prefix ui test`.
- WSL2/Linux: the same cargo gates, which are what actually compile/lint the
  `cfg(target_os = "linux")` code, plus `cargo build -p oxitide-app` with the Tauri
  dependencies installed.

### 5.2 Automated seams

- Pure mappings (ALSA format/packing, device hints, volume, `/proc` parsing) are tested
  on both platforms.
- ALSA integration tests against the `null` PCM in WSL2 (open/hw-params/write/xrun), no
  hardware required.
- Keyring unit tests use `MemoryStore`; the unavailable-Secret-Service probe path is
  tested via injection.
- MPRIS property/status mapping from `SessionUpdate`/`ControlCommand` is unit-tested; the
  live bus is manual.

### 5.3 Manual smoke

Checklist files follow the repo convention (`docs/superpowers/plans/notes/`):

- **WSLg-verifiable now:** GUI launch, shared audio, session-only keyring notice, login
  flow, MPRIS via `playerctl`.
- **Deferred to bare metal (recorded, gating §5.4):** exclusive/bit-perfect against a real
  DAC, hotplug, hardware mixer volume, real-DE MPRIS + media keys, `tidal://`
  registration/launch, Wayland behavior, AUR install, WebKitGTK 4.1 floor check,
  AppImage-on-22.04 launch.

### 5.4 Track completion gate

WSL2-verifiable plans (8.1–8.3, 8.5) may complete in WSL2. Track 8 closes only when the
deferred bare-metal checklist is run and recorded, or the owner explicitly records a
completion-note exemption naming what remains unverified.

## 6. Staged Plans (writing-plans handoff)

| Plan | Content | Parallel with Track 9 UI? |
|---|---|---|
| 8.1 | WSL2 Ubuntu 22.04 environment + `docs/linux-build.md`; target-specific keyring features; availability probe → session-only fallback; status command + minimal notice component. | Yes (minimal `ui/` touch) |
| 8.2 | Pure mappings + `alsa_output.rs` + `real_output()`/enumeration wiring + null-PCM tests + WSLg shared-audio smoke. | Yes |
| 8.3 | `tauri.linux.conf.json` (deb/AppImage, desktop/mimetypes), AUR `PKGBUILD`, build docs, artifacts produced in WSL2. | Yes |
| 8.4 | MPRIS module + `playerctl` smoke, `tidal://` verification, Wayland notes, Settings copy, WebKit CSS pass. | **No — after UI freeze** |
| 8.5 | `/proc` metrics + tests, harness-exemption docs, assemble the deferred bare-metal checklist. | Yes |

Execution hygiene: 8.1–8.3 and 8.5 land on a `track8-linux` branch/worktree while UI
work continues on `main` (`.worktrees/` is already gitignored); cfg-gating keeps merges
conflict-free.

## 7. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| PipeWire/PulseAudio interop or format-negotiation failures in shared mode | Automate open/write via `null` PCM; WSLg shared-audio smoke; `ExclusiveFallback` path already modeled in the engine. |
| Exclusive mode `EBUSY` on typical desktops (server holding the device) | Existing `ExclusiveFallback::Shared` semantics; notices flow through `OutputStatus`; documented behavior, not a failure. |
| keyring v3 silent mock-store fallback if features are wrong | Target-specific features pinned; sentinel probe detects non-persistence; MemoryStore in tests. |
| WebKitGTK 4.1 CSS differences | Phase 2 pass gated on UI freeze; deferred oldest-base check; adjustments limited to broken CSS features. |
| WSLg cannot verify real audio/desktop integration | Explicit deferred bare-metal checklist gating track closure (§5.4). |
| AUR recipe unverifiable without Arch | Recipe in-repo; install verification deferred and recorded. |
| No CI: Linux cfg code rots if WSL2 isn't run | Plans document the WSL2 gate commands; AGENTS.md updated with Linux commands. |

## 8. Performance Impact (PM-D1)

Windows: none — all runtime additions are cfg-gated; shared-code changes are limited to
neutral seams (`real_output` arm, keyring feature selection, `media_controls` dispatch,
`perf.rs` cfg arms). Linux: no budget exists until the `/proc` metrics port (8.5) makes
the Track 3 budgets measurable; shared-mode output carries the same server-resampling cost
as WASAPI shared, exclusive is direct. No new background work is introduced on either
platform.

## 9. Deferred Items (recorded)

- Bare-metal smoke checklist execution (§5.3) — gates track closure.
- AUR install verification (needs Arch) and AppImage-on-oldest-base launch.
- rpm, Flatpak, and the broader distro matrix (spike's Phase 3 leftovers).
- Full scripted perf-harness port to WebKitWebDriver/tauri-driver (T8-D3 exemption).
- Consuming Track 8 seams for Track 10 (tray, global hotkeys, AppIndicator/portals).

## 10. Sources

- Linux support spike findings: `plans/notes/2026-10-01-linux-support-spike.md`.
- Post-MVP roadmap Track 8: `2026-09-24-oxitide-post-mvp-roadmap.md` §3.
- Tauri prerequisites (webkit2gtk-4.1): <https://v2.tauri.app/start/prerequisites/>
- Tauri AppImage oldest-base guidance: <https://v2.tauri.app/distribute/appimage/>
- Deep linking on Linux: <https://v2.tauri.app/plugin/deep-linking/>
- keyring stores and features (mock fallback): <https://docs.rs/keyring/latest/keyring/>
- ALSA crate: <https://crates.io/crates/alsa>
- MPRIS server crate: <https://crates.io/crates/mpris-server>
