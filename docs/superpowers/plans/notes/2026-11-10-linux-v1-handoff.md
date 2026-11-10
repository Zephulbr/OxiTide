# Linux v1.0 handoff (2026-11-10)

For the next session on the CachyOS laptop (`Vivobooo`, KDE Plasma Wayland, PipeWire, `ufw`
active, MOONDROP Dawn Pro). It closes what is left of release gate §6
(`2026-10-17-v1-release-gate.md`) so `v1.0.0` can be tagged.

- **Read first:** `AGENTS.md`, `docs/linux-build.md`, `docs/agents/audio.md` (ALSA sections), the
  last run `2026-11-05-linux-v1-final-run.md` (results, issues L1–L7, the owner retest).
- **Code:** `main` (pull it first; this note is on it). Version `0.7.0`. The 11-05 run was on
  `53cef6d` (0.6.0); about 55 commits have landed since (list below).
- **Releases:** `v0.6.0` and `v0.7.0` are published on GitHub (`Zephulbr/OxiTide`, 0.7.0 is
  "Latest"). Both carry the AppImage, its `.sig`, the deb, `latest.json` (with
  `linux-x86_64` / `linux-x86_64-appimage`) and the rendered `PKGBUILD`. CI builds Linux bundles
  on `ubuntu-22.04`, so the release AppImage and deb are the oldest-base artifacts.
- **Split of work:** the agent runs builds, gates, logs, `playerctl`/D-Bus, `/proc/asound`,
  `hw_params`; the owner does cables, listening, clicks in the GUI, anything needing `sudo`
  (`makepkg -si`, `pacman -U`, installing a 22.04 environment). Record results in a new run note
  `2026-11-1x-linux-v1-handoff-run.md` (same table style as the 11-05 note), then update gate §6.

## 1. Sync and gates on current `main`

```bash
git pull
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui ci   # or install; node_modules must be built on Linux
npm --prefix ui run check
npm --prefix ui test
npm --prefix ui run app:build   # deb + AppImage, target/release/bundle/
```

Inspect the deb and AppImage desktop entries as in `docs/linux-build.md` § Packaging (no
`dpkg-deb` on Arch: read with `ar`/`tar` as on 11-05). Expect `OxiTide_0.7.0_amd64.*`.

## 2. Re-check what changed since the 11-05 run

Code that reaches Linux and has not run there yet. Quick rows on the locally built AppImage,
owner at the keyboard, agent on the logs (`RUST_LOG=info,alsa=debug`):

| # | Change | Row |
|---|---|---|
| C1 | `f99421a` an output switch moves the stream in place (same mode: stop the old device, open the new one, keep the decoder) | While playing, switch PipeWire `default` → Dawn Pro direct `hw:` (exclusive) → `default` → HDA `hw:`. Each switch plays on the new device within ~1 s at the same position, no restart, no `EBUSY` error (the L2/L7 7 s card wait should cover PipeWire still holding the DAC). Log line per move with its time. Repeat one switch while **paused**: Play then comes out of the new device. |
| C2 | `2e25fc1` the ring fails a blocked write as soon as its device thread is gone (mainly WASAPI; shared `ring.rs`) | No regression on ALSA: unplug the DAC mid-play on direct `hw:` exclusive, "Output device disconnected" appears promptly, queue and position kept (the L1 row again). |
| C3 | `9963b88` boot screen; the window waits for setup and page load before its first show, starting in the saved look's `--bg` | Cold start in a dark and a light mode: no white or empty frame, the mark then the ring, fades out on login restore. Then the L5 rows again (tray hide/show keeps size; maximized stays maximized; second launch), since `window.rs` changed around the first show. |
| C4 | `c56cc34` position ticks and beat clocks held while the window is hidden | Close to tray while playing for a minute, reopen: the progress bar and time jump to the right place at once, CatJam (if on) is in time again within a beat; MPRIS `Position` stays right while hidden (`playerctl position`). |
| C5 | `a1c1a2b` now-playing mark: Linux keeps the 30 fps `frameGate` loop (off Linux it moved to a Web Animation) | Atmosphere, playing, one mark on screen: it turns smoothly and eases on pause/play. Per-thread CPU of the `oxitide-app` main thread (`/proc/<pid>/task/*/stat`, `docs/linux-build.md` § Profiling) stays near the 30 fps figure (~8 %), not the 60 fps one (~25 %). |
| C6 | `9af1a67` colour and lamp glides stepped (`steps(36)` over 1.2 s) | Track changes in Atmosphere and Rack: the glide still reads as a glide (owner eye), no flash. |
| C7 | `0cd666f` device picker | Already smoked on Linux (`2026-11-05-device-picker-smoke.md`); no re-run unless C1 shows a problem. |
| C8 | Customizable My Collection (`040fcd7`), own profile and editor with uploads (`fa9bb53`, `fbffa7f`) | Their owner smokes are open on Windows (gate §5). On Linux just: edit mode drag works with WebKitGTK (pointer drag, `lib/dragDrop.svelte.ts`), and a profile picture upload opens the GTK file chooser and uploads. |

A failure here is fixed on `main`, retested on Linux, and noted for a Windows re-check (gate §6:
"Re-check any resulting fixes on Windows"). A fix in shared audio/UI code also means the Windows
final perf run (`2026-11-10-v1-final-perf-run.md`) must run after it.

## 3. AppImage updater path (never run)

The in-app updater is wired for the AppImage only (deb/AUR update through the package manager;
`docs/release.md`). Now testable without a new release: install 0.6.0, let it update to 0.7.0.

1. Download `OxiTide_0.6.0_amd64.AppImage` from the `v0.6.0` release into a writable folder
   (e.g. `~/Applications/`), `chmod +x`, and run it **directly** (not `--appimage-extract-and-run`:
   the updater replaces the file named by `$APPIMAGE`, which only AppRun sets). Quit any other
   OxiTide first (single instance would hand off to it).
2. Settings → System → Updates: it finds 0.7.0. Install. Expected: the AppImage file is replaced
   in place, the app restarts as 0.7.0 (Settings → System version, or Copy diagnostics).
3. Record: whether it found the update on its own at startup too, the time taken, file name and
   permissions after the update (still executable), and anything in the log (`updater`,
   `startup update check`). If the file is not writable (e.g. in `/opt`), note the error text the
   user gets.
4. The 0.6.0 AppImage uses the owner's real config and login (same config dir as every build);
   nothing to restore. The old AppImage's `tidal://` registration is re-pointed by the
   next run of whichever build starts last (known, `2026-10-23` issue #7 area).

## 4. AUR install (owner)

`v0.7.0` is the first release whose `PKGBUILD` has `options=('!lto')`.

```bash
mkdir -p ~/aur-oxitide && cd ~/aur-oxitide
gh release download v0.7.0 -R Zephulbr/OxiTide -p PKGBUILD   # or download it from the release page
makepkg -si
```

Then: `oxitide` launches from the menu, plays, MPRIS `DesktopEntry` is `oxitide`, `tidal://`
login works. `namcap PKGBUILD` and `namcap oxitide-0.7.0-1-x86_64.pkg.tar.zst` if installed (note
findings, do not block on warnings). Remove with `sudo pacman -R oxitide` if the owner prefers
the AppImage day to day.

## 5. Oldest base: Ubuntu 22.04 (WebKitGTK 4.1 floor, AppImage and deb launch)

Two rows from `2026-10-03-track8-bare-metal-checklist.md` § Packaging: the CI-built (22.04)
AppImage launches and plays on 22.04, and the UI renders correctly there in all modes
(`@property`, `backdrop-filter`, `color-mix` are the likely differences; 22.04 ships WebKitGTK
2.50.x, seen in WSL on 2026-10-04).

Cheapest way on this laptop, owner's choice:

- **distrobox / podman, Ubuntu 22.04** (recommended): checks the library floor (glibc,
  WebKitGTK, GTK) with the host's Wayland and PipeWire sockets passed through. Not a 22.04
  kernel or desktop, which this row does not need.
  ```bash
  distrobox create -n u2204 -i ubuntu:22.04 && distrobox enter u2204
  sudo apt update && sudo apt install -y libfuse2 libwebkit2gtk-4.1-0 libgtk-3-0 \
    libayatana-appindicator3-1 libasound2 libasound2-plugins xdg-utils desktop-file-utils
  ./OxiTide_0.7.0_amd64.AppImage --appimage-extract-and-run   # FUSE is often missing in a container
  sudo apt install -y ./OxiTide_0.7.0_amd64.deb && oxitide-app
  ```
  Record the versions with `dpkg -s libwebkit2gtk-4.1-0 | grep Version` and `ldd --version`.
- **Live USB or a VM of Ubuntu 22.04** if the owner wants the stricter reading (real 22.04
  session, PulseAudio rather than PipeWire).

Rows: launches with no `GLIBC_x.y not found` or missing-library error; login (browser + paste
back, Secret Service may be absent in a container: the session-only notice is expected); plays
through shared output; Atmosphere, Instrument and Hi-Fi Rack in light and dark look like the
host build (glass panels blur, colour glides, the Rack meters move); deb's desktop entry and
`tidal://` registration work. A CSS feature that does not render is a fix to the floor, not a
reason to raise it (Track 8 spec, WebKitGTK risk row).

## 6. Owner decisions for the 1.0 notes

From the 11-05 run's open choices (gate §6), to settle in this session:

1. **dmix clicks (L3 on `sysdefault:` entries):** ship as a known issue (entries already read
   "ALSA default, may resample"), or hide `sysdefault:` when a direct entry exists for the card,
   or warn when one is picked.
2. **WirePlumber replug quirk:** after OxiTide held a card directly, switching to `default` can
   leave the DAC missing from PipeWire until a replug or `systemctl --user restart wireplumber`.
   Known issue line, and whether C1 (in-place switch) makes it more or less frequent.
3. **~150 ms extra lag** on skip and seek on ALSA (faded end of the buffer): acceptable?
4. **Linux label** in the 1.0 release notes: "Supported" (as the README says) or "preview".
   With §2–§5 passing, the gate allows "Supported"; otherwise "preview" with the reasons.

## 7. Close out

- Run note with every row above; issues numbered on from L8.
- Gate §6: tick it (or record the exemption) and list the known issues for the release notes.
- Roadmap Track 8 status, `docs/linux-build.md` (a line on the WirePlumber recovery and the
  AppImage updater location requirement, if found), README Linux line if the label changes.
- Commit with explicit paths, push to both remotes (`origin` Codeberg, `github`).
- Back on Windows: any fix from here gets its Windows re-check, then the final perf run, the
  release notes, the `1.0.0` bump and the tag.
