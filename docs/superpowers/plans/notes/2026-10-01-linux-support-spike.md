# Linux support spike findings (2026-10-01)

Feature assessment for **Track 8 — Linux support** (proposed; sequenced before Track 7,
partially for owner use). Decisions — audio backend, packaging artifacts, exact baselines —
stay for the Track 8 spec. Read-only spike; no probe code, no dependency added.

## Current state

The tree already compiles for a silent non-Windows build:

- `real_output()` returns `NullOutput` off-Windows (`crates/oxitide-app/src/player.rs:70`);
  `enumerate_output_devices()` returns empty (`crates/oxitide-audio/src/output.rs:216`).
- Media controls have a no-op stub (`media_controls.rs:137`); `scheme.rs` no-ops
  (`scheme.rs:145,175`); perf process metrics return `None`
  (`crates/oxitide-app/src/perf.rs:338,416`).
- Deep-link `register_all()` is already called on Linux (`main.rs:251`).

So with GTK/WebKit dev packages installed, Linux gets a building app that cannot play audio
and has no desktop integrations.

Windows-only surfaces to replace: `wasapi_output.rs` + `endpoint_volume.rs` (audio),
`media_controls_win.rs` + `scheme.rs` HKCU registry handling + `perf.rs` process metrics,
keyring features, Tauri bundle targets/config, and the perf harness.

## What carries over

- `oxitide-tidal`: 100% (reqwest/rustls, serde; PKCE browser + paste-back is neutral).
- `oxitide-audio`: `engine`, `queue`, `decode` (symphonia), `demux`, `cache`, `seek`,
  `source`, `mpd`, `fetch`, plus the `AudioOutput` trait, `NullOutput`, and the i32/f32
  conversion logic.
- `oxitide-app`: config (`directories` already yields XDG paths), artwork, lyrics, scrobble,
  Discord RPC (discord-ipc resolves Unix sockets), window geometry via Tauri APIs, tracing.
- UI: Svelte/Vite; only rendering needs a WebKitGTK pass.
- Seams already built for this: `media_controls.rs` splits platform-neutral
  `plan()`/`SessionSender` from the Windows module, and the stub pattern is the hook for a
  Linux module.

## What is reworked

1. **Audio output (biggest chunk).** New `AudioOutput` impl. Recommendation: the `alsa`
   crate opening `hw:` devices directly — it maps cleanly onto the existing model:
   `Exclusive` ≈ direct `hw:` (no OS mixer, bit-perfect), `Shared` ≈ `default` (routes to
   PipeWire or PulseAudio), `ExclusiveFallback` ≈ EBUSY/unsupported → shared. Needs device
   enumeration (`snd_device_name_hint`), format negotiation (S32/S24_3/S16), ring-buffer
   writer, xrun recovery, flush/seek, and volume (ALSA mixer element for hardware, else
   software). cpal abstracts away the exclusive/bit-perfect semantics the WASAPI backend
   hand-rolls; PipeWire-native is future-proof but much more complex. Bluetooth devices
   exist only through the server (shared-only); HDMI/USB work over `hw:`.
2. **Media controls → MPRIS.** New `media_controls_linux.rs` over D-Bus (e.g.
   `mpris-server`/`zbus`); the neutral `plan()`/`SessionSender` plumbing is reused and the
   `hwnd` parameter becomes irrelevant. Settings copy ("Windows media controls",
   `Settings.svelte:651`) needs a platform-neutral label.
3. **Keyring.** Both `Cargo.toml`s hardcode `keyring` `windows-native`. Gotcha: keyring v3
   falls back to the **mock store** when no platform feature applies, so tokens would look
   saved but not persist. Linux needs `sync-secret-service` + `crypto-rust` (or
   `linux-native-sync-persistent`), plus graceful degradation when no Secret Service is
   present.
4. **Deep links / auth.** `tauri-plugin-deep-link` supports Linux and `register_all()` is
   already called; the Windows registry snapshot in `scheme.rs` is irrelevant there (no
   Linux TIDAL desktop client owns `tidal://`). Paste-back remains the fallback.
5. **Packaging.** `tauri.conf.json` bundles only msi/nsis with Windows webview settings;
   Linux needs deb/rpm/AppImage targets, a `.desktop` file with
   `MimeType=x-scheme-handler/tidal;`, the GTK/WebKit build deps, and updated copy
   ("for Windows" in the long description). Builds must run on Linux; Tauri does not
   cross-compile Windows→Linux.
6. **Performance harness.** Track 3's standing gate is Windows/WebView2-specific:
   `run-scenario.ps1` launches the `.exe` and sets `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`;
   `drive.mjs` speaks CDP. WebKitGTK has a different inspector protocol — port to
   WebKitWebDriver/tauri-driver or run manual-only. `perf.rs` also needs `/proc`-based
   process creation time and tree memory, or memory budgets cannot be enforced on Linux.

## Distro / WM / kernel sensitivity (ranked)

| Layer | Impact | Notes |
|---|---|---|
| WebKitGTK version | High (hard floor) | Tauri v2 needs webkit2gtk-4.1; floor is **Ubuntu 22.04 / Debian 12**. All targets clear it (Mint 21 = 22.04 base, Mint 22 = 24.04, Arch family rolling). `@property`/`backdrop-filter`/`color-mix` in `tokens.css`/`app.css` are the likeliest differences on older LTS WebKit. |
| Build base / glibc | High (process) | A binary built on Arch will not run on Ubuntu 22.04 (`GLIBC_x.y not found`). Tauri's guidance: build on the oldest supported base (22.04/Debian 12) via VM/Docker — a build-matrix constraint, not code. |
| Audio server | High (functional) | PipeWire (Arch/Cachy/Manjaro/EndeavourOS/Debian 12/Mint 22) vs PulseAudio (Ubuntu 22.04/Mint 21); ALSA `default` abstracts both for shared mode, direct `hw:` for exclusive. PipeWire resamples shared streams to its graph rate — same conceptual gap as WASAPI shared. |
| Session type (X11 vs Wayland) | Medium | Not the WM: Wayland forbids client-side positioning, so mini-player placement and saved-position restore degrade (size still works). Mint/Cinnamon default X11; GNOME/KDE/Arch default Wayland. |
| DE / D-Bus | Medium | MPRIS media keys work on GNOME/KDE/XFCE/Cinnamon; bare tiling WMs may need a user `playerctl` binding. Keyring provider: GNOME Keyring vs modern KWallet; sessions with neither need a fallback. |
| Package family | Medium (distribution) | Two families cover the target list: deb (Ubuntu/Mint/Debian) and Arch (AUR PKGBUILD reused by Cachy/Manjaro/EndeavourOS), plus AppImage universally. rpm/Flatpak are later options. |
| Kernel | Low | Mainline `snd-usb-audio` covers UAC1/UAC2 DACs; Ubuntu/Mint HWE, Arch, and CachyOS kernels are all fine. RT scheduling is irrelevant for large-buffer playback; custom kernels matter only for bleeding-edge hardware quirks. GPU/NVIDIA DMABUF quirks are the one kernel-adjacent wart (Tauri workaround). |

Practical reading: target Ubuntu 22.04/Debian 12 as the build baseline, ship deb + AppImage +
AUR, and treat PipeWire/PulseAudio, X11/Wayland, and GNOME-Keyring/KWallet as the test
matrix — not each distro individually.

## Effort

- **Phase 1 — playable alpha**: ALSA backend + keyring features + deb/AppImage + build docs.
  ~1–2 focused weeks; risk in PipeWire/Pulse interop and format negotiation.
- **Phase 2 — desktop integration**: MPRIS, deep-link/.desktop verification, Wayland
  geometry, WebKit CSS pass. ~1 week.
- **Phase 3 — parity/hardening**: perf-harness port + `/proc` metrics, distro matrix.
  ~2–3 weeks.

Total ~1–2 months part-time; code volume is moderate, environmental variance is the long
pole.

## Why before Track 7

- DSP/EQ is engine-level and platform-neutral; it lands on both backends once Linux output
  exists. What Linux gates is the output-side contract (exclusive/bit-perfect, volume,
  spatial/Atmos — Atmos is likely Windows-only or infeasible on Linux).
- Landing Linux first settles that contract across two implementations before DSP builds on
  it, and the Track 3 harness port then gates Track 7 on both platforms.
- Track 7 is an open-ended research bundle (casting protocol, Atmos DRM) while Linux is
  predictable; Track 7 stays parked last.
- Cost accepted: every later feature has a two-platform manual test matrix (no CI). Keep
  Windows-first guardrails and Linux smoke checklists; split Track 7 when it is called.

## Sources

- Tauri prerequisites (webkit2gtk-4.1 per distro): <https://v2.tauri.app/start/prerequisites/>
- AppImage limitations (build on oldest base; Ubuntu 22.04/Debian 12): <https://v2.tauri.app/distribute/appimage/>
- Deep linking on Linux (`register_all`): <https://v2.tauri.app/plugin/deep-linking/>
- keyring stores and features (mock fallback): <https://docs.rs/keyring/latest/keyring/>
- ALSA crate: <https://crates.io/crates/alsa>
