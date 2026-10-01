# Track 3 — Phase C (UI + large library) re-measurement

- **Date:** 2026-10-01
- **Build:** `track3-ui-library` worktree. Rust binary `target/debug/oxitide-app.exe` built in the worktree at `5a5b557` (no Rust changes in Phase C); the UI is served by the worktree's Vite dev server on 1420 at `fd17319`, so every measured run carries all Phase C UI changes. Scenarios driven over the WebView2 CDP endpoint (`scripts/perf/run-scenario.ps1` + `scripts/perf/drive.mjs`); budget gate `scripts/perf/check.mjs` with the spec §4.2 budget table plus the new Phase C rows. `t_ms` is process-relative (loader time included), same timing base as the baseline.
- **Machine:** Ryzen 7 5700X3D / 32 GB / RX 9070 XT / Windows 11 25H2 (same machine as the baseline).
- **Conditions:** `S1`, `S2`, `S5-instrument`, `S5-atmosphere`, `S6`, `S7` ×5, single app, foreground, Vite warm for every measured run, no deliberate background load, network as-is. Library at measurement time: 2140 favorite tracks (`S7` queue build). Raw JSONL is gitignored under `.superpowers/perf/runs/`; this worktree's corpus started empty. All invalid probe/validation runs (below) were quarantined, never deleted.
- **Harness fix during Task 6:** the scenario-mode pin read `get_config` at driver connect time, while WebView2 is still on `about:blank`; the read failed (`__TAURI_INTERNALS__` undefined), the pin was silently skipped, and the first S5-instrument/S6 batches recorded **atmosphere** samples. The pin now reads the config after login and `__perf` are up (`2e0d467`). Those first batches and a handful of CDP probe runs were quarantined to `.superpowers/perf/runs-quarantine/phase-c-invalid/`; every number below comes from the post-fix corpus.

## Before/after (p95 vs the 2026-09-30 baseline)

| Scenario | Metric | Budget | Baseline | Phase C | Status |
| --- | --- | --- | --- | --- | --- |
| S1 | first_paint_ms | ≤ 1500 | 1113.0 | **930.0** | PASS |
| S1 | ui_ready_ms | ≤ 2500 | 1331.0 | **1228.0** | PASS |
| S1 | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S1 | startup_step_max_ms | ≤ 100 | — | **1.0** | PASS (new row) |
| S1 | memory_max_mb | ≤ 300 | 518.0 | 491.0 | FAIL (Phase D) |
| S2 | first_paint_ms | ≤ 1000 | 1052.0 | **915.0** | PASS (was FAIL) |
| S2 | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S2 | startup_step_max_ms | ≤ 100 | — | **1.0** | PASS (new row) |
| S2 | memory_max_mb | ≤ 300 | 511.0 | 500.0 | FAIL (Phase D) |
| S5-instrument | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | 7.1 | PASS |
| S5-instrument | frame_p99_ms:instrument | ≤ 33 | 20.9 | **14.0** | PASS |
| S5-instrument | long_task_max_ms | ≤ 100 | 65.0 | **0.0** | PASS |
| S5-instrument | memory_max_mb | ≤ 450 | 586.0 | 605.0 | FAIL (Phase D) |
| S5-atmosphere | frame_p95_ms:atmosphere | ≤ 20 | 7.1 | 7.1 | PASS |
| S5-atmosphere | frame_p99_ms:atmosphere | ≤ 33 | 20.8 | **13.9** | PASS |
| S5-atmosphere | long_task_max_ms | ≤ 100 | 0.0 | 0.0 | PASS |
| S5-atmosphere | memory_max_mb | ≤ 450 | 588.0 | 599.0 | FAIL (Phase D) |
| S6 | frame_p95_ms:instrument | ≤ 16.7 | 7.1 | 7.1 | PASS |
| S6 | frame_p99_ms:instrument | ≤ 33 | 13.9 | 27.8 | PASS |
| S6 | filter_max_ms | ≤ 50 | — | **23.0** | PASS (new row) |
| S6 | sort_max_ms | ≤ 150 | — | **36.1** | PASS (new row) |
| S6 | memory_max_mb | ≤ 450 | 573.0 | 659.0 | FAIL (Phase D) |
| S7 | memory_max_mb | ≤ 450 | 596.0 | 585.0 | FAIL (Phase D) |

The baseline's S6 corpus filtered ~2.1k loaded rows; this phase's S6 loads a deterministic **10k-row synthetic library** and times real input→first-frame interactions (`filter_max_ms`, `sort_max_ms`), so the two S6 frame columns are comparable only in mode, not in list size. No existing budget was weakened; the three new rows are additions. `check.mjs` exits 1 on the memory rows only.

## Per-run detail

### S1 — cold start, 5 runs (`1790862487`–`1790862616`)

| Run | first_paint_ms | ui_ready_ms | startup_step_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- |
| 1 | 896 | 1200 | 1 | 490 |
| 2 | 930 | 1204 | 1 | 491 |
| 3 | 908 | 1226 | 1 | 489 |
| 4 | 884 | 1155 | 1 | 491 |
| 5 | 914 | 1228 | 1 | 487 |
| **p95** | **930.0** | **1228.0** | **1.0** | **491.0** |

`startup_step_max_ms` is the largest adjacent-mark delta across the application-owned startup steps (`logging_ready→config_loaded→cache_ready→services_ready`); 1 ms on every run confirms Phase B's backgrounding still holds at Phase C HEAD. Runtime-owned spans (loader → `logging_ready`, Tauri/WebView2 construction, dev-bundle render) are excluded by definition and documented in `check.mjs`.

### S2 — warm start, 5 runs (`1790862649`–`1790862778`)

| Run | first_paint_ms | ui_ready_ms | startup_step_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- |
| 1 | 904 | 1175 | 1 | 490 |
| 2 | 915 | 1394 | 1 | 499 |
| 3 | 892 | 1182 | 1 | 500 |
| 4 | 899 | 1177 | 1 | 493 |
| 5 | 884 | 1076 | 1 | 489 |
| **p95** | **915.0** | 1394.0 | **1.0** | **500.0** |

### S5-instrument — track scroll churn, 5 runs (`1790861613`–`1790861797`)

| Run | frame_p95_ms | frame_p99_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- |
| 1 | 7.1 | 13.9 | 0 | 568 |
| 2 | 7.1 | 13.9 | 0 | 591 |
| 3 | 7.1 | 13.9 | 0 | 605 |
| 4 | 7.1 | 13.9 | 0 | 603 |
| 5 | 7.1 | 14.0 | 0 | 603 |
| **p95** | **7.1** | **14.0** | **0.0** | **605.0** |

The baseline's 65 ms long task is gone; p99 improved 20.9 → 14.0. The scroll corpus is framed on the real 2140-track list (the synthetic fixture is only loaded by S6).

### S5-atmosphere — track scroll churn, 5 runs (`1790861852`–`1790862023`)

| Run | frame_p95_ms | frame_p99_ms | long_task_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- |
| 1 | 7.1 | 13.8 | 0 | 595 |
| 2 | 7.1 | 13.9 | 0 | 592 |
| 3 | 7.1 | 13.8 | 0 | 599 |
| 4 | 7.1 | 13.8 | 0 | 591 |
| 5 | 7.1 | 13.8 | 0 | 593 |
| **p95** | **7.1** | **13.9** | **0.0** | **599.0** |

Atmosphere overhead stays within the relative budget (p95 7.1 ≤ 20; p99 13.9 ≤ 33; no long tasks).

### S6 — 10k synthetic filter/sort, 5 runs (`1790862073`–`1790862237`)

| Run | frame_p95_ms | frame_p99_ms | filter_max_ms | sort_max_ms | memory_max_mb |
| --- | --- | --- | --- | --- | --- |
| 1 | 7.1 | 20.8 | 20.6 | 32.4 | 610 |
| 2 | 7.1 | 21.0 | 20.5 | 31.6 | 659 |
| 3 | 7.1 | 20.9 | 23.0 | 31.5 | 609 |
| 4 | 7.1 | 20.9 | 19.7 | 36.1 | 632 |
| 5 | 7.1 | 27.8 | 19.5 | 29.7 | 628 |
| **p95** | **7.1** | **27.8** | **23.0** | **36.1** | **659.0** |

Each run loads `makeSyntheticTracks(10000)` through the perf bridge, then times 5 real filter keystrokes and 3 column sorts (input dispatch → next animation frame). The measured filter costs 19.5–23.0 ms and sorts 29.7–36.1 ms across runs; both are inside budget with headroom.

### S7 — play-all from the full library, 5 runs (`1790862288`–`1790862437`)

| Run | play_tracks rows | driver round-trip ms | memory_max_mb |
| --- | --- | --- | --- |
| 1 | 2140 | 4776 | 585 |
| 2 | 2140 | 4364 | 580 |
| 3 | 2140 | 4562 | 585 |
| 4 | 2140 | 4295 | 578 |
| 5 | 2140 | 4825 | 584 |
| **p95** | 2140 | 4825 (informational) | **585.0** |

The driver round-trip includes the favorites fetch and TIDAL round-trip, so it is not the spec's queue-build metric; the queue-build mark remains a Phase D item.

## What produced the deltas

- `4b259bf perf(ui): apply player events in place…` (Task 1) — the 500 ms `position` tick no longer replaces the whole `player.state`, so only `position_ms`/`duration_ms` readers rerun instead of every player reader (backdrop, seek bar, queue rail, mini, now-playing).
- `0b23ef9 perf(ui): precompute track-view filter and sort keys…` (Task 2) — shared `Intl.Collator` plus per-track cached search parts/title keys; `sourceIndex` reads an identity map instead of scanning the source. Mechanism tests pin zero `localeCompare` calls per comparison and zero `indexOf` calls in `sourceIndex`.
- `2bc3b19 perf(ui): update pager rows in place and parallelize ensureAll…` (Task 3) — windows append with `push`, removals/moves splice in place, the dedupe set persists across windows, and `ensureAll` uses the capped parallel window burst above `prefetchMaxTotal` (the 20k-row unit test dropped from ~3.1 s sequential to ~0.26 s parallel). The session cache aliases the live array (mutations publish immediately).
- `fd17319 perf(ui): warm track search caches off the first keystroke` (Task 6 fix) — direct in-app probe measured the **first** filter at 87.8 ms cold vs 6.5–15.8 ms warm (sort 5.8–6.5 ms warm). A chunked `warmTrackSearch` now fills the caches after large lists load; the pre-fix S6 gate measured 102.0/78.6 ms, post-fix 23.0 ms p95.
- `2e0d467 fix(perf): pin the scenario mode after the page leaves about:blank` (Task 6 fix) — harness-only; without it S5-instrument/S6 recorded atmosphere samples and their instrument frame rows were unmeasured.
- `c6c1a35 feat(perf): load a 10k synthetic library…` (Task 4) and `5a5b557 feat(perf): gate S6 filter/sort and the app-owned startup span` (Task 5) — measurement only (synthetic fixture, interaction timings, budget rows); no runtime behavior outside perf sessions.

## Task 6 rulings (recorded in the SDD ledger)

- **Mode pin race.** `drive.mjs` read `get_config` at connect time; WebView2 was still on `about:blank`, the read threw, and the pin/restore pair was skipped. Moved the read into `applyScenarioMode` (post-login, post-`__perf`). Cost if wrong: a scenario still runs unpinned if the post-login read fails, and the gate then reports its frame rows unmeasured rather than silently mixing modes — the failure is visible.
- **First-keystroke warming.** The measured cold first filter (87.8 ms) breached the new 50 ms row while every warm filter met it. `warmTrackSearch` moves the same cache build into chunked macrotasks (500 rows per chunk) after list loads; matcher semantics and rendered order are unchanged, and the warming is only wired for lists ≥ 500 rows. Cost if wrong: a few short background tasks after large list loads.

## Memory and Phase D carryover

Every memory ceiling remains a genuine miss (S1 491 vs 300; S2 500 vs 300; S3–S7 585–659 vs 450) and stays a Phase D target. Also carried into Phase D: `S4.advance_max_gap_ms` (101 vs 100 in Phase B), the S3-miss TTFS attribution reference, the cached-seek defect recorded in the baseline note, the S8 default-cap question, the S9 soak, and the deferred page-open/queue-build marks. Budgets are unchanged.

## Raw run inventory

| Scenario | Runs (run ids) | Quarantine |
| --- | --- | --- |
| S1 | `1790862487`, `1790862519`, `1790862551`, `1790862584`, `1790862616` | — |
| S2 | `1790862649`, `1790862681`, `1790862713`, `1790862746`, `1790862778` | — |
| S5-instrument | `1790861613`, `1790861659`, `1790861705`, `1790861751`, `1790861797` | pre-fix batch (5 runs) + probe runs → `runs-quarantine/phase-c-invalid/` |
| S5-atmosphere | `1790861852`, `1790861894`, `1790861937`, `1790861980`, `1790862023` | — |
| S6 | `1790862073`, `1790862114`, `1790862155`, `1790862196`, `1790862237` | pre-fix batch (5) + validation/probe runs → `runs-quarantine/phase-c-invalid/` |
| S7 | `1790862288`, `1790862326`, `1790862363`, `1790862400`, `1790862437` | — |

Full local suite at the recorded HEAD: `cargo test --workspace` (496 passed, 1 ignored), `cargo clippy --workspace -- -D warnings` clean, `npm --prefix ui test` (379 passed), `npm --prefix ui run check` clean, `node --test scripts/perf/check.test.mjs` (12 passed).

## Post-measurement review fixes (2026-10-01, commits `7701adc` and `8420ae3`)

The whole-branch review found, and this phase fixed, two issues after the runs above:

- `7701adc fix(ui): keep ensureAll behind the pager single-flight` — two concurrent `ensureAll()` walks could burst the same windows from the same offset; playlists run `dedupe: false`, so every row was appended twice (regression test: 1891 → 1000 rows for a 1000-row list). `ensureAll` now drives the shared `loadMore(true)` single-flight, which also restores the `removeItem`-during-walk resync and the header loading flag. The measured S1–S7 numbers are unaffected: S6 uses the synthetic fixture (its `ensureAll` is a no-op) and S5/S7 do not exercise concurrent `ensureAll`.
- `8420ae3 fix(perf): fail the S6 interaction rows when nothing was measured` — a zero-count `interactions` submission used to record a 0 ms metric that passed the new rows; the S6 `filter_max_ms`/`sort_max_ms` rows are now `required` (missing → FAIL), count-0 submissions are ignored, and the driver throws when a measurement is unavailable. The rest of the table keeps the spec's invalid-run posture (an unmeasured non-required row does not fail the gate).

Final suite at the fix-pass HEAD (`8420ae3`): `npm --prefix ui test` 380 passed, `node --test scripts/perf/check.test.mjs` 14 passed, `cargo test --workspace` 496 passed (1 ignored), clippy clean, svelte-check clean.
