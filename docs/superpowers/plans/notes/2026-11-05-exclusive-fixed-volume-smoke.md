# Exclusive fixed volume — smoke checklist (2026-11-05)

Spec: `specs/2026-11-05-oxitide-exclusive-fixed-volume-design.md`. These checks need a human, a real
account and real devices. Automated coverage: the `exclusive_volume` policy table, the engine
tests (`exclusive_on_a_device_with_its_own_volume_plays_at_unity`,
`a_sleep_fade_on_a_fixed_output_fades_from_unity`,
`exclusive_without_a_device_volume_keeps_the_app_volume`), the remote `volumeFixed` test, and the
VolumeSlider / VolumeKnob component tests.

## Done during development (Linux, MOONDROP Dawn Pro, 2026-11-05)

- [x] `hw:CARD=Pro,DEV=0` exclusive open (strict fallback, no audio written): status
      `Exclusive, bit_perfect, volume_fixed`. A 0.3 volume call (what a sleep fade does) clears
      bit-perfect without a notice, and 1.0 restores it. The card's `PCM` element stayed at
      127/127 (0 dB) throughout (a throwaway ignored test, since removed).
- [x] Windows build: `cargo clippy --workspace --target x86_64-pc-windows-msvc -- -D warnings` is
      clean for every file this change touches. It still fails on `remote/firewall.rs` dead code
      from `e14320b` (Linux-only helpers), which predates this change.

## Before you start

- [ ] Reset the DAC's volume once: past exclusive sessions may have left the Windows device
      volume (or the ALSA `PCM` element) at an old slider value. Set it where you want it in
      the system mixer.

## Windows (Dawn Pro, WASAPI)

- [ ] Shared, app slider at 40%: turning exclusive on shows the caution dialog. Cancel leaves
      exclusive off and the toggle off.
- [ ] Confirm: playback restarts in exclusive at the system volume's level (no jump to the
      DAC's maximum unless the system volume is at 100%). The player-bar slider shows full
      with a lock, the Windows volume mixer value does **not** move, and `✓ BP` is lit.
- [ ] While exclusive: the Windows volume slider (or the DAC's own control) changes the level.
      App arrow keys, volume hotkeys, media keys and the phone remote do nothing; the phone
      shows a lock.
- [ ] Turn exclusive off: the slider returns to 40% and the level drops accordingly.
- [ ] With the slider at 100%, shared vs exclusive sound the same (or measure with `record` /
      `compare` through a line-in).
- [ ] Turn exclusive on again with "Don't show again" ticked: confirm it. The next time, no
      dialog appears.
- [ ] Sleep timer fade-to-pause in exclusive: fades out, pauses, and plays at full level on resume.
- [ ] A device without hardware volume (e.g. HDMI or S/PDIF), fallback "Use shared": exclusive
      opens, the slider stays active, and below 100% the "volume applied in software" notice
      shows. With fallback "Warn only": playback fails with the "no volume control of its own"
      message.
- [ ] An ASIO device: the slider is locked at 100% and the level follows the interface's knob or mixer.

## Linux (Dawn Pro, ALSA)

- [ ] `node scripts/audio/level-probe.mjs snapshot` in shared (`default`), `sysdefault:` and
      exclusive `hw:`: with the app slider at 100% every row totals the same, and the `PCM`
      element / PipeWire volume does not change when exclusive opens or closes.
- [ ] The desktop volume (KDE/GNOME) for the Dawn Pro still changes the level during exclusive
      playback on `hw:` (PipeWire drives the same `PCM` element).
