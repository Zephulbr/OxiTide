# Track 8.2 — ALSA smoke checklist (WSLg)

Human-in-the-loop checks for the Linux output backend. WSL2 with WSLg, repo
built per `docs/linux-build.md`. Check items off with the run date and notes.

## Shared output
- [ ] `default` appears in Settings → Audio → Output and is preselected.
- [ ] Play a 16/44.1 lossless track; audio is audible in WSLg.
- [ ] Play a lossy (AAC) track; audio is audible; status shows shared.
- [ ] Change the volume slider; loudness changes smoothly.

## Exclusive / bit-perfect (bare metal required)
- [ ] On a machine with a real card, select a `hw:CARD=…,DEV=…` device and
  Exclusive mode; status shows bit-perfect and the DAC locks to the track rate.
- [ ] Switch tracks of different sample rates; the DAC re-locks.
- [ ] Set Exclusive + WarnOnly with a busy device; playback pauses with the
  exclusive-unavailable notice.
- [ ] Set Exclusive + Shared; playback continues on `default` with the
  "exclusive mode unavailable" notice.
- [ ] Set volume below 100% with no writable mixer; the software-volume notice
  appears and bit-perfect clears.

## Robustness (bare metal required)
- [ ] Unplug the selected USB DAC during playback; playback fails with a
  notice. Linux shared mode does not rewrite the saved device, so reselect an
  output device in Settings to recover.
- [ ] With a real or server output, stop the sound server during playback;
  playback pauses with a notice within ~2 s instead of hanging.
- [ ] Seek repeatedly during exclusive playback; no dropouts or hangs (flush
  path).
- [ ] Re-open Settings after a device list change; Refresh lists the new card.
