# Linux Home lag and the reload crash (2026-10-28)

- **Machine:** CachyOS laptop, KDE Plasma Wayland, AMD Radeon 680M, webkit2gtk-4.1 2.54.1,
  device pixel ratio 2. Same host as `2026-10-28-linux-v1-close-run.md`.
- **Report (owner):** on Linux the UI lags behind playback. Play/pause acts at once, but the
  UI shows it seconds later. Windows is fine. Worst on Home, in Atmosphere.
- **Commits:** `7114add` (badge blur), `5bcc796` (reload crash).

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

## Open

- **FolderArt badge.** The Playlists page's folder badge (`FolderArt.svelte`) uses the same
  small-blurred-badge pattern. It was not measured; if that page lags with many folders, apply
  the same rule.
- **Windows rerun.** The `5bcc796` paths (re-login on Windows takes the same rebuild path) have
  not been rerun on Windows.
