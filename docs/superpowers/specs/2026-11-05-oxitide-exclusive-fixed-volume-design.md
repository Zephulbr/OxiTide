# OxiTide — Exclusive output plays at a fixed 100% volume

- **Status:** Approved by owner (2026-11-05)
- **Date:** 2026-11-05
- **Owner:** V
- **References:** `docs/agents/audio.md` (Volume) · `2026-10-11-oxitide-track7-asio-output-design.md` ·
  Microsoft `IAudioEndpointVolume` docs (hardware vs software endpoint volume in exclusive
  mode) · `scripts/audio/level-probe.mjs`
- **Supersedes:** the "exclusive output drives the DAC's own volume" behaviour recorded as
  passing in `plans/notes/2026-10-03-track8-bare-metal-checklist.md`,
  `plans/notes/2026-10-16-track13-smoke.md` and `plans/notes/2026-10-23-linux-bare-metal-run.md`.

---

## 1. Problem

The owner reported a volume mismatch on a MOONDROP Dawn Pro between output modes, and between
picker entries for the same card. It came from two bugs, both in how exclusive output used
the DAC's own volume control:

1. **A different curve.** Shared output scales samples on the loudness taper
   (`output::volume_gain`, −10 dB at 50%). Exclusive output instead wrote the slider to the
   device's volume: Windows `SetMasterVolumeLevelScalar`, or ALSA's `Master`/`PCM` element
   mapped *linearly onto raw steps*. The Dawn Pro's `PCM` element runs from −63.5 to 0 dB
   in 0.5 dB steps, so a linear raw mapping is linear in dB:

   | Slider | Exclusive (old, DAC volume) | Shared (software) | Difference |
   |---|---|---|---|
   | 90% | −6.5 dB | −1.5 dB | −5.0 dB |
   | 70% | −19.0 dB | −5.2 dB | −13.9 dB |
   | 50% | −31.5 dB | −10.0 dB | −21.5 dB |
   | 30% | −44.5 dB | −17.4 dB | −27.1 dB |

2. **A leak.** That value was never restored. The DAC's volume control is also the
   system volume (PipeWire drives it on Linux; setting the `PCM` element to −31.5 dB moved
   the Dawn Pro sink to 0.30. On Windows it is the device's endpoint volume, and Microsoft documents that
   changes made during exclusive mode carry over to shared mode). Every later shared
   session on that card stacked the app's software volume on top of it, so it got
   attenuated twice. Going the other way, exclusive overwrote a low system volume with the
   slider, so turning exclusive on could jump to full output.

## 2. Research: what other players do

- **Roon:** per device, *Fixed* (no volume control; fixed-level output), *Device* (the
  DAC's own volume) or *DSP* (software). A device without a volume control offers only
  Fixed or DSP.
- **TIDAL (official):** *Force Volume* keeps the app at maximum so the level is set on the
  DAC. Users report its exclusive mode driving the Windows volume to 100% by itself, which is
  the same hazard as bug 2.
- **Qobuz:** support's advice for bit-perfect playback is to keep the in-app volume at
  100%.
- **MPD:** `mixer_type = hardware | software | none`; its bit-perfect guide warns against
  software volume.
- **Audirvana:** software volume is off by default (the device's volume is used); it adds a
  maximum-volume limit and a confirm above 90%.
- **Windows:** with hardware volume, endpoint volume applies in shared *and* exclusive
  mode; without it, only in shared mode (exclusive bypasses the audio engine).

Locking the app's volume at 100% is a common, simple design. It is also the only one that
never touches the system volume.

## 3. Decisions (owner, 2026-11-05)

- **EV-D1: exclusive plays at 100% and never controls the DAC's volume.** On a device
  with a volume control of its own (Windows `QueryHardwareSupport` reports hardware volume;
  ALSA has a writable `Master`/`PCM` playback element), samples go out unscaled and the
  device's volume is never written. The user sets the level on the DAC or with the system
  volume. Shared and exclusive then match whenever the shared slider is at 100%.
- **EV-D2: the app's slider is kept and returns.** While fixed, the controls show 100%
  and are locked. The player bar and Rack knob show a lock, and volume hotkeys, media
  keys, MPRIS and the phone remote do nothing. The saved slider is not changed, and it
  applies again once the output takes the app's volume (shared mode), so leaving exclusive
  is only ever quieter.
- **EV-D3: a warning with "Don't show again".** Turning exclusive on, or picking an ASIO
  device, while the slider is below 100% asks first: "Caution: exclusive mode sets the
  in-app volume to 100%, which can be much louder than now…". Ticking the box sets
  `[audio] exclusive_volume_warning = false`.
- **EV-D4: devices without a volume control follow the fallback setting.** The system
  volume would not apply, so the stream would play at full scale.
  - `exclusive_fallback = shared` (non-bit-perfect fallback allowed): exclusive stays open
    and takes the app's volume in software. Below 100% it reports not bit-perfect, with the
    existing software-volume notice, as before.
  - `warn_only` (strict): the open fails with `output::NO_DEVICE_VOLUME_ERROR`.
  - Limitation: a strict user feeding a receiver over S/PDIF who wants fixed full scale is
    refused. A "fixed output, I control the volume downstream" override can come later.
- **EV-D5: ASIO is fixed.** ASIO cannot report a volume control, but its interfaces have
  one (an output knob, the driver's mixer). It plays at 100% and is bit-perfect, whatever
  the fallback setting.

## 4. Design

- `OutputStatus::volume_fixed`, set by the backend at open:
  `output::exclusive_volume(device_has_volume, fallback)` returns `Fixed`, `Software` or `Refuse`.
  WASAPI probes `EndpointVolume::has_hardware_volume` (never sets it). ALSA uses
  `has_hardware_volume` (mixer read only). ASIO is always fixed. The WASAPI endpoint glide
  and the ALSA mixer write are removed. Volume is now always a render-time software ramp,
  at unity (a pass-through) on a fixed output.
- Engine: `current_volume_locked` hands a fixed output unity. It reads the *open*
  output's status, never the settings, so a mode change cannot give a still-open shared
  output full volume. A sleep fade on a fixed output fades from unity and restores unity.
  `SetVolume` is ignored while fixed. `publish_open_status` records a fresh open's status
  before the volume is computed, so a fixed open never ramps up from the slider.
- `PlaybackState::volume_fixed`: the open output's value, or, before an open with the
  current settings (`status_live` cleared by mode, device and fallback changes), a
  prediction from the settings (`effective_mode == Exclusive`). `shown_volume()` gives
  clients 100% while fixed. `RemoteState` carries `volumeFixed`.
- App: `set_volume` and the volume hotkeys skip while fixed (the saved slider must not
  drift). `set_exclusive_volume_warning` persists EV-D3.

## 5. Measuring it

`scripts/audio/level-probe.mjs` (Linux) reads the whole volume chain back without
listening:

- `snapshot` prints the app slider as software dB, the card's mixer in dB, PipeWire's
  software part, and who holds the PCM, with the total for each path to the same card.
- `record` and `compare` give an analog check: record the DAC's output through a
  line-in in each mode, and `compare` aligns the two recordings and reports the level
  difference.

After this change, shared at 100% and exclusive should compare within about 0.1 dB.
