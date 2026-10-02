# OxiTide — Track 3 Performance & Stability

- **Status:** Done — Phases A-D complete 2026-10-02 (S10 manual smoke + official-client sanity pending owner)
- **Date:** 2026-09-30
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 3, PM-D1, PM-D5);
  `2026-09-21-oxitide-design.md`; `2026-09-21-oxitide-phase1-design.md`
- **Process note:** scope, budgets, harness shape, and approach were brainstormed live
  with the owner (2026-09-30). This document is the standalone source of truth for
  Track 3.

---

## 1. Purpose & Outcome

Track 3 operationalizes the project's core promise: OxiTide should beat the official
client on **responsiveness and reliability**, not just match its features. The parent
designs promise "near-instant search, browse, and navigation; no UI stalls" and a player
that never blocks on buffering — but those promises are qualitative. Track 3 turns them
into measured numbers, fixes what fails them, and leaves a harness that keeps them honest
for every later change.

**Outcome:** on the owner's machine, OxiTide meets fixed budgets for cold start,
time-to-first-sound, UI smoothness, memory, cache/eviction behavior, and large-library
interaction; every known structural offender identified in the survey is fixed; and a
repeatable harness (instrumentation + scripted scenarios + budget gate) exists so that
Track 7, remaining Track 2b polish, and all future work can prove they do not regress it.

**Success =** every budget in §4 passes on the owner's machine, or the owner explicitly
recalibrates it once after the baseline; baseline and final reports are committed; and
`scripts/perf/` can be re-run by a human with no external tooling.

## 2. Scope

**In scope:**

- A measurement harness: Rust milestone instrumentation, a UI frame/long-task sampler
  and hidden HUD, a scripted scenario driver, JSON run reports, and a manual budget gate.
- A recorded baseline of the wave's scenario set (§5.5).
- Fixes across the roadmap's seven areas: cold start, time-to-first-sound, UI jank,
  memory, cache/eviction churn, large-library handling, plus stability verification.
- Re-measurement and budget gating at the end of every phase; one owner-approved
  recalibration of budget numbers after the baseline.
- Documentation updates: baseline/final notes, roadmap status, AGENTS.md perf commands.

**Out of scope:**

- New user-facing features or visual changes. Track 3 is **behavior-preserving by
  default**; any change that visibly alters the UI or an animation must be justified by
  a bug or a measured performance win and discussed with the owner first (T3-D5).
- Hi-Fi Rack mode and its budgets (deferred per the convergence doc; its numbers are set
  when it lands).
- CI automation (the repo has none; the gate is manual), code signing, packaging.
- Audio-pipeline DSP, casting, Atmos (Track 7), and any change to playback semantics.
  Only latency/cost fixes on the existing path are in scope.
- Automating the official client for benchmarking; its comparison is a one-time manual
  sanity check (§4.4).
- Track 4b and Track 6b features themselves — both shipped 2026-09-30, before this spec
  was finalized, so the baseline includes them (T3-D8).

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| T3-D1 | **Phase-gated approach.** The spec locks target budgets now; the harness and baseline come first; every fix phase ends with re-measurement against the same scenarios; a phase is not done until its budgets pass or the owner renegotiates a number. | Evidence for every fix; the roadmap's harness mandate becomes a working gate. |
| T3-D2 | **In-app instrumentation + script harness.** Rust `tracing`/milestone marks and a JSONL sink; a UI long-task/frame sampler and hidden HUD; a PowerShell launcher plus a zero-dependency Node CDP driver; human-in-the-loop protocol where automation is unreliable. | Repeatable on the target machine, no external tooling, fits the no-CI manual workflow. |
| T3-D3 | **Absolute budgets on the owner's machine**, plus a one-time manual side-by-side sanity check against the official client (non-gating). | Deterministic and automatable; the official client cannot be scripted, and baselines drift with feature work. |
| T3-D4 | **Full roadmap wave**, phased: startup, time-to-first-sound, UI smoothness, large-library handling, memory, cache/eviction churn, stability. | Owner choice; matches the roadmap's candidate list. |
| T3-D5 | **Behavior-preserving constraint.** No visible UI or animation changes unless justified by a bug or a measured perf win, and discussed with the owner first. | Avoid churn in a just-stabilized visual track (2b); performance work must not quietly redesign the app. |
| T3-D6 | **Per-mode targets are separate.** Instrument mode is the baseline. Atmosphere gets a relative overhead budget now; exact per-mode budgets (and Hi-Fi Rack's) are fixed when the fixing phases for those surfaces start. | Owner note: modes may legitimately cost different amounts; premature numbers invite arbitrary tuning. |
| T3-D7 | **Reports**: raw per-run JSON in the gitignored `.superpowers/perf/runs/`; baseline and final summaries committed as notes under `docs/superpowers/plans/notes/`; machine-readable thresholds in `scripts/perf/budgets.json`; a `scripts/perf/check.mjs` gate (also exposed as `npm --prefix ui run perf:check`) exits non-zero on breach. | Matches the repo's notes/checklist practice; no CI needed. |
| T3-D8 | **Sequencing**: Track 4b and Track 6b shipped on 2026-09-30, before this spec was finalized, so the baseline measures the complete feature set (remaining Track 2b polish stays open but is gated by these budgets like everything else). Track 3's budgets become the standing PM-D1 gate for all later work. | PM-D5's rationale — optimize a stable feature set — is satisfied; the road is clear to start the wave. |
| T3-D9 | Only **one** budget recalibration round is allowed, immediately after the baseline, and with owner sign-off recorded in the baseline note. | Keeps targets meaningful while acknowledging they are estimates until measured. |

## 4. Goals & budgets

### 4.1 Machine profile

Budgets are absolute for the owner's development machine: AMD Ryzen 7 5700X3D (8C/16T),
32 GB RAM, AMD Radeon RX 9070 XT, Windows 11 Pro 25H2 (build 26200), NVMe storage.
Every report records CPU, RAM, GPU, OS build, app version/git rev, active mode, density,
cache state (cold/warm), and scenario-script version, so numbers from other machines are
never compared silently.

### 4.2 Budgets

All timing budgets are p95 across **≥ 5 measured runs** unless noted. "First paint" =
the UI's first `requestAnimationFrame` after the window is shown; "UI ready" = app
mounted with the auth gate resolved (login screen or home rendered).

| Area | Budget | Conditions |
|---|---|---|
| Cold start | process start → first paint **≤ 1.5 s**; → UI ready **≤ 2.5 s**; no single pre-paint blocking step **> 100 ms** (span-measured) | warm OS cache; cache present |
| Warm start | process start → first paint **≤ 1.0 s** | second launch within 5 min |
| Time-to-first-sound | play → first PCM written to output: cache hit **≤ 300 ms**; network miss **≤ 2.5 s** (median ≤ 1.2 s) | ≥ 100 Mbps; fixed reference tracks at High and Max |
| Gapless advance | last PCM of previous track → first PCM of next: **≤ 100 ms** | prefetched next, same format |
| Scroll smoothness | 2k+ row fling: frame p95 **≤ 16.7 ms**, p99 **≤ 33 ms**, no long task **> 100 ms** | Instrument mode |
| Atmosphere overhead | frame p95 **≤ 20 ms** during the same fling; no mode-specific long task **> 100 ms** | relative to Instrument baseline |
| Filter/sort | filter keystroke → repaint **≤ 50 ms** at 10k loaded rows; full sort of 10k **≤ 150 ms** | Instrument mode; debounce permitted |
| Play-all/queue | queue build for 10k tracks **≤ 250 ms** | loaded list |
| Large page open | 2k+ row first page visible **≤ 500 ms** after data resolves; 500-track album opens **≤ 1 s** | warm API caches |
| Memory | idle after auth **≤ 570 MB** (S1) / **≤ 560 MB** (S2); during playback **≤ 645-755 MB** per scenario (`budgets.json`); 2 h soak drift **≤ +50 MB** (settled baseline: median of the 1-5 min window vs the last 5 min) with app-process-only drift **≤ +25 MB** | working set = app process + its WebView2 child processes |
| Cache/eviction | repeat loop of 10 Max tracks (~2 GB) with the default 2 GiB cap: second pass **≥ 90 % hits**, no track re-downloaded twice within a loop; eviction commit cost stays O(evicted), no full-directory rescan on normal play | local NVMe |
| Stability | zero main-thread tasks **> 200 ms** outside startup; no crash or unhandled rejection in the 2 h soak; cache files never corrupted (atomic writes verified) | soak includes skip/quality-switch churn |

Notes:

- Network-bound numbers (TTFS miss, page opens) are only comparable at the recorded
  network class; a run that cannot reach the API is marked invalid, not failed.
- The cache-loop budget intentionally exercises the plan4-smoke observation that the
  default 2 GiB cap holds only a few Max tracks. If the outcome cannot be met with LRU
  alone, the fix may include protecting the current and next track and/or documenting a
  recommended cap, but not silently raising defaults beyond what the owner approves.
- The 2 h soak drift is anchored at a settled baseline (median of the samples 1-5 min
  after session start vs the median of the last 5 min) so WebView2's process-start ramp
  is not counted as leak; the app-process-only row (`memory_drift_app_mb`, ≤ +25 MB) is
  the leak detector for app-owned memory. Owner-approved metric-semantics ruling,
  2026-10-02, recorded in the final note.
- Owner-approved memory renegotiation (2026-10-02, `plans/notes/2026-10-02-track3-phase-d.md`):
  the per-scenario peak ceilings were raised to the measured WebView2-inclusive p95 + ~15%
  (S1 570, S2 560, S3/S4 645, S5-atmosphere 675, S5-instrument 690, S6 740, S7 670,
  S8 660, S9 755) because the app-owned share is only 52-91 MB and the WebView2 floor
  alone exceeds the old ceilings; `S4.advance_max_gap_ms` stays at 100 with the
  WASAPI-ring finding documented.
- All budgets are the source of truth; `scripts/perf/budgets.json` mirrors them for the
  gate and must be updated in the same commit as any owner-approved change.

### 4.3 Recalibration

After the baseline (Phase A), the owner may adjust budget numbers **once** (T3-D9).
Recalibration is recorded in the baseline note with old value, new value, and reason. No
further recalibration; later phases either meet budgets or are explicitly renegotiated
in writing.

### 4.4 Official-client sanity check

Once, after the final phase, the owner plays identical scenarios on the official TIDAL
desktop client and records a short qualitative note (cold start feel, TTFS, scroll
smoothness, memory in Task Manager) in the final note. This is context, not a gate; the
absolute budgets remain the contract.

## 5. Harness architecture

### 5.1 Rust instrumentation (`crates/oxitide-app/src/perf.rs`)

- `perf::start()` at the top of `main()` captures an `Instant` and the true process creation
  time via `GetProcessTimes` (the `windows` crate is already a dependency), exposing a
  process→mark span that includes loader time.
- Milestone marks (monotonic timestamps + thread id), emitted into a lock-free ring or
  short channel:
  `config_loaded`, `cache_ready`, `services_ready`, `window_shown`, `auth_restored`,
  `player_built`; engine marks `track_resolved`, `manifest_parsed`, `decode_started`,
  `first_pcm`, `cache_hit`, `cache_miss`, `eviction`, `advance_gap`; UI marks
  `first_paint`, `ui_ready`, and scenario markers.
- A `PerfSession` collects marks and, when a scenario is active (launch flag or perf
  command), writes JSONL to `.superpowers/perf/runs/<run-id>.jsonl`. Normal operation
  records only cheap atomics/timestamps — no file I/O, no logging.
- A `get_perf_snapshot` command feeds the HUD (frame stats, last TTFS, working set) and
  a `perf://mark` event lets the UI contribute marks without polling.
- Aggregation (deltas, percentiles) is pure and unit-tested; comparison against
  `budgets.json` lives in the report script, not the app.

### 5.2 UI sampler & HUD (`ui/src/lib/perf.svelte.ts`)

- `PerformanceObserver` for `longtask` entries (Chromium/WebView2 supports it) counts
  and records long tasks; an rAF sampler records frame times into a fixed-size ring.
- Sampling and the hidden HUD are **only active during a perf session**; the HUD toggles
  with a keyboard shortcut or dev flag and is absent from normal use (no visual change).
- The HUD shows: frame p50/p95 over the window, long-task count/max, working set, last
  TTFS, and the last milestone deltas.
- Percentile math and HUD formatting are pure helpers for vitest.

### 5.3 Scenario driver (`scripts/perf/`)

- `run-scenario.ps1` launches the app with a scenario id and perf flag, sets the
  WebView2 remote-debugging port (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`), waits for
  completion or timeout, then writes a JSON report per run.
- `drive.mjs` (Node, zero dependencies; Node 20+ and the WebView2 runtime are existing prerequisites) connects
  over CDP to click, scroll, type, and navigate for the scriptable scenarios.
- Scenarios that need a real account, network audio, or a human ear (TTFS, gapless, cache
  loop, soak) use the documented human-in-the-loop protocol: the app does the measuring,
  the owner performs the actions, and the session is written to JSONL like any other run.
  This matches the repo's existing GUI-smoke rules.
- `scripts/perf/check.mjs` (exposed as `npm --prefix ui run perf:check`) reads the
  collected reports, compares each metric to `budgets.json`, and exits non-zero with a
  per-budget table. It is invoked manually at each phase gate.

### 5.4 Report format

Each run report contains: run id/timestamp, machine profile, git rev, app version, mode,
density, cache state, scenario id, raw metric set, and pass/fail/unmeasured per budget.
A `summary` section aggregates the ≥ 5 runs; the summary is what gets committed to the
phase note.

### 5.5 Scenario set

| # | Scenario | Automation |
|---|---|---|
| S1 | Cold start (single-instance path, warm OS cache) | scripted |
| S2 | Warm start (relaunch within 5 min) | scripted |
| S3 | TTFS, cache miss then hit, High and Max | human-in-the-loop |
| S4 | Queue advance/gapless, album zip | human-in-the-loop |
| S5 | 2k+ row fling, Instrument and Atmosphere | scripted |
| S6 | Filter/sort 10k loaded rows | scripted (synthetic large list) |
| S7 | Play-all/queue build 10k | scripted |
| S8 | Cache loop: 10 Max tracks × 2 passes | human-in-the-loop |
| S9 | 2 h soak with skip/quality-switch churn, memory drift | human-in-the-loop |
| S10 | Manual stability: device hot-plug, cache-clear-on-exit integrity | manual checklist |

Human-in-the-loop timings are always measured app-side from the moment the play command
is received (or the milestone of interest), so human reaction time never enters a number.

### 5.6 Harness overhead (PM-D1)

Milestones are timestamps and counters only; the JSONL sink is off outside perf
sessions; the HUD and samplers are off outside perf sessions. Expected normal-operation
overhead is below measurement noise and is itself verified by the baseline runs.

## 6. Fix inventory & phasing

Track 3 spans four implementation plans, named by phase following the recent
track-named convention and dated when written: `<date>-track3-harness.md`,
`<date>-track3-startup-ttfs.md`, `<date>-track3-ui-library.md`,
`<date>-track3-memory-cache.md`. Both prerequisite tracks (4b and 6b) shipped
2026-09-30, so the plan cycle can start immediately (T3-D8). Line references below are
from the 2026-09-30 survey and may drift; treat them as pointers, not contracts.

### Phase A — Harness & baseline (no fixes)

Deliver `perf.rs`, the UI sampler/HUD, `scripts/perf/`, `budgets.json`, baseline runs of
S1–S9, the committed baseline note, and the one recalibration round.

### Phase B — Startup & time-to-first-sound

Known offenders to fix (files as surveyed 2026-09-30):

- Pre-paint blocking: `build_cache()` + `DiskCache::cleanup` full-directory scan
  (`main.rs:133`, `player.rs:16-31`, `cache.rs:187-209, 242-265`); synchronous
  `palettes.json`/`lyrics.json` loads (`artwork.rs:66-81`, `lyrics.rs:76-91`); keyring
  reads (`scrobble/mod.rs:139-140`); `reg query` + `register_all()`
  (`scheme.rs:109-126`, `main.rs:179-181`); duplicate `Config::load` on login
  (`commands.rs:169, 296`).
- Target shape: gate `show()` only on geometry-critical config; run cache cleanup,
  artwork/lyrics cache loads, keyring, and scheme work backgrounded; initialize services
  lazily or in parallel on first use.
- TTFS: replace the 200 ms `wait_for_info` poll (`engine.rs:846-868`) with an
  event-driven wait; review the resolve → manifest → fetch → output-open path
  (`player.rs:51-68`, `engine.rs:654-695`, `wasapi_output.rs:129-164`) for warm-up and
  pipelining wins, measured before/after.
- Re-run S1–S4; re-run the affected Plan 2/3 audio smoke checklists.

### Phase C — UI smoothness & large-library handling

Known offenders to fix:

- Whole-`player.state` replacement every 500 ms invalidating all readers
  (`ui/src/lib/player.svelte.ts:31-46`).
- Undebounced filter + `localeCompare` sort over the full list and O(n) `sourceIndex`
  (`ui/src/lib/trackView.svelte.ts:84-88, 162-166`); debounce and precomputed sort
  keys/index maps preserve the rendered result. If debouncing perceptibly changes typing
  feel, discuss with the owner first (T3-D5).
- O(n) array copies per pager window and in `removeItem`/`moveItem`
  (`ui/src/lib/pager.svelte.ts:119-125, 227-234`).
- Sequential `ensureAll()` walks for sort/filter/play-all; parallelize with a
  concurrency cap (the `favorite_ids` precedent) and keep first paint progressive.
- Non-virtualized lists (album grid, playlist/search result lists, lyrics lines) only if
  measurements breach budgets; virtualization must not change visible layout.
- Re-run S5–S7 including the Atmosphere-overhead budget.

### Phase D — Memory, cache/eviction, stability

Known offenders to fix:

- Unbounded palette `HashMap` + O(n) full-file rewrite with no cap
  (`artwork.rs:66-81, 131-138`); add an LRU/size bound consistent with `lyrics.rs`'s
  `CACHE_MAX_ENTRIES`.
- Startup scan + eviction path: add incremental on-disk size bookkeeping so normal play
  never rescans the directory; keep eviction O(evicted).
- Cache churn under the default 2 GiB cap (`config.rs:175`, plan4 smoke note): protect
  current/next track and test the S8 loop budget; any default-cap change is an owner
  decision.
- Stability: S9 soak (crash, unhandled rejection, drift) and S10 manual checks; fix what
  is found without behavior changes.
- Final S1–S10 run, committed final note, roadmap status update, AGENTS.md perf commands.

## 7. Verification & definition of done

Per phase:

- `cargo test --workspace` and `cargo clippy --workspace -- -D warnings`.
- `npm --prefix ui run check` and `npm --prefix ui run test`.
- Harness re-run of the phase's scenario subset; the budget check
  (`scripts/perf/check.mjs`) passes (or the single owner-approved recalibration for
  Phase A).
- Phase summary committed under `docs/superpowers/plans/notes/` with raw numbers.
- Audio-path phases (B, D) re-run the Plan 2/3 smoke checklists.

Wave definition of done:

- All S1–S10 budgets pass or are explicitly renegotiated in the final note.
- Baseline, phase summaries, and final report (including the official-client sanity
  note) committed.
- Roadmap Track 3 status updated; AGENTS.md documents `scripts/perf/` usage so the gate
  is discoverable for Track 7, Track 2b polish, and everything after.
- No visible UI/animation change shipped without owner discussion (T3-D5).

## 8. Risks & open verifications

- **CDP automation may not reliably drive WebView2** (pricey selectors, scroll physics).
  Fallback is the human-in-the-loop protocol per scenario; the harness must never block
  on automation.
- **Measurement noise** (background load, thermals, network): ≥ 5 runs, p95, recorded
  environment, invalid-run marking; results are compared only within the same scenario
  version.
- **Behavior-preserving constraint may cap available wins** on some surfaces (for
  example, expensive blur/animation composites). If a budget cannot be met without a
  visible change, that becomes an explicit owner discussion, not a silent redesign.
- **Per-mode budgets are only partially defined now** (T3-D6); Atmosphere/Hi-Fi numbers
  need an owner discussion when their fixing phase starts.
- **Audio-path changes risk playback regressions**; the Plan 2/3 smoke checklists are
  mandatory for Phases B and D.
- **Feature additions after the baseline** (Track 7, future bugfixes) must re-run the
  gate per PM-D1; the budgets file and report tooling are the contract.
- **The 2 GiB default cache cap versus Max-quality track sizes** may make the S8 budget
  unreachable without an owner decision; flagged early in Phase D.

## 9. Future / explicitly deferred

- Exact per-preset budgets for Atmosphere and Hi-Fi Rack, and any GPU/backdrop budget
  beyond the current relative overhead rule.
- CI-style automation of the harness and budget gate (no CI today).
- Power/thermal profiling, GPU timeline capture (WPA/ETW) as an escalation tool when a
  budget breach needs deep analysis — documented, not built.
- Track 7 (DSP/EQ, casting, Atmos) budgets, which build on this harness.
