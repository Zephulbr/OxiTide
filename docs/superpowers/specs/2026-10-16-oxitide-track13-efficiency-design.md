# OxiTide — Track 13: Efficiency (CPU, idle cost, IPC, memory)

Date: 2026-10-16. Status: in progress (branch `track13-efficiency`). Plan:
`docs/superpowers/plans/2026-10-16-track13-efficiency.md`.

## Goal

Speed and low resource use are OxiTide's main selling points. Track 3 (2026-10-02) gated
start-up, TTFS, scrolling and memory, but its corpus predates Tracks 7/7b (DSP, AutoMix),
9.5 (Rack), 10 (session, history, tray), 11 (phone remote) and the network-resilience work.
Track 3 never measured CPU or idle cost. This track:

1. brings the harness up to date and adds CPU, idle and IPC measurements (Phase 0);
2. removes the costs found by the 2026-10-14 audit, cheapest and safest first (Phases 1–4).

No feature changes. Bit-perfect output, gapless/crossfade behaviour and the IPC contract the
phone remote relies on must not change.

## Audit findings (2026-10-14, re-verified 2026-10-16 at `b88f8b0`)

| # | Finding | Where |
| --- | --- | --- |
| F1 | Every `State` event carries the whole queue: `build_state` copies it, `emit` copies the event per subscriber (4), the desktop subscriber serializes it, and the UI's `Object.assign` replaces `state.queue` with a new deep proxy. Volume, play/pause, shuffle, repeat, seek all emit `State`; the volume slider/knob send unthrottled `oninput`. | `engine.rs` `build_state`/`emit`; `commands.rs` subscriber; `player.svelte.ts` `applyEvent`; `VolumeSlider.svelte`, `VolumeKnob.svelte` |
| F2 | While playing, the session saver rewrites `session.json` (pretty JSON, whole queue) every 3 s because position ticks mark it dirty, and compares the whole queue on every `State`. | `session.rs` `saver_main`, `save` |
| F3 | Paused is not idle: the worker sleeps 20 ms in a loop and keeps emitting `Position` at 2 Hz; the WASAPI pump keeps the stream running on silence; DiscArt and the Rack recorder keep animation loops; the meter feed keeps running. | `engine.rs` paused branches; `wasapi_output.rs` `pump`; `DiscArt.svelte`; `LevelRecorder.svelte`; `levels.svelte.ts`; `meter.rs` |
| F4 | Output write path: `push_word` reads the clock per sample and polls `sleep(2ms)` when the ring is full (WASAPI and ASIO); `write_frames` allocates per device period; decoded packets are copied twice; the producer `notify_all`s per packet. | `wasapi_output.rs`, `asio_output.rs`, `decode.rs`, `engine.rs` `ChannelSink` |
| F5 | 88 of 150 Tauri commands are sync and run on the main thread, including `automix_render`, `get_stats`, `get_history_page`, `play_tracks`, `restore_session`, `get_playback_state`, `get_output_devices`, `clear_cache`, `set_volume` (config write). | `commands.rs` |
| F6 | `Fetcher::get` buffers each URL whole (`bytes().to_vec()`); a single-URL (BTS) plan downloads the whole file before the first byte reaches the decoder. | `fetch.rs` |
| F7 | WebView2 is ~85–90 % of the footprint and is never told to trim when the window is hidden to the tray or minimized. | — |
| F8 | Backdrop blurs a 640 px cover by 80 px over the whole window; per-card badge `backdrop-filter`. | `Backdrop.svelte`, `MediaCard.svelte` |
| F9 | No `[profile.release]` (no LTO, 16 codegen units, unstripped); `rand` at two versions. | `Cargo.toml` |
| F10 | Six HTTP clients and three Tokio runtimes. | `main.rs`, `player.rs`, `fetch.rs` |
| F11 | Rust `get_all_pages` walks pages sequentially. | `client.rs` |
| F12 | One ~470 KB main JS chunk; Settings, Rack, History, Profiles load eagerly. | `vite.config.ts`, `App.svelte` |
| F13 | History, track flags and the session saver load synchronously before the window is built. | `main.rs` |
| F14 | The pager's session cache is unbounded; large lists live in deep `$state` proxies. | `pager.svelte.ts` |
| F15 | The meter emits silent frames at 30 Hz and allocates its FFT window per read. | `meter.rs` (both crates) |

## Decisions

- **EF-D1 Queue revision.** `Queue` keeps its items in an `Arc<Vec<QueueItem>>` (copy-on-write)
  and a `rev: u64` bumped on every item change. `PlaybackState` gains `queue_rev`; its `queue`
  becomes `Arc<Vec<QueueItem>>`, so a snapshot is a pointer copy. The desktop subscriber
  serializes the queue only when `queue_rev` differs from the last one it sent (`queue` is
  omitted otherwise); `get_playback_state` always includes it. The UI keeps its queue when a
  state event has none, and holds it in `$state.raw` (items are replaced, never mutated in
  place). The phone remote keeps its own fingerprint/paging unchanged.
- **EF-D2 Volume coalescing.** The UI sends at most one `set_volume` per animation frame while
  dragging and persists once on release (wheel/keys persist through a short debounce).
- **EF-D3 Session saver.** Compact JSON; queue changes compare `queue_rev`; position-only
  changes flush at most every 30 s while playing (pause, stop, queue edits and exit flush
  immediately as today).
- **EF-D4 Idle when paused.** While paused the worker parks on its condvar instead of a 20 ms
  sleep loop and emits no `Position` events (the position cannot change). After a grace
  period (`PAUSE_RELEASE`, 30 s) the output stops its stream; resume restarts it on the same
  spec. UI animation loops stop once settled (DiscArt at speed 0, recorder frozen); the meter
  feed sends nothing while the tap is silent.
- **EF-D5 Output write path.** Bulk ring writes (rtrb chunks), a deadline computed only when
  the ring is full, a reusable device buffer, owned buffers through `PcmSink`, and producer
  wake-ups only when the worker waits. Exclusive `i32` output stays sample-exact.
- **EF-D6 Main thread.** Commands that touch disk, COM, the engine queue or do CPU work become
  `#[tauri::command(async)]`; a test lists the commands allowed to stay sync.
- **EF-D7..** F6–F15 are planned in Phases 3–4; each gets its own decision when it starts.

## Measurement (Phase 0)

- Runs use a **release** build (`run-scenario.ps1 -Release`): the `__perf` bridge is gated at
  run time by `OXITIDE_PERF_SCENARIO`, so it works without Vite. Debug runs stay available.
- Memory samples add **private bytes** next to the working set; a **CPU** sample (process
  times, per process group: app / WebView2) is written every 2 s, and the checker derives
  `cpu_app_pct` / `cpu_webview_pct` means.
- The desktop event subscriber counts `playback_event`s and their serialized bytes in perf
  runs (`ipc_events`, `ipc_bytes` rows).
- New scenarios: **S11** idle paused (track loaded, paused, 5 min), **S12** playing hidden
  (window hidden to tray, 10 min, human starts playback), **S13** Rack with meters
  (`S5`-style scroll plus 2 min playing), **S14** volume drag on a 10 k queue (scripted
  slider sweep; IPC bytes, long tasks).
- **S7** now measures end to end: `play_tracks` with 10 k items until the UI has applied the
  resulting state, not just `Queue::play_context`.
- The bridge's `setMode` accepts `rack`.
- Runs end with a graceful close (`perf_quit` command → normal exit work), falling back to
  `taskkill /F` after 10 s.
- Budgets for the new rows are set from the Phase 0 baseline (median + 15 %, owner-approved),
  then tightened as phases land; existing rows are re-baselined on the release build.

## Deterministic tests (CI)

Counting, not timing, so they run in the Track 12 gates: a `State` with an unchanged queue
serializes without it; no `Position` events while paused; session writes per minute of
playback; state payload size at 10 k items; the sync-command allowlist.

## Non-goals

No new features, no change to audio quality or transitions, no WebView replacement, no Linux
harness port (T8-D3 stands).
