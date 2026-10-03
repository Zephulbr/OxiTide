# Track 8 bare-metal completion checklist (2026-10-03)

Deferred bare-metal checks for Track 8, per
`docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` §5.3–§5.4.
These cannot be verified in WSL2 and **gate Track 8 closure**. Record each
item with date, distro/kernel, DE/session (X11/Wayland), audio server
(PipeWire/PulseAudio), and evidence. Cover the spike's representative matrix
(`2026-10-01-linux-support-spike.md`): one PipeWire distro + one PulseAudio
distro, one X11 session + one Wayland session.

## Audio

- [ ] **Exclusive / bit-perfect on a real DAC.** Select the DAC in Exclusive
      mode: the engine opens `hw:` directly, status reports bit-perfect, and
      the same file plays without server resampling. Hold the device in
      another app (or pick an unsupported format) → `ExclusiveFallback`
      reopens shared with the existing notice.
- [ ] **Hotplug.** Unplug/replug a USB DAC while paused → enumeration lists it
      again; default-device switch mid-playback and resume behave as on
      Windows. (On-demand enumeration only; no hotplug events is accepted.)
- [ ] **Hardware mixer volume.** With a writable card Master/PCM element the
      slider drives the mixer and stays bit-perfect; with none, software
      scaling applies the "volume in software" notice.

## Desktop integration

- [ ] **Real-DE MPRIS + media keys.** On GNOME and KDE (and XFCE/Cinnamon
      where available) the desktop media widget reflects metadata and
      playback, hardware media keys route to OxiTide, and `playerctl` agrees.
- [ ] **`tidal://` registration/launch.** After deb/AUR install,
      `xdg-open 'tidal://...'` launches or focuses the app and routes the
      deep link; a second invocation hits the single-instance path.
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
