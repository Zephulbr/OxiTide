# Track 13 — Efficiency: implementation plan

Spec: `docs/superpowers/specs/2026-10-16-oxitide-track13-efficiency-design.md`. Branch:
`track13-efficiency`. Every task ends with `cargo test --workspace`,
`cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check` and `npm --prefix ui test`.

## Phase 0 — measurement (lands first, so the baseline is taken before any fix)

- [x] 0.1 `perf.rs`: `memory_process` rows carry pid, private bytes and cumulative kernel+user
      CPU ms beside the working set (sampled every 2 s).
- [x] 0.2 IPC counters: the desktop engine subscriber records `playback_event` count and
      serialized bytes into the perf session (`ipc` rows, flushed with the memory sample).
- [x] 0.3 `check.mjs`: `cpu_app_pct`, `cpu_webview_pct`, `private_max_mb`, `ipc_events_per_min`,
      `ipc_kb_per_min` metrics (from the `measure_start` mark when set); tests in
      `check.test.mjs`.
- [x] 0.4 `run-scenario.ps1 -Release` (release exe, no Vite) and graceful close through a
      `perf_quit` command (perf sessions only; `NOT_EXPOSED` to the remote), `/F` fallback.
- [x] 0.5 Bridge: `setMode("rack")`; S7 end to end (`play_tracks` 10 k → state applied in the
      UI); scenarios S11 idle paused, S12 playing hidden (manual), S13 Rack meters, S14 volume
      drag on a 10 k queue.
- [ ] 0.6 **Owner:** release-build baseline of S1–S8, S11–S14 at the Phase 0 commit; record it
      in `notes/2026-10-xx-track13-baseline.md`; agree budgets for the new rows.

## Phase 1 — quick wins (F1, F2, F3 UI side, F15)

- [x] 1.1 Queue revision (EF-D1): `Arc<Vec>` items + `rev` in `Queue`; `PlaybackState.queue_rev`;
      desktop subscriber omits an unchanged queue; `api.ts` `queue?`/`queue_rev`;
      `applyEvent` keeps the queue when absent (assigned only on change, so no `$state.raw`
      needed). Tests: payload without queue on volume change (< 4 KB at 10 k items); UI
      reducer keeps the queue.
- [x] 1.2 Volume coalescing (EF-D2) in `VolumeSlider`, `VolumeKnob`, phone excluded (already
      throttled). Tests for the coalescer.
- [x] 1.3 Session saver (EF-D3): compact JSON, `queue_rev` comparison, 30 s position cadence.
- [x] 1.4 Paused worker (EF-D4, engine half): park on the condvar, no `Position` while paused.
      Test: no position events during a paused window.
- [x] 1.5 UI loops: DiscArt stops its loop at rest; LevelRecorder draws only on new points or
      while gliding; levels feed idles on silence.
- [x] 1.6 Meter: no `level_frame` once the tap has reported silence twice in a row, and a 10 Hz
      tick while quiet. (FFT buffer reuse dropped: ~30 small allocations/s, not worth it.)

## Phase 2 — audio write path (F4, EF-D5; F3 output half)

- [x] 2.1 Chunked ring writes with a wait sized to the room needed (1–20 ms, was a 2 ms poll and
      a clock read per sample), WASAPI + ASIO via `ring.rs`; reusable WASAPI period buffer;
      underruns pad whole frames (fixes a latent channel swap after a mid-frame underrun).
- [x] 2.2 Owned `PcmBuffer` through `PcmSink` (no per-packet copy). The "wake the worker only
      when it waits" half was dropped: one uncontended lock + notify per packet, not worth the
      extra state.
- [x] 2.3 Release the output stream after `PAUSE_RELEASE` (30 s) paused; resume reopens the
      same spec at the current volume. Volume still reaches an open output while paused.
- [ ] 2.4 Owner smoke: shared, exclusive (bit-perfect readout), ASIO, crossfade/AutoMix, S4.

## Phase 3 — threads, network, start-up (F5, F6, F10, F11, F13)

- [x] 3.1 Blocking, order-independent commands → `#[tauri::command(async)]` (`automix_render`,
      `get_stats`, `get_history_page`, `clear_history`, `get_restored_session`,
      `get_playback_state`, `get_output_devices`, `clear_cache`). Transport/queue commands and
      config writers stay sync: sync commands run in arrival order, and settings
      read-modify-write the whole config. `SYNC_COMMANDS` test guards new ones.
- [ ] 3.2 **Deferred, measure first.** Streaming `Fetcher` for single-file (BTS) plans. A
      streamed body that fails mid-way cannot be retried whole once the feeder consumed part of
      it, so it needs Range resume inside `fetch_recovering`. Baseline S3-miss at *Lossless*
      first to size the delay.
- [x] 3.3 One shared `reqwest::Client` for artwork/lyrics/scrobble (artwork had no timeout).
      The resolver's own runtime stays (it is the blocking bridge for the worker thread).
- [x] 3.4 `get_all_pages` fetches known windows 4 at a time, in offset order.
- [x] 3.5 History loads on its writer thread. Track flags were already lazy; the session saver
      only spawns a thread.

## Phase 4 — WebView, rendering, build (F7, F8, F9, F12, F14)

- [x] 4.1 `webview_idle.rs`: controller `IsVisible` + memory target Low while minimized or
      hidden to the tray (wry never updated either).
- [ ] 4.2 **Deferred, measure first.** The backdrop is a static, composited layer (its blur
      is paid on cover change, and Chromium downsamples large radii); S5-atmosphere frames
      already pass at p95 7 ms. Revisit only if the S13/S5 baseline shows backdrop cost; any
      change is visual and needs DESIGN.md.
- [x] 4.3 `[profile.release]` LTO fat, `codegen-units = 1`, `strip = "debuginfo"`; PKCE uses
      `getrandom` 0.3 (already in the tree) so rand 0.8 leaves the Windows build (Linux keeps
      it through keyring's secret-service/zbus). Release build: 6 min 07 s, 25.0 MB exe.
- [ ] 4.4 **Deferred, measure first.** Lazy routes: the main chunk is ~470 KB, but a route
      that renders a frame late can upset the queue-glide FLIP measurement (AR-9). Worth it
      only if the release S1 baseline shows script parse in the start-up path.
- [x] 4.5 Pager cache LRU (12 lists). `$state.raw` not needed: lists are appended in place.

## Close-out

Final corpus vs the Phase 0 baseline in `notes/`, owner smoke, AGENTS.md (performance harness
section and architecture notes), budgets.json updated.
