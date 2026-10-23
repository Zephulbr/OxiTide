# Track 8 bare-metal completion checklist (2026-10-03)

Deferred bare-metal checks for Track 8, per
`docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` §5.3–§5.4.
These cannot be verified in WSL2 and **gate Track 8 closure**. Record each
item with date, distro/kernel, DE/session (X11/Wayland), audio server
(PipeWire/PulseAudio), and evidence. Cover the spike's representative matrix
(`2026-10-01-linux-support-spike.md`): one PipeWire distro + one PulseAudio
distro, one X11 session + one Wayland session.

Status (2026-10-04): no bare-metal run has been recorded; every checkbox below
stays open. Machine-verifiable evidence and the deferred-row origins live in
the companion notes `2026-10-03-track8-packaging-smoke.md` and
`2026-10-03-track8-4-linux-desktop-smoke.md`; those results do not close any
item here.

Status (2026-10-23): the first bare-metal run executed on CachyOS (Arch) / KDE
Plasma 6.8.1 Wayland / PipeWire 1.6.10 / WebKitGTK 2.54.1 — evidence and
row-by-row results in `2026-10-23-linux-bare-metal-run.md`. Closed there:
`ExclusiveFallback` on a busy device, real-DE MPRIS + media keys (KDE),
`tidal://` + single instance, Wayland geometry, AppImage on the current distro.
A USB-DAC follow-up the same day (`MOONDROP Dawn Pro`) closed the two audio
rows after fixing two ALSA bugs (§11 there, and note the fixes are an
**uncommitted** working-tree patch — §12): exclusive bit-perfect playback
(S32_LE at the file's rate, no PipeWire stream, `✓ BP`) and hardware mixer
volume. DAC hotplug and the format step-down are still unexercised, and
exclusive cannot be selected from the UI yet — the picker never offers a `hw:`
device on this alsa-lib (issue #11 in the run note). Still open: AUR install
(recipe still points at Codeberg and `v0.4.0` is untagged), the oldest-base
WebKitGTK/AppImage checks (not available on this host), and the MPRIS
desktop-entry resolution per package (issue #4 in the run note).

## Audio

- [ ] **Exclusive / bit-perfect on a real DAC.** Select the DAC in Exclusive
      mode: the engine opens `hw:` directly, status reports bit-perfect, and
      the same file plays without server resampling. Hold the device in
      another app (or pick an unsupported format) → `ExclusiveFallback`
      reopens shared with the existing notice.
      Status 2026-10-23 (Dawn Pro): the engine path **passes** — exclusive
      S32_LE at 44.1/48/96/192, no PipeWire stream, `✓ BP`, pause releases and
      resume reopens. Selecting it in the UI is blocked (#11): the picker has
      no `hw:` entry, so the id was written into `config.toml` by hand.
- [ ] **Hotplug.** Unplug/replug a USB DAC while paused → enumeration lists it
      again; default-device switch mid-playback and resume behave as on
      Windows. (On-demand enumeration only; no hotplug events is accepted.)
      Status 2026-10-23: not exercised (needs the cable).
- [x] **Hardware mixer volume.** With a writable card Master/PCM element the
      slider drives the mixer and stays bit-perfect; with none, software
      scaling applies the "volume in software" notice.
      Done 2026-10-23 (Dawn Pro): one `PCM` element, slider drag set it to
      38 % with the stream RUNNING and `✓ BP` still lit. The no-element path
      was not seen on this DAC.

## Desktop integration

- [ ] **Real-DE MPRIS + media keys.** On GNOME and KDE (and XFCE/Cinnamon
      where available) the desktop media widget reflects metadata and
      playback, hardware media keys route to OxiTide, and `playerctl` agrees.
- [ ] **MPRIS desktop-entry resolution per package.** MPRIS advertises
      `DesktopEntry=OxiTide` (`crates/oxitide-app/src/media_controls_linux.rs:117`),
      but the installed entry basename differs by package: deb ships
      `/usr/share/applications/OxiTide.desktop` (matches); AUR installs
      `oxitide.desktop`; the AppImage's runtime handler writes
      `oxitide-app-handler.desktop`. On GNOME and KDE, confirm the media widget
      resolves the entry on each install path (name/icon shown, focus/raise
      works) and record any package where the name/case mismatch breaks the
      association. (Carried over from Plan 8.4.)
- [ ] **`tidal://` registration/launch.** After deb/AUR install,
      `xdg-open 'tidal://...'` launches or focuses the app and routes the
      deep link; a second invocation hits the single-instance path.
      Plan 8.4 note: in bare WSLg, `xdg-open` exits 4 on the dev-path entry
      because its generic fallback takes the first word of the spec-legal
      quoted `Exec` written by `register_all()` (dev entry:
      `Exec="/root/.../oxitide-app" %u`); GNOME/KDE route `xdg-open` through
      `open_gnome`/`open_kde` instead. Verify the installed-deb path
      (`/usr/share/applications/OxiTide.desktop`, `Exec=oxitide-app`,
      unquoted) on a real DE; if the quirk reproduces there, file it as a
      product issue. Evidence:
      `2026-10-03-track8-4-linux-desktop-smoke.md` (deep-link row).
- [ ] **Wayland behavior.** Under Wayland, size and normal restore keep
      working; mini-player placement and saved-position restore degrade
      (documented limitation) but never visibly misplace the window. Under
      X11 both restore fully.

## Packaging and platform floor

- [ ] **AUR install.** Build/install `packaging/aur/PKGBUILD` on Arch against
      `webkit2gtk-4.1`; the app launches and plays.
- [ ] **WebKitGTK 4.1 floor.** On Ubuntu 22.04 / Debian 12, the Phase 2 CSS
      pass (`@property`, `backdrop-filter`, `color-mix` in
      `ui/src/styles/`) renders correctly in both modes.
- [ ] **AppImage oldest-base launch.** Run the AppImage built on Ubuntu 22.04
      on 22.04 (the floor) and on a current distro; it launches and plays.

## Completion-gate protocol (spec §5.4)

Track 8 closes only when this checklist is run and recorded on bare metal, or
the owner explicitly records a completion-note exemption naming what remains
unverified. On closure: record the run evidence here (or in a dated completion
note), list every FAIL/exemption with its follow-up, and update the Track 8
status in the roadmap. WSLg-verifiable items are closed by
`2026-10-03-track8-wslg-smoke.md`; only the deferred items above gate closure.
