# Track 13 — Efficiency: implementation plan

Spec: `docs/superpowers/specs/2026-10-16-oxitide-track13-efficiency-design.md`. Branch:
`track13-efficiency`. Every task ends with `cargo test --workspace`,
`cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check` and `npm --prefix ui test`.

## Phase 0 — measurement (lands first, so the baseline is taken before any fix)

- [ ] 0.1 `perf.rs`: private bytes beside the working set in `memory_process` rows; a 2 s CPU
      sample per process group (`cpu` rows: app / webview, kernel+user ms).
- [ ] 0.2 IPC counters: the desktop engine subscriber records `playback_event` count and
      serialized bytes into the perf session (`ipc` rows, flushed with the memory sample).
- [ ] 0.3 `check.mjs`: `cpu_app_pct`, `cpu_webview_pct`, `private_max_mb`, `ipc_events_per_min`,
      `ipc_bytes_per_min` metrics; tests in `check.test.mjs`.
- [ ] 0.4 `run-scenario.ps1 -Release` (release exe, no Vite) and graceful close through a
      `perf_quit` command (perf sessions only; `NOT_EXPOSED` to the remote), `/F` fallback.
- [ ] 0.5 Bridge: `setMode("rack")`; S7 end to end (`play_tracks` 10 k → state applied in the
      UI); scenarios S11 idle paused, S12 playing hidden (manual), S13 Rack meters, S14 volume
      drag on a 10 k queue.
- [ ] 0.6 **Owner:** release-build baseline of S1–S8, S11–S14 at the Phase 0 commit; record it
      in `notes/2026-10-xx-track13-baseline.md`; agree budgets for the new rows.

## Phase 1 — quick wins (F1, F2, F3 UI side, F15)

- [ ] 1.1 Queue revision (EF-D1): `Arc<Vec>` items + `rev` in `Queue`; `PlaybackState.queue_rev`;
      desktop subscriber omits an unchanged queue; `api.ts` `queue?`/`queue_rev`;
      `applyEvent` keeps the queue when absent; queue in `$state.raw`. Tests: payload without
      queue on volume change; UI reducer keeps the queue.
- [ ] 1.2 Volume coalescing (EF-D2) in `VolumeSlider`, `VolumeKnob`, phone excluded (already
      throttled). Tests for the coalescer.
- [ ] 1.3 Session saver (EF-D3): compact JSON, `queue_rev` comparison, 30 s position cadence.
- [ ] 1.4 Paused worker (EF-D4, engine half): park on the condvar, no `Position` while paused.
      Test: no position events during a paused window.
- [ ] 1.5 UI loops: DiscArt stops its loop at rest; LevelRecorder draws only on new points or
      while gliding; levels feed idles on silence.
- [ ] 1.6 Meter: no `level_frame` while the tap reports silence twice in a row; reuse the FFT
      window buffers.

## Phase 2 — audio write path (F4, EF-D5; F3 output half)

- [ ] 2.1 Bulk ring writes and a blocking wait (WASAPI + ASIO), reusable device buffer.
- [ ] 2.2 Owned `PcmBuffer` through `PcmSink`; wake the worker only when it waits.
- [ ] 2.3 Release the output stream after `PAUSE_RELEASE` paused; restart on resume.
- [ ] 2.4 Owner smoke: shared, exclusive (bit-perfect readout), ASIO, crossfade/AutoMix, S4.

## Phase 3 — threads, network, start-up (F5, F6, F10, F11, F13)

- [ ] 3.1 Heavy commands → `#[tauri::command(async)]`; sync allowlist test.
- [ ] 3.2 Streaming `Fetcher` (body chunks into the ahead file) keeping `fetch_recovering`
      semantics; Range resume for single-file plans.
- [ ] 3.3 One shared `reqwest::Client` for artwork/lyrics/scrobble; resolver on the Tauri
      runtime.
- [ ] 3.4 Parallel `get_all_pages` after the first page (bounded, ordered).
- [ ] 3.5 History/track-flags/session loads off the pre-window path.

## Phase 4 — WebView, rendering, build (F7, F8, F9, F12, F14)

- [ ] 4.1 WebView2 `MemoryUsageTargetLevel::Low` + controller hidden while hidden/minimized.
- [ ] 4.2 Backdrop from a small cover with a small blur; drop per-badge backdrop filters.
- [ ] 4.3 `[profile.release]` LTO, `codegen-units = 1`, `strip`; dedupe `rand`.
- [ ] 4.4 Lazy routes (Settings, Rack shell, History, Profiles, Lyrics).
- [ ] 4.5 Pager cache LRU; `$state.raw` for large track lists.

## Close-out

Final corpus vs the Phase 0 baseline in `notes/`, owner smoke, AGENTS.md (performance harness
section and architecture notes), budgets.json updated.
