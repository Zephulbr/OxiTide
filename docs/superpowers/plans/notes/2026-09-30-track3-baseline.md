# Track 3 — scripted perf baseline

- **Date:** 2026-09-30
- **Build:** `track3-harness` worktree, debug build (`target/debug/oxitide-app.exe`). The S3–S8 corpus ran on HEAD `4d57e13`; S1/S2 were re-run in this commit after the process-relative timing fix (`fix(perf): mirror the full budget table and fix the timing base`). Served by the Vite dev server on port 1420; scenarios driven over the WebView2 CDP endpoint (`scripts/perf/run-scenario.ps1` + `scripts/perf/drive.mjs`); budget gate `scripts/perf/check.mjs` with the spec §4.2 budget table.
- **Machine:** Ryzen 7 5700X3D / 32 GB / RX 9070 XT / Windows 11 25H2.
- **Conditions:** per-scenario mode is auto-pinned by the driver (S5/S6 instrument, S5-atmosphere atmosphere; the owner's persisted mode is restored after each run). Network as-is (no throttling or isolation). All runs were single-app, foreground, no deliberate background load. The 14 scratch runs left over from harness development were moved to `.superpowers/perf/runs-prebaseline/`; the earlier S3/S4 scratch corpus (11 files) and the pre-fix S1/S2 corpus (10 files) plus one no-playback probe file were quarantined to `.superpowers/perf/runs-quarantine/`, and those scenarios re-run fresh. The current corpus is: S1 x5, S2 x5, S3-miss x5, S3-hit x5, S4 x5, S5-instrument x5, S5-atmosphere x5, S6 x3, S7 x4 (3 baseline + 1 S7 queue-mapping verification run), S8 x2. Raw JSONL is gitignored under `.superpowers/perf/runs/`; command logs are under `.superpowers/perf/logs/`.
- **Timing base:** as of this commit `t_ms` is process-relative (loader time included) and `unix_ms = process_creation + t_ms` for every line, so cold-start marks no longer omit the exe/DLL load span (`config_loaded` now lands at ~380–455 ms instead of ~6–37 ms).

## S1 — cold start, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 5`

```
PASS       S1               first_paint_ms               1113.0  (max 1500)
PASS       S1               ui_ready_ms                  1331.0  (max 2500)
FAIL       S1               memory_max_mb                518.0  (max 300)
PASS       S1               long_task_max_ms             0.0  (max 100)
```

Measured per run:

| Run | config_loaded_ms | first_paint_ms | ui_ready_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 455 | 1113 | 1330 | 0 | 510 |
| 2 | 378 | 956 | 1168 | 0 | 512 |
| 3 | 428 | 1018 | 1224 | 0 | 518 |
| 4 | 394 | 987 | 1312 | 0 | 513 |
| 5 | 403 | 1007 | 1331 | 0 | 501 |
| **p95 (5 runs)** | 455.0 | **1113.0** | **1331.0** | **0.0** | **518.0** |

`config_loaded` includes the loader span after the timing fix (378–455 ms); marks `config_loaded` → `cache_ready` → `services_ready` → `window_shown` → `first_paint` → `player_built`/`auth_restored` → `ui_ready` still emit in order. The idle-after-auth memory ceiling (300 MB) is a genuine Phase D miss: every run sits at 501–518 MB. `check.mjs` exit code 1 on the memory row only.

## S2 — warm start, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 5`

```
FAIL       S2               first_paint_ms               1052.0  (max 1000)
FAIL       S2               memory_max_mb                511.0  (max 300)
PASS       S2               long_task_max_ms             0.0  (max 100)
```

Measured per run:

| Run | config_loaded_ms | first_paint_ms | ui_ready_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 385 | 978 | 1187 | 0 | 504 |
| 2 | 388 | 978 | 1188 | 0 | 502 |
| 3 | 383 | 966 | 1182 | 0 | 511 |
| 4 | 384 | 976 | 1287 | 0 | 506 |
| 5 | 427 | 1052 | 1265 | 0 | 506 |
| **p95 (5 runs)** | 427.0 | **1052.0** | 1287.0 | **0.0** | **511.0** |

With the process-relative base, warm start now includes the same ~380–430 ms loader span as cold start, so the session-relative 683 ms from the original note becomes a measured 1052 ms and misses the 1000 ms budget — a genuine baseline miss, not a regression (the pre-fix runs measured only post-session time). Memory is the same Phase D miss as S1. `check.mjs` exit code 1.

## S5-instrument — track scroll churn in Instrument mode, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-instrument -Runs 5`

```
PASS       S5-instrument    frame_p95_ms:instrument      7.1  (max 16.7)
PASS       S5-instrument    frame_p99_ms:instrument      20.9  (max 33)
PASS       S5-instrument    long_task_max_ms             65.0  (max 100)
FAIL       S5-instrument    memory_max_mb                586.0  (max 450)
```

Measured per run:

| Run | frame_p95_ms:instrument | frame_p99_ms:instrument | atmosphere-labelled samples (outside the pin) | long_task_max_ms | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 7.1 | 13.9 | 7.1 | 0 | 570 | 34 |
| 2 | 7.1 | 20.9 | 7.1 | 63 | 560 | 52 |
| 3 | 7.1 | 20.8 | 7.1 | 65 | 565 | 51 |
| 4 | 7.1 | 20.8 | 7.1 | 0 | 586 | 41 |
| 5 | 7.1 | 20.8 | 7.1 | 0 | 572 | 45 |
| **p95 (5 runs)** | **7.1** | **20.9** | 7.1 | **65.0** | 586.0 | 52.0 |

Each run emits `instrument`-labelled `ui_metrics` during the scroll loop and some `atmosphere`-labelled samples outside that window (the driver restores the owner's persisted mode at the end of the run; only the `instrument` samples feed the frame budgets). `check.mjs` exit code 1 on memory only.

## S5-atmosphere — track scroll churn in Atmosphere mode, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere -Runs 5`

```
PASS       S5-atmosphere    frame_p95_ms:atmosphere      7.1  (max 20)
PASS       S5-atmosphere    frame_p99_ms:atmosphere      20.8  (max 33)
PASS       S5-atmosphere    long_task_max_ms             0.0  (max 100)
FAIL       S5-atmosphere    memory_max_mb                588.0  (max 450)
```

Measured per run:

| Run | frame_p95_ms:atmosphere | frame_p99_ms:atmosphere | long_task_max_ms | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 7.1 | 14.0 | 0 | 587 | 33 |
| 2 | 7.1 | 13.9 | 0 | 584 | 30 |
| 3 | 7.1 | 13.9 | 0 | 587 | 32 |
| 4 | 7.1 | 20.8 | 0 | 568 | 40 |
| 5 | 7.1 | 20.8 | 0 | 588 | 27 |
| **p95 (5 runs)** | **7.1** | **20.8** | **0.0** | 588.0 | 40.0 |

No long tasks were recorded in any Atmosphere run. `check.mjs` exit code 1 on memory only.

## S6 — filter churn at the current library size, 3 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S6 -Runs 3`

```
PASS       S6               frame_p95_ms:instrument      7.1  (max 16.7)
PASS       S6               frame_p99_ms:instrument      13.9  (max 33)
FAIL       S6               memory_max_mb                573.0  (max 450)
```

Measured per run:

| Run | frame_p95_ms:instrument | frame_p99_ms:instrument | long_task_max_ms | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 7.1 | 13.9 | 0 | 569 | 25 |
| 2 | 7.1 | 13.9 | 0 | 555 | 18 |
| 3 | 7.1 | 13.8 | 0 | 573 | 24 |
| **p95 (3 runs)** | **7.1** | **13.9** | **0.0** | 573.0 | 25.0 |

Row-count caveat: with the current (small) library the Tracks view displays **2137 loaded rows (238 unavailable)** at the time the S6 filter loop runs (separate CDP probe of the view's own "N tracks" line on the same build; `get_favorite_tracks` returns 2138, one more than the view shows). The 10k filter/sort budget is deferred (see below); these numbers are informational. The driver filters over all loaded rows (`ensureAll` paging completes during the first filter). `check.mjs` exit code 1 on memory only.

## S7 — playback start from favorites, 4 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S7 -Runs 3` (plus one queue-mapping verification run)

```
FAIL       S7               memory_max_mb                596.0  (max 450)
```

Measured per run:

| Run | play_tracks rows | driver round-trip ms | ttfs_ms | cache_hits / misses | cache_hit_rate | memory_max_mb | memory_drift_mb |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 2138 | 4362 | 441 | 0 / 2 | 0.0 | 593 | 101 |
| 2 | 2138 | 6993 | 404 | 0 / 2 | 0.0 | 589 | 96 |
| 3 | 2138 | 4330 | 394 | 0 / 2 | 0.0 | 591 | 93 |
| 4 | 2138 | 5032 | 380 | 0 / 2 | 0.0 | 596 | 51 |
| **p95 (4 runs)** | 2138 | 6993 | 441 | 2 misses | 0.0 | **596.0** | 101.0 |

Each run emitted `decode_start`, `cache_miss`, `first_pcm`, and `ttfs` engine events, so the audio marks path is live. S7 now has one budget row (playback memory ≤ 450 MB) and misses it. `check.mjs` exit code 1.

## S3-miss — cold-cache TTFS, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-miss -Runs 5`

```
PASS       S3-miss          ttfs_ms                      354.0  (max 2500)
PASS       S3-miss          ttfs_ms_mean                 340.2  (max 1200)
FAIL       S3-miss          memory_max_mb                584.0  (max 450)
```

Measured per run (driver clears the cache at the start of every run, then plays the first favorite):

| Run | ttfs_ms | cache hits | cache misses | memory_max_mb |
| --- | --- | --- | --- | --- |
| 1 | 348 | 0 | 1 | 582 |
| 2 | 329 | 0 | 1 | 584 |
| 3 | 332 | 0 | 1 | 577 |
| 4 | 354 | 0 | 1 | 574 |
| 5 | 338 | 0 | 1 | 581 |
| **p95 (5 runs)** | **354.0** | 0 | 1 | **584.0** |
| **mean** | **340.2** | | | |

Every run has zero hits and exactly one miss. `ttfs_ms_mean = 340.2 ms` is a **mean proxy for the spec's TTFS median ≤ 1.2 s** (the JSONL gate has no median mode; the mean is close to the median here since the five values span 329–354 ms). `check.mjs` exit code 1 on memory only.

## S3-hit — warm-cache TTFS with cache-hit floor, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-hit -Runs 5`

```
FAIL       S3-hit           ttfs_ms                      308.0  (max 300)
PASS       S3-hit           cache_hits                   1.0  (min 1)
FAIL       S3-hit           memory_max_mb                582.0  (max 450)
```

Measured per run (driver plays the first favorite, waits for the cache entry to finalize via `get_cache_status`, then replays it; the last `ttfs` is the replay):

| Run | first ttfs_ms (play-in) | last ttfs_ms (replay) | cache hits | cache misses | memory_max_mb |
| --- | --- | --- | --- | --- | --- |
| 1 (cold entry) | 479 (miss) | 308 (hit) | 1 | 1 | 581 |
| 2 | 199 (hit) | 247 (hit) | 2 | 0 | 582 |
| 3 | 211 (hit) | 176 (hit) | 2 | 0 | 576 |
| 4 | 202 (hit) | 285 (hit) | 2 | 0 | 576 |
| 5 | 205 (hit) | 292 (hit) | 2 | 0 | 576 |
| **p95 (5 runs)** | — | **308.0** | **2.0** | 0 | **582.0** |
| **minimum** | | | **1** | | |

Every run contains at least one `cache_hit` (run 1 warms the entry, then hits on replay); the `cache_hits ≥ 1` floor now evaluates the cross-run minimum (1) rather than the p95 (2), so a zero-hit run would fail. The `ttfs_ms` p95 misses the 300 ms budget by 8 ms (run 1's cold-start replay at 308 ms; warm replays are 176–292 ms). `check.mjs` exit code 1 on the ttfs and memory rows.

## S4 — natural gapless advance over the 3 shortest tracks, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S4 -Runs 5`

```
FAIL       S4               advance_max_gap_ms           103.0  (max 100)
FAIL       S4               memory_max_mb                591.0  (max 450)
```

The driver sorts favorites by duration ascending and plays the 3 shortest with `repeat off`: `41352612` (49 s), `458258760` (50 s), `458258761` (66 s); transition bound = 49 + 50 + 90 = 189 s. No seeks (natural playback only). Measured `advance` gaps per run (first transition → second transition):

| Run | advance → 458258760 | advance → 458258761 | max | memory_max_mb |
| --- | --- | --- | --- | --- |
| 1 | 22 | 40 | 40 | 591 |
| 2 | 26 | 38 | 38 | 578 |
| 3 | 26 | 40 | 40 | 570 |
| 4 | 103 | 40 | 103 | 582 |
| 5 | 103 | 39 | 103 | 580 |
| **p95 (5 runs)** | | | **103.0** | **591.0** |

Runs 1–3 pass; runs 4–5 exceed the 100 ms gap on the first transition. `check.mjs` exit code 1.

## S8 — cache loop over the 10 shortest tracks, 2 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S8 -Runs 2`

```
PASS       S8               cache_hit_rate_last10        1.0  (min 0.9)
FAIL       S8               memory_max_mb                575.0  (max 450)
```

The driver sorts favorites by duration ascending, plays the 10 shortest at Max quality with `repeat all`, waits `2 × 707 s + 90 s = 1504 s` per run, then restores the saved quality and `repeat off`. Chosen queue (playback order): `41352612` 49 s, `458258760` 50 s, `458258761` 66 s, `458258750` 68 s, `423198288` 72 s, `458258758` 75 s, `247718412` 80 s, `458258759` 82 s, `381937277` 82 s, `458258746` 83 s (total 707 s). Both runs logged `S8: 10 tracks, totalSecs=707, waiting ~1504s`; the batch took 3087 s (~51 min).

| Run | cache hits | cache misses | cache_hit_rate (whole run) | cache_hit_rate_last10 | memory_max_mb | first_pcm events |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 17 | 8 | 0.68 | 1.00 | 572 | 23 |
| 2 | 25 | 0 | 1.00 | 1.00 | 575 | 23 |
| **p95 (2 runs)** | | | 1.0 | **1.0** | **575.0** | |
| **minimum** | | | 0.68 | **1.0** | | |

The gate now uses `cache_hit_rate_last10 ≥ 0.9` (S8: second-pass hit rate), evaluated against the cross-run minimum: both runs' last 10 decisions are all hits, so it passes even though run 1's whole-run rate is 0.68. Run 1 started with 2 of the 10 tracks already cached by the S4 batch (the S4 driver exits 1.5 s after entering the third track, so only its first two tracks finalized): 8 misses in the first pass, then hits; the 1504 s window also covers ~90 s of the third pass (23 `first_pcm` events). Run 2 was fully warm (25 hits / 0 misses). Whole-run rates remain informational. `check.mjs` exit code 1 on memory only.

## Discovered during baseline — cached-seek defect (deferred to Phase D)

Seeking a fully cached track does not reposition playback. In `crates/oxitide-audio/src/engine.rs`, the cache path's `decode_from_file` passes the full plan (with `seek_ms`) into `decode_source`, while `ChannelSink::on_stream_info` computes a segment-relative discard via `seek::target_segment`; the network path instead truncates the download plan first with `plan.from_segment(target.segment_index)` in `decode_from_network`. Net effect on a cache hit: the seek resumes near the track start while `position_ms` is seeded at the target, so the reported position runs past `duration_ms` and no natural `advance` occurs near the seek target (observed during the S4 investigation: a 137 s seek on a 139 s cached track was still playing at position 257 s with no transition; an uncached network seek transitioned within ~2 s). This is recorded, not fixed; the scripted S4 protocol uses natural playback only. Deferred to Phase D.

## Deferred / ungated spec §4.2 rows

These spec §4.2 rows are measurable in principle but have no `budgets.json` entry yet:

| Spec row | Target | Disposition |
| --- | --- | --- |
| 10k filter/sort | ≤ 150 ms | Deferred to Phase C — needs the 10k-row fixture; the S6 corpus filters ~2.1k loaded rows, so a gate now would measure the wrong scale. |
| Play-all queue build | ≤ 250 ms | Deferred to Phase D — measured outside the JSONL (the S7 driver round-trip includes the favorite fetch and network); needs a mark/event before gating. |
| Page-open | ≤ 500 ms | Deferred to Phase D — no scripted scenario emits a page-open metric yet. |
| 500-track album | ≤ 1 s | Deferred to Phase D — no scripted scenario or fixture for a 500-track album yet. |
| Pre-paint span | span budget | Deferred to Phase C — the marks exist (`config_loaded` … `first_paint`), but the spec span definition and gate row are pending. |

## Summary

| Scenario | Metric | Budget | Measured | Status |
| --- | --- | --- | --- | --- |
| S1 | first_paint_ms | ≤ 1500 | 1113.0 | PASS |
| S1 | ui_ready_ms | ≤ 2500 | 1331.0 | PASS |
| S1 | memory_max_mb | ≤ 300 | 518.0 | FAIL (Phase D target) |
| S1 | long_task_max_ms | ≤ 100 | 0.0 | PASS |
| S2 | first_paint_ms | ≤ 1000 | 1052.0 | FAIL (over by 52 ms; process-relative base) |
| S2 | memory_max_mb | ≤ 300 | 511.0 | FAIL (Phase D target) |
| S2 | long_task_max_ms | ≤ 100 | 0.0 | PASS |
| S3-miss | ttfs_ms | ≤ 2500 | 354.0 | PASS |
| S3-miss | ttfs_ms_mean | ≤ 1200 | 340.2 | PASS (mean proxy for the spec median) |
| S3-miss | memory_max_mb | ≤ 450 | 584.0 | FAIL (Phase D target) |
| S3-hit | ttfs_ms | ≤ 300 | 308.0 | FAIL (over by 8 ms) |
| S3-hit | cache_hits | ≥ 1 | 1.0 (min) | PASS |
| S3-hit | memory_max_mb | ≤ 450 | 582.0 | FAIL (Phase D target) |
| S4 | advance_max_gap_ms | ≤ 100 | 103.0 | FAIL (2/5 runs at 103 ms) |
| S4 | memory_max_mb | ≤ 450 | 591.0 | FAIL (Phase D target) |
| S5-instrument | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | PASS |
| S5-instrument | frame_p99_ms:instrument | ≤ 33 | 20.9 | PASS |
| S5-instrument | long_task_max_ms | ≤ 100 | 65.0 | PASS |
| S5-instrument | memory_max_mb | ≤ 450 | 586.0 | FAIL (Phase D target) |
| S5-atmosphere | frame_p95_ms:atmosphere | ≤ 20 | 7.1 | PASS |
| S5-atmosphere | frame_p99_ms:atmosphere | ≤ 33 | 20.8 | PASS |
| S5-atmosphere | long_task_max_ms | ≤ 100 | 0.0 | PASS |
| S5-atmosphere | memory_max_mb | ≤ 450 | 588.0 | FAIL (Phase D target) |
| S6 | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | PASS |
| S6 | frame_p99_ms:instrument | ≤ 33 | 13.9 | PASS |
| S6 | memory_max_mb | ≤ 450 | 573.0 | FAIL (Phase D target) |
| S7 | memory_max_mb | ≤ 450 | 596.0 | FAIL (Phase D target) |
| S8 | cache_hit_rate_last10 | ≥ 0.9 | 1.0 (min) | PASS (whole-run rates 0.68 / 1.00) |
| S8 | memory_max_mb | ≤ 450 | 575.0 | FAIL (Phase D target) |
| S9 | memory_drift_mb | ≤ 50 | — | Pending (owner run when away) |
| S9 | memory_max_mb | ≤ 450 | — | Pending (owner run when away) |

Every scripted run launched, produced JSONL, and the driver exited 0; the gate exits 1 on the expanded table above. No harness-level failures (no torn or missing run files). Min budgets evaluate the cross-run minimum, max budgets the p95. The memory ceiling misses across S1–S8 (501–596 MB vs the 300/450 targets), plus `S2.first_paint_ms`, `S3-hit.ttfs_ms`, and `S4.advance_max_gap_ms`, are recorded as genuine Phase D targets; no budgets were weakened.

## S9 — pending owner run

S3-miss, S3-hit, S4, S7, and S8 are automated and recorded above (fresh runner-produced runs). S9 (2-hour soak) still needs an owner run when away:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S9 -Runs 1
```

The driver defaults to `--minutes 120`; the runner does not forward extra flags to `drive.mjs`, so the default 120-minute run is the owner protocol. A different duration would require a direct driver invocation (`node scripts/perf/drive.mjs --scenario S9 --minutes <N>`) against a manually started app. Leave a queue playing/skipping for the full duration (skip/quality-switch churn is allowed); do not restart the app.

## Recalibration (T3-D9)

`budgets.json` now mirrors the full measurable spec §4.2 table: memory ceilings (300 MB idle for S1/S2, 450 MB during playback for S3–S9), S5/S6 `frame_p99_ms` ≤ 33, S1/S2 `long_task_max_ms` ≤ 100, S3-miss `ttfs_ms_mean` ≤ 1200 (explicitly a mean proxy for the spec's median), and S8 moved to `cache_hit_rate_last10 ≥ 0.9` (second-pass hit rate; the whole-run `cache_hit_rate` stays informational). `check.mjs` now emits `frame_p99_ms:<mode>` and `<metric>_min` aggregates; min budgets evaluate the minimum across runs while max budgets keep the p95, so one warm run can no longer mask a cold one.

The expanded table surfaces genuine Phase D targets rather than threshold problems — memory (S1 518, S2 511 vs 300; S3–S8 573–596 vs 450), `S2.first_paint_ms` (1052 vs 1000, reflecting the corrected process-relative base), `S3-hit.ttfs_ms` (308 vs 300), and `S4.advance_max_gap_ms` (103 vs 100). No budgets were weakened and no thresholds changed; any adjustment still requires owner sign-off. S9 may surface one round of proposals once the owner runs it.
