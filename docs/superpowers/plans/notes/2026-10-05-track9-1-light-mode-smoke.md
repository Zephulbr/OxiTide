# Track 9.1 light mode — owner smoke checklist

- **Status:** Scripted perf results recorded 2026-10-06 (S1/S2 misses — see below); owner smoke pending (Windows, real account)
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §6.1

## Scripted perf results (2026-10-06)

- **Build/conditions:** branch `track9-light-mode`; debug binary rebuilt 2026-10-06 00:32 via `cargo build -p oxitide-app`; Vite dev server on 1420 warm; one OxiTide instance at a time, foreground; the machine was **not** quiet (other apps and their WebView2 processes were running).
- **Commands:**
  - `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere-light -Runs 2` (runner also checks)
  - `node scripts/perf/check.mjs --scenario S5-atmosphere-light`
  - `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 3`
  - `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 3`
  - `node scripts/perf/check.mjs`
- **Gate results (`check.mjs`, p95 over the accumulated runs dir):**

| Scenario | Metric | Value | Budget | Result |
| --- | --- | --- | --- | --- |
| S5-atmosphere-light | frame_p95_ms:atmosphere | 13.9 | ≤ 20 | PASS |
| S5-atmosphere-light | frame_p99_ms:atmosphere | 20.9 | ≤ 33 | PASS |
| S1 | first_paint_ms | 1113.0 | ≤ 1500 | PASS |
| S1 | ui_ready_ms | 1331.0 | ≤ 2500 | PASS |
| S1 | memory_max_mb | 593.0 | ≤ 570 | **FAIL** |
| S1 | long_task_max_ms | 0.0 | ≤ 100 | PASS |
| S1 | startup_step_max_ms | 7.0 | ≤ 100 | PASS |
| S2 | first_paint_ms | 1108.0 | ≤ 1000 | **FAIL** |
| S2 | memory_max_mb | 597.0 | ≤ 560 | **FAIL** |
| S2 | long_task_max_ms | 0.0 | ≤ 100 | PASS |
| S2 | startup_step_max_ms | 7.0 | ≤ 100 | PASS |

- **New-run detail (this corpus only):**
  - S5-light per run: p95 13.9 / p99 20.9 aggregate; 5000-frame final sample p95 7.1 / p99 7.1 both runs; memory max 670 / 640 MB (informational — this key has no memory budget).
  - S1 per run: first_paint 984 / 1016 / 969 ms; ui_ready 1230 / 1257 / 1198 ms; memory max 586 / 593 / 589 MB.
  - S2 per run: first_paint 1108 / 962 / 998 ms; ui_ready 1646 / 1506 / 1279 ms; memory max 595 / 592 / 597 MB.
- **Caveats and misses:**
  - **The theme pin changes backend `ui.theme` only — the rendered theme is not switched.** `drive.mjs` sets it with the raw `set_theme` invoke, but the UI's `appearance` singleton is initialized once (`App.svelte:220`) and there is no `__perf.setTheme` bridge (only `setMode`). A CDP probe on the running app confirmed: after raw `set_theme("light")`, `get_config().ui.theme == "light"` while `document.documentElement.dataset.theme` stays `"dark"` (and the localStorage mirror stays `"dark"`). So the S5-atmosphere-light frame rows above were measured with the **dark** rendering; the light pin needs a bridge/apply step before the result means what its name says.
  - **S1/S2 misses are global, not S5-light-specific.** Every current sample is ~60–100 MB above the 2026-10-02 Phase D corpus (S1 586–593 vs 492–518; S2 592–597 vs 487–511; S5-light 640–670 vs S5-atmosphere 584). The WebView2 runtime updated to 154.0.4258.53 on 2026-10-03, after Phase D, and the Windows `perf.rs` memory code is unchanged since 2026-10-01. No A/B baseline on the current runtime exists, so the cause is unproven; budgets were not adjusted.
  - The full-corpus `node scripts/perf/check.mjs` also reports stale-corpus failures unrelated to this change: S3-hit ttfs 308 (2026-10-01 runs), S4 gap 103 (documented debt), and `n/a` required interaction rows on the 2026-09-30 S5/S6/S7/S9 runs, which predate those metrics.
  - The owner's original theme (`dark`) was restored after the S5-light runs (`config.toml` ends with `theme = "dark"`).

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

## Regression
- [ ] Player bar, seek, lyrics, queue, modals, toasts legible in all four mode×theme combinations.
