# Track 9.1 light mode — owner smoke checklist

- **Status:** Scripted S5-light re-run under the true light pin recorded 2026-10-06 (all gate rows pass except a deferred `memory_max_mb` miss — see below); S1/S2 quiet re-run **BLOCKED** (machine not quiet — see below); owner smoke pending (Windows, real account)
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §6.1

## Scripted perf results (2026-10-06)

- **Build/conditions:** branch `track9-light-mode`; debug binary rebuilt 2026-10-06 00:32 via `cargo build -p oxitide-app`; Vite dev server on 1420 warm; one OxiTide instance at a time, foreground. The machine was **not quiet** (chrome/zen/OBS/official TIDAL active, total CPU sampled at 21–30%): the S5-light re-run went ahead (no quiet requirement), the S1/S2 budget re-run was blocked (see below).
- **Fix carried by these runs:** `__perf.setTheme` bridge in `App.svelte` drives `appearance.setTheme` (same path as `setMode`); `drive.mjs` prefers that bridge and **asserts** `document.documentElement.dataset.theme === "light"` before the S5 measurement loop, failing the run otherwise; the original theme is restored through the same path. Probe on the running app: backend/dataset/mirror all `light` after the bridge call and all back to `dark` after restore.
- **Commands:**
  - `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere-light -Runs 2` (runner also checks)
  - `node scripts/perf/check.mjs --scenario S5-atmosphere-light`
  - S1/S2 quiet re-run (`-Scenario S1 -Runs 3`, `-Scenario S2 -Runs 3`, `node scripts/perf/check.mjs`): **not run — blocked, machine not quiet**
- **S5-atmosphere-light (true light pin; run ids `1791237174`, `1791237221`):** final-review Finding 3 mirrored the sibling `S5-atmosphere` rows (`long_task_max_ms`, `page_open_ms` required, `memory_max_mb`) into this scenario's budget, so the pre-existing runs were gated on all five rows (`node scripts/perf/check.mjs --scenario S5-atmosphere-light`):

| Scenario | Metric | Value | Budget | Result |
| --- | --- | --- | --- | --- |
| S5-atmosphere-light | frame_p95_ms:atmosphere | 7.3 | ≤ 20 | PASS |
| S5-atmosphere-light | frame_p99_ms:atmosphere | 20.8 | ≤ 33 | PASS |
| S5-atmosphere-light | long_task_max_ms | 0.0 | ≤ 100 | PASS |
| S5-atmosphere-light | page_open_ms | 3.8 | ≤ 500 | PASS (required) |
| S5-atmosphere-light | memory_max_mb | 681.0 | ≤ 675 | FAIL (deferred — see S1/S2 adjudication) |

  - Frames, long task, and page open all pass. The sole miss is `memory_max_mb` (aggregate p95 681 vs 675; run max 667 / 681) — the same whole-WebView2 growth signature (app-owned 62–65 MB, children ~60–100 MB, WebView2 154.0.4258.53) already recorded for S1/S2, now pending the same quiet-machine adjudication. Not claimed as a Track 9.1 regression, and the 675 budget is **not** changed without quiet-machine evidence.
  - Evidence the runs rendered light: `drive.mjs` proceeds to the S5 loop only after the `dataset.theme === "light"` assertion resolves; both driver runs exited 0 with no pin/restore errors. Raw JSONL: `mode=atmosphere`, 5000-frame final samples p95 7.1 / 7.2. `config.toml` ends `theme = "dark"` (owner restored).
  - The two earlier first-pass runs (`1791236003`, `1791236052`) used the raw `set_theme` invoke and measured **dark** rendering; they were moved to `.superpowers/perf/runs-quarantine/2026-10-06-task9-dark-pin/` and their PASS is withdrawn.
- **S1/S2 first pass (2026-10-06, non-quiet) — unadjudicated; quiet re-run blocked.** S5-atmosphere-light's `memory_max_mb` miss (681/675) joins this same deferred environmental adjudication. Gate rows over the accumulated runs dir: S1 memory 593/570 FAIL (per-run 586/593/589), S2 first_paint 1108/1000 FAIL (per-run 1108/962/998), S2 memory 597/560 FAIL (per-run 595/592/597); S1 first_paint 1113/1500 (per-run 984/1016/969), S1 ui_ready 1331/2500 (per-run 1230/1257/1198), long_task and startup-step rows PASS. Attribution evidence (raw JSONL, all six runs):
  - app-side pre-paint spans `logging_ready → config_loaded → cache_ready → services_ready` max **6–7 ms** (budget 100) — **no Track 9.1 code signature in the startup steps**; the first_paint misses are not a new app-side step.
  - app-owned memory (`memory_process`, `self: true`) **62–65 MB**, inside the Phase D measured app-owned range (52–91 MB); the whole ~60–100 MB growth sits in the `msedgewebview2` children.
  - uniform across runs and scenarios (S1 586–593, S2 592–597, S5-light 640–681 vs Phase D: S1 492–518, S5-atmosphere 584), tracking the WebView2 runtime update to 154.0.4258.53 on 2026-10-03 (after the Phase D corpus) — i.e. a pre-existing/environmental signature, **not proven** as a Track 9.1 code regression, but not adjudicable without a quiet run.
- **Blocker (S1/S2):** the machine cannot produce a quiet run right now — chrome ~0.6 core, zen ~0.5, official TIDAL ~0.9, OBS active; total CPU 21–30% over repeated samples. Numbers recorded under this load would be ambiguous, so the quiet re-run and the final `node scripts/perf/check.mjs` adjudication are pending. Unblock: quiet those apps and re-run the two commands above.
- The full-corpus `node scripts/perf/check.mjs` also reports stale-corpus failures unrelated to this change: S3-hit ttfs 308 (2026-10-01 runs), S4 gap 103 (documented debt), S2 first_paint 1052 (2026-09-30 run, above the current 1000 budget), and `n/a` required interaction rows on the 2026-09-30 S5/S6/S7/S9 runs, which predate those metrics.

## Theme switching
- [ ] Settings → Appearance shows Dark / Light / System; each selection applies immediately.
- [ ] Light + Atmosphere: warm paper room, translucent surfaces, glass header/player, cover-art backdrop, no washed-out text.
- [ ] Light + Instrument: solid light surfaces, flat edges, no blur, no grain.
- [ ] Dark + both modes look unchanged from before this plan.
- [ ] System follows a Windows theme flip while the app is running (toggle Windows app mode and watch OxiTide follow).
- [ ] Restart in each setting; no dark/light flash before first paint.
- [ ] Hand-corrupt `localStorage["oxitide.theme"]` via devtools and restart: clean dark boot, no errors.

## Dynamic color
- [ ] Light + Atmosphere, dynamic color Default: nav/link/focus/playing-row colors follow the cover and stay readable.
- [ ] Switch a bright/warm cover and a dark/blue cover; no pale unreadable accents.
- [ ] Fully monochrome / near-black cover in Light mode: Info and playing text plus interactive accents remain readable (pins the accepted composite-ceiling caveat — the light plain-neutral primary is not contrast-fitted).
- [ ] Toggle theme mid-playback with dynamic color on: colors re-analyze once, no stale dark-room palette.
- [ ] Dynamic color Off in light theme: static light-safe teal accent (not dark-mode teal).
- [ ] Menus/overlays over bright art remain legible in light mode.

## Glare (spec §6.1)
- [ ] Light + Atmosphere, hover the Now Playing sleeve: confirm the white `screen` sheen (reduced alpha in light mode) reads acceptably over bright art; note if a dark/inverted glare is wanted instead. The final review flagged the spec §6.1 "glare inverts" wording as not literally implemented — this item records the owner's verdict, no code change made.

## Regression
- [ ] Player bar, seek, lyrics, queue, modals, toasts legible in all four mode×theme combinations.
