# OxiTide — Track 6a: Windows SMTC + hardware media keys

- **Status:** Draft for review
- **Date:** 2026-09-29
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 6
- **Process note:** scope, source, control set, and enablement were brainstormed live with
  the owner (2026-09-29). This document is the standalone source of truth for sub-track 6a;
  Discord Rich Presence is a separate spec (6b).

---

## 1. Purpose & Outcome

Expose OxiTide's playback to the Windows **System Media Transport Controls** so the OS media
flyout (volume OSD, lock screen, media keys) shows what is playing and can control it.

**Outcome:** while OxiTide is the active media session, the Windows flyout shows the current
title / artist / album and album art, a live position/scrubber, and working Play / Pause /
Next / Previous buttons. The hardware media keys (play/pause, next, previous) drive OxiTide
with the window unfocused. Scrubbing the flyout seeks OxiTide. The integration is **on by
default**, can be turned off in Settings → Connections (persisted as `integrations.smtc`), and
**degrades silently**: any WinRT or interop failure leaves playback untouched.

Success = the flyout and media keys behave as above on Windows 10/11, the toggle works without
a restart, teardown is clean on logout/exit, and the feature does not touch the audio path or
regress playback.

## 2. Scope

**In scope:** a dedicated SMTC worker thread in `oxitide-app` owning the WinRT objects; the
`GetForWindow` interop; metadata + album-art thumbnail + timeline; Play/Pause/Next/Prev and
scrub-to-seek wired to the existing engine commands; the `integrations.smtc` config flag
end-to-end (Rust config → `set_smtc` command → handler registration → `api.ts` → Settings
checkbox); a pure, unit-tested event-mapping module; a config round-trip test; a human GUI
smoke checklist.

**Out of scope:** Discord Rich Presence (spec 6b); Stop/Record/Channel buttons; shuffle and
repeat toggles or indicators through SMTC; reading other applications' media state; non-Windows
platforms; lock-screen-specific behavior beyond what SMTC provides; any change to playback,
queue, catalog, auth, or UI rendering outside the one Settings row.

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| S-D1 | **Sub-track split:** this spec covers SMTC + media keys only. Discord RPC is 6b. | The two integrations are independent and get their own spec→plan→implementation cycles (roadmap §1). |
| S-D2 | **Scope: metadata + art + timeline + transport.** Title/artist/album, album-art thumbnail, live timeline with scrub-to-seek, and Play/Pause/Next/Prev. | Owner choice; richest flyout, matches the official client. |
| S-D3 | **Enabled by default, toggleable off.** `integrations.smtc` defaults `true`; failures degrade silently. | Media keys/SMTC are baseline desktop expectations; a toggle covers preference and troubleshooting. |
| S-D4 | **Hand-rolled with the `windows` crate.** No `souvlaki` / plugin dependency; a Windows-gated module in `oxitide-app`. | Roadmap says "via the `windows` crate"; matches the existing `endpoint_volume.rs` pattern; full control over art and timeline. |
| S-D5 | **One dedicated worker thread owns all WinRT objects and pumps messages.** | `ButtonPressed` is delivered off the UI thread and WinRT event delivery needs a message pump; isolating this from Tauri's main thread avoids any WebView interference. |
| S-D6 | **One worker per player, created in `build_player`.** The engine fan-out captures the session sender once; `integrations.smtc`/`set_smtc` toggle SMTC on that worker through a shared flag (no respawn); logout drops it; re-login rebuilds it. | Respawn-on-toggle would strand the captured sender and silently stop updates; a flag keeps a single wiring point and still removes the session from the OS instantly. |
| S-D7 | **Album art from the TIDAL CDN URL built from `cover`.** Art is only set when the track changes (`plan` dedupes by track id), so no per-tick cache is needed; if the stream cannot be built, show metadata without art. | The CDN is unauthenticated; porting `coverUrl` keeps Rust and UI consistent; per-change updates avoid refetching. |
| S-D8 | **Failures are non-fatal and logged.** Any WinRT/interop error disables the session and returns; playback is never affected. | Integration is optional polish, never a startup dependency. |
| S-D9 | **A non-Windows no-op stub exists** so the command, config, and UI compile everywhere. | Keeps the door open for future platforms and avoids `cfg` leakage into shared code. |

## 4. Research summary

- **SMTC surface (windows 0.62.2).** `windows::Media::SystemMediaTransportControls` provides
  `SetPlaybackStatus`/`PlaybackStatus`, `SetIs{Play,Pause,Next,Previous}Enabled`,
  `ButtonPressed`/`RemoveButtonPressed`, `PlaybackPositionChangeRequested`/
  `RemovePlaybackPositionChangeRequested`, `UpdateTimelineProperties`, and
  `DisplayUpdater()`. `SystemMediaTransportControlsDisplayUpdater` provides `SetType`,
  `MusicProperties` (`MusicDisplayProperties`: title/artist/album), `SetThumbnail`,
  `ClearAll`, and `Update`.
- **`GetForWindow` comes from the `Win32::System::WinRT` interop.** The generated
  `SystemMediaTransportControls` type exposes only `GetForCurrentView`, but the crate also
  generates `windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop`. Obtaining it
  through `windows::core::factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()`
  and calling `unsafe { interop.GetForWindow(hwnd) }` yields the window-bound controls (this is
  exactly how the `souvlaki` crate does it). Requires the `Win32_System_WinRT` feature.
- **Button events are off the UI thread.** Microsoft's guidance documents the handler as not
  running on the UI thread; we only enqueue engine commands, so no UI marshalling is needed.
- **Thumbnail.** `SetThumbnail` takes a `RandomAccessStreamReference`;
  `RandomAccessStreamReference::CreateFromUri(&Uri::CreateUri(url)?)` is the path `souvlaki`
  uses for remote artwork and works for our CDN URL. A file/stream fallback is unnecessary
  unless the smoke check disproves the URI path.
- **HWND source.** Tauri 2 `WebviewWindow::hwnd()` (Windows-only) returns
  `tauri::Result<windows::Win32::Foundation::HWND>` for the `"main"` window. The mini player
  is the same window, so one HWND covers both layouts.
- **Integration points verified in-repo:** the single engine fan-out
  `engine.subscribe(...)` in `build_player` (`crates/oxitide-app/src/commands.rs:366`) already
  receives `State`/`NowPlaying`/`Position`/`Error`; `Engine::handle(EngineCmd)` is thread-safe
  and `Engine` is `Clone`; config toggle plumbing mirrors `ui.oneko` (`config.rs:130/152`,
  `commands.rs:1112`, `main.rs:282`, `api.ts:250`, `Settings.svelte:327`); the frontend cover
  rule is `ui/src/lib/artwork.ts:7`; the Settings **Connections** tab has a disabled
  "Windows media controls" placeholder at `ui/src/views/Settings.svelte:442`.

## 5. Architecture & components

### 5.1 Pure mapping — `crates/oxitide-app/src/media_controls.rs` (new)

Cross-platform, no WinRT types, unit-tested.

```rust
pub enum PlayState { Playing, Paused, Stopped }

pub struct TrackMeta {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub cover: Option<String>,
}

pub enum SessionUpdate {
    /// `None` means nothing is playing: clear the display and stop.
    Track(Option<TrackMeta>),
    Playback(PlayState),
    Position { position_ms: u64, duration_ms: u64 },
}

pub enum ControlCommand { PlayPause, Next, Prev, Seek(u64) }

/// TIDAL CDN cover for a `QueueItem.cover` value (uuid or absolute url).
pub fn cover_url(cover: &str, size: u32) -> Option<String>;

/// Map one engine event into SMTC updates, threading the last track id so a
/// `State` that repeats the current track does not re-emit metadata.
pub fn plan(event: &EngineEvent, last_track: Option<u64>) -> (Vec<SessionUpdate>, Option<u64>);

pub fn to_engine_cmd(cmd: ControlCommand) -> EngineCmd;
```

Mapping rules (each `EngineEvent` is mapped independently; `plan` only threads the last track
id so metadata is not re-emitted):

- `NowPlaying(Some(item))` → `Track(Some(..))` only, and only when
  `item.track_id != last_track`. Playback status comes from the `State` events, which the
  engine emits alongside track changes.
- `NowPlaying(None)` → `Track(None)` + `Playback(Stopped)`.
- `State(s)` → `Track(Some(..))` only when `s.current.track_id != last_track`; `Playback`
  from `s.current`: `None` → `Stopped`, else `Playing`/`Paused` from `s.playing`.
- `Position { position_ms, duration_ms }` → `Position` with `position_ms` clamped to
  `duration_ms` (when `duration_ms > 0`).
- `Error` → no updates.

`cover_url` ports `artwork.ts:7`: empty → `None`; starts with `http` → pass through; otherwise
`https://resources.tidal.com/images/{uuid with '-' replaced by '/'}/{size}x{size}.jpg`.
The SMTC thumbnail uses `size = 320`.

### 5.2 Windows layer — `crates/oxitide-app/src/media_controls_win.rs` (new, `#[cfg(windows)]`)

Owns the worker thread and all WinRT objects. Public facade (re-exported from
`media_controls.rs`):

```rust
pub struct MediaSession { /* thread join handle + update sender + enable flag */ }

impl MediaSession {
    /// Spawns the worker. `hwnd` is the Tauri main window's handle; `enabled`
    /// is the persisted `integrations.smtc` value at build time.
    pub fn spawn(engine: Engine, hwnd: isize, enabled: bool) -> Result<Self, String>;
    /// Turns the OS session on/off without stopping the worker.
    pub fn set_enabled(&self, enabled: bool);
    /// Queues one update for the worker (non-blocking; dropped if the worker is gone).
    pub fn send(&self, update: SessionUpdate);
}
```

`Drop` signals shutdown and joins the worker.

Worker algorithm:

1. Initialize COM on the thread (`CoInitializeEx`, MTA) and return on failure.
2. Obtain SMTC:
   `windows::core::factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()`,
   then `unsafe { interop.GetForWindow(HWND(hwnd)) }`.
3. Configure once: `SetIsEnabled(true)`, `SetIsPlayEnabled(true)`, `SetIsPauseEnabled(true)`,
   `SetIsNextEnabled(true)`, `SetIsPreviousEnabled(true)`; register `ButtonPressed` and
   `PlaybackPositionChangeRequested`, translating to `ControlCommand` → `to_engine_cmd` →
   `engine.handle(..)` (log on error).
4. Loop: block on the channel with a short timeout; apply `SessionUpdate`s while enabled;
   on each pass, if the enable flag changed, call `SetIsEnabled(flag)` and, when turning on,
   re-apply the cached last track/playback/position; on timeout/idle run a non-blocking
   `PeekMessageW` pump so WinRT callbacks are delivered; exit on shutdown.
5. Teardown: `SetIsEnabled(false)` and `DisplayUpdater().ClearAll()`.

Applying updates:

- `Track(Some(t))` → set `DisplayUpdater` type `Music`, `MusicProperties` (title/artist/album),
  thumbnail if the cover URL resolves and streams, `Update()`. `Track(None)` → `ClearAll()`.
- `Playback(state)` → `SetPlaybackStatus(Playing | Paused | Stopped)`.
- `Position { p, d }` → build `SystemMediaTransportControlsTimelineProperties`
  (`StartTime` 0, `EndTime` = `d`, `Position` = `p`, `MinSeekTime` 0, `MaxSeekTime` = `d`) in
  100 ns ticks and `UpdateTimelineProperties(..)`.

Album art: build the URL with `cover_url(cover, 320)` on a `Track` update;
`RandomAccessStreamReference::CreateFromUri(&Uri::CreateUri(url)?)` then `SetThumbnail`.
Because `plan` emits `Track` only on a change, no per-tick refetch or cache is required. The
spike (Task 0) confirms the URI path; a file/stream fallback is only added if it fails.

### 5.3 Off-Windows stub — `crates/oxitide-app/src/media_controls.rs`

`#[cfg(not(windows))] pub struct MediaSession;` whose `spawn` returns `Ok` with a no-op
session, `set_enabled`/`send` do nothing, and `Drop` is a no-op, so `AppState`, commands, and
config compile on any target.

### 5.4 Config & IPC

- `crates/oxitide-app/src/config.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationsConfig {
    /// Windows System Media Transport Controls (media keys + now-playing).
    pub smtc: bool,
}

impl Default for IntegrationsConfig {
    fn default() -> Self { Self { smtc: true } }
}
```

  Add `pub integrations: IntegrationsConfig` to `Config`; update the `Config { .. }` literal in
  `save_and_load_round_trip`; assert the default in `partial_file_fills_defaults`; add
  `smtc_defaults_true_and_round_trips`.

- `crates/oxitide-app/src/state.rs`:

```rust
pub media: Mutex<Option<MediaSession>>,
```

  with `set_media` and `take_media` accessors: `build_player` stores the session,
  `replace_player`/`logout` take and drop it (dropping joins the worker).

- `crates/oxitide-app/src/commands.rs`:
  - In `build_player`, after the engine is configured and before subscribing, drop any previous
    session (`state.take_media()`); if `config.integrations.smtc` and the `"main"` window has an
    `hwnd()`, `MediaSession::spawn(engine.clone(), hwnd.0 as isize, true)` and store it via
    `state.set_media(..)`. Capture a cloned sender in the `engine.subscribe` closure and forward
    `plan(event, last_track)` updates (threading `last_track`) alongside the existing
    `playback_event` emit. Any failure logs a warning and playback continues.
  - `#[tauri::command] set_smtc(state, enabled) -> Result<(), String>`: persist
    `config.integrations.smtc`, save, `set_config`, then call `state.media()`'s
    `set_enabled(enabled)` when a session exists. Enabling without a session (e.g. before login)
    takes effect at the next `build_player`.
- `crates/oxitide-app/src/main.rs`: register `commands::set_smtc` in `generate_handler![]`.
- `crates/oxitide-app/Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.62", features = [
    "Media",
    "Foundation",
    "Storage_Streams",
    "Win32_Foundation",
    "Win32_System_WinRT",
    "Win32_System_Com",
    "Win32_UI_WindowsAndMessaging",
] }
```

  `Win32_System_WinRT` provides the `GetForWindow` interop, `Win32_System_Com` the
  `CoInitializeEx` call, and `Win32_UI_WindowsAndMessaging` the `PeekMessageW` pump.

### 5.5 Frontend

- `ui/src/lib/api.ts`: `Config.integrations: { smtc: boolean }`; `api.setSmtc(enabled)` →
  `invoke<void>("set_smtc", { enabled })`.
- `ui/src/views/Settings.svelte` (Connections tab): replace the disabled "Windows media
  controls" planned row (`:442`) with a real checkbox bound to `config.integrations.smtc`
  through a `setSmtc` handler (optimistic patch + `api.setSmtc(enabled).catch(apply)`), and
  reword the tab header so it no longer says "nothing to configure yet". Last.fm,
  ListenBrainz, and Discord Rich Presence remain planned placeholders (Tracks 5 and 6b).

## 6. Behavior specification

- **Track change:** flyout updates title/artist/album and art; timeline resets to the new
  duration; playback status reflects the current state.
- **Play/pause:** the flyout button and the hardware play/pause key both toggle playback via
  `PlayPause`; the flyout icon reflects `Playing`/`Paused`.
- **Next/previous:** the flyout buttons and hardware keys call `Next`/`Prev`.
- **Position:** the flyout scrubber advances from `Position` updates (≤ once per 500 ms); the
  value never exceeds the track duration.
- **Scrub:** dragging the flyout scrubber raises `PlaybackPositionChangeRequested`; OxiTide
  seeks with `Seek(position_ms)`.
- **Idle:** when nothing is playing (`NowPlaying(None)`), the display is cleared and status is
  `Stopped`.
- **Enable/disable:** toggling off calls `SetIsEnabled(false)` and clears the display; toggling
  on re-enables and re-applies the current track. No restart or re-login is needed. The choice
  persists.
- **Logout:** the session is torn down with the player.
- **Restart:** the flag is read from config; on login the session starts if enabled.

## 7. Error handling & degradation

- Interop/WinRT error at any point → `tracing::warn!`, disable the session, return; never
  panic or block; playback is unaffected.
- No `"main"` HWND (unexpected) → warn and skip; other features keep working.
- Channel send fails or is full → drop the update (only `Position` is high-frequency and it is
  already throttled).
- Cover fetch or stream construction fails → update metadata without a thumbnail; do not
  retry every tick (cache the attempted URL).
- Config save failure → surfaced by the Settings `apply()` toast, as with every setting.

## 8. Performance

One background thread that blocks on a channel when idle; when disabled it still parks but does
no SMTC work. No allocations on the audio thread. Metadata/art updates only on track change;
timeline updates reuse the engine's existing 500 ms cadence. Startup adds a thread spawn and an
off-main-thread SMTC init; it never blocks the UI. No new polling loops.

## 9. Testing & verification

**Unit (Rust, pure — `media_controls.rs`):**
- `cover_url`: empty → `None`; `http…` passthrough; dashed uuid → exploded CDN path at 320.
- `plan`: `NowPlaying(Some)` emits `Track(Some)`; repeated `State` for the same track emits no
  `Track`; `State` with `None` current → `Track(None)` + `Playback(Stopped)`; `State` playing vs
  paused; `Position` clamps to duration and when `duration_ms == 0`; `Error` emits nothing.
- `to_engine_cmd`: each `ControlCommand` maps to the expected `EngineCmd`.
- Timeline tick conversion: ms → 100 ns ticks.

**Rust (config):** `smtc_defaults_true_and_round_trips` (default true, `smtc = false`
round-trips); `partial_file_fills_defaults` asserts `config.integrations.smtc`.

**Checks:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`;
`npm --prefix ui run check`; `npm --prefix ui test`.

**Human GUI smoke** (`docs/superpowers/plans/notes/2026-09-29-smtc-smoke.md`): flyout shows
title / artist / album + art; play/pause/next/previous from the flyout and the hardware keys
(unfocused window); scrubber tracks and seeks; pause/resume icon; no-art / no-lyrics track;
mini player; toggle off removes the session and media keys stop working; toggle on restores it
without a restart; restart honors the flag; logout clears the flyout; app exit leaves no
orphaned session; WebView interaction and playback are unaffected.

## 10. Risks & open verifications

- **Interop construction.** The interop type is generated under `Win32::System::WinRT` and the
  call shape is proven by `souvlaki`; the spike (Task 0) confirms it in this crate and pins the
  exact `windows` 0.62 feature set before the implementation tasks.
- **Event delivery/pump.** `ButtonPressed` is confirmed to fire off the UI thread; the spike
  confirms a dedicated message loop on the worker receives events while the window is
  unfocused.
- **Thumbnail strategy.** `CreateFromUri` works for `souvlaki`; the spike confirms it renders
  in the flyout. A stream fallback is only added if it does not.
- **Media-key routing.** Windows routes media keys to the *active* SMTC session; if another
  player is active, keys may go there. This is standard OS behavior, not a bug; the smoke
  checklist documents the expected behavior rather than promising exclusivity.
- **windows-crate version.** `oxitide-audio` already pulls `windows` 0.62; the app crate must
  use the same major/minor to avoid duplicate-version friction.

## 11. Future / explicitly deferred

Discord Rich Presence (6b); Stop/Record buttons; shuffle/repeat through SMTC; macOS/Linux
equivalents (MPRIS / MPNowPlayingInfoCenter) behind the S-D9 stub; richer lock-screen art
fallbacks. None of these change the pure mapping API beyond adding `SessionUpdate` variants.
