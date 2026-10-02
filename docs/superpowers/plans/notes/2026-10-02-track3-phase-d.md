# Track 3 — Phase D (memory, cache/eviction, stability) final report

- **Date:** 2026-10-02
- **Build:** `track3-memory-cache` worktree. Rust binary `target/debug/oxitide-app.exe` built at `f51d9c8` (Phase D implementation HEAD; no Rust changes after it); the UI is served by the worktree's Vite dev server on 1420 (warm for every measured run). Scenarios driven over the WebView2 CDP endpoint (`scripts/perf/run-scenario.ps1` + `scripts/perf/drive.mjs`); budget gate `scripts/perf/check.mjs` against the spec §4.2 table plus the Phase C/D rows. `t_ms` is process-relative (loader time included), same timing base as the baseline.
- **Machine:** Ryzen 7 5700X3D / 32 GB / RX 9070 XT / Windows 11 25H2 (same machine as the baseline).
- **Conditions:** quiet machine for the whole final corpus — VMs and browsers closed, only two idle Brave tabs; single app, foreground, Vite warm, network as-is. `S9` is the Task 8 120-minute soak (`1790869962`, 2026-10-01) reused as the final S9 corpus; `S10` is the owner checklist and is **pending**.
- **Corpus:** S1, S2, S3-miss, S3-hit, S4, S5-instrument, S5-atmosphere, S6, S7 ×5, S8 ×1, S9 ×1. Raw JSONL is gitignored under `.superpowers/perf/runs/`; every invalid or noisy batch was quarantined, never deleted (inventory at the end).

## Final budget table (S1–S10)

| Scenario | Metric | Budget | Baseline | Final | Status |
| --- | --- | --- | --- | --- | --- |
| S1 | first_paint_ms | ≤ 1500 | 1113.0 | 1228.0 | PASS |
| S1 | ui_ready_ms | ≤ 2500 | 1331.0 | 1564.0 | PASS |
| S1 | memory_max_mb | ≤ 570 | 518.0 | 492.0 | PASS (renegotiated) |
| S1 | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S1 | startup_step_max_ms | ≤ 100 | — | 3.0 | PASS (Phase C row) |
| S2 | first_paint_ms | ≤ 1000 | 1052.0 | 931.0 | PASS |
| S2 | memory_max_mb | ≤ 560 | 511.0 | 487.0 | PASS (renegotiated) |
| S2 | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S2 | startup_step_max_ms | ≤ 100 | — | 1.0 | PASS (Phase C row) |
| S3-miss | ttfs_ms | ≤ 2500 | 354.0 | 383.0 | PASS |
| S3-miss | ttfs_ms_mean | ≤ 1200 | 340.2 | 349.8 | PASS |
| S3-miss | memory_max_mb | ≤ 645 | 584.0 | 557.0 | PASS (renegotiated) |
| S3-hit | ttfs_ms | ≤ 300 | 308.0 | 229.0 | PASS |
| S3-hit | cache_hits | ≥ 1 | 1.0 (min) | 2.0 (min) | PASS |
| S3-hit | memory_max_mb | ≤ 645 | 582.0 | 559.0 | PASS (renegotiated) |
| S4 | advance_max_gap_ms | ≤ 100 | 103.0 | 105.0 | **FAIL (documented, owner-ruled)** |
| S4 | memory_max_mb | ≤ 645 | 591.0 | 560.0 | PASS (renegotiated) |
| S5-instrument | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | 7.1 | PASS |
| S5-instrument | frame_p99_ms:instrument | ≤ 33 | 20.9 | 13.9 | PASS |
| S5-instrument | long_task_max_ms | ≤ 100 | 65.0 | 0.0 | PASS |
| S5-instrument | page_open_ms | ≤ 500 | — | 2.2 | PASS (Phase D row) |
| S5-instrument | memory_max_mb | ≤ 690 | 586.0 | 597.0 | PASS (renegotiated) |
| S5-atmosphere | frame_p95_ms:atmosphere | ≤ 20 | 7.1 | 7.0 | PASS |
| S5-atmosphere | frame_p99_ms:atmosphere | ≤ 33 | 20.8 | 13.9 | PASS |
| S5-atmosphere | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S5-atmosphere | page_open_ms | ≤ 500 | — | 2.2 | PASS (Phase D row) |
| S5-atmosphere | memory_max_mb | ≤ 675 | 588.0 | 584.0 | PASS (renegotiated) |
| S6 | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | 7.1 | PASS |
| S6 | frame_p99_ms:instrument | ≤ 33 | 13.9 | 20.9 | PASS |
| S6 | filter_max_ms | ≤ 50 | — | 22.1 | PASS (Phase C row) |
| S6 | sort_max_ms | ≤ 150 | — | 33.1 | PASS (Phase C row) |
| S6 | album_open_ms | ≤ 1000 | — | 17.6 | PASS (Phase D row) |
| S6 | memory_max_mb | ≤ 740 | 573.0 | 641.0 | PASS (renegotiated) |
| S7 | queue_build_ms | ≤ 250 | — | 117.4 | PASS (Phase D row) |
| S7 | memory_max_mb | ≤ 670 | 596.0 | 580.0 | PASS (renegotiated) |
| S8 | cache_hit_rate_last10 | ≥ 0.9 | 1.0 (min) | 0.9 (min) | PASS |
| S8 | memory_max_mb | ≤ 660 | 575.0 | 574.0 | PASS (renegotiated) |
| S9 | memory_drift_mb | ≤ 50 | 56.0 (raw) | 14.0 (settled) | PASS (metric corrected) |
| S9 | memory_drift_app_mb | ≤ 25 | — | 1.0 | PASS (Phase D row) |
| S9 | memory_max_mb | ≤ 755 | 585.0 | 655.0 | PASS (renegotiated) |
| S10 | manual stability | checklist | — | pending | UNMEASURED (owner) |

The gate (`node scripts/perf/check.mjs`) exits 1 on the single documented S4 row only. Every other row passes; no budget was weakened (the memory ceilings were raised only under the owner-approved renegotiation below, and the drift row's threshold is unchanged).

### Renegotiation and metric rulings (owner-approved 2026-10-02)

- **Peak memory ceilings.** All-process p95 vs the old 300/450 ceilings, with the app-owned share measured from the `memory_process` rows at the peak sample:

  | Scenario | Old | p95 all | App-only | WebView2 | Proposed (signed) |
  | --- | --- | --- | --- | --- | --- |
  | S1 idle | 300 | 492 | 52 | 430 | **570** |
  | S2 idle | 300 | 487 | 53 | 424 | **560** |
  | S3-miss | 450 | 557 | 72 | 476 | **645** |
  | S3-hit | 450 | 559 | 72 | 480 | **645** |
  | S4 | 450 | 560 | 79 | 478 | **645** |
  | S5-atmosphere | 450 | 584 | 53 | 522 | **675** |
  | S5-instrument | 450 | 597 | 53 | 534 | **690** |
  | S6 | 450 | 641 | 53 | 578 | **740** |
  | S7 | 450 | 580 | 76 | 494 | **670** |
  | S8 | 450 | 574 | 80 | 484 | **660** |
  | S9 | 450 | 655 | 91 | 554 | **755** |

  The app process never exceeds 93 MB at any point in any scenario (conhost 10 MB); the WebView2 floor alone exceeds the old ceilings, so the +15 % headroom over the all-process p95 is a regression catch, not an app-side allowance. `budgets.json` and spec §4.2 carry the signed values.
- **S9 drift semantics.** The raw `last − first` metric anchored at the first sample (t≈2.5 s), which lands mid-WebView2 startup (363 MB total; 47 app / 306 WebView2 / largest child 110). Measured from a settled baseline (median of the 1–5 min window vs the last 5 min), the same 2 h soak drifts **+14 MB all-process and +1 MB app-only**; the Phase A 3 h baseline reads **+2 MB** under the same definition (its raw 56 was the same artifact). The +50 threshold is unchanged; the app-only row (`memory_drift_app_mb ≤ 25`) is the leak detector. WebView2 sawtooths with 23 GC drops >15 MB, plateaus ~560–566 MB all-process after ~95 min, and its largest child plateaus ~225 MB; the app is flat (84–86 MB bucket medians from minute 5).
- **S5/S6 frame percentiles.** Each S5 run's first `ui_metrics` flush is a partial window (n≈95) emitted by the page-open interaction right after `goto("tracks")`; its p99 is effectively the max of the startup frames (34.7–41.8), which is not the fling the budget describes. `check.mjs` now ignores frame windows with fewer than 200 frames; S5-instrument/atmosphere re-gate at p99 13.9 (Phase C read 14.0) and S6 is unchanged at 20.9.
- **S4 `advance_max_gap_ms`.** Final 105 ms (per-run maxima 100/100/100/101/105; transition gaps 89–105; the first transition is the slow one). Per the owner ruling the budget stays at 100 with the WASAPI-ring backpressure finding documented and no metric-semantics change; it remains the only FAIL in the table.

## Cache/eviction evidence (Phase D offenders)

- **Palette cache bounded** — `7a07b44 perf(app): bound and prune the palette cache`: the `palettes.json` map keeps the newest N entries with a `fetched_at` timestamp, prunes on load/write, and reparses the old format; the unbounded map from the baseline survey is gone (unit tests: cap, recency pruning, old-format parse).
- **Scan-free cache accounting** — `a6ad086 perf(audio): track cache bytes incrementally and skip scans under the cap`: on-disk/pending bytes are maintained as atomics across commit/abort/clear/remove, so `finish_commit` no longer rescans the directory on the normal path; eviction commit cost is O(evicted). Bookkeeping is pinned across clear/abort interleavings.
- **Playing/prefetched protection** — `af4827e perf(audio): protect playing and prefetched cache entries for the stream lifetime`: the current stream's entry cannot be evicted mid-playback, so the S8 loop does not re-download its own playing track; the 2 GiB default cap is unchanged (owner: keep 2 GiB).
- **S8 result** — 10 shortest favorites at Max, `repeat all`, `2 × 707 s + 90 s` window: 15 hits / 11 misses over the whole run (the first pass is cold by design), **`cache_hit_rate_last10` 0.9** (gate min 0.9) — the second pass is hit-dominated with no track re-downloaded twice within a loop. The S8 memory row now passes at 574 MB (ceiling 660).

## Cached-seek fix

- Baseline defect: a cached seek passed the full plan (with `seek_ms`) into `decode_source` while the sink discarded relative to the segment start, so playback resumed near the track start with `position_ms` seeded at the target (reported position ran past `duration_ms`).
- Fix `ccfdcb1 fix(audio): discard cached seeks relative to the source start` records `source_start_frames` and discards relative to the actual source start; the network path is unchanged. Engine tests: `discard_is_relative_to_the_actual_source_start`, `cached_seek_starts_at_the_target`.
- **Owner smoke: pending** — checklist in `2026-10-01-track3-phase-d-smoke.md` (play to completion, seek mid-track twice and backwards, uncached seek unchanged).

## S10 — manual stability (pending owner)

Checklist in `2026-10-01-track3-phase-d-smoke.md`: device hot-plug (pause + notice, resume, default-device switch mid-playback), cache clear-on-exit integrity (clean exit, empty cache, `get_cache_status` 0, no `.part` files), and the cached-seek items above. The Task 8 S9 soak already covers the automated stability half: 120 min, 3,607 memory samples, 0 error lines, no crash, cache files intact.

## Official-client sanity (pending owner)

Non-gating qualitative comparison (cold start feel, TTFS, scroll smoothness, Task Manager memory) against the official TIDAL desktop client, to be recorded here by the owner.

## What each Phase D commit changed

- `05a6d09 feat(perf): attribute memory samples per process` — `memory_process` JSONL rows (name, mb, self) alongside the aggregate; the evidence base for the renegotiation.
- `7a07b44 perf(app): bound and prune the palette cache` — newest-N bound + `fetched_at` recency.
- `a6ad086 perf(audio): track cache bytes incrementally and skip scans under the cap` — incremental on-disk/pending atomics; no full-directory rescan on normal play.
- `af4827e perf(audio): protect playing and prefetched cache entries for the stream lifetime`.
- `ccfdcb1 fix(audio): discard cached seeks relative to the source start` — the baseline cached-seek defect.
- `cb84690 feat(perf): gate the 10k play-all queue build` — `queue_build_ms` (S7).
- `bf42b06 feat(perf): gate page-open and the 500-track album` — `page_open_ms` (S5), `album_open_ms` (S6).
- `9f2e4f5 feat(perf): pass the S9 soak duration through the runner`; `a81c5bb test: add the Phase D smoke checklist`.
- `83a8010 fix(perf): reject PID-reuse false positives in the process-tree walk`; `276a2d7 fix(perf): sample only the app and WebView2 processes` — memory rows count `oxitide-app.exe` + `msedgewebview2.exe` + `conhost.exe` only.
- `f51d9c8 fix(perf): build the synthetic queue from the app shell` — S7 queue-build fixture.
- `586ac42 feat(perf): anchor soak drift at a settled baseline and gate app-only drift`; `5959325 fix(perf): ignore partial frame windows in the frame percentiles` — the two owner-approved metric corrections above.
- `b1f95e7 Merge branch 'main' into track3-memory-cache` — brings the LICENSE/CSP/docs-refresh commits into the branch.

## Raw run inventory

| Scenario | Runs (run ids) | Quarantine |
| --- | --- | --- |
| S1 | `1790893963`, `1790893995`, `1790894027`, `1790894060`, `1790894092` | 22:04 noisy batch → `runs-quarantine/phase-d-pre-quiet/` |
| S2 | `1790894130`, `1790894162`, `1790894194`, `1790894226`, `1790894258` | same |
| S3-miss | `1790894296`, `1790894337`, `1790894378`, `1790894419`, `1790894461` | same |
| S3-hit | `1790898105`, `1790898146`, `1790898185`, `1790898224`, `1790898265` | first-run spike batch `1790894507`–`1790894772` → `runs-quarantine/phase-d-s3hit-firstrun-spike/`; re-run passed at 229 ms |
| S4 | `1790894861`, `1790894999`, `1790895136`, `1790895273`, `1790895410` | earlier PID-reuse and pre-allowlist corpora → `phase-d-pid-reuse/`, `phase-d-pre-allowlist/` |
| S5-instrument | `1790895556`, `1790895602`, `1790895648`, `1790895694`, `1790895740` | 22:04 noisy batch → `phase-d-pre-quiet/` |
| S5-atmosphere | `1790895912`, `1790895954`, `1790895997`, `1790896040`, `1790896082` | same |
| S6 | `1790896129`, `1790896173`, `1790896216`, `1790896259`, `1790896302` | same |
| S7 | `1790896350`, `1790896387`, `1790896425`, `1790896462`, `1790896500` | same |
| S8 | `1790896544` | — |
| S9 | `1790869962` (2026-10-01 Task 8, 120 min) | — |
| S10 | owner checklist | pending |

Quarantine trail also includes `phase-d-s3hit-cold/` (pre-fix S3-hit) and the Phase D pre-allowlist corpus. Nothing was deleted.

## Rulings recorded

- **PID-reuse false positive** (`83a8010`): the process-tree walk now orders by creation time so an ancestor whose PID was reused by another app cannot fold its memory into the aggregate. Cost if wrong: an ancestor whose creation time cannot be read is still followed, and a same-millisecond reuse could slip through.
- **S9 drift semantics** (`586ac42`, owner-approved): settled baseline + app-only row, threshold unchanged. Cost if wrong: a WebView2 leak between +14 and +50 MB per 2 h passes (bounded by the +50 row) and an app leak below +25 MB per 2 h passes.
- **Frame percentiles** (`5959325`, owner-approved): partial windows (<200 frames) excluded. Cost if wrong: a genuine jank burst confined to the first ~200 frames of a run would not set the percentile rows.
- **Peak ceilings** (owner-approved 2026-10-02): per-scenario values above. Cost if wrong: a WebView2 regression up to +15 % over the measured p95 passes silently.
- **S4** (owner-ruled): budget stays at 100; the WASAPI-ring finding is documented, no metric change.
