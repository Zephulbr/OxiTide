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

**Corpus 2026-11-04** (`corpus-20261104-025308`, unattended with `scripts/perf/run-corpus.ps1`):
release exe built from `ee5fba8` (`app:build:perf`), Ryzen 7 5700X3D, 144 Hz, WebView2 156.0.4314.58,
owner config (Instrument dark, crossfade 4 s, CatJam on), scrobbling off for the run. 5 runs each
of S1, S2, S3-miss, S3-hit, S4, S5-instrument, S5-atmosphere, S5-atmosphere-light, S6, S7, S11, S13,
S14; S8 and S9 (120 min) once. No timeouts; every playback run has `first_pcm`; zero underruns in
every run. S12 not run (needs the owner to close the window).

| Ruling | Outcome |
|---|---|
| R3 (S4 gap) | **Resolved, no debt:** `advance_max_gap_ms` 42 median / 43 worst (≤ 100). |
| R4 (S6 p99) | Still over: 34.7 median / 62.5 worst. Budget set to **72** (worst + 15 %). |
| R5 | S2 first paint 403 (≤ 1000); S3-hit `cache_hits` 2 (≥ 1); S3-hit TTFS 222, S3-miss 1391 / mean 638: all PASS. |
| R7 | `underrun_count` max 0 (required) added to S4 and S9. |
| R2 (memory) | **Applied to S1–S7** (2026-11-04): S1 735, S2 730, S3-miss 720, S3-hit 720, S4 855, S5-instrument 750, S5-atmosphere 745, S5-atmosphere-light 805, S6 805, S7 850. The ~110–130 MB rise over Track 13 (S1 497 → 609, …) is not the app: the Track 13 UI on today's WebView2 (156, installed 2026-11-01) reads the same start-page memory as today's UI (601–615 vs 584–631 MB). **S8 and S9 held:** their worst runs include the dynamic-colour glide's heap growth (`2026-11-04-v1-perf-regressions-webview.md`); they take R2 from a re-run after that fix. |

The three apparent regressions, pinned (`2026-11-04-v1-perf-regressions-webview.md`):

- **S13** and **S14** were harness artifacts. CPU: Windows process times are tick-sampled, and the
  same S13 runs read 7.8–32.0 % on them but 20.1–20.5 % on exact cycle counts; a cycle A/B shows the
  Track 13 UI costs the same (−1.5 %). Frames and long tasks: start-up and the mode switch before
  `measure_start`. S14 IPC: the same 369 events over a shorter window. All fixed in the harness; a
  re-run (`corpus-20261104-125420`) passes every S13/S14 row except S13 `cpu_webview_pct`, whose
  13.6 budget was set on tick-sampled CPU: S11–S14 CPU budgets need a cycle-based re-baseline.
  **S14 re-baselined** (owner, 2026-11-04; rule unchanged, median + 15 %, ≥ +0.5 pt): WebView2
  16.7 (median 14.5), app 2.0 (1.5). S11 next (unattended); S13 after the 30 Hz meter decision; S12
  from its attended run.
- **S9** is real: the dynamic-colour glide restyles the whole page every frame for 1.2 s per track
  change and Blink's GC heap keeps the space. Every fix changes how the glide looks (owner).
