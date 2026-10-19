# Track 13 — release-build perf results and S11–S14 budgets (2026-10-19)

Owner runs, release builds (`run-scenario.ps1 -Release`), Ryzen 7 5700X3D (16 threads), 144 Hz
display. Baseline: `aa12e87` (Phase 0), 2026-10-17. Track 13: `27fac8a` for S1–S10, S12, S14;
S11 re-run at `ca7b695`; S13 at `32f291c` (the first commit where S13 really plays, see below).
Medians of 5 runs. CPU is percent of **one core** (Task Manager shows ~1/16 of it).

## Results

| Scenario | Metric | Baseline | Track 13 |
| --- | --- | --- | --- |
| S14 volume drag, 10 k queue | IPC | 3.4 GB/min (1.96 MB/event) | 808 KB/min (558 B/event) |
| | app / WebView2 CPU | 105 % / 131 % | 2.0 % / 15.7 % |
| | memory (median) | 871 MB | 517 MB |
| S12 playing, hidden to tray | WebView2 CPU | 5.5 % | 0.09 % |
| | memory (median) | 548 MB | 214 MB |
| | `playback_event`s | 118/min | 0 |
| S11 idle paused | WebView2 CPU | 12.3 % | 3.7 % |
| | `playback_event`s | 118/min | 0 |
| S13 Rack, meters, playing | WebView2 CPU | (silent, not comparable) | 11.8 % (124 % before EF-D8) |
| S4 gapless | WebView2 CPU | 96 % | 7.7 % |
| | memory (median) | 876 MB | 630 MB |
| S3-hit / S3-miss | TTFS | 430 / 493 ms | 165 / 367 ms |
| S1 / S2 | memory (median) | 594 / 585 MB | 497 / 499 MB |
| S7 | queue build | 265 ms | 225 ms |

Still over their (Track 3, debug-build) budgets: memory in S4, S7, S8; S6 p99 frame time (41 ms
both sides). Re-baselining S1–S9 on the release build is a close-out item.

## Findings while measuring

- **Silent S13 (fixed, `ca7b695` + `32f291c`).** A restored session leaves the engine paused;
  playing a new context relied on the worker to clear it, and a seek landing first (likely an SMTC
  client replaying the restored position) kept the pause. Every baseline S13 run and the first
  `416ef0d` set were silent. `start_context` now clears the pause and the pending seek itself.
  The bug is on `main` too.
- **The harness passed silent runs.** `awaitPlaying` accepted `playing && position_ms > 0`, which a
  stuck session at a restored position satisfies. It now requires the position to advance; S13
  plays on repeat (its 139 s track could end in the window) and fails if playback stopped.
- **Rack meters (EF-D8).** Once S13 played, WebView2 held 124 % of a core: the meters repainted
  at 144 Hz with a glow filter per spectrum bar, a full VU face per needle move, and a tuner
  pointer animating `left`. Fixed in `416ef0d`: 124 % → 11.8 %.
- **IPC counters are cumulative.** An ad hoc comparison that summed `ipc` rows overstated every
  IPC figure (e.g. S11 read 9,918 events/min; the real rate is 118). `check.mjs` was always
  right. `level_frame` is now counted too, so S13 shows ~1,900 events/min (30 Hz meter frames).

## Not yet trustworthy

- S3-hit's 0-cache-hit FAIL: the first run follows S3-miss's cache clear, and `awaitWarmCache`'s
  `!sawPending` shortcut can go on before the file is committed (harness).
- S4's first-transition gap of ~18 s on every run, both sides; the real gaps are 37–44 ms.
- `ui_metrics` frame counts cap at 5000, so whether a paused window still repaints is unknown
  (S11's 3.7 % WebView2 CPU suggests something does).

## S11–S14 budgets (`budgets.json`)

Rule (owner, 2026-10-19): median + 15 %, checked on the **median run** (`"of": "median"`) for CPU
and IPC rates, with CPU at least +0.5 pt; memory on the worst run (median + 15 %, S14 worst run
+ 15 % for its one 602 MB outlier); standard frame (p95 16.7 / 20, p99 33 ms) and long-task
(100 ms) caps; zero `playback_event`s while paused or hidden. Frame rows are left off S11/S12
(paused / hidden window). All rows pass on the corpus above; the pre-EF-D8 S13 runs fail
(`cpu_webview_pct` 123.7, `memory_max_mb` 733).

## Follow-ups

1. Default-mode playback (S4/S8 ~8–10 % WebView2): profile the spinning cover and seek bar at
   144 Hz, the pattern EF-D8 fixed in Rack.
2. Rack meters at 30 Hz (tap rate) would take S13 to roughly 8 % — a visual call.
3. The three harness items above, then re-baseline S1–S9 on the release build.
