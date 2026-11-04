# v1.0 perf: the corpus regressions, pinned, and WebView2 memory (2026-11-04)

- **Inputs:** `corpus-20261104-025308` (Results in `2026-10-23-v1-perf-rulings.md`), then probes on
  the same machine (Ryzen 7 5700X3D, RX 9070 XT, 2560×1440 @ 144 Hz, WebView2 156.0.4314.58,
  installed 2026-11-01). Probe scripts (ad hoc, gitignored) live in `.superpowers/perf/probe/`:
  `regress-probe.mjs` (scene + counters, DOM mutations, layer paints, invalidation trace),
  `memdump.mjs` (Chromium memory-infra allocator dumps per process), `threadcpu.ps1` (per-thread
  CPU with Chromium's thread names, tick-sampled and cycle-exact), `ab.ps1` (the UI of any commit
  as a production bundle on `vite preview`, driven by HEAD's debug exe: only the UI varies).

## Verdicts

| Corpus breach | Cause | Status |
| --- | --- | --- |
| S13 `cpu_webview_pct` 23.1 (13.6) | **Measurement.** Windows process CPU time is sampled at the 15.6 ms clock tick and charged to the thread running at the tick; the five S13 runs read 7.2 / 12.8 / 23.1 / 32.6 / 39.0 % with identical page work (layouts, task time and IPC within 0.3 %). Cycle-exact counts of four identical launches: 0.620–0.637 Gcycles/s (±1.4 %) while the tick metric read 8.0–23.7 %. | Harness fixed (cycle CPU). Cycle A/B, same backend and runtime: Track 13 UI `32f291c` 0.757 / 0.752 Gcy/s, today `ee5fba8` 0.766 / 0.765 (+1.5 %, ~650 more DOM nodes). **No regression.** |
| S13 frame p99 34.6 (33), long task 105 (100) | The first ~5 s: start-up and the driver's switch to Rack, before `measure_start`. Steady p99 is 7.2 ms on every run. | Harness fixed (samples restart at `measure_start`; checker drops earlier rows). |
| S14 IPC 1,842 events/min (1,706), 1,034 KB/min (930) | **Window length.** Every run sends the same 369 events (one per volume step) at 561 B/event (Track 13: 558). The sweeps step once per frame, so at a full 144 Hz they finish sooner and the per-minute rate over a window that ends a fixed time after them rises. `fd27406` changed nothing here. | Harness fixed (fixed 15 s window). |
| S14 `cpu_webview_pct` 20.7 (18.1) | Tick sampling (runs read 13.5–32.8) plus the shorter window. | Harness fixed; CPU budgets need a cycle-based re-baseline. |
| S14 frame p95 20.8 (20) | Frames from start-up, the mode switch and the 10k queue load (steady p95 7.1). | Harness fixed. |
| S9 drift +138 MB (≤ 50) | **Real.** The dynamic-colour glide (below). | Fix is visual: owner decision. |

## S9: the dynamic-colour glide fragments Blink's GC heap

- The growth is one process, the renderer: 162 → 286 MB private over the 2 h, linear (~1 MB/min,
  ~0.8 MB per track change; 160 changes). JS heap stays ~12 MB; every other process is flat.
- Memory-infra: it is `blink_gc` (Oilpan, Blink's GC heap: DOM, computed styles). An 8-minute
  soak (a change every 6 s) took the renderer from 132 to 369 MB with `blink_gc` at 276 MB holding
  9.9 MB of live objects. A forced full GC only took it from 195 to 176 MB (4-minute soak): the
  space is not garbage waiting for a GC but freed heap Oilpan keeps (fragmented pages).
- The churn: `tokens.css` transitions the five `--dyn-*` colours on `:root` over `--dur-mood`
  (1.2 s). They are registered `inherits: true`, so every animation frame changes an inherited
  value at the root and Blink computes a new style for every element: ~170 full restyles of ~5,700
  nodes per track change at 144 Hz. The soak averaged 148 ms/s of style recalc, so during a glide
  restyling fills most of each 6.9 ms frame (it also drops frames on every track change).
- Soaks of 40 changes (renderer footprint start → end, `blink_gc`):

  | Variant | Footprint | `blink_gc` |
  | --- | --- | --- |
  | today (smooth glide) | 132.6 → 282.1 | 57.0 → 194.9 |
  | glide off | 130.6 → 133.1 | 55.3 → 55.5 |
  | `steps(36)` (30 updates/s) | 138.8 → 153.2 | 60.0 → 68.7 |
  | `steps(24)` (20 updates/s) | 131.7 → 133.7 | 54.0 → 52.7 |

  A stepped glide restyles only when the colour changes; 24 steps over the same 1.2 s is
  indistinguishable from off for memory.
- The glide is older than Phase D's +14 MB S9 (2026-09-27); what grew since is the DOM the
  restyles walk (Home, the player bar, track tables), Full dynamic colour, and the runtime.

## Harness fixes (applied)

- `crates/oxitide-app/src/perf.rs`: `memory_process` rows carry `cpu_cycle_ms`
  (`QueryProcessCycleTime` over the TSC rate measured once against the monotonic clock; test:
  a 150 ms busy spin is charged 0.5–1.1× its wall time). `scripts/perf/check.mjs` uses it when
  every row has it.
- `ui/src/lib/perf.svelte.ts` + `__perf.resetSamples()`: `measure_start` restarts the frame and
  long-task samples; `check.mjs` drops frame/long-task rows before the mark.
- `scripts/perf/drive.mjs` S14: a fixed 15 s window from `measure_start`.
- Consequence: the S11–S14 CPU budgets (set on tick-sampled `cpu_ms`) are not comparable with
  cycle-based runs; they need a re-baseline under the same rule (median + 15 %, ≥ +0.5 pt).
- Re-run on the fixed harness (`corpus-20261104-125420`, release build of the working tree, 5 runs
  each): S13 cycle CPU 20.1–20.5 % (tick: 7.8–32.0 % in the same runs), IPC 1,905/min, frame p99
  13.9, long task 0, memory 644 worst; S14 cycle CPU 13.8–15.3 %, IPC 1,228/min and 689 KB/min,
  p95 7.1, p99 7.2. Every row passes except S13 `cpu_webview_pct` (20.4 vs the tick-era 13.6).
  Under the existing rule the re-baseline would be S13 ≈ 23.5 and S14 ≈ 16.7 (owner).

## WebView2 memory

- **What the harness counts.** `memory_max_mb` sums each process's working set. Shared pages
  (`msedge.dll` alone is 339 MB in runtime 156) are counted once per process, so the sum (~600 MB on
  the start page) is well above what the app holds. Chromium's private footprint on the start page:
  renderer ~125–135 MB, GPU process ~140–185, browser ~40, network ~15, storage ~8 (≈ 360 MB), plus
  the app's own ~20–40 MB.
- **Higher than October.** A same-runtime start-page A/B (only the UI varies) reads 615 / 601 MB for
  the Track 13 UI and 631 / 584 MB for today's: the rise from S1's 497 MB is the environment
  (WebView2 156 installed 2026-11-01, today's account state), not app code. Today's UI adds ~55–65 MB
  of private commit, mostly GPU-process textures, with no working-set change. Rack memory is level too
  (606 / 603 vs 613 / 586 MB).
- **The renderer's buckets** (memory-infra, playing, start page): `blink_gc` ~55 MB (only ~8 MB live:
  start-up churn, likely the first palette glide), `cc` tiles ~30–48, malloc ~32–40, V8 ~11.5,
  GPU transfer buffers ~17, partition_alloc ~10, resource cache ~3. The JS heap is never the issue.
- **Tested, no effect:** the Backdrop's two full-window blurred `<img>`s stay mounted at opacity 0 in
  Studio and Rack, but Chromium culls them (GPU 169–170 MB, renderer ~130 MB with or without).
  CSS filters/shadows/masks/backdrop-filters off in Rack: no change once measured on cycles.

## Suggestions

Applied (under the hood): the three harness fixes above; R2 ceilings for S1–S7.

For the owner (visual):

1. **Step the dynamic-colour glide** (`tokens.css` `:root` transition): `steps(24, jump-none)` keeps
   the 1.2 s fade at 20 colour updates a second: no heap growth (table above) and ~7× fewer full-page
   restyles per track change, which also removes the dropped frames during each glide. `steps(36)`
   (30/s) is smoother and still ~16× better than today. Alternatives: a shorter glide, or none.
   Rack's lamp glide (`rack.css`: `--lamp-h` / `--lamp-c`, registered `inherits: true`,
   transitioned on `:root` over the same 1.2 s) has the same shape and takes the same fix (not
   soak-tested in Rack).
2. **Rack meters at the tap rate** (Track 13 follow-up 2): S13 is 20 % of a core, the needles redraw
   at ~48 Hz from 30 Hz tap frames; drawing per tap frame cuts the main-thread meter work by ~⅓.
3. **Needles as compositor rotations:** rotate the already-separate needle layer with a CSS transform
   instead of re-painting the SVG line at each angle: no paint/raster per meter frame, but the line is
   resampled rather than re-drawn at each angle (a slightly softer edge), so it needs an eye on it.

Harness/methodology (owner rulings):

4. Re-baseline the S11–S14 CPU budgets on cycle CPU (rule unchanged); S12 needs its attended run.
5. Budget memory on private footprint (or `private_max_mb`) rather than summed working sets, so a
   bigger runtime DLL does not read as app growth.
6. After (1): re-run S8 and S9, apply R2 to them and re-check the S9 drift budget.

Later, measure first (under the hood): Rack's level loop and CatJam both run a rAF callback on every
144 Hz frame and skip most of them; waiting with a timer until the next draw is due would cut the
wake-ups without changing what is drawn (CatJam's beat sync must stay frame-exact).
