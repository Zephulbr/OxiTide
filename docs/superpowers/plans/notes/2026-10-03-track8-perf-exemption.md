# Track 8 perf-harness exemption (2026-10-03)

Authority: `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md`
§4.8 and T8-D3. Implemented by Plan 8.5. There is no `scripts/perf/README*`
in the repo (checked 2026-10-03), so the exemption is recorded here and
linked from `AGENTS.md`.

## What stays Windows-only

`scripts/perf/run-scenario.ps1` and `scripts/perf/drive.mjs` drive the app
through the **WebView2 CDP endpoint**: the runner sets
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`, and
the driver resolves `/json`, connects with a WebSocket, and drives
`Runtime.evaluate` against `window.__perf` / Tauri `invoke`. They are tied to
the Windows WebView2 runtime and the `.exe`, so the scripted S1–S10 scenarios
are **not ported** in Track 8 (spec §9 records the full
WebKitWebDriver/tauri-driver port as deferred).

## What Linux gets instead

`crates/oxitide-app/src/perf.rs` implements the process metrics for
`#[cfg(target_os = "linux")]` from `/proc`:

- process creation time: `/proc/self/stat` `starttime` (parsed after the last
  `)`, so `comm` with spaces/parens survives) anchored to the wall clock with
  `CLOCK_BOOTTIME`;
- tree memory: walk `/proc/<pid>/task/*/children`, then read each PID's
  `smaps_rollup` `Rss:` (fallback: `statm` resident pages × page size).

Launching the Linux binary with `OXITIDE_PERF_SCENARIO=<id>` (and optionally
`OXITIDE_PERF_DIR=<dir>`) writes the same JSONL schema as Windows, and
`node scripts/perf/check.mjs --dir <dir>` aggregates it against
`scripts/perf/budgets.json`.

## Budget coverage on Linux

Measurable now: process-relative `t_ms` marks (`first_paint_ms`,
`ui_ready_ms`, `startup_step_max_ms`), memory rows (`memory_max_mb`,
`memory_drift_mb`, `memory_drift_app_mb`), and playback-driven audio rows
(`ttfs_ms`, `advance_max_gap_ms`, `cache_hits`/`cache_misses`) when a human
drives the app as S4/S8/S9/S10 always required.

Not measurable until the deferred harness port: the scripted scenario runs
themselves and the `ui_metrics` rows they produce (`frame_p95/p99_ms`,
`long_task_max_ms`, S5/S6 interaction rows, S7 `queue_build_ms`, S8's
scripted loop). `check.mjs` marks `required` rows as failing when a corpus
lacks them, so a Linux corpus is read under this exemption; the Windows
corpus remains the gate for interaction rows. The Windows Track 3 budgets and
the documented S4 finding are unchanged
(`docs/superpowers/plans/notes/2026-10-02-track3-phase-d.md`).
