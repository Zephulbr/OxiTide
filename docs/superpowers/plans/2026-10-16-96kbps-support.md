# 96 kbps HE-AAC support

Approved 2026-10-16. Implementation is isolated in `.worktrees/96kbps`, branch
`feat/96kbps`, based on `9499e21`. The main checkout's uncommitted work is not
included and must not be modified. Do not commit or merge without owner approval.

## Contract and boundaries

Restore TIDAL Low (nominal 96 kbps HE-AAC) on Windows and Linux. Keep Symphonia's
container demuxer, existing compressed-byte cache, streaming fetch, PCM sinks,
DSP, and output backends. Keep Lossless as the default. Preserve actual served
quality and the existing allow-fallback policy. Do not add IPC/types, a cache
migration, external-player processes, transcoding, or an audio plugin framework.

## Steps

1. Baseline the audio crate in the worktree.
2. Validate a portable packet decoder with self-generated HE-AAC v1/v2 fixtures,
   explicit/backward-compatible/implicit signaling, independent FFmpeg PCM
   comparisons, AAC-LC regression, and bounded malformed-input checks.
3. Integrate only AAC packet decoding in `decode.rs`, using raw ASC and actual
   decoded output rate/channels. Leave non-AAC decoding and integer FLAC intact.
4. Remove the HE-AAC engine guard; carry source-start timing until the actual
   output rate is known so seeking never mixes core-rate and output-rate frames.
   Confirm early decode failure cannot block forever joining a full feeder.
5. Stop converting persisted Low to High; add Low / HE-AAC 96 kbps to Settings.
   Keep existing badges and quality-display behavior; document the setting.
6. Add real playback/cache/offline/seek/cancellation/regression tests, dependency
   notices, reproducible fixture provenance, and an owner smoke checklist.
7. Run targeted gates, then Windows and Linux workspace tests/Clippy and UI
   tests/check/build. Report human-only checks as pending, not passed.

## Decoder acceptance and error policy

Candidate: `rusty_aac = 1.1.0` (Apache-2.0, no runtime dependencies), pinned because
this exact implementation is being evaluated. Decode all AAC through the local
adapter to cover implicit SBR/PS; keep the AAC-LC fixture as a mandatory regression.
No Symphonia feature enables full HE-AAC.

The acceptance spike found excellent agreement with independent FFmpeg decoding
on six generated HE fixtures: 48 kHz stereo from 24 kHz cores, exact frame counts,
about 103 dB v1 / 124 dB v2 PCM SNR across SIMD/scalar release and debug builds.
These findings do not establish full codec conformance or arbitrary-input bounds.

A rejected packet can poison this decoder's channel layout. Therefore any AAC
packet error must terminate that track immediately; never skip a rejected packet
and continue with the same decoder. A new track always creates a fresh decoder.
Reject unsupported mid-stream format changes instead of silently misplaying them.
Runtime sample rate/channels come from decoded PCM, not the initial core hint or
`sbr_support()` (which does not reflect implicit SBR discovery).

## Validation

Baseline Windows: `cargo test -p oxitide-audio --locked` passed: 329 unit tests,
1 pipeline test, 2 ignored hardware-dependent tests. Separate worktree target
folder: `C:/Users/V/Documents/Opencode/OxiTide/target/96kbps`.

Final results and pending manual checks will be recorded in
`docs/superpowers/plans/notes/2026-10-16-96kbps-validation.md`.
