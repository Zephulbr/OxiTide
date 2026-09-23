# Plan 2 Smoke Checklist

> Note: The items below require a human with a logged-in TIDAL account, a browser
> login, and an audio device. They are PENDING a human run and are intentionally
> left unchecked. Automated checks (cargo test/clippy, UI test/check) were run
> separately and passed — see the Task 16 report for exact counts.
>
> Run with `npm --prefix ui run app:dev`.

- [x] Clicking ▶ on an album track starts playback (sound is audible).
- [x] Player bar shows the track title and artist.
- [x] Play/pause toggles playback; the icon updates.
- [x] Next/prev move through the album's tracks.
- [x] The progress bar and elapsed time advance during playback (seeking itself is not implemented yet).
- [x] The volume slider changes loudness.
- [x] Clicking Queue shows the queue with the current track.
- [x] Queue reorder (drag), remove, and clear work. # drag handle added; use "＋" to add tracks without replacing the queue
- [x] Settings lists output devices and changing quality persists.
- [x] A track at Max (Hi-Res) plays (24-bit/192 kHz source is decoded).
- [x] Logging out stops playback and clears the queue.

## Results (2026-09-22)

Automated gates run on `feat/plan3-bitperfect-gapless` at commit 6ff0370 (all green):

- `cargo test --workspace`: 97 passed, 0 failed, 1 ignored (the ignored test is `wasapi_output::tests::opens_default_device_and_plays_a_second_of_silence`, which requires an audio device; breakdown: oxitide-app 11, oxitide-audio 45 + 1 ignored, pipeline integration 1, oxitide-tidal 40).
- `cargo clippy --workspace -- -D warnings`: clean, exit 0, no warnings.
- `npm --prefix ui test`: 5 passed, 0 failed (2 files).
- `npm --prefix ui run check`: 0 errors, 0 warnings.

The manual checklist items above remain unchecked: they are pending a human run with a real account, browser login, and audio device.

## Results (2026-09-23)

Human run completed with a real account and DAC; every item above passes.
Fixes landed during the run: the blocking fetcher is built on the engine worker
thread (no more "Cannot drop a runtime in an async context" on dev runs),
`$Number$` expands from `startNumber` with a `SegmentTimeline`, same-format
seeks flush the open device instead of reopening it (no DAC re-lock/hiss),
`RPC_E_CHANGED_MODE` is accepted so device enumeration lists every output,
volume persists on slider release, playback errors stay visible in the player
bar, HTML5 drag-and-drop is enabled on Windows, and album rows gained an
"add to queue" action.

- `cargo test --workspace`: 145 passed, 0 failed, 1 ignored (requires an audio device).
- `cargo clippy --workspace -- -D warnings`: clean, exit 0.
- `npm --prefix ui test`: 7 passed, 0 failed (2 files).
- `npm --prefix ui run check`: 0 errors, 0 warnings.

## Plan 3 exclusive check

PENDING a human run with a real DAC/account (Task 4 does not launch the GUI). Run
`npm --prefix ui run app:dev`, enable the bit-perfect toggle in Settings, and confirm:

- [x] The DAC/device shows the source sample rate (Windows Sound settings → device properties → Advanced, or a DAC display).
- [x] The PlayerBar shows the "Bit-perfect" badge.
- [x] Changing the volume still changes loudness.
- [x] Open/close is clean (unplugging is covered in Task 9).

## Plan 3 gapless check

PENDING a human run with a real account and audio device (Task 7 does not launch the
GUI). Run `npm --prefix ui run app:dev`, play an album whose tracks share a format
(most albums), and confirm:

- [x] Tracks run into each other with no device click or re-open at the boundary.
- [x] No audible gap, duplicate, or missing audio between tracks.
- [x] The position counter resets at each boundary.

## Plan 3 seek check

PENDING a human run with a real account and audio device (Task 8 does not launch the
GUI). Run `npm --prefix ui run app:dev`, drag the seek bar to the middle of a track and
release, and confirm:

- [x] Audio restarts at the dragged point.
- [x] The time label does not jump back.
- [x] Seeking near the end clamps without an error.

## Plan 3 config check

PENDING a human run with a real account and audio device (Task 10 does not launch the
GUI). Run `npm --prefix ui run app:dev`, then confirm:

- [x] Toggle bit-perfect on, pick a non-default output device and Max quality, then restart the app; all three must come back selected. 
- [x] Quit, corrupt `%APPDATA%\OxiTide\config.toml` (write `not = [valid`), start again: the app must start with defaults and a `config.toml.bak` must exist.
- [x] Disable/rename the saved device, restart: the Settings select must show System default and the config file must have been updated. 

## Plan 3 hot-plug check

PENDING a human run with a real DAC/account (Task 9 does not launch the GUI). Run
`npm --prefix ui run app:dev` with a USB DAC and bit-perfect mode enabled, then confirm:

- [x] Unplugging the DAC mid-track pauses playback, shows the amber "output device disconnected" notice in the PlayerBar, and keeps the queue.
- [x] Replugging the DAC and pressing play resumes at approximately the failure point; the notice clears on the next successful open.
- [x] A stored device that no longer exists (renamed/disabled in Windows sound settings before starting) falls back to the system default with the "selected output device unavailable; using system default" notice instead of failing.
