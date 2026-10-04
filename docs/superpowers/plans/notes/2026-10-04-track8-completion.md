# Track 8 completion note (2026-10-04)

- **Status:** Track 8 closed by owner exemption (spec §5.3–§5.4, decision 2026-10-04)
- **Owner:** V
- **Scope:** Plans 8.1–8.5 (Linux support), merged to `main` at `d14f5f4`
- **Authority:** `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` §5.4;
  checklists `2026-10-03-track8-wslg-smoke.md` and `2026-10-03-track8-bare-metal-checklist.md`

The owner chose to close the track with an exemption rather than wait for bare-metal
hardware. This note names exactly what is and is not verified; the exempted items remain
the resume list for the first bare-metal session.

## Verified

- **Windows (merged tree, 2026-10-04):** `cargo test --workspace`, `cargo clippy
  --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test` — all
  green; Linux code paths are cfg-gated and absent from these builds.
- **Linux dev gates (WSL2 Ubuntu 26.04):** workspace tests, `clippy -D warnings`, and
  `cargo build -p oxitide-app` green; the Track 8 baseline warnings were fully closed
  (scheme.rs cfg cleanup, `f9999a7`).
- **Artifact baseline (WSL2 Ubuntu 22.04):** deb + AppImage built from the packaging
  commit; structural acceptance PASS (`MimeType=x-scheme-handler/tidal;`,
  `Categories=AudioVideo;Audio;Music;`, `Exec=oxitide-app`, deb `Depends`, `amd64`);
  both artifacts launched under WSLg and stayed alive 20 s (process-level; window
  visibility is human).
- **Deep links (WSLg):** runtime `.desktop` registration, `xdg-mime query default`, and
  GIO delivery to the running instance PASS; the installed-deb entry carries the
  trailing-semicolon MimeType. (`xdg-open` on the dev-path quoted `Exec` is an xdg-utils
  parser quirk, not an OxiTide defect.)
- **WebKit engine floor (WebKitGTK 2.50.4):** `color-mix`, `backdrop-filter` /
  `-webkit-backdrop-filter`, and `@property` all supported, with and without compositing
  disabled; no CSS fallbacks needed at this version.

## Exempted — unverified on bare metal (resume list)

From `2026-10-03-track8-bare-metal-checklist.md`:

- Exclusive / bit-perfect playback on a real DAC, incl. format step-down and
  `ExclusiveFallback` on a busy device.
- Hotplug and default-device switching; hardware mixer volume vs software notice.
- Real-DE MPRIS widget + media keys (GNOME/KDE), and the live MPRIS checks that need a
  logged-in session (metadata/seek/control/toggle-gating via `playerctl`).
- `tidal://` registration/launch on a real DE through `xdg-open` (installed deb/AUR
  entries), including the single-instance path.
- Wayland vs X11 window geometry (wayland guard was not needed in WSLg; unobserved
  failures stay unconfirmed).
- AUR install (`makepkg -si`) on Arch; the recipe pin is fetchable only after the branch
  is pushed to Codeberg.
- WebKitGTK 4.1 visual pass on the oldest base (Ubuntu 22.04 / Debian 12) in both modes.
- AppImage oldest-base launch.
- First Linux memory corpus as the calibration baseline (Linux sums all descendants; the
  unchanged budgets are Windows-derived).

## Follow-ups (not closure-gating)

- Push `main` to Codeberg (remote credentials were expired at closure time); the AUR pin
  then resolves.
- MPRIS `serve` reconnect after a transient D-Bus failure; per-package `set_desktop_entry`
  basename (8.4 parked items).
- Perf-exemption note wording (`ttfs_ms` grouping, `OXITIDE_PERF_DIR`, idle `ui_metrics`
  frame rows) and the stale "(Check once Plan 8.4 has landed.)" line.
