# Linux v1 final run (2026-11-05)

- **Machine:** CachyOS laptop (`Vivobooo`), KDE Plasma Wayland, PipeWire, `ufw` active. Same host as
  `2026-10-23-linux-bare-metal-run.md` and `2026-10-28-linux-v1-close-run.md`.
- **Code:** `main` at `53cef6d` (version 0.6.0), tree clean before the run.
- **DAC:** MOONDROP Dawn Pro connected at the start (card 3, `Pro`).
- **Goal:** walk every row left open by `2026-10-28-linux-v1-close-run.md` ("Still open for the
  owner", "Open choices") and §14.3 of the 2026-10-23 note. The owner did the cable, clicks, phone
  and listening. The agent read `playerctl`/D-Bus, `/proc/asound`, `hw_params` and the logs
  (`RUST_LOG=info,alsa=debug`).
- **Not in scope:** the 22.04 oldest-base checks and the AppImage updater path (they need other
  hosts or a release), and the `v1.0.0` tag.

## Build

| Check | Result |
|---|---|
| `npm --prefix ui run app:build` | **PASS**: `OxiTide_0.6.0_amd64.deb` (10.57 MiB), `OxiTide_0.6.0_amd64.AppImage` (115.53 MiB) |
| deb control | **PASS**: `Package: oxi-tide`, `Version: 0.6.0`, `amd64`, `Depends: libasound2, xdg-utils, desktop-file-utils, libayatana-appindicator3-1, libwebkit2gtk-4.1-0, libgtk-3-0` (read with `ar`/`tar`; no `dpkg-deb` here) |
| deb desktop entry | **PASS**: `Name=OxiTide`, `Exec=oxitide-app`, `Icon=oxitide-app`, `StartupWMClass=oxitide-app`, `Categories=AudioVideo;Audio;Music;`, `MimeType=x-scheme-handler/tidal;` |
| AppImage extract | **PASS**: `OxiTide.desktop` identical to the deb's |

The rows below ran on the AppImage, started with an empty scratch `XDG_DATA_HOME` so the stale
23 Oct entries in `~/.local/share/applications` (`OxiTide.desktop`, `oxitide.desktop`,
`oxitide-handler.desktop`, `oxitide-app-handler.desktop`) could not answer the desktop-entry lookup.

## Results

### MPRIS, SNI, desktop entry

| Row | Result |
|---|---|
| `playerctl volume 0.35` moves the app slider | **PASS**: reads back `0.350000`; the owner saw the slider at 35 %. |
| Dragging the slider updates `playerctl volume` | **PASS**: dragged to about 75 %, `playerctl volume` read `0.71`. |
| SNI `Title` | **PASS**: `"OxiTide"` (item `tray_icon_tray_app_main`). |
| MPRIS `DesktopEntry`, AppImage | **PASS**: `OxiTide`, resolved from the mount's `usr/share/applications/OxiTide.desktop` (AppRun puts it first on `XDG_DATA_DIRS`). Only processes started by AppRun see that path, so the desktop's own lookup cannot; that is inherent to AppImages. |
| MPRIS `DesktopEntry`, deb | **PASS**: the extracted deb binary with `XDG_DATA_DIRS=<deb>/usr/share` reports `OxiTide`. `dpkg` is not available on this host, so this is the deb's own tree, not a `dpkg -i` install. |
| MPRIS `DesktopEntry`, AUR | **PASS**: the `makepkg` package's `/usr/bin/oxitide` with its `usr/share` reports `oxitide`. Because the result differs per package, the lookup reads the installed file rather than falling back to `OxiTide`. |

### DAC and HDA (§14.3 audio rows)

| Row | Result |
|---|---|
| Picker lists `MOONDROP Dawn Pro, USB Audio (direct hardware)`; exclusive on plays `✓ BP` | **PASS**: picked in Settings (no hand edit; `config.toml` then read `output_device = "hw:CARD=Pro,DEV=0"`, `exclusive = true`). `hw_params`: `S32_LE`, 44100, `period_size 5504`, `buffer_size 22016`. Owner saw `✓ BP` and heard the DAC. |
| Pause and resume on the exclusive DAC | **PASS**: paused, the PCM sits in `XRUN` (the engine stops writing); Play reopens `hw:CARD=Pro,DEV=0` and the PCM is `RUNNING` again at the same format and position. |
| Same device, exclusive **off** | **PASS**: `open requested=Shared device=hw:CARD=Pro,DEV=0` opens `plughw:CARD=Pro,DEV=0` at 44100 / `S32_LE`; PipeWire's sinks stayed `SUSPENDED`; no notice (owner did not report one). |
| HDA `hw:` entry, exclusive 44.1 kHz | **PASS**: `hw:CARD=Generic_1,DEV=0` (ALC294 Analog, card 1; card 0 is HDMI, card 2 ACP is capture only) opens exclusive at 44100, `S32_LE`, period 5504 / buffer 22016, `RUNNING`, no fallback. The 64-frame period alignment from §14.1 holds (5504 × 8 B = 344 × 128 B). |
| Unplug while playing | **PARTIAL**: pauses and keeps the queue and position, like WASAPI. The UI shows the raw `ALSA function 'snd_pcm_recover' failed with error 'No such device (19)'` instead of a friendly message (issue L1). |
| Unplug, then start a track | **PASS**: `shared open of sysdefault:CARD=Pro failed: … No such device (19)`, opens `default`, the speakers play with "Selected output device unavailable; using system default". The Settings "(not connected)" label was not confirmed. |
| Replug: playback moves back by itself (`53cef6d`) | **FAIL on ALSA**: no reopen after the replug. `needs_reopen` is implemented only in `wasapi_output.rs`; `AlsaOutput` keeps the trait default (`false`). The sound still reached the DAC because WirePlumber moved the `default` stream to the new USB sink (PipeWire sink `RUNNING` at 48 kHz, our stream 44.1 kHz into it), but the notice stays and the output is PipeWire, not the selected device (issue L2). |
| Replug, next track plays on the DAC | **FAIL**: the skip made no `open` (same spec, so the engine keeps the fallback device; see `docs/agents/audio.md` "never rely on a spec comparison to leave a fallback"). |
| Replug, re-select the device while playing | **FAIL**: the re-select reaches the engine (`open requested=Shared device=sysdefault:CARD=Pro` at 10:24:32), but the open fails with `Device or resource busy (16)`: PipeWire still holds the DAC for our fallback stream, so it falls back again (issue L2). |
| Replug, pause about 10 s, re-select, Play | **PASS (workaround)**: after WirePlumber suspends the sink, the open succeeds, the notice clears, and PipeWire has no stream left. |

Note for the picker: the owner's replug runs used `sysdefault:CARD=Pro` (the hint entry). On this
box that PCM goes through `dmix`: `hw_params` showed 48000 Hz, period 1024, buffer 16384, so
44.1 kHz material is resampled. The "direct hardware" entry (`hw:` through `plughw:`) kept
44.1 kHz in shared mode. Working as designed, but the two entries look alike in the picker.

### Owner rows

| Row | Result |
|---|---|
| Issue #8: mix artwork on Linux | **PASS**: mix tiles show their covers. |
| Feed gray-out (Settings → Content → Block explicit) | **PASS**: explicit tracks gray out in the Feed and do not play. |
| Issue #10: empty AT-SPI tree | **Not reproduced, closed.** Read with `gi.Atspi` (at-spi bus running): `application oxitide-app` → `frame "OxiTide"` → `document web "Explore · OxiTide"` with the skip link, landmarks, headings, 169 buttons, 82 links, 2 sliders, 2 combo boxes and the track table. The 10-23 empty tree was most likely read before an AT client had made WebKitGTK build its tree, which it does lazily. No owner decision needed. |
| Issue #3: empty tray tooltip | Accepted caveat (no tooltip API in libappindicator), unchanged. |
| Phone pairing from a real phone | **PASS**: remote on, listening on `*:47820`, the phone paired (approval on) and play/pause, skip, volume, track and cover all worked. `ufw` is active and blocked it until the owner ran `sudo ufw allow 47820/tcp` by hand (issue L4). |
| Listening check | **ISSUES** (issue L3): audible clicks when sound starts and stops (skips, seeks, progress-bar clicks) and a slight delay on transport. Heard on `sysdefault:CARD=Pro`, exclusive **off** (the setting left from the replug rows), not the exclusive direct path. Underrun logs show only the expected start-of-open underrun. |
| Tray hide-and-show size (from 10-28) | **REPRODUCED** (issue L5): the first tray cycle kept the size; later cycles, through the tray icon or a second launch, came back resized to fill the work area. Not maximized: the maximized state stays off. |
| `tidal://` and single instance | Not re-run (PASS on 10-23). A second launch did hand off to the running instance and exit 0. |

### AUR

The template now points at GitHub (`url="https://github.com/Zephulbr/OxiTide"`), and GitHub has the
`v0.6.0` tag; Codeberg still refuses anonymous clones (`could not read Username`), so the 10-28 host
question is moot. `node scripts/release/make-pkgbuild.mjs v0.6.0 <dir>`, then `makepkg --nodeps`
in a scratch dir (`--nodeps` only because cargo comes from a user rustup, not the `rust` package):

- **FAIL as published:** `build()` fails at the link: `rust-lld: undefined symbol:
  ring_core_0_17_14__…` for every `ring` C function. This host's `makepkg.conf` has `lto` in
  `OPTIONS` (stock Arch does too), so `CFLAGS` gets `-flto=auto` and ring's C objects are GCC LTO
  bitcode that `rust-lld` cannot read.
- **PASS with `options=('!lto')`:** `oxitide-0.6.0-1-x86_64.pkg.tar.zst` (9.1 MB) with
  `/usr/bin/oxitide`, `oxitide.desktop` (`Exec=oxitide`, `StartupWMClass=oxitide`,
  `MimeType=x-scheme-handler/tidal;`), the four icons and both license files. It launches and
  registers MPRIS (`DesktopEntry=oxitide`).
- **Fixed in this commit:** `packaging/aur/PKGBUILD.in` sets `options=('!lto')`;
  `node --test scripts/release/release.test.mjs` passes. Releases up to `v0.6.0` carry the old
  PKGBUILD. Installing (`makepkg -si` / `pacman -U`) stays with the owner.

## Issues

| # | Issue | Next step |
|---|---|---|
| L1 | Unplugging the ALSA device mid-play shows the raw `snd_pcm_recover … No such device (19)` text. `AlsaOutput::write` passes `alsa::Error` straight through `to_error`; WASAPI returns `output device unavailable` and sets the notice `output device disconnected`. | Map `ENODEV` (and `EBADFD` after unplug) in `write`/`start` to the WASAPI error and notice. Small; recheck on the DAC. |
| L2 | `53cef6d`'s return-to-device does not exist on ALSA (`needs_reopen` is WASAPI-only). On PipeWire the fallback stream also pins the replugged DAC, so even an immediate re-select fails with `EBUSY`. | Give `AlsaOutput` a `needs_reopen`: on a fallback open, poll (or watch udev/`snd_ctl` card events) for the selected card to come back. Before reopening, close the `default` PCM and retry `EBUSY` for a few seconds while WirePlumber suspends the sink. |
| L3 | Clicks at start, stop, seek and skip; slight transport delay. In code: pause only stops writing, so the ~0.5 s ALSA buffer (22 016 frames at 44.1 kHz on `hw:`, ~340 ms on `dmix`) plays out and ends in an `XRUN` edge; `flush` is `snd_pcm_drop` mid-waveform; an open starts at full level ("never gliding to it"); format changes between tracks close and reopen the device. The only fade is the sleep timer's `FadeAndPause`. | Short declick ramp (a few ms) on pause, seek, skip and stop, `drop` on pause instead of play-out, and review the 4 × ~125 ms buffer. A sample-altering ramp only at transport edges must keep steady-state exclusive output bit-perfect. Check whether WASAPI clicks too. Re-listen on the exclusive direct path. |
| L4 | `ufw` blocks the phone remote until the user adds a rule by hand; the footer hint is the only guidance. | Detect an active `ufw`/`firewalld` and say why ("phones on your Wi-Fi need to reach TCP port 47820") with the exact command; consider a one-click `pkexec ufw allow 47820/tcp`. Owner request. |
| L5 | Tray hide and show comes back resized to the work area (not maximized) after the first cycle. Tray and single-instance show both call `show`/`unminimize`/`set_focus`; hiding does not save the geometry (`config.toml` kept the stale 1646×1001). Cause not verified (GTK/KWin on remap). | Linux: record the outer size and position on hide-to-tray and re-apply them after `show()`. Owner call. |
| L6 | `sysdefault:CARD=<x>` and the "direct hardware" entry look alike in the picker; the former resamples through `dmix`. | Consider labelling or hiding `sysdefault:` entries when a direct entry exists for the same card. |
| — | Issue #7 still logs `xdg-mime: line 885: qtpaths: command not found` with `qt6-tools` installed (Arch ships `qtpaths6`). | Unchanged (registration still works). |
| — | Each open logs `WARN output underrun during track … (1)` right after the expected "start underran; the first write starts it". | Cosmetic: do not count the deliberate empty-start underrun. |

## Open choices (owner)

- **Linux label for 1.0.** Everything on the 10-28 owner list has now run. L1–L3 are audio-path
  quality issues on Linux (L3 may not be Linux-only): fix before the tag, or ship Linux as
  "preview" with them listed in the notes.
- **L5** (tray size) and **L4** (firewall hint): fix for 1.0 or after.
- Still not run: the 22.04 oldest-base launch and the AppImage updater path (release gate §6).

## Side effects on this machine

- `config.toml`: `output_device = "sysdefault:CARD=Pro"`, `exclusive = false`, `[remote] enabled =
  true`, Block explicit set back by the owner. `ufw` has the owner's `47820/tcp` rule.
- No OxiTide process left running. Scratch `XDG_DATA_HOME` dirs and the extracted packages are in the
  session scratchpad; `~/.local/share/applications` was not touched.
