# Linux Home lag and the reload crash (2026-10-28)

- **Machine:** CachyOS laptop, KDE Plasma Wayland, AMD Radeon 680M, webkit2gtk-4.1 2.54.1,
  device pixel ratio 2. Same host as `2026-10-28-linux-v1-close-run.md`.
- **Report (owner):** on Linux the UI lags behind playback. Play/pause acts at once, but the
  UI shows it seconds later. Windows is fine. Worst on Home, in Atmosphere.
- **Commits:** `7114add` (badge blur), `5bcc796` (reload crash); the final pass below.

## Symptom 1: Home lag in Atmosphere

### What was ruled out

- **Backend and IPC.** `play_pause` emits the `State` event synchronously, and the event
  reached the page's JS in 2–4 ms in every measurement.
- **Event volume.** `Position` events arrive at 2 Hz.
- **WebKit env workarounds.** `WEBKIT_DISABLE_DMABUF_RENDERER=1` did nothing.
  `WEBKIT_DISABLE_COMPOSITING_MODE=1` helped a little, because software compositing moved the
  cost rather than removing it. Neither is needed after the fix.
- **Full-window effects.** Hiding the grain, the backdrop art, or setting `--glass-blur: 0px`
  made no difference. Neither did hiding the now-playing marks or the carousel masks.
- **Script.** No `requestAnimationFrame` callers and no running CSS animations on Home.

### Measurement

- **Per-thread CPU** from `/proc/<pid>/task/*/stat`: on Home with playback running, the
  WebKit web process's `ThreadedCompositor` thread ran at ~130–150% and the Mesa `gdrv0`
  thread at ~40%. The web process main thread and the app's Rust side were idle. Settings
  and Explore were quiet.
- **Removing every `backdrop-filter`** took it from 116% to 13%, and the culprit was then
  bisected through the WebKit remote inspector (`docs/linux-build.md`, "Profiling WebKitGTK").
- **The cost was MediaCard's `.badge`**, the radio badge on mix cards. Home's "Made for you"
  row shows 16 of them.

| Home, playing, Atmosphere | Web-process CPU | Frame time (median / p95) |
|---|---|---|
| Badge blur on (before) | 114% | 903 / 936 ms |
| Badge blur off (after) | 2% | 16 / 17 ms |

The sidebar's `backdrop-filter` (one large element) costs nothing measurable. The cost scales
with the number of filtered layers, and each position tick recomposited all of them.

### Fix (`7114add`)

- **Platform tag.** `ui/index.html`'s first-frame script sets `data-platform="linux"` when the
  user agent says Linux. WebView2 never does, so Windows is untouched.
- **Badge rule.** `:root[data-platform="linux"] .badge { backdrop-filter: none }`. The badge
  fill is ~82% opaque, so it looks the same.

## Symptom 2: a page reload broke the backend

A reload (Ctrl+R, or Vite's full reload when `index.html` changes) runs `auth_restore`, which
replaces the player. When that dropped the old engine's last reference, `TidalResolver` dropped
its tokio runtime inside the async command. Tokio panicked ("Cannot drop a runtime in a context
where blocking is not allowed"). The drop ran under `AppState::player`'s write lock, so the
panic poisoned it. Every later command failed and the UI stayed on "Loading…". It is a race
with the old worker thread, so it only happened on some reloads.

### Fix (`5bcc796`)

- **Runtime shutdown.** `TidalResolver`'s `Drop` calls `Runtime::shutdown_background`.
- **Lock order.** `set_player` and `logout` drop the old player after releasing the lock.
- **Regression test.** `player::tests::resolver_drops_inside_an_async_context` reproduced the
  panic before the fix.
- **Live check.** Six reloads during playback in the dev build gave no panics, and commands
  kept answering.

## Final Linux pass (same day)

Dev build unless noted, playing, Atmosphere. CPU is percent of one core per thread
(`/proc/<pid>/task/*/stat`); "app" is the `oxitide-app` main (GTK) thread.

### Every page after the badge fix

Home 1%, Explore 4%, Feed 1%, History 3%, Settings 2%, Playlists 1%, Albums 3%, Tracks 4%,
Save for Later 1%, an album 3%, an artist 2%, a 702-track playlist 5% (web process). Frames
16 ms median everywhere. Scrolling the 702-track table: 16 ms median, 19–23 ms p95, 60–77%
web process CPU while it moves.

Playlists keeps 17 shuffle buttons with `backdrop-filter` in the DOM, but they sit at
`opacity: 0` until hover and cost nothing measurable. The folder chip (`FolderArt`) now
drops its blur on Linux like the card badge.

### Presented frames cost the GTK thread

Any continuous animation costs the app's main thread, not just the web process:

| One 40 px box turning (Settings page) | app | web main | compositor |
|---|---|---|---|
| nothing animating | <2% | <2% | <2% |
| 60 fps | 25% | 8% | 9% |
| 30 fps (rAF frames skipped) | 8% | 4% | 3% |

- **Environment variables don't lower it.** Default: 22% app, 17 ms frames.
  `WEBKIT_DISABLE_DMABUF_RENDERER=1`: 32% app, 126% web, 75 ms frames.
  `GDK_BACKEND=x11`: 21% app.
- **Event volume isn't the cause.** 30 Hz `level_frame` events into a no-op listener left
  every thread under 10% (the sampler's floor in that run).
- **A CSS animation presents every display frame,** even with a `steps()` timing function.

### Fix: `ui/src/lib/frameGate.ts`

- **The gate.** On Linux, the rAF loops that run for as long as playback does draw on one
  shared 30 fps beat (`frameDue`). Those loops are DiscArt, NowPlayingMark, Rack's levels
  and LevelRecorder.
- **The platter.** Rack's turntable label turns through `gatedSpin` instead of its CSS
  animation.
- **Why one beat.** Loops gated on their own clocks drifted onto alternate frames and
  presented 60 fps between them.
- **Rounding slack.** WebKit rounds rAF times to whole ms, so two 60 Hz frames can measure
  32 ms. The step keeps 4 ms of slack, because a 1 ms slack landed at ~21 fps.
- **Other platforms.** Off Linux every helper is a no-op, so Windows is unchanged.

| Playing | before (app / web / compositor) | after |
|---|---|---|
| Queue open, the now-playing mark spinning | 26 / 12–16 / 13 | 12 / 6 / 6 |
| Now Playing (disc and mark) | 25 / 25 / 16 | 12–20 / 9–16 / 5–8 |
| Rack, Now Playing (meters, platter) | 27 / 25 / 7 | 13 / 24 / 4 |

The mark also shows in the sidebar whenever playback comes from a playlist. Before the fix
that kept every page presenting 60 fps, at about 50% of a core.

### Other checks

- **ALSA output lead.** `AlsaOutput` had no `queued_ms`, so `lead_ms` was 0 on Linux.
  CatJam's beat and the sleep-timer fade ran early by the whole output buffer. It now reports
  `snd_pcm_delay` after each write: ~530 ms through PipeWire 1.6 (quantum 1024). The same
  change keeps `write`'s scratch buffers between calls, and passes `i32` samples through
  without a copy when no software volume applies.
- **MPRIS across a page reload.** The rebuild drops the old session (and its bus name)
  before the new one asks for it, and `PlayPause` over D-Bus reached the new engine.
- **Resume.** The position moves on the first 2 Hz tick (~0.5 s) after a 3 s or a 35 s
  pause.

## Open

- **Windows rerun.** The `5bcc796` paths (re-login on Windows takes the same rebuild path) have
  not been rerun on Windows.
- **Rack's web-process cost.** Rack still holds ~25% of a core in the web process main thread
  (meter style updates at 30 fps). Windows budgets 13.6% for its whole webview (S13).
