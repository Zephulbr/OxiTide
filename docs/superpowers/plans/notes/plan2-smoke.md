# Plan 2 Smoke Checklist

> Note: The items below require a human with a logged-in TIDAL account, a browser
> login, and an audio device. They are PENDING a human run and are intentionally
> left unchecked. Automated checks (cargo test/clippy, UI test/check) were run
> separately and passed — see the Task 16 report for exact counts.
>
> Run with `npm --prefix ui run app:dev`.

- [ ] Clicking ▶ on an album track starts playback (sound is audible).
- [ ] Player bar shows the track title and artist.
- [ ] Play/pause toggles playback; the icon updates.
- [ ] Next/prev move through the album's tracks.
- [ ] The progress bar and elapsed time advance during playback (seeking itself is not implemented yet).
- [ ] The volume slider changes loudness.
- [ ] Clicking Queue shows the queue with the current track.
- [ ] Queue reorder (drag), remove, and clear work.
- [ ] Settings lists output devices and changing quality persists.
- [ ] A track at Max (Hi-Res) plays (24-bit/192 kHz source is decoded).
- [ ] Logging out stops playback and clears the queue.

## Results (2026-09-22)

Automated gates run on `feat/plan3-bitperfect-gapless` at commit 6ff0370 (all green):

- `cargo test --workspace`: 97 passed, 0 failed, 1 ignored (the ignored test is `wasapi_output::tests::opens_default_device_and_plays_a_second_of_silence`, which requires an audio device; breakdown: oxitide-app 11, oxitide-audio 45 + 1 ignored, pipeline integration 1, oxitide-tidal 40).
- `cargo clippy --workspace -- -D warnings`: clean, exit 0, no warnings.
- `npm --prefix ui test`: 5 passed, 0 failed (2 files).
- `npm --prefix ui run check`: 0 errors, 0 warnings.

The manual checklist items above remain unchecked: they are pending a human run with a real account, browser login, and audio device.

## Plan 3 exclusive check

PENDING a human run with a real DAC/account (Task 4 does not launch the GUI). Run
`npm --prefix ui run app:dev`, enable the bit-perfect toggle in Settings, and confirm:

- [ ] The DAC/device shows the source sample rate (Windows Sound settings → device properties → Advanced, or a DAC display).
- [ ] The PlayerBar shows the "Bit-perfect" badge.
- [ ] Changing the volume still changes loudness.
- [ ] Open/close is clean (unplugging is covered in Task 9).

## Plan 3 gapless check

PENDING a human run with a real account and audio device (Task 7 does not launch the
GUI). Run `npm --prefix ui run app:dev`, play an album whose tracks share a format
(most albums), and confirm:

- [ ] Tracks run into each other with no device click or re-open at the boundary.
- [ ] No audible gap, duplicate, or missing audio between tracks.
- [ ] The position counter resets at each boundary.

## Plan 3 seek check

PENDING a human run with a real account and audio device (Task 8 does not launch the
GUI). Run `npm --prefix ui run app:dev`, drag the seek bar to the middle of a track and
release, and confirm:

- [ ] Audio restarts at the dragged point.
- [ ] The time label does not jump back.
- [ ] Seeking near the end clamps without an error.

## Plan 3 config check

PENDING a human run with a real account and audio device (Task 10 does not launch the
GUI). Run `npm --prefix ui run app:dev`, then confirm:

- [ ] Toggle bit-perfect on, pick a non-default output device and Max quality, then restart the app; all three must come back selected.
- [ ] Quit, corrupt `%APPDATA%\OxiTide\config.toml` (write `not = [valid`), start again: the app must start with defaults and a `config.toml.bak` must exist.
- [ ] Disable/rename the saved device, restart: the Settings select must show System default and the config file must have been updated.

## Plan 3 hot-plug check

PENDING a human run with a real DAC/account (Task 9 does not launch the GUI). Run
`npm --prefix ui run app:dev` with a USB DAC and bit-perfect mode enabled, then confirm:

- [ ] Unplugging the DAC mid-track pauses playback, shows the amber "output device disconnected" notice in the PlayerBar, and keeps the queue.
- [ ] Replugging the DAC and pressing play resumes at approximately the failure point; the notice clears on the next successful open.
- [ ] A stored device that no longer exists (renamed/disabled in Windows sound settings before starting) falls back to the system default with the "selected output device unavailable; using system default" notice instead of failing.
