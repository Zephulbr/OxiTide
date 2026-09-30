# Track 3 — scripted perf baseline

- **Date:** 2026-09-30
- **Build:** `track3-harness` worktree, debug build (`target/debug/oxitide-app.exe`, HEAD `2a11cde`) served by the Vite dev server on port 1420; scenarios driven over the WebView2 CDP endpoint (`scripts/perf/run-scenario.ps1` + `scripts/perf/drive.mjs`); budget gate `scripts/perf/check.mjs` (`budgets.json` unchanged).
- **Machine:** Ryzen 7 5700X3D / 32 GB / RX 9070 XT / Windows 11 25H2.
- **Conditions:** per-scenario mode is auto-pinned by the driver (S5/S6 instrument, S5-atmosphere atmosphere; the owner's persisted mode is restored after each run). Network as-is (no throttling or isolation). All runs were single-app, foreground, no deliberate background load. The 14 scratch runs left over from harness development were moved to `.superpowers/perf/runs-prebaseline/` before this baseline, so every aggregate below covers only the 26 baseline runs recorded today (S1 x5, S2 x5, S5-instrument x5, S5-atmosphere x5, S6 x3, S7 x3). Raw JSONL is gitignored under `.superpowers/perf/runs/`; command logs are under `.superpowers/perf/logs/`.

## S1 — cold start, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 5`

```
PASS       S1               first_paint_ms               840.0  (max 1500)
PASS       S1               ui_ready_ms                  1282.0  (max 2500)
```

Measured per run (from the run JSONL summaries; budgeted values in bold at the aggregate p95):

| Run | first_paint_ms | ui_ready_ms | player_built_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 840 | 1182 | 1163 | 69 | 487 |
| 2 | 642 | 859 | 842 | 60 | 496 |
| 3 | 593 | 792 | 774 | 54 | 491 |
| 4 | 593 | 868 | 851 | 0 | 484 |
| 5 | 643 | 1282 | 1266 | 0 | 491 |
| **p95 (5 runs)** | **840.0** | **1282.0** | 1266.0 | 69.0 | 496.0 |

Marks `config_loaded` (6–9 ms) → `cache_ready` (9–14 ms) → `services_ready` (13–20 ms) → `window_shown` (414–637 ms) → `first_paint` → `player_built`/`auth_restored` → `ui_ready` were emitted in order in all 5 runs. `check.mjs` exit code 0.

## S2 — warm start, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 5`

```
PASS       S2               first_paint_ms               683.0  (max 1000)
```

Measured per run:

| Run | first_paint_ms | ui_ready_ms | player_built_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 588 | 911 | 893 | 51 | 483 |
| 2 | 566 | 846 | 830 | 0 | 486 |
| 3 | 614 | 1062 | 1046 | 53 | 494 |
| 4 | 683 | 893 | 874 | 57 | 487 |
| 5 | 604 | 811 | 793 | 0 | 490 |
| **p95 (5 runs)** | **683.0** | 1062.0 | 1046.0 | 57.0 | 494.0 |

`check.mjs` exit code 0.

## S5-instrument — track scroll churn in Instrument mode, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-instrument -Runs 5`

```
PASS       S5-instrument    frame_p95_ms:instrument      7.1  (max 16.7)
PASS       S5-instrument    long_task_max_ms             65.0  (max 100)
```

Measured per run:

| Run | frame_p95_ms:instrument | atmosphere-labelled samples (outside the pin) | long_task_max_ms | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 7.1 | 7.1 | 0 | 570 | 34 |
| 2 | 7.1 | 7.1 | 63 | 560 | 52 |
| 3 | 7.1 | 7.1 | 65 | 565 | 51 |
| 4 | 7.1 | 7.1 | 0 | 586 | 41 |
| 5 | 7.1 | 7.1 | 0 | 572 | 45 |
| **p95 (5 runs)** | **7.1** | 7.1 | **65.0** | 586.0 | 52.0 |

Each run emits `instrument`-labelled `ui_metrics` during the scroll loop and some `atmosphere`-labelled samples outside that window (the driver restores the owner's persisted mode at the end of the run; only the `instrument` samples feed the `frame_p95_ms:instrument` budget). `check.mjs` exit code 0.

## S5-atmosphere — track scroll churn in Atmosphere mode, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere -Runs 5`

```
PASS       S5-atmosphere    frame_p95_ms:atmosphere      7.1  (max 20)
PASS       S5-atmosphere    long_task_max_ms             0.0  (max 100)
```

Measured per run:

| Run | frame_p95_ms:atmosphere | long_task_max_ms | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- |
| 1 | 7.1 | 0 | 587 | 33 |
| 2 | 7.1 | 0 | 584 | 30 |
| 3 | 7.1 | 0 | 587 | 32 |
| 4 | 7.1 | 0 | 568 | 40 |
| 5 | 7.1 | 0 | 588 | 27 |
| **p95 (5 runs)** | **7.1** | **0.0** | 588.0 | 40.0 |

No long tasks were recorded in any Atmosphere run. `check.mjs` exit code 0.

## S6 — filter churn at the current library size, 3 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S6 -Runs 3`

```
PASS       S6               frame_p95_ms:instrument      7.1  (max 16.7)
```

Measured per run:

| Run | frame_p95_ms:instrument | long_task_max_ms | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- |
| 1 | 7.1 | 0 | 569 | 25 |
| 2 | 7.1 | 0 | 555 | 18 |
| 3 | 7.1 | 0 | 573 | 24 |
| **p95 (3 runs)** | **7.1** | **0.0** | 573.0 | 25.0 |

Row-count caveat: with the current (small) library the Tracks view displays **2137 loaded rows (238 unavailable)** at the time the S6 filter loop runs (separate CDP probe of the view's own "N tracks" line on the same build; `get_favorite_tracks` returns 2138, one more than the view shows). The 10k budget is **unmeasured (fixture pending Phase C)**; these numbers are informational only. The driver filters over all loaded rows (`ensureAll` paging completes during the first filter). `check.mjs` exit code 0.

## S7 — playback start from favorites, 3 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S7 -Runs 3`

```
INFO       S7               no metric budgets (informational)
```

Measured per run (no budgets; `budgets.json` `"S7": []`):

| Run | play_tracks rows | driver round-trip ms | ttfs_ms | cache_hits / misses | cache_hit_rate | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 2138 | 4362 | 441 | 0 / 2 | 0.0 | 593 | 101 |
| 2 | 2138 | 6993 | 404 | 0 / 2 | 0.0 | 589 | 96 |
| 3 | 2138 | 4330 | 394 | 0 / 2 | 0.0 | 591 | 93 |
| **p95 (3 runs)** | 2138 | 6993 | **441** | 2 misses | 0.0 | 593.0 | 101.0 |

Each run emitted `decode_start`, `cache_miss`, `first_pcm`, and `ttfs` engine events, so the audio marks path is live. The 2 misses per run are the cold first tracks (no cache-hit run was exercised here; S3/S8 own that). `check.mjs` exit code 0.

## Summary

| Scenario | Metric | Budget | Measured p95 | Status |
| --- | --- | --- | --- | --- |
| S1 | first_paint_ms | ≤ 1500 | 840.0 | PASS |
| S1 | ui_ready_ms | ≤ 2500 | 1282.0 | PASS |
| S2 | first_paint_ms | ≤ 1000 | 683.0 | PASS |
| S5-instrument | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | PASS |
| S5-instrument | long_task_max_ms | ≤ 100 | 65.0 | PASS |
| S5-atmosphere | frame_p95_ms:atmosphere | ≤ 20 | 7.1 | PASS |
| S5-atmosphere | long_task_max_ms | ≤ 100 | 0.0 | PASS |
| S6 | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | PASS |
| S7 | (none in `budgets.json`) | — | ttfs p95 441 ms; drift p95 101 MB (informational) | INFO (no budget) |
| S3-miss | ttfs_ms | ≤ 2500 | — | UNMEASURED (owner run pending) |
| S3-hit | ttfs_ms | ≤ 300 | — | UNMEASURED (owner run pending) |
| S4 | advance_max_gap_ms | ≤ 100 | — | UNMEASURED (owner run pending) |
| S8 | cache_hit_rate | ≥ 0.9 | — | UNMEASURED (owner run pending) |
| S9 | memory_drift_mb | ≤ 50 | — | UNMEASURED (owner run pending) |

All scripted `check.mjs` invocations exited 0. No harness-level failures (every run launched, produced JSONL, and the driver exited 0; no torn or missing run files). The `track3-harness` branch fixes landed before this baseline (`2a11cde` pins scenario mode and allows empty budgets), so the driver mode pin was active throughout.

## Manual scenarios — pending owner run

These require a real account, owner playback, and (for S3) cache clearing; none were run in this scripted dispatch.

Commands:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-miss -Runs 5 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-hit -Runs 5 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S4 -Runs 5 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S8 -Runs 2 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S9 -Runs 1 -Manual
```

Protocol per scenario (the runner prompts on each run):

- **S3-miss — Pending.** Clear the cache once before the first run (Settings → clear cache), play the reference track from a fresh album; play only one track.
- **S3-hit — Pending.** Replay the same track immediately; do not touch the cache between runs.
- **S4 — Pending.** Play a reference album and let it play through 2–3 track transitions without skipping.
- **S8 — Pending.** Play the same 10 Max-quality tracks in a loop twice (album repeat); note the queue actually uses Max.
- **S9 — Pending.** Leave a queue playing/skipping for 2 hours (skip/quality-switch churn is allowed); do not restart the app.

## Recalibration (T3-D9)

_Empty — no budgets were changed. Any adjustment to `budgets.json` or the spec §4.2 table requires owner sign-off; the scripted baseline above passes every current budget, so no proposals to record yet. The manual scenarios may surface one round of proposals once the owner runs them._
