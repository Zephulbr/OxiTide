# Track 9.0 carry-in triage (2026-10-06)

- **Status:** Triage complete; queued owner runs pending
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §6.0 and T9-D9
- **Scope:** classify every open UI smoke checklist (run / retire / leave), queue the
  Track 9 slice smokes, and record the scripted-gate state. No code changes.

## Scripted state on merged `main` (`9c753d7`)

- `cargo test --workspace` — 329 + 194 (1 ignored) + 1 + 183 passed, 0 failed.
- `cargo clippy --workspace -- -D warnings` — clean.
- `npm --prefix ui run check` — 0 errors, 0 warnings.
- `npm --prefix ui test` — 62 files / 578 tests passed.
- Track 10 owner smokes are **closed** (`c132c3a`, `0aeb5b9`) and are not in scope here.

## Triage table

"Open" counts unchecked boxes at triage time. "When" schedules re-runs so each surface
is verified after the slice that last touches it (owner may override).

| File | Open | Class | When | Reason / superseded-by |
|---|---|---|---|---|
| `2026-10-03-artist-page-smoke.md` | 22 | re-run | **now** | Queued Track 9 slice; 9.3 closes its leftovers. |
| `2026-10-03-home-rhythm-smoke.md` | 12 | re-run | **now** + after 9.3 | Queued slice; 9.3 re-checks the cards (gray-out/hover/Play all). |
| `2026-10-05-track9-1-light-mode-smoke.md` | 15 | re-run | **now** (UI only) | Active 9.1 smoke; scripted perf rows already recorded there. |
| `plan4-smoke.md` | 2 | re-run | **now** | Two cheap human re-checks still current (Storage readout, High-AAC). |
| `2026-10-03-detail-header-overflow-smoke.md` | 15 | re-run | after 9.2 | 9.2 adds album facts/volume grouping to the same headers. |
| `2026-09-28-settings-tabs-smoke.md` | 11 | re-run (scoped) | after 9.3 | Drop "Connections: four Planned rows" (all shipped); add Content tab and the 9.1 Theme control. |
| `2026-09-28-sidebar-collection-smoke.md` | 21 | re-run (scoped) | after 9.3 | Nav order changed (History/Settings); Purchases changes in 9.3. |
| `2026-09-28-track-row-rework-smoke.md` | 12 | re-run | after 9.4 | Lists are a 9.4 a11y surface. |
| `2026-09-29-search-suggestions-smoke.md` | 23 | re-run (scoped) | after 9.4 | Explore combobox reworked in 9.4; re-check the dark-only rows in Light too. |
| `2026-09-29-floating-player-bar-smoke.md` | 26 | re-run (scoped) | after 9.4 | Retire the "Sidebar header popover" section (gear popover removed); glass/layering rows current. |
| `2026-09-28-sidebar-scale-playing-marker-smoke.md` | 17 | re-run (scoped) | after 9.4 | Retire the Footer/scroll-fix section (no pinned footer exists); marker rows current. |
| `2026-10-01-track3-phase-b-smoke.md` | 11 | retire | — | Track 3 closed 2026-10-02; Phase D report + S10 owner smoke supersede. |
| `2026-10-01-track3-phase-c-smoke.md` | 12 | retire | — | Same closure; Phase D owns the budget table; synthetic corpus quarantined. |
| `2026-09-30-track4b-discovery-smoke.md` | 11 | retire | — | Owner PASS recorded in its own Results; surfaces since reworked (artist/home). |
| `plan7-smoke.md` | 15 | retire | — | Superseded by closed playlist hotfix/list-interaction smokes + Plan 13 rework. |
| `plan9-smoke.md` | 11 | retire | — | Row expectations stale (no grip, whole-row drag, 46/38px); covered by later row smoke. |
| `plan10-smoke.md` | 15 | retire | — | 2b-era tokens/queue/sidebar expectations predate Plans 11–13 and the 9.1 token layer. |
| `plan11-smoke.md` | 18 | retire | — | Palette/mode checklist predates room-parameterized palettes and light mode. |
| `2026-09-28-track-row-and-shuffle-smoke.md` | 20 | retire | — | Earlier 09-28 row iteration; superseded by track-row-rework. |
| `2026-10-02-detail-headers-smoke.md` | 23 | retire | — | Owner PASS recorded 2026-10-03; live header check is the overflow note. |
| `2026-09-28-player-bar-polish-smoke.md` | 32 | retire | — | Superseded by floating-bar rework + closed Now Playing polish pass. |
| `2026-09-29-lyrics-smoke.md` | 41 | retire | — | Karaoke/layout closed by the polish smoke; 9.2 moves Now Playing verification to the Info tab. |
| `2026-09-28-seamless-login-smoke.md` | 1 | leave | — | Single open row is a non-UI security audit; redaction covered by tests. |
| `2026-10-01-hygiene-smoke.md` | 11 | leave | — | CSP/legal/docs surface, non-UI; packaged row overlaps Track 8. |
| `2026-09-29-smtc-smoke.md` | 14 | leave | — | Windows shell integration, not a Track 9 UI surface. |
| `oneko-smoke.md` | 15 | leave | — | oneko extras are an explicit Track 9 non-goal. |
| `2026-10-03-track8-packaging-smoke.md` | 6 | leave | — | Track 8 Linux resume list. |
| `2026-10-03-track8-wslg-smoke.md` | 8 | leave | — | Track 8 resume list. |
| `2026-10-03-track8-bare-metal-checklist.md` | 10 | leave | — | Track 8 exempted bare-metal checks. |
| `2026-10-03-track8-1-keyring-smoke.md` | 10 | leave | — | Track 8.1 WSLg/keyring resume list. |
| `2026-10-03-track8-2-alsa-smoke.md` | 13 | leave | — | Track 8.2 Linux audio resume list. |
| `2026-10-03-track8-4-linux-desktop-smoke.md` | 23 | leave | — | Track 8.4 MPRIS/deep-link/Wayland resume list. |

Totals: 11 re-run, 11 retire, 10 leave (32 files with open boxes; a `track8-3-mpris`
file does not exist — MPRIS checks live in the Track 8.4/WSLg/bare-metal notes).

## Queued owner runs (now)

1. `2026-10-03-artist-page-smoke.md` — **skip the stale "Show all" main-list item**:
   Top tracks scrolls in place (20 fetched); use the catalog follow-up section's
   "View all" behavior instead. Everything else runs as written.
2. `2026-10-03-home-rhythm-smoke.md` — live tuning + GUI smoke + perf re-check. Use the
   current budgets in `scripts/perf/budgets.json` (S1 570 / S2 560 MB after the
   2026-10-02 renegotiation, not this note's historical context). Card rows will be
   re-checked after 9.3.
3. `2026-10-05-track9-1-light-mode-smoke.md` — UI sections only; the scripted perf
   section is already recorded. The glare item is an accept/deny verdict, not pass/fail.
4. `plan4-smoke.md` — the two remaining human re-checks.

## Deferred perf adjudication (carry-over from 9.1)

The home-rhythm perf re-check (S5, S1×3, S2×3, `node scripts/perf/check.mjs`) and the
9.1 S1/S2 + S5-light `memory_max_mb` rows (681 vs 675) need a quiet machine (single app,
foreground, Vite warm). On 2026-10-06 the machine was busy (owner apps); first-pass
attribution says environmental (startup spans 6–7 ms, app-owned memory 62–65 MB within
the Phase D range, all-process growth in `msedgewebview2` 154.0.4258.53). Run when quiet:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere-light -Runs 2
node scripts/perf/check.mjs
```

Record results in `2026-10-05-track9-1-light-mode-smoke.md` and the home-rhythm note.

## Defect protocol

Any defect found by these runs is triaged into 9.2 (credits/album info), 9.3 (polish),
9.4 (a11y), or recorded as a follow-up. Expanding 9.2/9.3 scope requires owner sign-off
(T9-D9). Retired checklists need no further action; their reasons live in the table above.
