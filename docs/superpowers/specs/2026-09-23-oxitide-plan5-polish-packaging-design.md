# OxiTide — Plan 5 (Polish, Packaging & Paged Library) Design Spec

- **Status:** Draft for review
- **Date:** 2026-09-23
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md`
- **Predecessor:** `2026-09-23-oxitide-plan4-streaming-cache-design.md`
- **Phase:** Phase 1 (MVP) — the fifth and final implementation **plan** within Phase 1, covering milestone
  **M8** (polish: error handling, tests, packaging) plus the follow-ups from the Plan 4 smoke run
  (Library/Playlist load time, session-expiry recovery).

---

## 1. Purpose & Outcome

Plan 5 turns the working Plan 4 player into a **shareable, installable daily driver**. It produces a
verified NSIS per-user installer (MSI kept buildable), and removes the remaining dead-end states:
an expired session becomes a re-login that preserves the queue and position, network failures become
retryable instead of terminal, crashes leave a log file to diagnose, the window remembers its
geometry, and the Library/Playlist views render immediately instead of walking every page before the
first paint.

Success = the owner can hand the installer to someone else, they can install it without admin rights,
and the app survives a session expiry, a network drop, and a large favorites list without a dead end
or a multi-second blank screen.

## 2. Scope

### In scope (Plan 5)
- **Packaging**: bundle metadata, NSIS `currentUser` install, WebView2 download bootstrapper,
  version bump to 0.2.0, and a recorded install/upgrade/uninstall checklist.
- **Auth expiry recovery**: a `session_expired` signal from commands and the engine; a banner with
  Re-login; after login the queue and last position are restored (paused if it was paused).
- **Network errors + Retry**: a shared `ErrorState` component with Retry in every data view, and a
  Retry action in the player bar that restarts the current track at its last position.
- **File logging**: `%APPDATA%\OxiTide\logs\oxitide.log`, daily rotation, 5 files kept, default
  `info`, `RUST_LOG` override; a Settings button to open the logs folder.
- **Window state**: size/position/maximized persisted in `config.toml` and restored at startup with
  on-screen clamping.
- **Paged Library/Playlist**: scroll-paged favorite tracks and playlist tracks with a session cache;
  "Play all" and row-play load the remainder first with a visible loading state.

### Explicitly deferred
- Code signing, auto-updater, release hosting/CI.
- Keyboard shortcuts and media keys (media keys are Phase 3).
- List virtualization (only needed if a real library proves too heavy after paging).
- The Plan 4 review's deferred minor findings (cache publish window, `StreamMeta` on queue end,
  optimistic UI rollback, extra test gaps) unless a task touches the same code.
- Phase 2/3/4 features (editing, lyrics, offline, integrations, DSP/EQ, casting, Atmos).

## 3. Tech Stack Additions

| Concern | Crate | Notes |
|---|---|---|
| Log file rotation | `tracing-appender = "0.2"` | daily rotation + `max_log_files(5)`; pairs with the existing `tracing-subscriber 0.3` |

No new Tauri plugins: the logs folder opens through the already-installed `tauri-plugin-opener`
(from Rust, so the URL-only capability does not change), and window state is persisted through the
existing `Config`/`directories` stack.

## 4. Repository Layout Changes

```
crates/oxitide-tidal/src/
  library.rs          # favorite_tracks_page(limit, offset) -> (Vec<Track>, u32)
  catalog.rs          # playlist_tracks_page(id, limit, offset) -> (Vec<Track>, u32)
crates/oxitide-app/src/
  config.rs           # [ui] window_* fields; logs_dir()
  state.rs            # session_expired flag, AppHandle holder, fail_tidal()
  window.rs           # NEW: geometry capture/apply + on-screen clamp (pure helper)
  commands.rs         # page commands, retry, get_logs_dir, open_logs_folder; fail_tidal wiring
  player.rs           # engine auth-error code; shutdown-before-replace helper
  main.rs             # logging init, window geometry apply/show, exit save, new command registration
crates/oxitide-app/
  tauri.conf.json     # metadata, version 0.2.0, NSIS currentUser, WebView2 bootstrapper, visible:false
ui/src/
  lib/pager.svelte.ts        # NEW: session-cached pager store
  lib/pager.test.ts          # NEW
  lib/components/ErrorState.svelte   # NEW
  lib/components/TrackTable.svelte   # optional onPlay/onPlayAll overrides
  lib/api.ts                 # page commands, retry, logs commands, TrackPage type
  lib/player.svelte.ts       # retry(); restore(queue, index, position, wasPlaying)
  App.svelte                 # session_expired listener + banner; login/restore flow
  views/*.svelte             # load() extraction + ErrorState; Library/Playlist paging
docs/superpowers/plans/notes/
  plan5-release.md    # NEW: release checklist
```

## 5. Architecture

### 5.1 Packaging & release

- `crates/oxitide-app/tauri.conf.json`:
  - `version: "0.2.0"`; the workspace version in the root `Cargo.toml` bumps to `0.2.0` in the same
    commit so `Cargo.lock` and the bundle agree.
  - `bundle` gains `publisher`, `copyright` ("AGPL-3.0-or-later"), `category: "Music"`,
    `shortDescription`, `longDescription`.
  - `bundle.windows.webviewInstallMode = { "type": "downloadBootstrapper" }` — the installer fetches
    WebView2 when missing instead of bundling ~150 MB.
  - `bundle.nsis.installMode = "currentUser"` — no admin prompt, per-user install under
    `%LOCALAPPDATA%\OxiTide`; the MSI target stays configured for later.
- The app window is declared `"visible": false` and shown by `main.rs` after geometry is applied
  (§5.5), which also removes the startup resize flash.
- A release checklist lands in `docs/superpowers/plans/notes/plan5-release.md`: version-bump steps,
  the exact `npm --prefix ui run app:build` command, artifact paths, install → launch →
  upgrade-in-place → uninstall verification, the expected SmartScreen warning (unsigned), and the
  data-preservation checks (`config.toml`, cache, keyring survive an upgrade; uninstall removes the
  install directory but not `%APPDATA%\OxiTide`).
- README's build section and AGENTS.md's packaging line are updated with the 0.2.0 artifact names.

### 5.2 Auth expiry and re-login

- **Detection.**
  - `AppState` gains `session_expired: AtomicBool` and a `handle: Mutex<Option<AppHandle>>` set once
    in `main`. `notify_session_expired()` sets the flag, logs a warning, and emits a
    `session_expired` event when a handle is present. `clear_session_expired()` resets it.
  - `AppState::fail_tidal(&self, e: oxitide_tidal::Error) -> String` calls
    `notify_session_expired()` when `matches!(e, Error::Unauthorized)` and returns `e.to_string()`.
    Every command that reaches the Tidal client maps its error through it; non-Tidal errors keep the
    plain mapping.
  - The engine distinguishes auth failures: `worker_main`'s `StreamError` arm emits
    `EngineEvent::Error { code: "auth", .. }` when the error is
    `oxitide_audio::Error::Tidal(oxitide_tidal::Error::Unauthorized)`, otherwise `"playback"`. The
    `build_player` listener checks `code == "auth"` and calls
    `app.state::<AppState>().notify_session_expired()`.
- **UI.** `App.svelte` listens for `session_expired`. When set, it captures the queue, current
  index, `position_ms`, and whether playback was running, then renders the existing `Login` view
  (which gains an optional `message` prop) with a "Session expired — sign in to continue" heading.
  On successful login it calls `player.restore(...)`:
  - `playTracks(queue, index)` → `seek(position_ms)` → `playPause()` only when the captured state
    was **paused** (`play_context` always starts playback, so a paused session must be paused again
    after the restore; a playing session needs no extra call).
  - `restore` refreshes state from `get_playback_state` afterwards. Sub-second position accuracy is
    best-effort.
- **Player replacement.** `auth_complete`/`auth_restore` shut down an existing engine before
  replacing it (today the old worker thread would keep running), then clear the session-expired
  flag.

### 5.3 Network errors and Retry

- New `ui/src/lib/components/ErrorState.svelte`: `{ message, onRetry? }`; renders the message and a
  Retry button when `onRetry` is provided. Every data view (Home, Album, Playlist, Artist, Search,
  Library, Settings) extracts its fetch into a `load()` function and renders `ErrorState` on error
  so Retry re-runs the same load.
- **Playback retry.** New `EngineCmd::Retry` + `Engine::retry()`:
  - Requires a current item; sets the queue back to playing (`Queue::resume()`), clears `paused`,
    and installs `PendingSeek { track_id, position_ms: inner.position_ms, auto_start: true }`, then
    bumps the generation and wakes the worker. This also recovers a device-loss pause.
  - New Tauri command `retry()`; `player.svelte.ts` gains `retry()`; the PlayerBar renders the error
    message with a Retry button.
- A network drop while browsing now shows a retryable message instead of a raw error string; a drop
  mid-stream shows the player-bar error with Retry at the last position.

### 5.4 Logging to file

- `config::logs_dir() -> Option<PathBuf>` returns the `%APPDATA%\OxiTide\logs` path (via
  `config_dir()`); the logging init creates the directory.
- `main.rs` initializes tracing with an `EnvFilter` from `RUST_LOG`, defaulting to `info`, and two
  layers: the existing stderr layer and a file layer (`with_ansi(false)`) over
  `tracing_appender::non_blocking` with a daily rotation, `filename_prefix("oxitide")`, and
  `max_log_files(5)`. The non-blocking guard is kept alive for the process lifetime (leaked into a
  `OnceLock`). If the logs directory cannot be created, the app logs to stderr only.
- New commands: `get_logs_dir() -> String` and `open_logs_folder(app)` which calls the opener
  plugin from Rust (`app.opener().open_path(dir, None::<&str>)`), so no capability change is needed.
- Settings gains a "Logs" row with the directory and an **Open logs folder** button.
- Token redaction is unchanged: no token values are logged, and the existing signed-URL redaction
  applies to file logs too.

### 5.5 Window state

- `[ui]` config gains `window_width: u32` (default 1200), `window_height: u32` (800),
  `window_x: Option<i32>` / `window_y: Option<i32>` (absent until the first save),
  `window_maximized: bool` (false).
- New `crates/oxitide-app/src/window.rs` with a pure, unit-testable helper:
  - `visible_position(x, y, monitors: &[MonitorRect]) -> Option<(i32, i32)>` returns the position
    when at least a 200×100 corner of the window intersects a monitor's work area, else `None`.
  - `apply(window, config)` maximizes when configured, otherwise restores size and (when visible)
    position; `capture(window, config)` reads outer position, inner size, and maximized state.
- `main.rs` applies the saved geometry to the `main` window after build and then `show()`s it; the
  `RunEvent::Exit` handler captures the geometry and saves the config (best-effort, ignored on
  failure) alongside the existing clear-on-exit logic.

### 5.6 Paged favorite tracks and playlists

- **Tidal client.** Replace the single-page helpers with total-carrying variants:
  - `favorite_tracks_page(limit, offset) -> (Vec<Track>, u32)`
  - `playlist_tracks_page(id, limit, offset) -> (Vec<Track>, u32)`
  The existing `all_*` walkers stay for the small lists (Library albums/artists/playlists, album
  tracks).
- **App.** New `TrackPageDto { items: Vec<Track>, total: u32, offset: u32, limit: u32 }` and
  commands `get_favorite_tracks_page { offset, limit }` and
  `get_playlist_tracks_page { id, offset, limit }`.
- **Pager store** `ui/src/lib/pager.svelte.ts`:
  - `createTrackPager(key, load, pageSize = 100)` returns `{ items, total, loading, error, done,
    loadMore(), ensureAll(), refresh() }`.
  - A module-level cache keyed by `key` (`"favorites"` / `"playlist:{uuid}"`) stores loaded items,
    total, and the next offset; a new pager for the same key hydrates from it, so revisiting a view
    within a session does not refetch.
  - `loadMore()` fetches the next page, appends, dedupes by track id, and updates `total`/`done`
    (done when `offset >= total` or a short page arrives). `ensureAll()` loops `loadMore()` until
    done. `refresh()` clears the key's cache and reloads from offset 0.
- **UI.**
  - `Library.svelte` renders playlists/albums as today and uses
    `createTrackPager("favorites", …)` for favorite tracks, with an IntersectionObserver sentinel
    that calls `loadMore()` when visible, and a Refresh action.
  - `Playlist.svelte` uses `createTrackPager("playlist:" + id, …)` the same way.
  - `TrackTable.svelte` gains optional `onPlay(index)` / `onPlayAll()` overrides. Library/Playlist
    pass handlers that `await pager.ensureAll()` first (showing a "Loading all…" state on the
    button) so the queue matches the full list, then call `playTracks` as today.
  - Pager errors render through `ErrorState` with Retry (`loadMore` again).

## 6. Config (`%APPDATA%\OxiTide\config.toml`)

```toml
[audio]
output_device = ""          # "" = system default
exclusive = false
exclusive_fallback = "shared"
volume = 1.0

[streaming]
quality = "lossless"        # "high" | "lossless" | "max" ("low" is coerced to "high")
allow_fallback = true

[ui]
quality_display = "compact"
window_width = 1200
window_height = 800
# window_x / window_y appear after the first exit
window_maximized = false

[cache]
dir = ""
max_size_mb = 2048
clear_on_exit = false
```

- Missing keys keep defaults; malformed files still back up to `config.toml.bak`; a persisted
  `quality = "low"` keeps coercing to `high` (P4-D10).

## 7. IPC Surface Changes

| Command / event | Change |
|---|---|
| `get_favorite_tracks_page` | NEW — `{ offset, limit } -> { items, total, offset, limit }` |
| `get_playlist_tracks_page` | NEW — `{ id, offset, limit } -> { items, total, offset, limit }` |
| `retry` | NEW — restart the current track at its last position |
| `get_logs_dir` | NEW — `string` |
| `open_logs_folder` | NEW |
| `session_expired` (event) | NEW — emitted when a command or playback call hits `Unauthorized` |
| `EngineEvent::Error` | may now carry `code: "auth"` (previously always `"playback"`) |

Existing commands and events are unchanged.

## 8. UI Changes

- **App.svelte**: `session_expired` listener, capture/restore flow, and the expired-session login
  heading.
- **Views**: `load()` extraction + `ErrorState` (Home, Album, Playlist, Artist, Search, Library,
  Settings); Library and Playlist gain the paged track lists, sentinel, Refresh, and
  ensure-all play handlers.
- **PlayerBar**: error message with a Retry button.
- **Settings**: Logs row (directory + Open logs folder).
- **api.ts / player.svelte.ts**: new commands, `TrackPage`, `retry()`, and `restore()`.

## 9. Error Handling

| Situation | Behaviour |
|---|---|
| Token refresh fails (`Unauthorized`) | `session_expired` event; banner + Re-login; queue/position restored after login |
| Network failure while browsing | `ErrorState` with Retry re-runs the view's `load()` |
| Network failure mid-stream | Player-bar error with Retry; retry restarts the track at its last position |
| Playlist/favorites page fetch fails | Pager `error` + Retry on the same offset; loaded items stay |
| Logs directory cannot be created | stderr-only logging; Settings shows the failure when opening logs |
| Saved window position is off-screen | position ignored, default/centered placement used; size still restored |
| Config save at exit fails | logged and ignored (same policy as clear-on-exit) |
| Session expires while paused | re-login restores the queue paused at the last position |

## 10. Testing Strategy

- **Unit (tidal):** `favorite_tracks_page` / `playlist_tracks_page` return items + total and send
  `limit`/`offset` (wiremock); short/empty page handling in the pager is UI-tested.
- **Unit (app):** `fail_tidal(Unauthorized)` sets `session_expired` and other errors do not;
  `window::visible_position` inside/outside/partially-visible monitor rects; config window fields
  round-trip and default; `logs_dir()` returns a path ending in `logs`.
- **Engine (MapFetcher/NullOutput):** `EngineCmd::Retry` after a stream failure restarts the current
  track at the recorded position and emits a playing state; `Retry` with no current item is a no-op;
  an `Unauthorized` resolver error emits `code: "auth"`.
- **UI (vitest):** pager append/dedupe/total/done, `ensureAll`, cache hydration across instances,
  `refresh` clears the cache, error propagation; existing 13 tests stay green.
- **Manual checklist:** `plan5-release.md` for install/upgrade/uninstall; session expiry (delete the
  keyring entry mid-session → banner → re-login → queue resumes); network off → Retry; window
  geometry across restarts and monitor changes; log file written and rotated; large-library scroll
  and Play all.
- **Gates:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui test`, `npm --prefix ui run check`, plus one successful
  `npm --prefix ui run app:build`.

## 11. Milestones / Planned Tasks (detailed in the implementation plan)

| # | Task | Exit criteria |
|---|---|---|
| P5-1 | Packaging & release checklist | NSIS + MSI artifacts built; install/upgrade/uninstall verified; checklist recorded |
| P5-2 | File logging | log file written with rotation; Settings opens the folder |
| P5-3 | Window state | geometry survives a restart; off-screen positions clamped |
| P5-4 | Auth expiry & re-login | expiry banner; queue/position restored after login |
| P5-5 | Retry (views + playback) | view retries re-run loads; player retry resumes at position |
| P5-6 | Paged favorite tracks | Library paints after one page; scroll appends; session cache |
| P5-7 | Paged playlist tracks | Playlist uses the same pager |
| P5-8 | Verification | full gates + manual checklist recorded |

## 12. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| WebView2 bootstrapper needs internet at install time | Documented in the release checklist; `downloadBootstrapper` keeps the installer small |
| Unsigned installer triggers SmartScreen | Expected and documented; signing deferred |
| Re-login replay is approximate | Position restored to the last reported ms; paused state preserved; documented |
| Off-screen window position after monitor changes | Pure clamp helper with unit tests; size still restored |
| Session cache shows stale favorites after changes on another device | Refresh action clears the key's cache; session-only by design |
| `tracing-appender` adds a dependency | Small, official tracing-ecosystem crate; only in `oxitide-app` |
| Pager + ensure-all still downloads everything before Play all | One-time per session thanks to the cache; button shows a loading state |
| Installer metadata drift between `Cargo.toml` and `tauri.conf.json` | Both versions bumped in one commit; release checklist names both files |

## 13. Decisions Log (Plan 5)

| # | Decision | Rationale |
|---|---|---|
| P5-D1 | NSIS `currentUser` primary; MSI kept buildable; unsigned. | No admin prompt for a personal/shareable app; signing infrastructure is out of scope. |
| P5-D2 | WebView2 `downloadBootstrapper`. | Small installer; Windows 10/11 usually already has WebView2. |
| P5-D3 | Window state in `config.toml`, no plugin. | Reuses the existing config stack and tests; avoids a new plugin/capability. |
| P5-D4 | Daily-rotated file logs, 5 files, default `info`, `RUST_LOG` override. | Diagnosable without a console; no telemetry. |
| P5-D5 | Session expiry detected at the command boundary + engine error code; queue and position restored. | The only complete detection point for both browsing and playback. |
| P5-D6 | Scroll-paged favorite/playlist tracks with a session cache; Play all does `ensureAll`. | Fast first paint with correct queue semantics. |
| P5-D7 | Version 0.2.0 for the first shareable installer. | Marks the Phase 1 MVP. |
| P5-D8 | Signing, updater, keyboard shortcuts, virtualization, and deferred review minors are out of scope. | Keep M8 tight; revisit from real use. |

## 14. Out of Scope (tracked for later phases)

- **Phase 2:** playlist/favorites editing, synced lyrics, discovery polish.
- **Phase 3:** offline downloads/playback, Last.fm, Discord RPC, SMTC/media keys.
- **Phase 4:** DSP/EQ, casting/TIDAL Connect, Dolby Atmos.
- **Explicitly not in this plan:** code signing, auto-update, CI, keyboard shortcuts, list
  virtualization, cache statistics, and the Plan 4 review's deferred minor findings.
