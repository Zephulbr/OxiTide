# Linux v1.0 handoff run (2026-11-10)

- **Machine:** CachyOS laptop (`Vivobooo`), KDE Plasma Wayland, PipeWire, `ufw` active. Same host as
  `2026-11-05-linux-v1-final-run.md`.
- **Code:** `main` at `2c564c3` (version 0.7.0), tree clean before the run.
- **DAC:** MOONDROP Dawn Pro connected (card 3, `Pro`); HDA analog is card 1 (`Generic_1`).
- **Plan:** `2026-11-10-linux-v1-handoff.md` §1–§7.
- **How it ran:** the owner was away from the machine, so the agent drove the GUI itself:
  - AT-SPI (`gi.Atspi`: press buttons, radios and tabs, read sliders and states);
  - a uinput virtual absolute pointer for real pointer drags and clicks, needed by instances that
    are not on the host's a11y bus;
  - KWin scripting over D-Bus for window geometry, and the tray's `com.canonical.dbusmenu` for
    Show/Hide and Quit;
  - `spectacle -b` screenshots, `playerctl`/MPRIS, and `/proc/asound/*/hw_params` and `status`;
  - `pactl`, and logs at `RUST_LOG=info,alsa=debug,window=debug`.

  Rows that need hands or ears are marked **owner**. The scripts were throwaway and stayed in the
  session scratchpad.

## 1. Gates and build

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | **PASS** (no output) |
| `cargo test --workspace` | **PASS**: 1237 passed, 0 failed |
| `cargo clippy --workspace -- -D warnings` | **PASS** |
| `npm --prefix ui ci`, `run check` | **PASS**: svelte-check 732 files, 0 errors, 0 warnings |
| `npm --prefix ui test` | **PASS**: 169 files, 1373 tests |
| `npm --prefix ui run app:build` | **PASS**: `OxiTide_0.7.0_amd64.deb` (10.84 MiB), `OxiTide_0.7.0_amd64.AppImage` (115.80 MiB) |
| deb control (read with `ar`/`tar`) | **PASS**: `Package: oxi-tide`, `Version: 0.7.0`, `amd64`, `Depends: libasound2, xdg-utils, desktop-file-utils, libayatana-appindicator3-1, libwebkit2gtk-4.1-0, libgtk-3-0` |
| deb desktop entry | **PASS**: `Name=OxiTide`, `Exec=oxitide-app`, `Icon=oxitide-app`, `StartupWMClass=oxitide-app`, `Categories=AudioVideo;Audio;Music;`, `MimeType=x-scheme-handler/tidal;` |
| AppImage extract | **PASS**: `OxiTide.desktop` identical to the deb's |

## 2. Re-checks of what changed since 11-05 (locally built 0.7.0 AppImage)

### C1: output switch (`f99421a`)

Started on the Dawn Pro direct `hw:` exclusive, `✓ BP` (`hw_params`: `S32_LE`, 44100, period 1600,
buffer 6400). Each row: the click time, the engine log, the position over the next 2.5–3 s
(`playerctl position`), and the card state.

| Switch | Result |
|---|---|
| Dawn Pro exclusive → `default` | **PASS**: mode change, so it restarts at the position as designed (`open requested=Shared device=default` 0.8 s after the click; position 54.5 → 56.7 s over 2.5 s). The PipeWire stream lands on the Dawn Pro sink. |
| `default` → Dawn Pro exclusive | **PASS, slow** (issue L8). `open requested=Exclusive device=hw:CARD=Pro` 0.5 s after the click, then the L2/L7 card wait: PipeWire still held the DAC (card 3 at 48 kHz `MMAP_INTERLEAVED`, sink `IDLE`), and the first write ran at +5.5 s. The position held at 75.0 s during the wait, then continued. No `EBUSY` error, no notice; it ended `S32_LE` 44100 `RUNNING`, bit-perfect. |
| Dawn Pro exclusive → `default` | **PASS**: restart at the position in 0.6 s. |
| `default` → HDA `hw:` (both shared) | **PASS, in place**: `output moved to hw:CARD=Generic_1,DEV=0 in 139 ms` (through `plughw:`, 44100 `S32_LE`). Position continuous. |
| HDA shared → HDA exclusive | **PASS**: mode change; restart at the position, `hw:CARD=Generic_1` exclusive open and first write within 2 ms. |
| Paused: HDA → Dawn Pro, then Play | **PASS**: while paused, `output moved to hw:CARD=Pro,DEV=0 in 30 ms` (PCM `PREPARED`, position 114.7 s held). Play: card 3 `RUNNING` at 44100, position continues from 114.7 s. |

Every open logged `start underran …; the first write starts it` (expected). Two moves into
`default` logged one `output underrun during track … (1)` each, the cosmetic warning noted on
11-05.

### C2: unplug on direct `hw:` exclusive (`2e25fc1`)

**FAIL, fixed (issue L9), then PASS** (owner unplugs). The agent cannot unplug the DAC without root
(USB `authorized`/unbind), so the owner pulled the cable.

- First unplug (0.7.0): the error showed as raw ALSA text, `ALSA function 'snd_pcm_recover' failed
  with error 'I/O error (5)'`. The kernel logged the USB disconnect in the same second. On 11-05 the
  same unplug surfaced as `ENODEV (19)`, which already maps to "Output device disconnected".
- Fix: `EIO`/`EBADFD` from the write, start and flush paths now map to "output device disconnected"
  when the card's `/proc/asound` entry is gone within 500 ms (`device_error` in `alsa_output.rs`); a
  card still present keeps the raw error.
- Retest with the fix build: "Output device disconnected", queue and position kept
  (`device gone: … 'No such device (19)'` in the log; this unplug came as `ENODEV` again, so the
  `EIO` branch is covered by the unit test, not by this run).

### C3: boot screen (`9963b88`) and L5 window rows

Cold starts were screenshotted every ~0.4 s from launch.

| Row | Result |
|---|---|
| Dark (Atmosphere) | **PASS**: desktop → the window's first frame is the dark `--bg` with the mark and ring → the mark fades over the shell → the page. No white or empty frame. |
| Light (Atmosphere) | **PASS**: the first frame is already the light `--bg` with the mark, then the fade. No flash. |
| Maximized: three tray hide/show cycles | **PASS**: `show: mini or maximized; size left alone`, stays 1646×1001 maximized. |
| Unmaximized 1300×732: three tray cycles | **PASS**. In the first cycle KWin re-mapped the window at the work area (1646×1001); the 150 ms check restored 1300×732 (`show: size is Some((1646, 1001)) after 150 ms; restoring 1300x732`). Cycles 2 and 3 came back at 1300×732 directly. |
| Second launch while hidden | **PASS**: handed off to the running instance (one process), which showed at 1300×732. |

### C4: hidden window (`c56cc34`)

Playing, hidden to the tray for 61 s:

| Row | Result |
|---|---|
| MPRIS `Position` while hidden | **PASS**: 150.47 → 170.53 → 190.59 → 210.65 s at +1, +21, +41, +61 s. |
| Progress on show | **PASS**: the `Playback position` slider read 211 208 ms 0.3 s after Show, the same as MPRIS (211.21 s), and 212 323 ms a second later. |
| CatJam back in time | **PASS** (owner, 2026-11-11). |

### C5: now-playing mark (`a1c1a2b`)

Atmosphere, playing, Tracks page scrolled to the playing row (one mark on screen, confirmed by
screenshot). Per-thread CPU over 10 s from `/proc/<pid>/task/*/stat`:

| State | `oxitide-app` main thread |
|---|---|
| Tracks page, playing row off screen | 3.2 % |
| Playing row with the mark on screen | **8.4 %** (the 30 fps figure, ~8 %; 60 fps was ~25 %) **PASS** |
| Full Now Playing (spinning disc + lyrics), for reference | 10.1 % |

Smoothness and the pause/play ease: **PASS** (owner eye check, 2026-11-11).

### C6: colour and lamp glides (`9af1a67`)

Skipped with the window visible and took six screenshots across the 1.2 s glide.

- **Atmosphere: PASS.** The background moves gray → olive → gold in steps, with no flash.
- **Hi-Fi Rack: PASS.** The lamps and meters go amber → green → teal → blue, with no flash.

Smoothness: **PASS** (owner eye check, 2026-11-11).

### C8: My Collection edit mode and profile editor (`040fcd7`, `fa9bb53`, `fbffa7f`)

| Row | Result |
|---|---|
| Edit mode, pointer drag on WebKitGTK | **PASS**. A uinput pointer drag on Albums' grip reordered the list to Playlists, Tracks, Save for Later, Albums (read from the AT-SPI list). A second drag put it back. |
| Profile picture: file chooser | **PASS**. Edit profile → Change picture opens the GTK file chooser ("Select File", `xdg-desktop-portal-gtk`). Cancelled. |
| Profile picture: upload | **Owner, not run.** It would change the public TIDAL profile picture. |

## 3. AppImage updater (published 0.6.0 → 0.7.0)

The release `OxiTide_0.6.0_amd64.AppImage` (SHA-256 `49ca967b…`, matches `SHA256SUMS.txt`) went in
`~/Applications/`, `chmod +x`, and ran directly. `APPIMAGE` was set by AppRun. Real config and
login.

| Row | Result |
|---|---|
| Startup check finds it on its own | **PASS**: `update available version=0.7.0` 11 s after launch (`STARTUP_CHECK_DELAY`). The result is a toast with "Open Settings". The Settings card itself shows only "Check for updates" until clicked, then "Version 0.7.0 is available" (by design). |
| Install | **PASS**: Download and install → a confirmation dialog ("download from its GitHub releases, check its signature, run the installer and restart") → confirm. The app restarted after **42 s** (first run) and **31 s** (second run). |
| File after update | **PASS**: same path and name (`OxiTide_0.6.0_amd64.AppImage`), replaced in place, `-rwxr-xr-x`, 88 947 192 bytes, SHA-256 `fe96fa4f…` = the release `OxiTide_0.7.0_amd64.AppImage`. Settings → System reads "OxiTide 0.7.0 · You are on the latest version." |
| Log | First run: the restarted 0.7.0 logged `TIDAL unreachable at startup … dns error`, a failed Save for Later GET, and `startup update check failed` in its first 16 s. It then played normally. A cold start of the same file and a second full update run (fresh 0.6.0 download) were clean (`no update available`), so this is taken as a transient network blip, not reproduced. |
| Non-writable location | Not run (nothing in `/opt` here). |

The file keeps the old version in its name after an update; that is the updater's design (it
replaces `$APPIMAGE`), so desktop entries pointing at it keep working.

## 4. AUR

| Row | Result |
|---|---|
| `makepkg` of the `v0.7.0` release `PKGBUILD` (this host's stock `makepkg.conf`, `lto` in `OPTIONS`) | **PASS**. Built with `--nodeps` only because cargo is a user rustup; `options=('!lto')` took effect. `oxitide-0.7.0-1-x86_64.pkg.tar.zst` (9.2 MB) contains `/usr/bin/oxitide`, `oxitide.desktop` (`Exec=oxitide`, `StartupWMClass=oxitide`, `MimeType=x-scheme-handler/tidal;`), the four icons and both license files. |
| `sudo pacman -U` (2026-11-11, `oxitide 0.7.0-1`) | **PASS**. The old user-level installs went first (the `~/Applications` AppImage, `~/.local/bin/oxitide`, four `~/.local/share/applications` entries and the user icons); config and login kept. Launched through the packaged entry (`gtk-launch oxitide`): the session restored with no login prompt, and the window looks right (Atmosphere dark, sidebar, player bar). MPRIS `DesktopEntry=oxitide`, `Identity=OxiTide`. |
| `tidal://` handler | **FAIL, fixed (`2c84d0f`)**. The packaged entry declared `MimeType=x-scheme-handler/tidal;` with `Exec=oxitide` and no `%u`, so a login redirect opened before the app's first run would launch it without the link. After its first run the app writes its own `oxitide-handler.desktop` (`Exec="/usr/bin/oxitide" %u`), which `xdg-mime` then picks, so a login works from then on. `packaging/aur/oxitide.desktop` now has `Exec=oxitide %u`. No live login was sent (no fake auth codes). |
| Upgrade to a build of `main` (`oxitide 0.7.0-2`, commit `2c84d0f`, `makepkg -d` of the release `PKGBUILD` with a local `git+file` source) | **PASS**. `pacman -U` upgraded in place; the entry has `Exec=oxitide %u`; the binary has the C2 unplug fix; launch, session and MPRIS as above. Left installed as the day-to-day app until the next release replaces it. |
| `namcap` | Not installed. Not run. |

## 5. Oldest base: Ubuntu 22.04 (distrobox)

`distrobox create -n u2204 -i docker.io/library/ubuntu:22.04` (podman 6.1.4, `crun`). The host's
Wayland, PipeWire, session D-Bus and `/dev/snd` are passed through. The apt packages are the
handoff's list. Floor in the box: `libwebkit2gtk-4.1-0` **2.50.4-0ubuntu0.22.04.1**,
`libgtk-3-0` 3.24.33, glibc **2.35**. Release artifacts from `v0.7.0` (CI-built on 22.04; hashes
match `SHA256SUMS.txt`).

| Row | Result |
|---|---|
| AppImage launches (`--appimage-extract-and-run`) | **PASS**: no `GLIBC_x.y not found`, no missing library. Only warnings: `Locale not supported by C library` (the box has no `el_GR` locale) and tauri's `APPIMAGE … not detected as an AppImage` (expected with extract-and-run). |
| Login | **PASS (session restore)**: the box shares the host session bus, so the Secret Service is reachable and the saved login restored. There was no session-only notice. A fresh browser login was not run in the box. |
| Shared output | **PASS** after one container fix. The box had no ALSA `default` at first, so `snd_pcm_open … No such file or directory (2)` was shown as a raw playback error. A stock 22.04 desktop gets `default` from PulseAudio's ALSA plugin. With `/etc/alsa/conf.d/99-pulse-default.conf` (`pcm.!default { type pulse }`, the closest to stock 22.04) and an app restart, it plays as `ALSA plug-in [oxitide-app]` into the host's sink. |
| Atmosphere dark / light | **PASS**: like the host build (glass sidebar tint, dynamic colour, artwork, boot fade). The one difference is a classic GTK scrollbar on the page's right edge: the box has no Breeze GTK theme, so Adwaita's non-overlay scrollbar shows. That is a theme matter, not WebKitGTK. |
| Studio light / dark | **PASS**: solid surfaces as on the host. |
| Hi-Fi Rack | **PASS**: renders like the host (always dark). Two screenshots 0.4 s apart differ in the transport and VU regions (718 and 462 px changed), so the meters move. |
| deb installs and runs | **PASS**: `apt-get install ./OxiTide_0.7.0_amd64.deb` → `oxi-tide 0.7.0 install ok installed`. `/usr/bin/oxitide-app` runs, plays shared, and registers MPRIS `DesktopEntry=OxiTide`. |
| deb `tidal://` registration | **PASS**: `xdg-mime query default x-scheme-handler/tidal` → `OxiTide.desktop` in the box, and the shared `~/.local/share/applications/oxitide-app-handler.desktop` was re-pointed to `/usr/bin/oxitide-app` at startup. An end-to-end `xdg-open tidal://…` in the box hung in its xdg-open (no browser/opener in the box) and was killed; it never reached the app. |

No CSS feature failed to render on WebKitGTK 2.50.4, so the floor stays where it is.

## 6. Owner decisions (2026-11-10)

1. **dmix clicks (L3 on `sysdefault:` entries):** ship as a known issue (entries already read
   "ALSA default, may resample").
2. **WirePlumber replug quirk:** known issue line. C1 did not trigger it: all four switches,
   including direct `hw:` → `default` twice, left the Dawn Pro sink present in PipeWire.
3. **~150 ms extra lag** on skip and seek on ALSA: acceptable for 1.0; look at it again for 2.0.
4. **Linux label:** "Supported" in the 1.0 notes and the README.

## Issues

| # | Issue | Next step |
|---|---|---|
| L8 | Switching from `default` (PipeWire) to the same DAC's direct `hw:` exclusive entry plays nothing for ~5 s. PipeWire holds the card until WirePlumber suspends the idle sink (its default `session.suspend-timeout-seconds` is 5); the L2/L7 card wait (up to 7 s) covers it, so it ends bit-perfect with no error. A switch between different cards, or from a direct entry, is immediate (30–139 ms in place). | Known issue for 1.0 (the wait is the designed recovery). For 2.0: ask the sound server to release the card (e.g. suspend that sink over the Pulse/PipeWire API) before the exclusive open, or show "waiting for the sound server" during the wait. |
| L9 | An unplug on direct `hw:` exclusive can surface as `EIO (5)` from `snd_pcm_recover` instead of `ENODEV`, and showed the raw ALSA message. | Fixed in this run (`device_error`, see C2). Windows is unaffected (Linux-only code). |
| — | With no ALSA `default` PCM at all (seen only in the bare 22.04 container) the playback error is the raw `ALSA function 'snd_pcm_open' failed with error 'No such file or directory (2)'`. | Cosmetic, container only. Optional: map `ENOENT` on `default` to "no system audio output found". |
| — | The startup update check announces a found update only by toast; the Settings card shows "Check for updates" until it is clicked. | By design; noted only. |

## Side effects on this machine

- `config.toml`: back to `output_device = "hw:CARD=Pro,DEV=0"` exclusive and Atmosphere dark.
  My Collection order restored. Nothing uploaded to the TIDAL profile.
- `~/Applications/OxiTide_0.6.0_amd64.AppImage` (0.7.0 after the update) was removed on 11-11 in
  favour of the AUR package.
- distrobox `u2204` is left in place (with the deb installed and the pulse `default` config), for
  re-runs. Remove it with `distrobox rm u2204`.
- `~/aur-oxitide/` holds the `0.7.0-1` package. The installed app is the pacman package
  `oxitide 0.7.0-2`, built from `main` at `2c84d0f`; the user-level AppImage and `~/.local/bin`
  installs are removed.
