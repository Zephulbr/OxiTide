# Plan 3 Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and an audio
> device (ideally a USB DAC). Run with `npm --prefix ui run app:dev`.

## Automated checks (tree at cefc0117b2e5dd6361d8f3d265e4cd7b5247ac06)
- cargo test --workspace: 138 passed, 0 failed, 1 ignored (oxitide-app 15, oxitide-audio 82 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 40, doc-tests 0); run three times, identical results (one run concurrently with vitest)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 7 passed, 0 failed across 2 files (format 2, player 5); run twice, identical results
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

## Manual checks
- [x] Bit-perfect toggle is on; the PlayerBar shows the "Bit-perfect" badge.
- [x] A 44.1 kHz track opens the DAC at 44.1 kHz (DAC display or Windows device properties → Advanced).
- [x] A 96 kHz or 192 kHz track opens the DAC at that rate — no resampling.
- [x] Shared mode still plays when the bit-perfect toggle is off.
- [x] The volume slider changes loudness in exclusive mode (endpoint volume); if it shows the amber
      software-volume notice, loudness still changes.
- [x] Setting the fallback to "Stop with a message" and disabling the DAC in Windows sound settings
      produces an error/paused state with a notice, not a crash or silence that pretends to play.
- [x] Two tracks from the same album run into each other with no gap and a single device open.
- [x] A track at a different sample rate correctly reopens the device at the boundary.
- [x] The seek bar drags, restarts audio at the requested position, and the time label does not snap back.
- [x] Seeking near the end of a track clamps without an error and the queue advances.
- [x] Unplugging the DAC mid-track pauses playback with an amber notice and keeps the queue.
- [x] Replugging and pressing play resumes at approximately the failure point.
- [x] Choosing a device id that no longer exists falls back to the system default with a notice.
- [x] Settings: bit-perfect, fallback, device, and quality survive an app restart.
- [x] Corrupting `%APPDATA%\OxiTide\config.toml` starts with defaults and leaves `config.toml.bak`.
- [x] Logging out stops playback and releases the exclusive device.
