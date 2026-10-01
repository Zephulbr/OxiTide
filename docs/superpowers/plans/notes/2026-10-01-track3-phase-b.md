# Track 3 — Phase B (startup + TTFS) re-measurement

- **Date:** 2026-10-01
- **Build:** `track3-startup-ttfs` worktree (base `c02b81b`), debug build (`target/debug/oxitide-app.exe`, HEAD `468630f`). Served by the Vite dev server on port 1420; scenarios driven over the WebView2 CDP endpoint (`scripts/perf/run-scenario.ps1` + `scripts/perf/drive.mjs`); budget gate `scripts/perf/check.mjs` with the spec §4.2 budget table. `t_ms` is process-relative (loader time included), same timing base as the baseline.
- **Machine:** Ryzen 7 5700X3D / 32 GB / RX 9070 XT / Windows 11 25H2 (same machine as the baseline).
- **Conditions:** `S1`/`S2`/`S3-miss`/`S3-hit`/`S4` x5, single app, foreground, no deliberate background load, network as-is, Vite warm for every measured run. Raw JSONL is gitignored under `.superpowers/perf/runs/`; this worktree's corpus started empty, so no Phase A artifacts were involved. One exception: the very first S1 batch (5 runs, `1790851213`–`1790851343`) was taken while the harness was starting Vite cold, so run 1 carried the first-bundle transform cost (`first_paint` 2723 ms; warm runs 908–945 ms). That batch was quarantined to `.superpowers/perf/runs-quarantine/s1-vite-cold/` and S1 was re-run x5 with Vite warm, matching the baseline's warm-Vite condition. Nothing was deleted.

## Before/after (p95 vs the 2026-09-30 baseline)

| Scenario | Metric | Budget | Baseline | Phase B | Status |
| --- | --- | --- | --- | --- | --- |
| S1 | first_paint_ms | ≤ 1500 | 1113.0 | **938.0** | PASS (was PASS) |
| S1 | ui_ready_ms | ≤ 2500 | 1331.0 | **1207.0** | PASS (was PASS) |
| S1 | memory_max_mb | ≤ 300 | 518.0 | 492.0 | FAIL (Phase D) |
| S1 | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S2 | first_paint_ms | ≤ 1000 | 1052.0 | **949.0** | **PASS (was FAIL)** |
| S2 | memory_max_mb | ≤ 300 | 511.0 | 496.0 | FAIL (Phase D) |
| S2 | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S3-miss | ttfs_ms | ≤ 2500 | 354.0 | 496.0 | PASS (was PASS) |
| S3-miss | ttfs_ms_mean | ≤ 1200 | 340.2 | 424.6 | PASS (was PASS) |
| S3-miss | memory_max_mb | ≤ 450 | 584.0 | 562.0 | FAIL (Phase D) |
| S3-hit | ttfs_ms | ≤ 300 | 308.0 | **294.0** | **PASS (was FAIL)** |
| S3-hit | cache_hits | ≥ 1 | 1.0 (min) | 1.0 (min) | PASS |
| S3-hit | memory_max_mb | ≤ 450 | 582.0 | 566.0 | FAIL (Phase D) |
| S4 | advance_max_gap_ms | ≤ 100 | 103.0 | 101.0 | FAIL (Phase D) |
| S4 | memory_max_mb | ≤ 450 | 591.0 | 570.0 | FAIL (Phase D) |

`S2.first_paint_ms` and `S3-hit.ttfs_ms` moved inside their budgets; no budget was weakened. The memory ceilings (300 MB idle / 450 MB playback) remain genuine misses (485–570 MB) and stay Phase D targets, as do `S4.advance_max_gap_ms` (101 vs 100, improved from 103) — all five `check.mjs` runs exited 1 on memory only, except S4, which also failed the advance row.

### S1 — cold start, 5 runs (`1790851412`–`1790851541`)

```
PASS       S1               first_paint_ms               938.0  (max 1500)
PASS       S1               ui_ready_ms                  1207.0  (max 2500)
FAIL       S1               memory_max_mb                492.0  (max 300)
PASS       S1               long_task_max_ms             0.0  (max 100)
```

| Run | config_loaded_ms | window_shown_ms | first_paint_ms | ui_ready_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 366 | 710 | 900 | 1105 | 0 | 489 |
| 2 | 367 | 727 | 921 | 1207 | 0 | 488 |
| 3 | 384 | 750 | 932 | 1164 | 0 | 492 |
| 4 | 371 | 727 | 915 | 1117 | 0 | 487 |
| 5 | 390 | 750 | 938 | 1205 | 0 | 491 |
| **p95** | 390.0 | 750.0 | **938.0** | **1207.0** | **0.0** | **492.0** |

### S2 — warm start, 5 runs (`1790851578`–`1790851707`)

```
PASS       S2               first_paint_ms               949.0  (max 1000)
FAIL       S2               memory_max_mb                496.0  (max 300)
PASS       S2               long_task_max_ms             0.0  (max 100)
```

| Run | config_loaded_ms | window_shown_ms | first_paint_ms | ui_ready_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 362 | 718 | 903 | 1138 | 0 | 496 |
| 2 | 358 | 706 | 892 | 1090 | 0 | 487 |
| 3 | 362 | 711 | 904 | 1208 | 0 | 485 |
| 4 | 397 | 756 | 949 | 1203 | 0 | 486 |
| 5 | 361 | 719 | 907 | 1375 | 0 | 491 |
| **p95** | 397.0 | 756.0 | **949.0** | **1375.0** | **0.0** | **496.0** |

## Startup breakdown (S1 p95; S2 matches S1 within 10 ms through `window_shown`; the post-window marks diverge — S2 `first_paint` 949 vs 938 and `ui_ready` 1375 vs 1207)

Marks, p95 over the 5 runs. `t_ms` is the p95 of each mark's absolute
process-relative time; `Delta` is the p95 of the per-run difference between
adjacent marks (so it is not derivable from the `t_ms` column by subtraction):

| Mark | t_ms | Delta |
| --- | --- | --- |
| logging_ready | 390 | loader span 0 → 390 (exe/DLL load, runtime-owned) |
| config_loaded | 390 | **+1 ms** from logging_ready |
| cache_ready | 391 | **+1 ms** from config_loaded |
| services_ready | 392 | **+2 ms** from cache_ready (artwork/lyrics/scrobbler construction) |
| window_shown | 750 | +364 ms from services_ready (Tauri builder + WebView2 environment/window construction, geometry apply, show) |
| cache_cleanup_done | 754 | background thread, **+4 ms after show** |
| startup_background_done | 771 | scheme remember + `register_all`, **+21 ms after show** |
| first_paint | 938 | +194 ms from window_shown (WebView dev bundle + first render) |
| ui_ready | 1207 | +286 ms from first_paint (player built + auth restored) |

Every application-owned step before `window_shown` is now ≤ 2 ms: config load, cache construction, and service construction. The former pre-paint work (full cache cleanup scan, `palettes.json`/`lyrics.json` parses, keyring read, `reg query`/`register_all`, scheme snapshot) now completes 4–21 ms *after* the window is shown and cannot block first paint. The only remaining >100 ms spans before first paint are runtime-owned: the loader (390 ms), Tauri/WebView2 window construction (364 ms), and the dev-bundle first render (194 ms) — none of these is app work the Phase B design covers. Baseline comparison: `config_loaded → first_paint` was 578–658 ms; it is now 534–554 ms (p95-of-delta 554 vs 658).

## TTFS attribution

### S3-miss — cold-cache, 5 runs (`1790851745`–`1790851908`)

Every run: 0 hits, 1 miss, `cache_miss` + `first_pcm` emitted.

| Run | ttfs_ms | resolve_ms | fetch_ms | output_open_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 496 | 170 | 205 | 120 | 562 |
| 2 | 429 | 169 | 169 | 90 | 558 |
| 3 | 384 | 161 | 128 | 94 | 556 |
| 4 | 402 | 167 | 139 | 94 | 561 |
| 5 | 412 | 169 | 155 | 87 | 550 |
| **p95** | **496.0** | **170.0** | **205.0** | **120.0** | **562.0** |
| **mean** | 424.6 | 167.2 | 159.2 | 97.0 | |
| **share of p95 ttfs** | 100% | 34.3% | 41.3% | 24.2% | |

The three stages account for the whole pipeline (per run, `resolve + fetch + open` = ttfs ± 2 ms). `resolve` is the TIDAL stream/manifest resolution (network), `fetch` is first-segment download to stream-info (network), `open` is WASAPI output start (device). No stage is anywhere near dominating: the largest is `fetch` at 41%.

### S3-hit — warm cache, 5 runs (`1790851957`–`1790852222`)

The gate reads the **last** `ttfs` (the replay); the attribution metrics record the **first** occurrence of each stage (the play-in), so stage sums match the first play, not the replay. Run 1 is the only cold entry.

| Run | first ttfs_ms (play-in) | last ttfs_ms (replay) | hits | misses | resolve_ms | fetch_ms | output_open_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 412 (miss) | 294 (hit) | 1 | 1 | 179 | 140 | 92 | 557 |
| 2 | 264 (hit) | 222 (hit) | 2 | 0 | 165 | 6 | 93 | 563 |
| 3 | 271 (hit) | 232 (hit) | 2 | 0 | 175 | 7 | 89 | 558 |
| 4 | 264 (hit) | 239 (hit) | 2 | 0 | 165 | 6 | 92 | 557 |
| 5 | 266 (hit) | 177 (hit) | 2 | 0 | 167 | 6 | 92 | 566 |
| **p95 (last)** | — | **294.0** | **2.0** | 0 | **179.0** | **140.0** | **93.0** | **566.0** |
| **minimum** | | | **1** | | | | | |

Warm play-ins show `fetch` collapsing to 6–7 ms (cache read) while `resolve` stays ~165–175 ms and `open` ~90 ms; the replay is faster still (177–239 ms) because the output is already open (no `output_opened` emit) and buffers carry over. The cache-hit floor (≥1) is met by the cross-run minimum.

## Task 6 Step 2 decision — documented stop, no speculative fix

The bounded rule requires one stage > 60% of `ttfs_ms` **and** a safe, behavior-preserving change following directly from the data. On the gated S3-miss corpus the shares are `fetch` 41.3%, `resolve` 34.3%, `output_open` 24.2% — no dominant stage. On S3-hit warm replays the replay's ~165 ms resolve is the bulk of 177–239 ms, but (a) the budget row now passes (294 ≤ 300), (b) removing it would mean caching/pre-resolving TIDAL stream plans across plays — stream URLs expire and reusing them changes server-side session/licensing behavior, so it is **not** provably behavior-preserving and needs a Phase D design review, not a Phase B drive-by. `output_open` (90–120 ms) is device-open on WASAPI and pre-opening is not provably safe without changing device handling. Per the rule: **document the breakdown and stop; no fix applied, no S3 re-run needed.** These stage p95s (resolve 170 ms, fetch 205 ms, open 120 ms cold) become the Phase D reference if the TTFS budgets ever tighten.

Two measured notes for Phase D:

- `S3-miss.ttfs_ms` p95 rose 354 → 496 (mean 340 → 425) with no budget miss (5x headroom). With no baseline stage data we cannot attribute the difference to a stage; `resolve` is near-constant (161–170 ms) across all runs and both cache states, while `fetch` swings 128–205 ms — consistent with ordinary TIDAL API/CDN network variation between the two sessions rather than a Phase B regression.
- `S4.advance_max_gap_ms` improved 103 → 101 (5-run max; transitions were 91–101 and 89–90) but stays 1 ms over. Every run's first transition is the slow one; gaps are bounded by output-start scheduling, not by the old poll (which would have shown 200 ms steps). Phase D target.

## Which Phase B changes produced the deltas

- `112036b perf(app): gate the window on geometry only and background the rest` (Task 3) — main driver of S1/S2: `show()` now follows only handler registration + geometry; cache cleanup, scheme snapshot/remember, and `register_all` run on the `oxitide-startup` thread (observed `cache_cleanup_done` / `startup_background_done` at +4/+21 ms after `window_shown`), and the duplicate login `Config::load` is gone. `config_loaded → first_paint` improved ~104 ms (p95-of-delta 658 → 554) and `S2.first_paint_ms` crossed back under 1000 ms.
- `fa8c730 perf(app): load artwork and lyrics caches lazily` (Task 1) — removes the synchronous `palettes.json`/`lyrics.json` parse from service construction; `config_loaded → services_ready` is now 2 ms.
- `736cd16 perf(app): load scrobbler credentials lazily` (Task 2) — keyring I/O leaves the pre-show path (one-shot load on first `start()`/`status()`).
- `031ed5b perf(audio): wake the worker on stream events instead of polling` (Task 4) — removes the 200 ms `wait_for_info` poll; consistent with the S4 max gap 103 → 101 and the S3-hit replay p95 308 → 294 (both still tight margins; the replay is dominated by the resolve stage, not the wait).
- `468630f feat(perf): attribute time-to-first-sound by stage` (Task 5) — measurement only; no runtime behavior.

## Raw run inventory

| Scenario | Runs (run ids) | Quarantine |
| --- | --- | --- |
| S1 | `1790851412`, `1790851444`, `1790851476`, `1790851509`, `1790851541` | cold-Vite batch `1790851213`–`1790851343` → `runs-quarantine/s1-vite-cold/` |
| S2 | `1790851578`, `1790851610`, `1790851643`, `1790851675`, `1790851707` | — |
| S3-miss | `1790851745`, `1790851785`, `1790851825`, `1790851867`, `1790851908` | — |
| S3-hit | `1790851957`, `1790852105`, `1790852144`, `1790852183`, `1790852222` | — |
| S4 | `1790852266`, `1790852403`, `1790852539`, `1790852676`, `1790852812` | — |

All 25 measured runs launched, produced JSONL, and the driver exited 0; no login timeouts. `check.mjs` exits were 1 on the memory rows (plus S4's advance row), as expected for Phase B.
