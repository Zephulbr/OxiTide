# 96 kbps HE-AAC validation and owner smoke

Date: 2026-10-16. Implementation: `.worktrees/96kbps`, branch `feat/96kbps`,
based on `9499e21`. No commit or merge was made. The main checkout's unrelated
work was not copied, changed, or reverted. Results apply to this worktree, not
the owner's subsequently advanced main checkout.

## Implemented scope

- Restore Low / HE-AAC 96 kbps in the existing Settings selector and preserve
  saved `low`; Lossless remains the default. No new IPC or DTO/schema changes.
- Keep Symphonia demuxing; decode HE-AAC with pinned `rusty_aac 1.1.0` using raw
  AudioSpecificConfig. Implicit SBR/PS cannot be read from an LC-looking header,
  so `rusty_aac` runs when the plan is Low, its codec string is HE-AAC, or the
  ASC signals SBR/PS (`StreamPlan::needs_he_aac`, legacy Low cache entries too).
  High (AAC-LC 320) stays on Symphonia and keeps its skip-bad-packet recovery.
  Non-AAC decoding and the integer/sample-exact FLAC path remain unchanged.
- Derive actual PCM rate/channels from decoded packets, keep lossy audio F32,
  reject invalid PCM and unsupported mid-stream format changes. Known explicit
  HE profiles can be probed; an implicit header is conservatively `mp4a`, not
  falsely `mp4a.40.2`. API served-quality metadata retains precedence.
- Keep source-start ticks until the output rate is known; seeking no longer
  subtracts core-rate frames from output-rate frames. Fractional-millisecond
  boundaries are compared on the manifest timeline.
- Any AAC packet error terminates the track. A fresh decoder is constructed for
  the next track. Cached errors after emitting PCM are not silently refetched
  into the same channel (which would repeat the prefix), but the corrupt entry
  and its sidecar are evicted (online and offline) so a later play refetches;
  failures before PCM still remove and refetch at once. Cancellation (a newer
  generation or a closed sink) never evicts. Failed network decoding cancels
  the feeder before joining, preserving the original network error first.
- No output-backend, DSP, cache-format/migration, badge, or phone-remote redesign.

## Decoder acceptance

Six self-generated, streamable fMP4 fixtures cover HE-AAC v1/v2 with hierarchical,
sync-extension, and implicit signaling. They use 24 kHz cores and emit 48 kHz
stereo (including v2's mono core reconstructed through PS). Independent FFmpeg
9.0.2 comparisons found identical frame counts and approximately 103 dB v1 /
124 dB v2 PCM SNR. All 21 fixture/build combinations (including existing AAC-LC,
SIMD/scalar release, and debug) passed the original correctness thresholds.

The spike found a real rejected-packet state-poisoning issue in this decoder:
`a0 00 00 00 00 00` can install an empty channel layout before failing. In-place
recovery is NOT supported by this integration. Fail-fast/fresh-next-track
controls pass. No arbitrary-input resource bound, full codec conformance,
patent clearance, or exhaustive source-origin audit is claimed.

All six fixtures plus their standalone stdlib generator, hashes, provenance,
reference commands, and detailed findings are under
`crates/oxitide-audio/tests/fixtures/`. Tests need no external tools, downloads,
account, or live TIDAL request. The external fixture encoder is not bundled.
The checked-in decoder test includes independent FFmpeg PCM checkpoints.

## Automated results

Windows: Rust 1.98.1, Node 26.7.0. Separate target directory:
`C:/Users/V/Documents/Opencode/OxiTide/target/96kbps`.

- PASS: `cargo test --workspace --locked --target-dir <Windows target>` on the
  final implementation: 434 app + 344 audio + 2 pipeline + 208 TIDAL tests =
  **988 passed**, with **2 hardware-dependent audio tests ignored**.
- PASS: `cargo clippy --workspace --locked --target-dir <Windows target> -- -D warnings`.
- PASS: `npm --prefix ui run check`: 0 errors, 0 warnings.
- PASS: `npm --prefix ui test`: 882 tests in 109 files.
- PASS: `npm --prefix ui run build` (desktop and remote Vite entries).
- PASS: `git diff --check` and fixture generator `--help`.

Linux: WSL2 Ubuntu-22.04, Rust 1.99.0, ALSA development library 1.2.6.1.

- PASS on final implementation: `cargo test -p oxitide-audio --locked` and
  `cargo clippy -p oxitide-audio --locked -- -D warnings`, through WSL using
  `/tmp/oxitide-96kbps-target`.
- An earlier full `cargo test --workspace --locked` passed before the final
  cached-prefix error follow-up. The final full-workspace rerun hit its 120 s
  compilation limit; it is NOT recorded as a final pass.
- Full Linux workspace Clippy was not completed within the earlier combined
  240 s test/Clippy run. The targeted final audio Clippy gate did pass.

Some Windows broad reruns also timed out during compilation/build-lock waits;
the later final workspace tests and standard Clippy completed successfully.
Extra `--all-targets` Clippy trials expose existing warnings in unchanged tests
(`playlists.rs` needless borrow and audio `len_zero`); the required standard
workspace Clippy gate above is clean on Windows. These unrelated tests were
not edited. Worktree `npm ci` reported 7 existing dependency vulnerabilities;
no unrelated dependency remediation was attempted.

Focused regressions cover all six actual HE fixtures, forward-only HTTP playback
via wiremock, network/cache/offline playback with/without sidecars, output-rate
seek math, no partial-track cache commitment, fallback policy, fresh decoders
on prefetch/next, network and cached malformed-track termination, and an 8 MiB
backpressured early-failure feeder case. The cached-prefix test was observed
failing before its fix and passing afterward. Existing AAC-LC and FLAC tests pass.
The prefetch test proves matching-format device chaining and PCM concatenation,
not encoder-priming/padding-correct musical gaplessness.

## Owner smoke — passed 2026-10-24

**Result: PASS, 2026-10-24** (owner, Windows, real account; the branch is merged as `48184d7`).
All ten rows below confirmed. This closes the live-TIDAL Low coverage the automated fixtures
could not establish; WSL tests still do not certify bare-metal Linux output.

- [x] Select Low in Settings; play several tracks with audible stereo/high end.
- [x] Verify a served Low stream's detailed bitrate/rate and the compact AAC badge.
- [x] Restart; Low remains selected. A new config still defaults to Lossless.
- [x] Seek forward/backward while streaming and after a complete cache hit.
- [x] Replay a completed cached Low track offline; restore networking afterward.
- [x] Pause/resume, skip, natural next/prefetch, and the existing crossfade/AutoMix
      settings behave acceptably; no new hangs, repeated prefixes, or stale audio.
- [x] Switch to High, Lossless, and Max; verify normal playback and FLAC's existing
      exclusive/bit-perfect behavior. Low must never be advertised as bit-perfect.
- [x] Check the existing exclusive fallback/error policy with real WASAPI/ASIO.
- [x] Confirm the selector is usable in Atmosphere, Instrument, and Rack.
- [x] If Linux owner smoke is desired, verify shared playback on a real device;
      WSL tests do not certify bare-metal/exclusive output. (Shared playback PASS in the
      2026-10-23 CachyOS/PipeWire bare-metal run, `2026-10-23-linux-bare-metal-run.md`.)

## Remaining release checks

- [ ] Complete final full Linux workspace tests and standard workspace Clippy
      with a sufficient build budget (no final failure is being waived here).
- [x] Owner listening/GUI smoke above.
- [x] Review/rebase against the owner's current main branch before any merge;
      this worktree intentionally excludes unrelated work made during the task. (Merged into
      `main` as `48184d7`.)
