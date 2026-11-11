# v1.0 final perf run (release gate §4): runbook

- **Status:** **PASS 2026-11-11** (result below; rulings in `2026-10-23-v1-perf-rulings.md`).
- **Authority:** `2026-10-17-v1-release-gate.md` §4; rulings and every budget's history in
  `2026-10-23-v1-perf-rulings.md`; harness guide `docs/agents/perf.md`.
- **Budgets:** `scripts/perf/budgets.json` as of `413462a`. Every S1–S9 and S11–S14 row is set
  from release runs (S1–S7 on 11-04, S11/S13/S14 cycle CPU on 11-04, S12 on 11-06, S8/S9 on 11-10).
  The final run does **not** re-derive any budget: it checks the code that ships against them.

## What it is

One unattended release corpus with every scripted scenario, against the release exe built from
the commit that will be tagged (or as close as possible: re-run if audio, output, UI animation or
WebView-facing code changes afterwards). It passes when `node scripts/perf/check.mjs` exits 0.

Default list and runs (`scripts/perf/run-corpus.ps1`): S1, S2, S5-instrument, S5-atmosphere,
S5-atmosphere-light, S6, S7, S14, S13, S3-miss, S3-hit, S11, S12, S4 (5 runs each), then S8 and
S9 (once each; S9 is the 2 h soak). Expected wall time **about 5–6 h**.

## Before starting

1. **Quiet machine:** close the installed OxiTide (`%LOCALAPPDATA%\OxiTide\oxitide-app.exe`,
   tray included), VMs (qemu), browsers, other agents' builds/tests, games. Plug in, keep the
   monitor connected (the script keeps the display on; an off or occluded display throttles
   WebView frames).
2. **Owner config as it normally is:** logged in, close to tray **on** (S12), real audio output
   connected, favourites as usual. The script turns scrobbling off and restores `config.toml`,
   `session.json` and `history.jsonl` at the end by itself.
3. **Tree:** `git status` clean at the commit under test; note its hash.
4. **Build the release exe** (7–9 min):

   ```powershell
   npm --prefix ui run app:build:perf
   ```

   Check `target\release\oxitide-app.exe`'s timestamp is newer than the last commit. The corpus
   log's first line records the commit and the exe time; a stale exe is the usual mistake.

## Run

From the repo root, in a terminal that stays open:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-corpus.ps1
```

An agent whose background commands stop at 2 h must start it detached instead (the corpus is
longer than that), then wait on `corpus.log`:

```powershell
$root = (Get-Location).Path
Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{
  CommandLine = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$root\scripts\perf\run-corpus.ps1`""
  CurrentDirectory = $root
}
```

Progress: `.superpowers\perf\corpus-<stamp>\corpus.log` (one line per scenario start/end).
Do not touch the PC while it runs; S5/S6/S13/S14 measure frames, and S8/S9/S12 play audio.

If it is interrupted: kill only this run's `oxitide-app.exe` / `node` / `powershell` processes,
then copy the three files from `corpus-<stamp>\state-backup\` back to
`%APPDATA%\OxiTide\config\` (scrobbling is off until you do).

## After

1. Read `corpus-<stamp>\check.txt` (the budget check) and `metrics.txt` (`check.mjs --metrics`).
   Re-run the check by hand with `node scripts/perf/check.mjs`.
2. Sanity rows before trusting the numbers: every playback run (S3, S4, S8, S9, S12) has
   `first_pcm`; `underrun_count` 0 everywhere; no scenario timed out (`corpus.log`); S8/S9 logged
   `look {mode: atmosphere, …}`.
3. Confirm the state came back: `config.toml` has scrobbling as before, the History page shows no
   perf plays.
4. **Exit 0:** record the result under "Result" below (corpus id, commit, WebView2 version,
   one line per scenario family) and tick §4 in the release gate.
5. **A breach:** it is a regression to fix (R5), not a budget to raise. Re-run the one scenario
   first (`run-corpus.ps1 -Scenarios S6 -Runs 5`) to rule out a noisy run, then bisect against
   the last passing corpus (`corpus-20261110-032608` for S8/S9, `corpus-20261106-090933` for
   S12, `corpus-20261104-*` for the rest). Known swing: S4/S12 gaps follow whatever the
   favourites are (rulings note, "S12 advance gap").

## Result

**PASS 2026-11-11** after two re-runs and three owner rulings.

- **Corpus** `corpus-20261111-043215`: release exe of `a332ed2` (built 04:32), WebView2
  157.0.4341.42, Ryzen 7 5700X3D, owner config (Atmosphere dark, DX5, crossfade 4 s `blend`,
  close to tray on). 5:52 h, no timeouts, 0 warning lines, `first_pcm` in every playback run, zero
  underruns everywhere, state restored. Machine quieted first (two Android emulators were using
  ~12 cores; stopped before the build).
- **First check: 71 PASS, 4 FAIL.** All four are resolved:
  - **S9 `memory_drift_mb` +106** (app +1): noise. Re-run (`corpus-20261111-101202`): **+23**
    (app +6), PASS.
  - **S12 `ipc_idle_events` 9** (3 per track change, every run): this week's favourites change
    format at all three S12 transitions, and a crossfade across a format change fades the old
    tail out for the whole 4 s and then reopens (`engine.rs`,
    `crossfade_across_a_format_change_fades_out_instead`), so `now_playing` lands ~4 s before
    `first_pcm`, outside the checker's 2 s lead. Not a leak: nothing arrives between changes, and
    a re-run with crossfade style `auto` (`corpus-20261111-122647`) read the same 9. **Checker
    fix:** a track change opens at its `info_ready` (cap 15 s). Both corpora then read 0; the
    11-06 beat-clock corpus still fails with 32.
  - **S13 `memory_max_mb` 749 / `ipc_kb_per_min` 660**: an A/B on today's WebView2 with the
    11-04 UI (`06a9c8e` exe, `corpus-20261111-144938`) reads 745 worst / 641 KB/min against
    today's 722–749 / 660, with the app process lighter today (65 vs 80 MB). The rise is WebView2
    156 → 157 and the track S13 plays (first favourite now `31848667`, was `520770334`: level
    frames are JSON floats, so bytes follow the music), not the UI. **Re-baselined** (owner):
    memory 865 (worst 749 + 15 %), IPC 760 KB/min (median 660 + 15 %).
- **Final check** with the fixed checker and new S13 budgets: every row of the main corpus
  passes except S9's drift, which its re-run passes.

| Family | Result |
|---|---|
| S1/S2 start-up | PASS (first paint 580 / 476 ms, UI ready 1108 ms) |
| S3 TTFS | PASS (miss 444 ms, hit 191 ms) |
| S4 gapless | PASS (max gap 43 ms, 0 underruns) |
| S5/S6/S7/S14 UI | PASS (S6 p99 62.4 ≤ 72, queue build 177 ms) |
| S8 cache loop | PASS (hit rate 1.0, memory 718) |
| S9 soak | PASS on re-run (drift +23, memory 679) |
| S11/S12 idle | PASS (S12 idle events 0 with the checker fix) |
| S13 Rack | PASS on the re-baselined memory/IPC rows; CPU and frames within budget |

Open follow-up (not a gate item): pin S4/S12/S13 to fixed tracks so the favourites stop moving
their gaps and IPC bytes.
