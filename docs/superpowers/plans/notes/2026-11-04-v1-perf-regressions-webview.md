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
2. **Rack meters** (S13 is ~20 % of a core). Measured on builds side by side, Rack playing, the
   harness frame sampler off (as in real use), cycle CPU of WebView2 + app:

   | Variant | Gcycles/s | Change |
   | --- | --- | --- |
   | today (needles redrawn at ~48 Hz) | 0.53–0.58 | — |
   | needle layer rotated by the compositor, crisp redraw at rest | 0.53–0.55 | ≈ −5 % |
   | meter loop sleeps through the 144 Hz frames it skips (rAF 144 → 49/s) | 0.52–0.54 | ≈ 0 |
   | meters drawn at ~30 Hz (`MIN_STEP_MS` 1000/30 − 1) | 0.37–0.40 | **−27 %** |

   The cost is per *changed* frame: each one re-layerizes the whole Rack page (~0.8 ms) and is
   composited; skipped frames were already nearly free, and a cheaper needle still changes the frame.
   Hiding the needles (0.25) only showed what no change at all costs. Rotation and loop sleeping were
   tried and reverted; 30 Hz is the one lever (visual: needles move ~29 times a second, not ~48).
3. The S13 scenario keeps the harness frame sampler on, a rAF loop of its own, so S13 cannot show
   frame-count savings in Rack; read Rack CPU work with the sampler off.

Harness/methodology (owner rulings):

4. Re-baseline the S11–S14 CPU budgets on cycle CPU (rule unchanged); S12 needs its attended run.
5. Budget memory on private footprint (or `private_max_mb`) rather than summed working sets, so a
   bigger runtime DLL does not read as app growth.
6. After (1): re-run S8 and S9, apply R2 to them and re-check the S9 drift budget.

**Atmosphere playback (2026-11-04 evening).** The owner's look became Atmosphere with a maximized
window (2560×1440) before the S8/S9/S11 re-runs, so those ran in a different shell from the morning
corpus (Studio, 2068×1438): their memory is not comparable with it. It exposed the now-playing mark:
it turned by rewriting an SVG `transform` every frame, a layout per frame (144/s, ~60 ms/s) while
playing in Atmosphere. Now the compositor turns an HTML wrapper and the angle returns to the SVG at
rest (`08c2815`): start-page playback 2.67 → 1.30 Gcycles/s (−51 %), layouts 144 → 2/s. No scenario
covered playing in Atmosphere (S4/S8/S9 run in the owner's mode; S5-atmosphere scrolls, S14 is
paused); 1.30 Gcycles/s is still ~45 % of a core and worth its own look.

**Atmosphere playback, the rest (2026-11-05).** Still the mark. Every frame in which it moves
costs a frame of the whole page: the main thread's lifecycle (style, prepaint, commit; ~1.5 ms)
and the compositor + GPU process drawing the window, whatever the glyph's size. 144 Hz means 144
of them a second. The blurs are not part of it. UI-only A/B (`ab.ps1`'s method: production
bundles on `vite preview`, the debug exe), playing on Tracks with two marks on screen
(queue thumb, table row), 2164×1293, harness frame sampler off, cycle CPU of WebView2 + app, two
runs each:

| Variant | Gcycles/s | Change |
| --- | --- | --- |
| `53cef6d` (rAF turn at 144 Hz) | 1.29 / 1.47 | — |
| marks hidden (the floor) | 0.31 / 0.31 | −78 % |
| every `backdrop-filter` off | 1.29 / 1.31 | ≈ 0 |
| the 80 px Backdrop blur hidden | 1.27 / 1.28 | ≈ 0 |
| no redundant SVG `transform` / `will-change` writes per frame | 1.32 / 1.29 | ≈ 0 |
| Web Animation on the wrapper, smooth | 0.71 / 0.73 | −48 % (main thread idle; GPU still at 144 Hz) |
| rAF turn drawn at ~30 fps | 0.49 / 0.42 | −67 % |
| Web Animation, `steps(60)` per 2 s turn (30 changes/s) | 0.35 / 0.38 | −74 % |

Chromium skips the frames between a stepped compositor animation's changes, so the stepped turn
saves both the main thread and the GPU process. **Applied (owner: option C):** off Linux the mark
turns as that stepped Web Animation; JS runs only while the speed eases on play/pause (playback
rate ramp, ~600 ms) and writes the stepped angle back into the SVG at rest. As shipped: 0.357 /
0.359 Gcycles/s (−74 %). Linux keeps the 30 fps `frameDue` loop (`framesGated`): WebKitGTK
presents every frame of an animation, stepped or not (`2026-10-28-linux-home-lag.md`).
DiscArt (Now Playing) has the same 144 Hz rAF turn and is untouched; at its size a 30/s step
may judder at the rim (~6 px a step), so it needs its own look.

**Pinned for Linux (later).** The mark there is still a JS loop presenting 30 fps through the
GTK main thread (app 12 %, web 6 % with the queue open, the 2026-10-28 table). Look at whether WebKitGTK skips a
stepped Web Animation's unchanged frames as Chromium does (the 10-28 finding was for CSS
animations), else a lower gate for the mark alone, and re-measure Atmosphere playback on
bare metal.

~~Later, measure first (under the hood): Rack's level loop and CatJam both run a rAF callback on every
144 Hz frame and skip most of them; waiting with a timer until the next draw is due would cut the
wake-ups without changing what is drawn (CatJam's beat sync must stay frame-exact).~~
**Answered, not pursued (owner, 2026-11-05).** The level loop was already measured in Suggestion 2:
sleeping through skipped frames (rAF 144 → 49/s) left cycle CPU unchanged (≈ 0), because a skipped
frame writes nothing and is nearly free; the cost is in changed frames. CatJam's skipped frames are
cheaper still (a frame-index compare, a style write only on change), and a timer wake risks its
frame-exact beat sync (Windows timer resolution can make it miss the change frame), so it was not
built.
