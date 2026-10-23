# Native Linux bare-metal run (2026-10-23)

- **Status:** first bare-metal batch executed, then a USB-DAC follow-up the same day (§11):
  bit-perfect playback works end-to-end once two ALSA bugs are patched (#13, #14 — fixes sit
  uncommitted in the working tree), while the device picker still cannot select a direct device
  (#11, open). 14 issues recorded, 1 row blocked (AUR), the oldest-base rows not
  available on this host. Docs only: no fix was committed
  or pushed with this note (§12).
- **Owner:** V (hardware and TIDAL account); the run was driven by the agent (build, gates, GUI automation, D-Bus/MPRIS checks, perf corpus).
- **Authority:** `2026-10-03-track8-bare-metal-checklist.md` (Track 8 closure gate),
  `2026-10-13-track9-completion.md` § Linux bare-metal run, `2026-10-17-v1-release-gate.md` §6.
- **Tree:** `main` at `db6e320`, version 0.4.0; the run added the docs commit `d71ea22` and
  the ALSA fix `7498179` (§12).
- **Machine:** CachyOS (Arch rolling), kernel 7.3.0-3-cachyos, KDE Plasma 6.8.1 on **Wayland**,
  PipeWire 1.6.10, WebKitGTK 2.54.1, ASUS Vivobook M3402RA (AMD), internal HDA speakers plus a
  MOONDROP Dawn Pro USB DAC in the §11 follow-up; 1.75 display scale, 32 GB RAM. **This is not
  the Windows 11 machine the earlier smoke rows were recorded on** — hardware, GPU/audio stack
  and WebKit engine differ, so results are a Linux baseline, not a comparison.

Toolchain used: Rust 1.99.1 (stable, local install), Node 24.22.1 (local), `webkit2gtk-4.1`
2.54.1, `patchelf` 0.19.2, `playerctl` 2.4.1, `xdotool`/libxdo installed.

## 0. Method and tooling

- Release build from `npm --prefix ui run app:build` (bundles) and `app:build:perf`
  (`tauri build --no-bundle`, for the perf and DAC passes). Plain `cargo build --release`
  produces the dev-URL binary and shows "Could not connect to localhost" — not used.
- GUI driven over the KWin MCP (AT-SPI first, pointer/uinput as fallback). The WebKitGTK
  window exposes no a11y tree, so pointer coordinates were read from desktop captures and
  converted from the 1.75× physical framebuffer (2880×1800) to compositor coordinates.
- Transport and metadata through `playerctl` (MPRIS); tray through `com.canonical.dbusmenu`;
  deep links through `xdg-open`; audio state from `/proc/asound/card*/pcm*p/sub0/*`, `pactl`,
  `amixer`, and `fuser /dev/snd/pcm*`.
- Perf from `OXITIDE_PERF_SCENARIO` + `node scripts/perf/check.mjs`; screenshots, JSONL and
  logs under `.superpowers/linux-run/` (gitignored).

## 1. Gates and build

| Check | Result |
|---|---|
| `cargo test --workspace` | **PASS** — 1092 passed, 0 failed (includes `cfg(target_os = "linux")` code) |
| `cargo clippy --workspace -- -D warnings` | **PASS** — clean |
| `npm --prefix ui run check` | **PASS** — 0 errors, 0 warnings |
| `npm --prefix ui test` | **PASS** — 151 files, 1228 tests |
| `npm --prefix ui run app:build` | **PASS** — release binary 5m 00s; `OxiTide_0.4.0_amd64.deb` (10.5 MiB) + `OxiTide_0.4.0_amd64.AppImage` (115 MiB) |
| deb metadata | **PASS** — `Package=oxi-tide`, `Version=0.4.0`, `Architecture=amd64`, `Depends` = libasound2, xdg-utils, desktop-file-utils, libayatana-appindicator3-1, libwebkit2gtk-4.1-0, libgtk-3-0; `.desktop` valid (`Exec=oxitide-app`, `Categories=AudioVideo;Audio;Music;`, `MimeType=x-scheme-handler/tidal;`, `StartupWMClass=oxitide-app`) |
| AppImage | **PASS** — `--appimage-extract` OK, same desktop fields, launches on this distro |

This clears the "Linux cargo gates for code merged since Track 8" row and the outstanding full
workspace gates from `2026-10-16-96kbps-validation.md`.

## 2. Track 8 bare-metal checklist

| Row | Result |
|---|---|
| Exclusive / bit-perfect on a real DAC | **PASS with a caveat (§11)** — with `MOONDROP Dawn Pro` (`hw:CARD=Pro,DEV=0`) the app opens exclusive at S32_LE 44.1 kHz, no PipeWire stream, `✓ BP` lit. The id has to be written into `config.toml`: the picker cannot offer a `hw:` name (issue #11), and Settings rewrites the unknown id to `""` on open. |
| `ExclusiveFallback` on a busy device | **PASS** — exclusive ON with the PipeWire sink active: playback continues in shared mode and the notice "exclusive mode unavailable; using shared output" appears in Settings → Playback and in the Rack display. |
| Hotplug | **NOT EXERCISED** — the DAC is removable, but unplug/replug needs a hand on the cable; the device list is re-enumerated on every `get_output_devices`, so plug-in is covered by inspection only. |
| Hardware mixer volume | **PASS** — the DAC exposes a single `PCM` play-volume element (no `Master`); a player-bar volume drag set it to 38 % (48/127, −39.5 dB) while the stream kept running and `✓ BP` stayed lit. |
| Real-DE MPRIS + media keys | **PASS (KDE)** — `playerctl -l` lists `OxiTide`; metadata (`xesam:title/artist/album`, `mpris:length` 144000000 µs, `mpris:artUrl` on `resources.tidal.com`) correct; position advances; `playerctl position 30` lands at 30.000 s; play-pause, next, previous drive the app; KDE `mediacontrol` shortcuts (`playpausemedia`, `nextmedia`, `stopmedia`) reach it. GNOME/XFCE/Cinnamon: **not run**. |
| MPRIS desktop-entry resolution per package | **PARTIAL / issue #4** — `DesktopEntry=OxiTide` is advertised, but only `oxitide-app-handler.desktop` (dev/AppImage registration) exists in this run; the deb ships `OxiTide.desktop`, the AUR recipe installs `oxitide.desktop` (case mismatch). Resolution was not observable without an installed package. |
| `tidal://` registration/launch | **PASS** — `xdg-open 'tidal://login/auth?code=smoke'` exit 0, running instance logged `deep-link login failed: no login in progress`; AppImage re-registration points `Exec` at the AppImage and also delivers. The WSLg `xdg-open` quoted-`Exec` quirk does **not** reproduce on KDE. |
| Single instance | **PASS** — second launch exits 0 immediately, no second window. |
| Wayland behavior | **PASS** — size and layout restore work (900×620 window restored at 900×620); mini player enters/exits, compact rail layout at 900 px; position is compositor-chosen (restored at 372,204 instead of the saved 222,100) — the documented Wayland limitation, no crash or off-screen window. |
| AUR install | **BLOCKED** — the recipe template still points at `codeberg.org/Zephulbr/OxiTide.git` while `origin` is GitHub, and `v0.4.0` is untagged/unpushed; `makepkg` has no fetchable source. Recipe content was not exercised. |
| WebKitGTK 4.1 floor (Ubuntu 22.04 / Debian 12) | **N/A** — host has WebKitGTK 2.54.1; the floor needs the 22.04 environment. CSS features render correctly on 2.54.1. |
| AppImage oldest-base launch | **N/A** — launched on CachyOS (current distro) instead; the 22.04 launch needs that base. |

## 3. Track 8 resume rows (WSLg/8.1/8.4)

| Row | Result |
|---|---|
| GUI launch | **PASS** — window renders, no GTK/WebKit init errors (only missing KDE GTK modules and the `qtpaths` warning, issue #7). |
| Login flow (PKCE + browser) | **PASS** — "Log in with TIDAL" opened `login.tidal.com/authorize?…redirect_uri=tidal://…` in Firefox; the `tidal://` callback completed the login without paste-back. |
| Session-only keyring notice | **PASS (absent)** — KDE `ksecretd` provides `org.freedesktop.secrets`; no notice shown and a restart stayed signed in (persistent keyring works). |
| Shared audio | **PASS** — lossless FLAC plays through PipeWire (sink RUNNING, 44.1 kHz s32le, position advances); pause/resume/seek/next/previous verified. Audibility not machine-checkable (owner confirmed audio was audible on the call). |
| MPRIS via playerctl | **PASS** — see above; `Raise` returns success and focuses the window. |
| Settings copy platform-neutral | **PASS** — "Media controls — Hardware media keys and the system media overlay" (no "Windows" wording). |
| Deep links (registration, association, delivery) | **PASS** — handler entry written with `MimeType=x-scheme-handler/tidal`; `xdg-mime query default x-scheme-handler/tidal` → `oxitide-app-handler.desktop`; delivery verified. |

## 4. Track 9 surfaces on WebKitGTK

| Surface | Result |
|---|---|
| Light mode + dynamic color | **PASS** — Dark/Light/System renders correctly; album-art tint flows into surfaces; theme persists in `config.toml`. |
| Hi-Fi Rack | **PASS** — shell renders (network transport, VU meters, stereo amp, EQ, playlists); DSEG7 segment display works; `FLAC 16BIT 44.1kHz` and the output notice show in the transport; mini player (300×458) enters/exits cleanly. |
| Track info / credits | **PASS** — context menu → Track info shows album, artists, duration, quality, mixer, additional producer, A&R, producer, writer, vocals, masterer. |
| History / stats | **PASS** — completed plays are recorded (`history.jsonl`), Stats shows 7 min, 4 plays/tracks/artists/albums, 100% played-through, listen-hour histogram. |
| a11y focus/keyboard | **PARTIAL** — Tab focus ring visible (sidebar collapse control); the WebKitGTK window exposes an **empty AT-SPI tree** to the desktop client, so automated role/name checks could not run (issue #10). |
| Feed gray-out | **NOT RUN** — needs content blocks configured for the account; no blocked items present. |

## 5. Track 10 tray / hotkeys on Linux

| Row | Result |
|---|---|
| Tray icon appears | **PASS** — StatusNotifierItem registered, KDE host reports it. |
| Tray menu actions | **PASS** — Show/Hide toggles the window, Play/Pause pauses, Quit exits cleanly (invoked through `com.canonical.dbusmenu` to mirror a real click). Menu: Show/Hide, Play/Pause, Next, Previous, ─, Quit. |
| Tooltip / track title | **FAIL (expected caveat) + issue #3** — SNI `Title` is `oxitide-app` and `ToolTip` is empty (tooltips are Windows/macOS-only in `tray.rs`); the track tooltip could not be observed. |
| Left-click behavior | **PASS with caveat** — the SNI item has no `Activate` handler, so KDE opens the menu on click (the documented "menu on left click" caveat). |
| Global hotkeys (Wayland portal) | **NOT RUN** — Settings → System hotkey chords were not exercised in this session. |

## 6. Track 11 phone remote

| Row | Result |
|---|---|
| Listener / reachability | **PASS** — `Listening on http://192.168.1.116:47820/remote`; `GET /remote` → 200 on loopback and LAN IP; `POST /api/rpc` without a cookie → 401 `unauthorized: pair this phone first`; spoofed `Host` → 403. |
| `ufw` hint (Linux) | **PASS** — footer shows "If a phone cannot connect, allow the port through your firewall (for example: sudo ufw allow 47820/tcp)." |
| Pairing from a real phone | **NOT RUN** (Windows run covered it; Linux server is reachable). |

## 7. Perf — first Linux corpus (T8-D3 exemption)

`OXITIDE_PERF_SCENARIO=S1`/`S2` + `node scripts/perf/check.mjs` on the release build:

| Scenario | first_paint | ui_ready | startup_step_max | memory_max | Windows budget |
|---|---|---|---|---|---|
| S1 cold | 460 ms (max 1500 ✓) | 1013 ms (max 2500 ✓) | 36 ms (max 100 ✓) | **633 MB** | 570 MB ✗ |
| S2 warm | 424 ms (max 1000 ✓) | 716 ms | 37 ms | **618 MB** | 560 MB ✗ |

As expected, the memory rows exceed the Windows-derived budgets because Linux sums all
descendants (WebKit web/network/GPU processes); frame/interaction rows are absent under the
T8-D3 exemption. **633/618 MB is the first Linux baseline** — budgets need a Linux-specific
calibration before they can gate anything.

## 8. Issues

Register first, detail below. #13/#14 are fixed by `crates/oxitide-audio/src/alsa_output.rs`
in commit `7498179`; §12 records what the patch changes.

| # | Issue | Severity | Status |
|---|---|---|---|
| 1 | Split state directories (`oxitide` vs `OxiTide`) | low | open |
| 2 | MPRIS `Stop` behaves as pause | medium | open |
| 3 | Tray SNI title `oxitide-app`, empty tooltip | cosmetic | open |
| 4 | MPRIS `DesktopEntry` does not resolve per package | packaging | open |
| 5 | Exclusive-fallback notice starts lowercase | cosmetic | open |
| 6 | Window geometry persisted at 2× the logical size | low | open |
| 7 | `xdg-mime: qtpaths: command not found` at startup | cosmetic | open (environment) |
| 8 | Mix cards show placeholder art | data | open (confirm payload) |
| 9 | Updater check fails on an unpublished release | expected | open (needs two releases) |
| 10 | Empty AT-SPI tree for the WebKitGTK window | a11y | open |
| 11 | Exclusive mode unreachable from the picker | **high** | open — blocks the audio path |
| 12 | MPRIS `Volume` is read-only | low | open |
| 13 | 44.1 kHz exclusive opens fail on buffer/period math (`EINVAL`) | **high** | fixed (`7498179`) |
| 14 | `snd_pcm_start` on an empty buffer returns `EPIPE` (USB DAC) | **high** | fixed (`7498179`) |

1. **Split state directories (functional, low).** Config/session/cache/history live in
   `~/.config/oxitide/`, logs in `~/.config/OxiTide/logs/` (`config.rs:541-553` mixes
   `ProjectDirs` with `BaseDirs` + the literal "OxiTide"). Two directories for one app; the
   "open logs folder" path and the docs disagree.
2. **MPRIS `Stop` behaves as pause (functional, medium).** `playerctl stop` and KDE's
   `stopmedia` leave `PlaybackStatus=Paused` with the last track's metadata; `ControlCommand`
   has no `Stop` variant (`media_controls.rs:221-223` maps Stop to PlayPause while playing).
   The smoke row "stopping playback clears the metadata" fails; spec §4.3 lists Stop.
3. **Tray identity (cosmetic).** SNI `Title` is `oxitide-app`, not "OxiTide"; `ToolTip` is
   empty on Linux (documented), so the desktop tooltip never shows the track.
4. **MPRIS desktop-entry resolution (packaging).** `DesktopEntry=OxiTide`
   (`media_controls_linux.rs:117`) does not match the AppImage's
   `oxitide-app-handler.desktop` or the AUR recipe's `oxitide.desktop`; only the deb's
   `OxiTide.desktop` matches. Confirm the KDE/GNOME widget name/icon per package; consider
   `set_desktop_entry` per install path (already a parked 8.4 item).
5. **Notice casing (cosmetic).** The exclusive-fallback notice begins lowercase
   ("exclusive mode unavailable; using shared output", `alsa_output.rs:427`) and shows verbatim
   in Settings and the Rack display.
6. **Window geometry persisted at 2× the logical size (low, robustness).** On a 1.75-scale
   display a 900×620 window is saved as `window_width=1800`, `window_height=1184`, and a
   maximized 1646×1029 as 3292×2002. The round-trip is self-consistent (restores 900×620), but
   the stored units do not match the compositor's; a scale-factor change would restore the
   wrong size.
7. **`xdg-mime: qtpaths: command not found` at startup (cosmetic).** KDE has no `qtpaths` on
   this install; `xdg-mime` falls back and registration still works, but the warning is noisy.
8. **Mix artwork placeholders (data, to confirm).** Home → "Made for you" (My Daily Discovery,
   My Mix 1–5) renders gray placeholder art with a note icon, while playlist/album covers load.
   Either TIDAL returns no `image` for mixes or the mix path drops it — check the API payload.
9. **Updater check (expected).** AppImage startup logs `startup update check failed: Could not
   fetch a valid release JSON from the remote` for 0.4.0; the in-app update path still needs two
   published releases.
10. **No AT-SPI tree for the WebKitGTK window (a11y, to investigate).** The KWin AT-SPI client
    sees zero elements for the OxiTide window, so automated accessibility checks cannot run;
    this may also affect screen readers on Linux. Keyboard focus itself works.
11. **Exclusive mode is unreachable from the UI (functional, high — open).** `snd_device_name_hint`
    on this alsa-lib version emits no `hw:`/`plughw:` names at all (`aplay -L` lists only
    `default`, `sysdefault:*`, `front:*`, `surround*`, `iec958:*`, `usbstream:*`), and
    `hint_is_pickable` keeps just `default`/`sysdefault`/`hw:`/`plughw:`. The picker therefore
    offers `default` and `sysdefault:CARD=Pro` only, while exclusive is attempted only for an
    id that starts with `hw:` (`is_direct_hw`). To make it worse, `Settings.svelte:516`
    normalizes a saved id that is missing from that list to `""` and persists it, so a hand-set
    `hw:CARD=Pro,DEV=0` survives only until Settings is opened. Two changes are needed: surface
    direct PCMs (synthesize `hw:CARD=…,DEV=…` from the card hints, or map
    `sysdefault:CARD=x`/`front:CARD=x,DEV=n` to their `hw:` equivalent) and decide how they are
    labelled/deduped against the plugin entries.
12. **MPRIS `Volume` is read-only (low).** `playerctl volume 0.35` is accepted by D-Bus but the
    property stays `1.0` and the app volume does not move; hardware volume changes only come
    from the in-app slider. Either expose it (the engine already takes `set_volume`) or leave it
    documented as unsupported.
13. **44.1 kHz exclusive opens failed on buffer/period math (high, fixed in `7498179`).**
    `AlsaOutput::open_pcm` asked for `buffer = rate/2` frames (22 050 at 44.1 kHz) and then
    `set_periods(4, Nearest)`. Four periods cannot split 22 050, and
    `snd_pcm_hw_params_set_periods` answered `EINVAL`, so *every* exclusive open of a 44.1 kHz
    file — the overwhelming majority of lossless music — fell back to shared, on any device.
    48/96 kHz divided evenly, which is why the DAC initially looked like the problem. The fix
    rounds the buffer down to a multiple of the requested periods (`buffer_frames`, 22 048
    frames at 44.1 kHz) and keeps 48 kHz at 24 000.
14. **`snd_pcm_start` on an empty prepared PCM returns `EPIPE` (high, fixed in `7498179`).**
    On the Dawn Pro the exclusive open succeeds, then `snd_pcm_start` fails with
    `Broken pipe (32)` at 44.1/48/96/192 kHz, and the engine turned that into a playback error
    before a single frame was written. A start with nothing queued makes the USB driver begin
    streaming and underrun at once; the following `write` starts the stream itself once the
    buffer reaches the start threshold. The fix treats `EPIPE` from start as benign
    (`is_start_underrun`), re-prepares, and lets the first write start the stream. Both fixes
    carry unit tests (period divisibility across 8–192 kHz; EPIPE-only classification).

## 9. Not verified / blocked

- DAC hotplug (unplug/replug needs a hand on the cable); format step-down (`S32_LE → S24_3LE →
  S16LE`) because the Dawn Pro accepts 32-bit containers at every rate.
- Sample-rate following through the GUI: the restored queue is CD-quality (44.1 kHz), so
  multi-rate opens were verified through `AlsaOutput` directly (44.1/48/96/192, §11), not by
  playing a 48/96 kHz track in the app.
- PulseAudio-distro row, X11 session row (only Plasma Wayland is installed).
- WebKitGTK 4.1 floor on Ubuntu 22.04/Debian 12; AppImage-on-22.04.
- AUR `makepkg -si` (recipe URL/tag drift, issue #4-adjacent).
- GNOME media widget (KDE only here).
- Wayland global hotkeys (Settings → System chords).
- Drag-and-drop and Save-for-Later interactions (menu entries present; drop/click paths not
  exercised by automation).
- Feed content-blocking gray-out (no blocked items configured).
- Real-phone pairing on Linux (server reachable and protected, verified above).

## 10. Closure notes

- Track 8's closure gate is **closer**: exclusive/bit-perfect and the hardware mixer now pass
  on real hardware. Remaining: the picker fix (issue #11) so a direct device can actually be
  selected, DAC hotplug, the two oldest-base checks, AUR and the MPRIS desktop-entry
  resolution.
- Fix issue #2 (MPRIS Stop) or renegotiate the row; #11 is the only blocker on the audio path.
- #13/#14 are fixed in the working tree only (§12). They should be committed (or rewritten)
  before anyone reruns the DAC rows, otherwise the run repeats the shared fallback.
- Before `v1.0.0`: render the AUR `PKGBUILD` from a pushed tag and point the template at the
  current `origin` (GitHub), then run `makepkg -si`; otherwise label Linux "preview" in the
  1.0 notes as `2026-10-17-v1-release-gate.md` §6 allows.

## 11. DAC follow-up (MOONDROP Dawn Pro)

Plugged in the same day, after the batch above. `Comtrue` USB (`2fc6:f06a`), ALSA card 3,
`hw:CARD=Pro,DEV=0`; `S16_LE`/`S24_3LE`/`S32_LE` at 44.1–384 kHz, one play-volume element
(`PCM`, no `Master`). The device is the PipeWire default sink and is held open by WirePlumber
while a stream runs, released after its suspend timeout.

**Three bugs found; two fixed (#13, #14 in `7498179`), one open (#11).**

1. **Issue #13 — 44.1 kHz exclusive opens failed on buffer/period math.** The first exclusive
   attempt on the DAC opened as `Shared` at every rate, and the step-by-step ALSA probe showed
   `snd_pcm_hw_params_set_periods` returning `EINVAL` at 44.1 kHz only. Fixed by rounding the
   buffer down to a multiple of `ALSA_PERIODS` (`buffer_frames`).
2. **Issue #14 — `snd_pcm_start` on an empty PCM returns `EPIPE` on this DAC.** After the
   buffer fix, the open succeeded as `Exclusive` in the app log and then died in `start`
   (`Broken pipe (32)`) at 44.1/48/96/192 alike. Fixed by treating a start underrun as benign
   (`is_start_underrun`), re-preparing and letting the first `write` start the stream.
3. **Issue #11 — the picker cannot offer a direct device.** The reason the device id had to be
   written into `config.toml` by hand, and the remaining blocker for real use.

### How the bugs were isolated

The first exclusive attempt reported the fallback notice at every rate, which could plausibly
have been "PipeWire holds the card". Two checks separated the causes:

- `fuser /dev/snd/pcmC3D0p` empty and `aplay -D hw:CARD=Pro,DEV=0 -f S32_LE -r 44100 …`
  exiting 0 proved the device was free and openable outside the app.
- A throwaway `cargo run -p oxitide-audio --example dac_steps` (since deleted) reproduced
  `AlsaOutput::open_pcm` step by step at `hw:CARD=Pro,DEV=0`; the failing call printed as
  `rate FAILED` / `periods FAILED` / `start FAILED` per rate, which named the two bugs. The
  same harness then verified the fixes (`mode=Exclusive`, `bit_perfect=true` at
  44.1/48/96/192) before the app was rebuilt.

**Evidence after the fixes.**

| Check | Result |
|---|---|
| `AlsaOutput` exclusive, 44.1/48/96/192 kHz, 16/24-bit | **PASS** — `mode=Exclusive`, `bit_perfect=true`, no notice, hardware volume OK at every rate |
| App end to end (device id written into `config.toml`) | **PASS** — `/proc/asound/card3/pcm0p/sub0/hw_params`: `S32_LE`, `rate 44100`, `period 5512`, `buffer 22048`; `status` owner is a thread of the app process; `pactl list short sink-inputs` empty; player bar shows `✓ BP` |
| Track/reopen behaviour | **PASS** — a 24-bit track reopened at `rate=44100 bits=24`; pause past `PAUSE_RELEASE` (30 s) closed the PCM, resume reopened it exclusively with no PipeWire hijack in between |
| Hardware volume | **PASS** — slider drag → DAC `PCM` 48/127 (38 %, −39.5 dB) with the stream RUNNING and `✓ BP` still lit |
| Rate following (backend) | **PASS** — the same `AlsaOutput` opened 44.1/48/96/192 sequentially; a 48 kHz-only device would have been accepted at a nearby rate, because `set_rate(…, Nearest)` is never read back (follow-up: make exclusive asks `Exact`) |

**Not exercised:** hotplug (needs the cable), format step-down (device accepts `S32_LE`
everywhere), GUI sample-rate switching (queue is CD-quality).

### Reproduction commands (all run as the desktop user)

```
cat /proc/asound/card3/stream0                 # formats/rates the DAC advertises
aplay -D hw:CARD=Pro,DEV=0 -f S32_LE -r 44100 -c 2 -d 1 /dev/zero   # free-device probe
fuser -v /dev/snd/pcmC3D0p                     # who holds the PCM (pipewire while it plays)
amixer -c Pro scontrols                        # single 'PCM' play-volume element
cat /proc/asound/card3/pcm0p/sub0/hw_params    # S32_LE / 44100 / period 5512 / buffer 22048
sed -n 1,3p /proc/asound/card3/pcm0p/sub0/status   # RUNNING + owner_pid (app worker thread)
pactl list short sink-inputs                   # empty => the app bypasses PipeWire
playerctl play | pause | next | volume 0.35
```

## 12. ALSA patch (`7498179`)

The docs were committed first (`d71ea22`) while the fixes were still working-tree-only; the
owner then asked for them to go up too, so they land as `7498179`
(`crates/oxitide-audio/src/alsa_output.rs`, +68/−6):

- `ALSA_PERIODS`, `buffer_frames(rate)` and their use in `open_pcm` (fixes #13);
- `is_start_underrun(&alsa::Error)` and the re-preparing `start()` (fixes #14);
- two unit tests (`buffer_frames_divide_evenly_into_periods_at_every_rate`,
  `only_epipe_counts_as_a_start_underrun`) and three debug-level `tracing` lines that report
  which device/mode/rate was opened and why an exclusive format attempt failed.

Verified with the patch present: `cargo test --workspace` (1094 passed, 0 failed),
`cargo clippy --workspace -- -D warnings` clean, and the end-to-end DAC checks above. `main` is
at `7498179`; the release binary used for the pass was built from that tree.

Issue #11 (picker) is **not** patched — exclusive still cannot be selected from the UI; the DAC
rows above used a device id written into `config.toml` by hand.

## 13. Evidence index (local, gitignored)

| Path | Contents |
|---|---|
| `.superpowers/linux-run/screens/` | `dac-01`…`dac-11` (start state, playing bar with `✓ BP`, 38 % volume), earlier smoke captures |
| `.superpowers/linux-run/perf/` | S1/S2 JSONL from the perf corpus |
| `.superpowers/linux-run/logs/` | `app*.log` (startup, deep-link, MPRIS), `dac-app*.log`, `cargo-test.log`, `config-before-dac.toml`, `localstorage-before-dac.sqlite` |
| `.superpowers/linux-run/app.pid` | last launch pid (used to stop the app cleanly, never `pkill -f`) |

Nothing under `.superpowers/` is committed; this note quotes the numbers so the record survives
without it.
