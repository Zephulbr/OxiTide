# Track 3 — scripted perf baseline

- **Date:** 2026-09-30
- **Build:** `track3-harness` worktree, debug build (`target/debug/oxitide-app.exe`, HEAD `c67fb5e` plus this commit's S4 natural-transition rewrite and `S3-hit` cache-hit floor) served by the Vite dev server on port 1420; scenarios driven over the WebView2 CDP endpoint (`scripts/perf/run-scenario.ps1` + `scripts/perf/drive.mjs`); budget gate `scripts/perf/check.mjs`.
- **Machine:** Ryzen 7 5700X3D / 32 GB / RX 9070 XT / Windows 11 25H2.
- **Conditions:** per-scenario mode is auto-pinned by the driver (S5/S6 instrument, S5-atmosphere atmosphere; the owner's persisted mode is restored after each run). Network as-is (no throttling or isolation). All runs were single-app, foreground, no deliberate background load. The 14 scratch runs left over from harness development were moved to `.superpowers/perf/runs-prebaseline/` before this baseline; after the Task 8b review, the earlier S3/S4 scratch corpus (11 files) was quarantined to `.superpowers/perf/runs-quarantine/` and S3/S4 were re-run fresh. Every S3/S4 aggregate below therefore covers only the 15 uniform runner-produced runs (S3-miss x5, S3-hit x5, S4 x5); the other sections cover the original 26-run corpus (S1 x5, S2 x5, S5-instrument x5, S5-atmosphere x5, S6 x3, S7 x3). Raw JSONL is gitignored under `.superpowers/perf/runs/`; command logs are under `.superpowers/perf/logs/`.

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

## S3-miss — cold-cache TTFS, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-miss -Runs 5`

```
PASS       S3-miss          ttfs_ms                      354.0  (max 2500)
```

Measured per run (driver clears the cache at the start of every run, then plays the first favorite):

| Run | ttfs_ms | cache hits | cache misses |
| --- | --- | --- | --- |
| 1 | 348 | 0 | 1 |
| 2 | 329 | 0 | 1 |
| 3 | 332 | 0 | 1 |
| 4 | 354 | 0 | 1 |
| 5 | 338 | 0 | 1 |
| **p95 (5 runs)** | **354.0** | 0 | 1 |

Every run has zero hits and exactly one miss. `check.mjs` exit code 0.

## S3-hit — warm-cache TTFS with cache-hit floor, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S3-hit -Runs 5`

```
FAIL       S3-hit           ttfs_ms                      308.0  (max 300)
PASS       S3-hit           cache_hits                   2.0  (min 1)
```

Measured per run (driver plays the first favorite, waits for the cache entry to finalize via `get_cache_status`, then replays it; the last `ttfs` is the replay):

| Run | first ttfs_ms (play-in) | last ttfs_ms (replay) | cache hits | cache misses |
| --- | --- | --- | --- | --- |
| 1 (cold entry) | 479 (miss) | 308 (hit) | 1 | 1 |
| 2 | 199 (hit) | 247 (hit) | 2 | 0 |
| 3 | 211 (hit) | 176 (hit) | 2 | 0 |
| 4 | 202 (hit) | 285 (hit) | 2 | 0 |
| 5 | 205 (hit) | 292 (hit) | 2 | 0 |
| **p95 (5 runs)** | — | **308.0** | **2.0** | 0 |

Every run contains at least one `cache_hit` (run 1 warms the entry, then hits on replay); the new `cache_hits ≥ 1` budget row passes. The `ttfs_ms` p95 misses the 300 ms budget by 8 ms (run 1's cold-start replay at 308 ms; warm replays are 176–292 ms). `check.mjs` exit code 1 on the ttfs budget only.

## S4 — natural gapless advance over the 3 shortest tracks, 5 runs

Command: `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S4 -Runs 5`

```
FAIL       S4               advance_max_gap_ms           103.0  (max 100)
```

The driver sorts favorites by duration ascending and plays the 3 shortest with `repeat off`: `41352612` (49 s), `458258760` (50 s), `458258761` (66 s); transition bound = 49 + 50 + 90 = 189 s. No seeks (natural playback only). Measured `advance` gaps per run (first transition → second transition):

| Run | advance → 458258760 | advance → 458258761 | max |
| --- | --- | --- | --- |
| 1 | 22 | 40 | 40 |
| 2 | 26 | 38 | 38 |
| 3 | 26 | 40 | 40 |
| 4 | 103 | 40 | 103 |
| 5 | 103 | 39 | 103 |
| **p95 (5 runs)** | | | **103.0** |

Runs 1–3 pass; runs 4–5 exceed the 100 ms gap on the first transition. `check.mjs` exit code 1.

## Discovered during baseline — cached-seek defect (deferred to Phase D)

Seeking a fully cached track does not reposition playback. In `crates/oxitide-audio/src/engine.rs`, the cache path's `decode_from_file` passes the full plan (with `seek_ms`) into `decode_source`, while `ChannelSink::on_stream_info` computes a segment-relative discard via `seek::target_segment`; the network path instead truncates the download plan first with `plan.from_segment(target.segment_index)` in `decode_from_network`. Net effect on a cache hit: the seek resumes near the track start while `position_ms` is seeded at the target, so the reported position runs past `duration_ms` and no natural `advance` occurs near the seek target (observed during the S4 investigation: a 137 s seek on a 139 s cached track was still playing at position 257 s with no transition; an uncached network seek transitioned within ~2 s). This is recorded, not fixed; the scripted S4 protocol now uses natural playback only. Deferred to Phase D.

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
| S3-miss | ttfs_ms | ≤ 2500 | 354.0 | PASS |
| S3-hit | ttfs_ms | ≤ 300 | 308.0 | FAIL (over by 8 ms) |
| S3-hit | cache_hits | ≥ 1 | 2.0 | PASS |
| S4 | advance_max_gap_ms | ≤ 100 | 103.0 | FAIL (2/5 runs at 103 ms) |
| S8 | cache_hit_rate | ≥ 0.9 | — | UNMEASURED (owner run pending) |
| S9 | memory_drift_mb | ≤ 50 | — | UNMEASURED (owner run pending) |

The original scripted baseline invocations exited 0; the fresh S3/S4 dispatch records two budget misses (`S3-hit` ttfs 308 > 300, `S4` 103 > 100) while every gate ran and every run produced JSONL. No harness-level failures (every run launched, produced JSONL, and the driver exited 0; no torn or missing run files). The `track3-harness` branch fixes landed before this baseline (`2a11cde` pins scenario mode and allows empty budgets), so the driver mode pin was active throughout.

## Manual scenarios — pending owner run

S3-miss, S3-hit, and S4 are now automated and recorded above (fresh runner-produced runs). S8 and S9 still need an owner run with a real account and playback.

Commands:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S8 -Runs 2 -Manual
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S9 -Runs 1 -Manual
```

Protocol per scenario (the runner prompts on each run):

- **S8 — Pending.** Play the same 10 Max-quality tracks in a loop twice (album repeat); note the queue actually uses Max.
- **S9 — Pending.** Leave a queue playing/skipping for 2 hours (skip/quality-switch churn is allowed); do not restart the app.

## Recalibration (T3-D9)

`budgets.json` gained one row for this batch: `S3-hit.cache_hits ≥ 1` (Task 8b review ruling), so a zero-hit corpus can no longer pass the hit scenario on `ttfs` alone. No threshold recalibration is proposed yet: the fresh S3/S4 batch records two baseline misses (`S3-hit` ttfs p95 308 ms vs 300; `S4` gap p95 103 ms vs 100) that are optimization targets, not threshold changes. Any adjustment to `budgets.json` or the spec §4.2 table still requires owner sign-off; S8/S9 may surface one round of proposals once the owner runs them.
