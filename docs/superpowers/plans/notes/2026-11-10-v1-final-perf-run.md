# v1.0 final perf run (release gate §4): runbook

- **Status:** ready, not run. The owner says when (overnight, PC not needed).
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

_Not run yet._
