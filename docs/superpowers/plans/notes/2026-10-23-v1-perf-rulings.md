# v1.0 perf debt: rulings and the release re-baseline (2026-10-23)

- **Owner:** V. **Authority:** `2026-10-17-v1-release-gate.md` §1 (PM-D1 debt, measure the shipped
  artifact) and §4 (final perf run).
- **Inputs:** the seven pre-existing breaches (`2026-10-10-track11-completion.md` § PM-D1), the
  unrecorded 9.1/9.3/9.5 re-runs (`2026-10-13-track9-completion.md`), the S4 ruling
  (`2026-10-02-track3-phase-d.md`), and Track 13's release corpus
  (`2026-10-19-track13-perf-results.md`).

## Rulings

| # | Ruling |
|---|---|
| R1 | **Release is the reference build** for S1–S9, as it already is for S11–S14: `run-scenario.ps1 -Release` against `npm --prefix ui run app:build:perf`. Debug-build numbers (Vite, dev-mode Svelte) are informational from here on. |
| R2 | **Memory ceilings** (`memory_max_mb` on S1–S9) are re-derived from the release corpus: **worst run + 15 %**, rounded up to the next 5 MB. One rule for every row; no per-row negotiation. It replaces the 2026-10-02 ceilings, which were set on the debug build. |
| R3 | **S4 `advance_max_gap_ms`** (105 vs 100, owner-ruled 2026-10-02): re-measure on release (the gap timer moved on every output write in `perf-harness-gaps`, and a natural format-change advance now drains before reopening with the timer restarted after the drain). If it is still over 100, the budget stays at 100 and the row is **accepted debt listed in the 1.0 release notes**. |
| R4 | **S6 `frame_p99_ms:instrument`** (41–56 vs 33): re-measure on release. If it is still over 33, set the S6 p99 budget to **worst run + 15 %** with a note in `budgets.json`'s history here. S6 is heavy 10k-row filter/sort/album work, not scrolling; its p95 (16.7) and the interaction rows stay as they are. |
| R5 | Every other row keeps its value. A release run over budget is a regression to fix. In particular: S2 `first_paint_ms` ≤ 1000 (1130 was debug + Vite), S3-hit `cache_hits` ≥ 1 (0 was the harness bug fixed in `perf-harness-gaps`; confirm). |
| R6 | **9.1 `S5-light` 681 vs 675** and the S1/S2 quiet re-run: closed by R1/R2 (the release corpus decides). **9.3 S5/S6/S7 re-runs:** closed by the release corpus. **Rack S1/S5 re-runs:** superseded by S13 (Rack with meters, budgeted); no Rack variants of S1/S5 exist in the driver. |
| R7 | **`underrun_count`:** after the first release corpus, add `{ "metric": "underrun_count", "max": 0, "required": true }` to S4 and S9 if both read 0; any non-zero reading is investigated before the row lands. |
| R8 | **Linux** stays under the T8-D3 exemption for 1.0. The 2026-10-23 bare-metal S1/S2 (633/618 MB, all descendants summed) are an informational baseline; Linux budgets need their own calibration later. |

## Run list (quiet machine: single app, foreground, VMs/browsers closed)

1. `npm --prefix ui run app:build:perf` (release exe, UI embedded, no Vite).
2. Scripted, five runs each:
   `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Release -Scenario <id> -SkipCheck`
   for `S1`, `S2`, `S3-miss`, `S3-hit`, `S5-instrument`, `S5-atmosphere`, `S5-atmosphere-light`,
   `S6`, `S7`. Re-run S11–S14 as well (S11's CPU budget may tighten now that the frame sampler is off).
3. Human-in-the-loop: `S4` ×5, `S8` ×1, `S9` ×1 (2 h), with real audio. Check the `first_pcm`
   rows before trusting a playback run.
4. `node scripts/perf/check.mjs --metrics` prints every metric's median / worst run; apply R2–R4
   and R7 to `scripts/perf/budgets.json`, then `node scripts/perf/check.mjs` must exit 0 (or fail
   only on the R3 debt row).
5. Record the corpus (run ids), the new ceilings and the R3/R4 outcomes below.

## Results

_Pending the owner's release corpus._
