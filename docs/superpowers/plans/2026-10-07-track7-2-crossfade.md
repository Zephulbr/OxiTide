# Track 7.2 — Crossfade Implementation Plan

**Goal:** Equal-power crossfade (1–12 s) on natural track ends using the existing prefetch stream as the second voice, keeping same-album transitions gapless by default.

**Spec:** `docs/superpowers/specs/2026-10-07-oxitide-track7-advanced-audio-design.md` (T7-D6, §4 end detection, §6.3)

## Global Constraints

Same as Plan 7.1. Crossfade off must leave the gapless path byte-for-byte unchanged (S4 regression row).

### Task 1: Crossfade primitives (`dsp.rs`)

- [x] `equal_power(t) -> (out, in)`; `Tail` (queued `f32` frames + cursor) with `mix_into(&mut [f32], channels)` and `fade_out_remaining()`.
- [x] Tests: endpoints, constant power, partial mixes across buffer boundaries, fade-out tail.

### Task 2: Hold-back FIFO in the worker

- [x] While armed (`duration_ms > 0`, a next item exists, not excluded by `skip_same_album`), read ahead with `try_recv` until the FIFO holds the crossfade length; write from the FIFO head.
- [x] Disarmed: drain the FIFO in order, then return to the direct path (no samples lost when the setting or the next item changes).
- [x] On disconnect: the FIFO becomes the carried `Tail`.

### Task 3: Boundary

- [x] Same output spec: mix the `Tail` into the next stream's first frames (after its DSP), `NowPlaying` at overlap start.
- [x] Different spec: write the `Tail` with a fade-out to the old output, then reopen.
- [x] Queue end with a `Tail`: write it unfaded (natural ending).
- [x] Engine tests: overlap shortens the total; same-album skip keeps the gapless total; Next is a cut.

### Task 4: UI + verification

- [x] Settings "Crossfade" controls (duration slider 0–12 s, skip same album).
- [x] Gates; `plans/notes/DONE-2026-10-07-track7-2-crossfade-smoke.md`; completion note; roadmap/AGENTS.md updates.
