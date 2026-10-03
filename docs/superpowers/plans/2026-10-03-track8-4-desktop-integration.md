# Linux Desktop Integration (Track 8.4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Linux build real desktop integration: MPRIS media controls over D-Bus (metadata, playback status, position + `Seeked`, Play/Pause/Next/Previous/Stop, Raise), platform-neutral Settings copy, verified `tidal://` registration in WSLg, the documented Wayland geometry degradation, and a recorded WebKitGTK CSS pass — with zero Windows behavior changes.

**Architecture:** `media_controls.rs` gains a small cfg-neutral MPRIS seam (`MprisState` fold, seek classification, control gating, `track_path`, all unit-tested on Windows). A new Linux-only `media_controls_linux.rs` is the only D-Bus code: it owns a current-thread tokio runtime plus `LocalSet`, builds a `mpris_server::Player`, connects the sync method callbacks to the engine through the existing `ControlCommand` seam, and paints `SessionUpdate`s into MPRIS properties from an async poll loop. The spawn dispatch stays in `commands.rs` (Windows arm byte-for-byte unchanged) and the existing `set_smtc` toggle drives whichever backend was compiled. Deep links, Wayland behavior, and the WebKit CSS floor are verified and documented, not re-implemented.

**Tech Stack:** Rust (Tauri 2, tokio current-thread runtime + `LocalSet`, `mpris-server` 0.10/zbus 5 on Linux only), Svelte 5 runes, TypeScript, vitest, `playerctl`/`xdg-mime`/`gdbus` in WSLg.

**Spec:** `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md`

## Global Constraints

- Windows-first: every Linux runtime addition is `#[cfg(target_os = "linux")]` or under `[target.'cfg(target_os = "linux")'.dependencies]`. `media_controls_win.rs`, `scheme.rs`, and Windows `window.rs` behavior are unchanged.
- No CI: every check is manual. Windows gates after every Rust task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- MPRIS code compiles only on Linux. WSL2 Ubuntu 22.04 (the Plan 8.1 environment from `docs/linux-build.md`) is the compile/lint gate: `cargo build -p oxitide-app` and `cargo clippy --workspace -- -D warnings`.
- No new Windows dependencies: `mpris-server` is a Linux target dependency only.
- Never log token values; no token code is in scope. HTTP tests (none needed here) use `wiremock` and never hit the live TIDAL API.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case. This plan adds no `#[tauri::command]`, so `generate_handler![...]` is unchanged.
- **Track 9 UI freeze gate:** Tasks 3 and 5 touch `ui/`. Do not start them until the owner declares the Track 9 UI freeze and the in-flight UI edits (`ui/src/App.svelte`, `ui/src/app.css`) are committed. Tasks 1, 2, and 4 are Rust/docs only and may proceed before the freeze.
- One commit per task, exact messages below. If the owner prefers to commit themselves, stop after each task's green run instead.

## Review Focus

Most likely failure classes for this slice, most likely first, each pinned by a named check in its owning task:

1. **Stale metadata on track change.** Every `SessionUpdate::Track` must replace title/artist/album/art/length (and reset position) in MPRIS; a missed update leaves the previous track in the desktop widget. Pinned by `mpris_state_track_change_replaces_metadata_and_resets_position` (Task 1) and the playerctl track-change check (Task 4 smoke).
2. **Seek position jumps.** MPRIS `Seek` carries a relative offset and `SetPosition` an absolute one guarded by `mpris:trackid`; both must clamp to `[0, duration]`, optimistically update the neutral state so a second seek never computes from a stale position, and emit `Seeked` only for real jumps. Pinned by `mpris_seek_target_clamps_to_the_track` and the seek assertions in `mpris_state_flags_external_seeks_but_not_progress_or_pauses` (Task 1), plus `playerctl position` checks (Task 4 smoke).
3. **Control commands racing playback.** MPRIS `Play`/`Pause`/`Stop` are separate methods; a blind `PlayPause` would invert state when the engine already moved. Gating is pure and testable. Pinned by `mpris_controls_gate_on_playback_state` (Task 1) and rapid `playerctl play-pause`/`stop` (Task 4 smoke).
4. **Missing desktop entry / handler in WSLg.** `register_all()` must leave an `x-scheme-handler/tidal` desktop entry that `xdg-mime` resolves, the Plan 8.3 `.desktop` must carry the trailing-semicolon `MimeType`, and Windows `scheme.rs` must stay untouched. Pinned by the Task 4 commands (`grep -l`, `xdg-mime query default`, `git diff -- scheme.rs`).
5. **WebKit CSS feature fallback.** A WebKitGTK floor may not parse `@property`/`color-mix` or may disable compositing (`backdrop-filter`), leaving invalid `--accent` values or missing glass. Pinned by the Task 5 feature-probe snippet and the recorded observations; fixes only where observed.

---

## File Structure

**Rust**

- Modify `crates/oxitide-app/src/media_controls.rs` — cfg-neutral MPRIS seam plus Linux module dispatch.
- Create `crates/oxitide-app/src/media_controls_linux.rs` — MPRIS adapter (the only D-Bus code).
- Modify `crates/oxitide-app/Cargo.toml` — `mpris-server` under the Linux target dependency only.
- Modify `crates/oxitide-app/src/commands.rs` — Linux spawn arm in `build_player`.
- Modify `crates/oxitide-app/src/window.rs` — only if the Task 4 WSLg Wayland check observes a mini-exit error (guard code provided).

**UI (after the Track 9 UI freeze)**

- Modify `ui/src/views/Settings.svelte` — platform-neutral media-controls copy.
- Modify `ui/src/styles/tokens.css` and/or `ui/src/app.css` (and component styles) — only for failures observed in Task 5.

**Docs**

- Create `docs/superpowers/plans/notes/2026-10-03-track8-4-linux-desktop-smoke.md` — WSLg items, deferred real-DE items, CSS findings (Task 4; updated by Task 5).

---

### Task 1: Platform-neutral MPRIS seam (unit-tested on Windows)

**Files:**
- Modify: `crates/oxitide-app/src/media_controls.rs` (seam after `to_engine_cmd`; tests appended to the existing `mod tests`)

**Interfaces:**
- Consumes: `PlayState`, `TrackMeta`, `SessionUpdate`, `ControlCommand` (all already in the file).
- Produces: `MprisState { track: Option<TrackMeta>, status: PlayState, position_ms: u64, duration_ms: u64 }`; `MprisEffect::{Track, Playback, Position { seeked }}`; `apply_session_update(&mut MprisState, &SessionUpdate, elapsed_ms: u64) -> MprisEffect`; private `is_seek(PlayState, u64, u64, u64) -> bool`; `control_for_play/pause/stop(&MprisState) -> Option<ControlCommand>`; `seek_target(&MprisState, i64) -> u64`; `track_path(u64) -> String`. Every produced item carries `#[cfg(any(target_os = "linux", test))]`: on Windows those items are compiled only for tests, so `cargo clippy --workspace -- -D warnings` (no `--tests`) never sees them as dead code.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/oxitide-app/src/media_controls.rs` (after `control_commands_map_to_engine_commands`):

```rust
    fn meta(track_id: u64) -> TrackMeta {
        TrackMeta {
            track_id,
            title: format!("Title {track_id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: 200_000,
            cover: Some("x".into()),
        }
    }

    #[test]
    fn mpris_state_folds_track_playback_and_position() {
        let mut snapshot = MprisState::default();
        assert_eq!(
            apply_session_update(&mut snapshot, &SessionUpdate::Track(Some(meta(7))), 0),
            MprisEffect::Track
        );
        assert_eq!(snapshot.track.as_ref().unwrap().track_id, 7);
        assert_eq!(snapshot.duration_ms, 200_000);
        assert_eq!(snapshot.position_ms, 0);

        assert_eq!(
            apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Playing), 0),
            MprisEffect::Playback
        );
        assert_eq!(snapshot.status, PlayState::Playing);

        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 500, duration_ms: 200_000 },
                500,
            ),
            MprisEffect::Position { seeked: false }
        );
        assert_eq!(snapshot.position_ms, 500);
    }

    #[test]
    fn mpris_state_track_change_replaces_metadata_and_resets_position() {
        let mut snapshot = MprisState::default();
        apply_session_update(&mut snapshot, &SessionUpdate::Track(Some(meta(1))), 0);
        apply_session_update(
            &mut snapshot,
            &SessionUpdate::Position { position_ms: 60_000, duration_ms: 200_000 },
            0,
        );
        assert_eq!(
            apply_session_update(&mut snapshot, &SessionUpdate::Track(Some(meta(2))), 0),
            MprisEffect::Track
        );
        let track = snapshot.track.expect("track");
        assert_eq!(track.track_id, 2);
        assert_eq!(snapshot.position_ms, 0);
        assert_eq!(snapshot.duration_ms, 200_000);
    }

    #[test]
    fn mpris_state_flags_external_seeks_but_not_progress_or_pauses() {
        let mut snapshot = MprisState::default();
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Playing), 0);
        apply_session_update(
            &mut snapshot,
            &SessionUpdate::Position { position_ms: 10_000, duration_ms: 200_000 },
            0,
        );
        // 600 ms later, 10.6 s of progress is not a seek.
        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 10_600, duration_ms: 200_000 },
                600,
            ),
            MprisEffect::Position { seeked: false }
        );
        // A 50 s jump forward while playing is an external seek.
        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 60_000, duration_ms: 200_000 },
                600,
            ),
            MprisEffect::Position { seeked: true }
        );
        // A paused gap resumes where it left off: not a seek.
        apply_session_update(&mut snapshot, &SessionUpdate::Playback(PlayState::Paused), 0);
        assert_eq!(
            apply_session_update(
                &mut snapshot,
                &SessionUpdate::Position { position_ms: 60_000, duration_ms: 200_000 },
                5_000,
            ),
            MprisEffect::Position { seeked: false }
        );
    }

    #[test]
    fn mpris_seek_target_clamps_to_the_track() {
        let snapshot = MprisState {
            track: None,
            status: PlayState::Playing,
            position_ms: 30_000,
            duration_ms: 200_000,
        };
        assert_eq!(seek_target(&snapshot, -45_000), 0);
        assert_eq!(seek_target(&snapshot, 10_000), 40_000);
        assert_eq!(seek_target(&snapshot, 500_000), 200_000);
        let unknown = MprisState { duration_ms: 0, ..snapshot };
        assert_eq!(seek_target(&unknown, 500_000), 530_000);
    }

    #[test]
    fn mpris_controls_gate_on_playback_state() {
        let playing = MprisState { status: PlayState::Playing, ..Default::default() };
        let paused = MprisState { status: PlayState::Paused, ..Default::default() };
        assert_eq!(control_for_play(&playing), None);
        assert_eq!(control_for_pause(&playing), Some(ControlCommand::PlayPause));
        assert_eq!(control_for_stop(&playing), Some(ControlCommand::PlayPause));
        assert_eq!(control_for_play(&paused), Some(ControlCommand::PlayPause));
        assert_eq!(control_for_pause(&paused), None);
        assert_eq!(control_for_stop(&paused), None);
    }

    #[test]
    fn mpris_track_path_is_a_valid_dbus_object_path() {
        assert_eq!(track_path(7), "/com/oxitide/track/t7");
        assert_eq!(track_path(u64::MAX), "/com/oxitide/track/t18446744073709551615");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-app mpris_`

Expected: compile error (`cannot find type MprisState`, `cannot find function apply_session_update`) — the failing state.

- [ ] **Step 3: Implement**

Give `PlayState` a default so `MprisState` can derive `Default`. Replace:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}
```

with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    Playing,
    Paused,
    #[default]
    Stopped,
}
```

Then add after `to_engine_cmd` (before `#[cfg(not(windows))] mod stub`):

```rust
/// The MPRIS-facing playback snapshot. Platform-neutral so the fold below is
/// unit-tested on Windows; only the Linux adapter turns it into D-Bus calls.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MprisState {
    pub track: Option<TrackMeta>,
    pub status: PlayState,
    pub position_ms: u64,
    pub duration_ms: u64,
}

/// What the adapter must push after folding one session update.
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MprisEffect {
    /// Metadata (or its clearing) changed.
    Track,
    /// Playback status changed.
    Playback,
    /// Position advanced; `seeked` is true when it jumped unexpectedly.
    Position { seeked: bool },
}

/// Position updates arrive about every 500 ms while playing; a jump larger
/// than this from the expected progression is an external seek.
#[cfg(any(target_os = "linux", test))]
const SEEK_TOLERANCE_MS: u64 = 1_500;

/// Whether a position update jumped (an external seek) rather than progressed.
#[cfg(any(target_os = "linux", test))]
fn is_seek(previous: PlayState, previous_ms: u64, elapsed_ms: u64, incoming_ms: u64) -> bool {
    previous == PlayState::Playing
        && incoming_ms.abs_diff(previous_ms.saturating_add(elapsed_ms)) > SEEK_TOLERANCE_MS
}

/// Fold one neutral session update into the snapshot and report what changed.
#[cfg(any(target_os = "linux", test))]
pub fn apply_session_update(
    state: &mut MprisState,
    update: &SessionUpdate,
    elapsed_ms: u64,
) -> MprisEffect {
    match update {
        SessionUpdate::Track(track) => {
            state.track = track.clone();
            state.position_ms = 0;
            state.duration_ms = track.as_ref().map_or(0, |track| track.duration_ms);
            MprisEffect::Track
        }
        SessionUpdate::Playback(play) => {
            state.status = *play;
            MprisEffect::Playback
        }
        SessionUpdate::Position { position_ms, duration_ms } => {
            let seeked = is_seek(state.status, state.position_ms, elapsed_ms, *position_ms);
            state.position_ms = *position_ms;
            state.duration_ms = *duration_ms;
            MprisEffect::Position { seeked }
        }
    }
}

/// MPRIS `Play`: no-op unless paused or stopped.
#[cfg(any(target_os = "linux", test))]
pub fn control_for_play(state: &MprisState) -> Option<ControlCommand> {
    (state.status != PlayState::Playing).then_some(ControlCommand::PlayPause)
}

/// MPRIS `Pause`: no-op unless playing.
#[cfg(any(target_os = "linux", test))]
pub fn control_for_pause(state: &MprisState) -> Option<ControlCommand> {
    (state.status == PlayState::Playing).then_some(ControlCommand::PlayPause)
}

/// MPRIS `Stop`: the engine has no separate stop state, so stop pauses.
#[cfg(any(target_os = "linux", test))]
pub fn control_for_stop(state: &MprisState) -> Option<ControlCommand> {
    (state.status == PlayState::Playing).then_some(ControlCommand::PlayPause)
}

/// MPRIS relative `Seek` offset -> absolute engine target, clamped to the
/// track bounds when the duration is known.
#[cfg(any(target_os = "linux", test))]
pub fn seek_target(state: &MprisState, offset_ms: i64) -> u64 {
    let target = (state.position_ms as i64).saturating_add(offset_ms).max(0) as u64;
    if state.duration_ms > 0 {
        target.min(state.duration_ms)
    } else {
        target
    }
}

/// D-Bus object path for `mpris:trackid`. Object-path elements may not start
/// with a digit, hence the `t` prefix.
#[cfg(any(target_os = "linux", test))]
pub fn track_path(track_id: u64) -> String {
    format!("/com/oxitide/track/t{track_id}")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-app mpris_`

Expected: PASS (6 tests). Then run `cargo clippy --workspace -- -D warnings` and expect no warnings — this proves the cfg gates keep Windows free of dead-code warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/media_controls.rs
git commit -m "feat(media): add the platform-neutral MPRIS state seam"
```

---

### Task 2: Linux MPRIS adapter and spawn dispatch

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml` (add the Linux target dependency)
- Modify: `crates/oxitide-app/src/media_controls.rs` (module + re-export dispatch)
- Create: `crates/oxitide-app/src/media_controls_linux.rs` (full file below)
- Modify: `crates/oxitide-app/src/commands.rs` (`build_player` Linux arm)

**Interfaces:**
- Consumes: the Task 1 seam, `SessionSender`, `plan`, `to_engine_cmd`, `Engine`, `tauri::AppHandle`.
- Produces: `MediaSession::spawn(engine: Engine, enabled: bool, raise: tauri::AppHandle) -> Result<Self, String>`, `MediaSession::sender() -> SessionSender`, `MediaSession::set_enabled(bool)` on Linux. `commands::set_smtc` toggles it with no changes.

- [ ] **Step 1: Pin the Linux dependency**

In `crates/oxitide-app/Cargo.toml`, append after the existing `[target.'cfg(windows)'.dependencies]` block:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
mpris-server = { version = "0.10", default-features = false, features = ["tokio"] }
```

Version note: `mpris-server` 0.10.0 is the current release; its docs.rs build failed for environment reasons (a `futures-lite` docs-only feature and a zvariant/serde pairing), **not** a released-crate compile failure. The API used below (`Player::builder`, `can_play/can_pause/can_go_previous/can_go_next`, `build().await`, `connect_*`, `set_identity`, `set_desktop_entry`, `set_can_raise`, `set_can_seek`, `set_metadata`, `set_playback_status`, `set_position`, `seeked`, `run`, `Metadata::builder`, `TrackId`, `Time`) was read from the 0.10.0 source and matches the author's current tokio example; the 0.9 docs (`https://docs.rs/mpris-server/0.9.0/`) have the identical surface if a symbol lookup is needed. If `cargo build` rejects a symbol, the errors are confined to `media_controls_linux.rs` (Task 2, Step 3) — fix the adapter, not the seam.

On WSL2, confirm resolution before writing code:

```bash
cd /mnt/c/Users/V/Documents/Opencode/OxiTide
cargo tree -p oxitide-app -i mpris-server
```

Expected: a single `mpris-server v0.10.x` node with `zbus v5.x` under it. There is no `Cargo.lock` entry yet; cargo adds it (the repo tracks `Cargo.lock`, so it is part of this task's commit on Linux).

- [ ] **Step 2: Add the dispatch in `media_controls.rs`**

Replace the module/ re-export header (currently lines 6–14):

```rust
#[cfg(windows)]
#[path = "media_controls_win.rs"]
mod win;

#[cfg(windows)]
pub use win::MediaSession;

#[cfg(not(windows))]
pub use stub::MediaSession;
```

with:

```rust
#[cfg(windows)]
#[path = "media_controls_win.rs"]
mod win;

#[cfg(target_os = "linux")]
#[path = "media_controls_linux.rs"]
mod linux;

#[cfg(windows)]
pub use win::MediaSession;

#[cfg(target_os = "linux")]
pub use linux::MediaSession;

#[cfg(not(any(windows, target_os = "linux")))]
pub use stub::MediaSession;
```

and change the stub module's attribute at the bottom of the file:

```rust
#[cfg(not(windows))]
mod stub {
```

to:

```rust
#[cfg(not(any(windows, target_os = "linux")))]
mod stub {
```

- [ ] **Step 3: Create the Linux adapter**

Create `crates/oxitide-app/src/media_controls_linux.rs`:

```rust
//! Linux MPRIS media controls (spec §4.3). This is the only D-Bus code; the
//! session-update fold, seek classification, and control gating live in
//! `media_controls.rs` so they are unit-tested on Windows.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use mpris_server::zbus;
use mpris_server::{Metadata, PlaybackStatus, Player, Time, TrackId};
use oxitide_audio::engine::Engine;

use super::{
    apply_session_update, control_for_pause, control_for_play, control_for_stop, cover_url,
    seek_target, to_engine_cmd, track_path, ControlCommand, MprisEffect, MprisState, PlayState,
    SessionSender, SessionUpdate, TrackMeta,
};

const CHANNEL_CAPACITY: usize = 64;
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const THUMBNAIL_SIZE: u32 = 320;
const BUS_NAME_SUFFIX: &str = "OxiTide";
const IDENTITY: &str = "OxiTide";

/// Adapter-originated MPRIS signals, drained by the async loop so the sync
/// method callbacks never need to await.
enum AdapterEffect {
    Seeked(u64),
}

pub struct MediaSession {
    tx: Option<SyncSender<SessionUpdate>>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MediaSession {
    pub fn spawn(engine: Engine, enabled: bool, raise: tauri::AppHandle) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<SessionUpdate>(CHANNEL_CAPACITY);
        let flag = Arc::new(AtomicBool::new(enabled));
        let worker_flag = flag.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_shutdown = shutdown.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-mpris".into())
            .spawn(move || worker(engine, raise, rx, worker_flag, worker_shutdown))
            .map_err(|e| e.to_string())?;
        Ok(Self { tx: Some(tx), enabled: flag, shutdown, handle: Some(handle) })
    }

    pub fn sender(&self) -> SessionSender {
        SessionSender::new(self.tx.as_ref().expect("live sender").clone())
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn worker(
    engine: Engine,
    raise: tauri::AppHandle,
    rx: Receiver<SessionUpdate>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(e) => {
            tracing::warn!("MPRIS disabled: {e}");
            enabled.store(false, Ordering::Relaxed);
            return;
        }
    };
    let failure = enabled.clone();
    let serve = tokio::task::LocalSet::new().run_until(serve(engine, raise, rx, enabled, shutdown));
    if let Err(e) = runtime.block_on(serve) {
        tracing::warn!("MPRIS stopped: {e}");
        failure.store(false, Ordering::Relaxed);
    }
}

async fn serve(
    engine: Engine,
    raise: tauri::AppHandle,
    rx: Receiver<SessionUpdate>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
) -> zbus::Result<()> {
    let player = Player::builder(BUS_NAME_SUFFIX)
        .can_play(true)
        .can_pause(true)
        .can_go_previous(true)
        .can_go_next(true)
        .build()
        .await?;
    let state = Arc::new(Mutex::new(MprisState::default()));
    let (effect_tx, effect_rx) = mpsc::channel::<AdapterEffect>();
    connect(&player, &engine, &raise, &state, &effect_tx);
    tokio::task::spawn_local(player.run());
    player.set_identity(IDENTITY).await?;
    player.set_desktop_entry(IDENTITY).await?;
    player.set_can_raise(true).await?;
    player.set_can_seek(true).await?;

    let mut shown = true;
    let mut last_position_at = Instant::now();
    while !shutdown.load(Ordering::Relaxed) {
        let want = enabled.load(Ordering::Relaxed);
        if want != shown {
            set_enabled(&player, want, &state).await;
            shown = want;
            last_position_at = Instant::now();
        }
        while let Ok(update) = rx.try_recv() {
            let elapsed_ms = last_position_at.elapsed().as_millis() as u64;
            let effect = {
                let mut snapshot = state.lock().unwrap();
                apply_session_update(&mut snapshot, &update, elapsed_ms)
            };
            last_position_at = Instant::now();
            if want {
                push_effect(&player, &state, effect).await?;
            }
        }
        while let Ok(effect) = effect_rx.try_recv() {
            if want {
                match effect {
                    AdapterEffect::Seeked(position_ms) => {
                        let position = Time::from_millis(position_ms as i64);
                        player.set_position(position);
                        player.seeked(position).await?;
                    }
                }
            }
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    Ok(())
}

async fn push_effect(
    player: &Player,
    state: &Mutex<MprisState>,
    effect: MprisEffect,
) -> zbus::Result<()> {
    match effect {
        MprisEffect::Track => {
            let metadata = {
                let snapshot = state.lock().unwrap();
                match &snapshot.track {
                    Some(track) => track_metadata(track),
                    None => empty_metadata(),
                }
            };
            player.set_metadata(metadata).await?;
            player.set_position(Time::ZERO);
            player.seeked(Time::ZERO).await?;
        }
        MprisEffect::Playback => {
            let status = {
                let snapshot = state.lock().unwrap();
                status_of(snapshot.status)
            };
            player.set_playback_status(status).await?;
        }
        MprisEffect::Position { seeked } => {
            let position = {
                let snapshot = state.lock().unwrap();
                Time::from_millis(snapshot.position_ms as i64)
            };
            player.set_position(position);
            if seeked {
                player.seeked(position).await?;
            }
        }
    }
    Ok(())
}

async fn set_enabled(player: &Player, enabled: bool, state: &Mutex<MprisState>) {
    let _ = player.set_can_play(enabled).await;
    let _ = player.set_can_pause(enabled).await;
    let _ = player.set_can_seek(enabled).await;
    let _ = player.set_can_go_next(enabled).await;
    let _ = player.set_can_go_previous(enabled).await;
    if enabled {
        let snapshot = state.lock().unwrap().clone();
        let metadata = match &snapshot.track {
            Some(track) => track_metadata(track),
            None => empty_metadata(),
        };
        let _ = player.set_metadata(metadata).await;
        let _ = player.set_playback_status(status_of(snapshot.status)).await;
        player.set_position(Time::from_millis(snapshot.position_ms as i64));
    } else {
        let _ = player.set_metadata(empty_metadata()).await;
        let _ = player.set_playback_status(PlaybackStatus::Stopped).await;
    }
}

fn connect(
    player: &Player,
    engine: &Engine,
    raise: &tauri::AppHandle,
    state: &Arc<Mutex<MprisState>>,
    effects: &mpsc::Sender<AdapterEffect>,
) {
    {
        let engine = engine.clone();
        player.connect_play_pause(move |_| {
            let _ = engine.handle(to_engine_cmd(ControlCommand::PlayPause));
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        player.connect_play(move |_| dispatch_gated(&engine, &state, control_for_play));
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        player.connect_pause(move |_| dispatch_gated(&engine, &state, control_for_pause));
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        player.connect_stop(move |_| dispatch_gated(&engine, &state, control_for_stop));
    }
    {
        let engine = engine.clone();
        player.connect_next(move |_| {
            let _ = engine.handle(to_engine_cmd(ControlCommand::Next));
        });
    }
    {
        let engine = engine.clone();
        player.connect_previous(move |_| {
            let _ = engine.handle(to_engine_cmd(ControlCommand::Prev));
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        let effects = effects.clone();
        player.connect_seek(move |_player, offset| {
            let target = {
                let snapshot = state.lock().unwrap();
                seek_target(&snapshot, offset.as_millis())
            };
            issue_seek(&engine, &state, &effects, target);
        });
    }
    {
        let engine = engine.clone();
        let state = state.clone();
        let effects = effects.clone();
        player.connect_set_position(move |_player, track_id, position| {
            let current = {
                let snapshot = state.lock().unwrap();
                snapshot.track.as_ref().map(|track| track.track_id)
            };
            if current.map(track_path).as_deref() != Some(track_id.as_str()) {
                return;
            }
            issue_seek(&engine, &state, &effects, position.as_millis().max(0) as u64);
        });
    }
    {
        let raise = raise.clone();
        player.connect_raise(move |_| {
            let app = raise.clone();
            let _ = app.run_on_main_thread(move || {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });
        });
    }
}

fn dispatch_gated(
    engine: &Engine,
    state: &Mutex<MprisState>,
    pick: fn(&MprisState) -> Option<ControlCommand>,
) {
    let command = {
        let snapshot = state.lock().unwrap();
        pick(&snapshot)
    };
    if let Some(command) = command {
        let _ = engine.handle(to_engine_cmd(command));
    }
}

fn issue_seek(
    engine: &Engine,
    state: &Mutex<MprisState>,
    effects: &mpsc::Sender<AdapterEffect>,
    target_ms: u64,
) {
    {
        let mut snapshot = state.lock().unwrap();
        snapshot.position_ms = target_ms;
    }
    let _ = engine.handle(to_engine_cmd(ControlCommand::Seek(target_ms)));
    let _ = effects.send(AdapterEffect::Seeked(target_ms));
}

fn track_metadata(track: &TrackMeta) -> Metadata {
    let mut builder = Metadata::builder()
        .trackid(track_id(track.track_id))
        .title(track.title.clone())
        .artist([track.artist.clone()])
        .album(track.album.clone())
        .length(Time::from_millis(track.duration_ms as i64));
    if let Some(url) =
        track.cover.as_deref().and_then(|cover| cover_url(cover, THUMBNAIL_SIZE))
    {
        builder = builder.art_url(url);
    }
    builder.build()
}

fn empty_metadata() -> Metadata {
    Metadata::builder().trackid(TrackId::NO_TRACK).build()
}

fn track_id(track_id: u64) -> TrackId {
    TrackId::try_from(track_path(track_id)).unwrap_or(TrackId::NO_TRACK)
}

fn status_of(state: PlayState) -> PlaybackStatus {
    match state {
        PlayState::Playing => PlaybackStatus::Playing,
        PlayState::Paused => PlaybackStatus::Paused,
        PlayState::Stopped => PlaybackStatus::Stopped,
    }
}
```

- [ ] **Step 4: Wire the Linux spawn arm**

In `crates/oxitide-app/src/commands.rs` inside `build_player`, replace:

```rust
    #[cfg(not(windows))]
    let smtc_sender: Option<crate::media_controls::SessionSender> = None;
```

with:

```rust
    #[cfg(target_os = "linux")]
    let smtc_sender = {
        let mut found = None;
        match crate::media_controls::MediaSession::spawn(
            engine.clone(),
            config.integrations.smtc,
            app.clone(),
        ) {
            Ok(session) => {
                found = Some(session.sender());
                state.set_media(session);
            }
            Err(e) => tracing::warn!("MPRIS disabled: {e}"),
        }
        found
    };
    #[cfg(not(any(windows, target_os = "linux")))]
    let smtc_sender: Option<crate::media_controls::SessionSender> = None;
```

The Windows block above it is untouched; the engine fan-out, `set_smtc`, and `state.take_media()` all keep working through the shared `SessionSender`/`set_enabled` shape.

- [ ] **Step 5: Windows gates**

Run:

```powershell
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

Expected: all pass, 0 warnings. On Windows the adapter is not compiled; only the Task 1 seam and its tests are.

- [ ] **Step 6: WSL2 gates (compile the Linux code)**

In WSL2 Ubuntu 22.04, from the repository root (`docs/linux-build.md` has the setup):

```bash
cargo build -p oxitide-app
cargo clippy --workspace -- -D warnings
```

Expected: both succeed; clippy has 0 warnings. This is the only place `media_controls_linux.rs` is compiled. Fix any adapter-only issues here (symbol drift from the version note, unused imports) without touching Windows code.

- [ ] **Step 7: Commit**

```bash
git add crates/oxitide-app/Cargo.toml Cargo.lock crates/oxitide-app/src/media_controls.rs crates/oxitide-app/src/media_controls_linux.rs crates/oxitide-app/src/commands.rs
git commit -m "feat(media): add Linux MPRIS media controls"
```

On Windows the `Cargo.lock` change (a Linux-only dependency entry) is committed by whoever runs the WSL2 step; if `Cargo.lock` cannot be regenerated on the current host, commit it in WSL2 with the same message.

---

### Task 3: Platform-neutral Settings copy (after the Track 9 UI freeze)

**Files:**
- Modify: `ui/src/views/Settings.svelte` (label at line 651, description at line 653)

**Interfaces:**
- Consumes: nothing (copy only; `config.integrations.smtc` and `api.setSmtc` keep their names).
- Produces: user-visible "Media controls" wording on both platforms.

- [ ] **Step 1: Check for string references**

Run:

```powershell
Select-String -Path ui\src -Pattern "Windows media controls" -Recurse
```

Expected: no matches in `ui/` (the only repo hits are historical specs under `docs/superpowers/`), so no test or snapshot needs updating. If a hit appears in `ui/`, update that file in this task as well.

- [ ] **Step 2: Make the copy platform-neutral**

Replace:

```svelte
            <span>Windows media controls</span>
          </label>
          <span class="muted">Hardware media keys and the Windows media overlay (SMTC).</span>
```

with:

```svelte
            <span>Media controls</span>
          </label>
          <span class="muted">Hardware media keys and the system media overlay.</span>
```

- [ ] **Step 3: Verify**

Run: `npm --prefix ui run check`

Expected: 0 errors.

Run: `npm --prefix ui test`

Expected: PASS (no suite references the old copy).

- [ ] **Step 4: Commit**

```bash
git add ui/src/views/Settings.svelte
git commit -m "feat(ui): make the media controls copy platform-neutral"
```

---

### Task 4: WSLg deep-link, Wayland, and playerctl verification

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-03-track8-4-linux-desktop-smoke.md` (content in Step 1)
- Modify (only if Step 4 observes it): `crates/oxitide-app/src/window.rs` (guard in Step 5)

**Interfaces:**
- Consumes: the installed app (Plan 8.3's `tauri.linux.conf.json` / deb entry), `main.rs:251` `register_all()`, the Task 2 MPRIS adapter.
- Produces: a committed checklist with recorded results, plus the Wayland finding (guard applied or explicitly not needed).

- [ ] **Step 1: Create the smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-03-track8-4-linux-desktop-smoke.md` with exactly:

~~~markdown
# Track 8.4 Linux desktop integration smoke checklist (2026-10-03)

WSLg-verifiable checks for Plan 8.4 (MPRIS, Settings copy, app launch, deep
links, Wayland geometry) plus the deferred real-DE items. Companions:
`2026-10-03-track8-wslg-smoke.md` (umbrella WSLg run),
`2026-10-03-track8-bare-metal-checklist.md` (Track 8 closure gate), and the
Track 8 spec §4.3–§4.6.

Environment: WSL2 Ubuntu 22.04 (Plan 8.1, `docs/linux-build.md`), WSLg, a
session D-Bus. Install once:

```bash
sudo apt install -y playerctl xdg-utils desktop-file-utils
```

Record each item as PASS/FAIL/BLOCKED with date and evidence. FAIL becomes an
issue; do not silently skip.

## App launch and Settings copy (Task 3)

- [ ] Launch the app (`npm --prefix ui run app:dev` with Vite warm, or the
      installed deb from Plan 8.3) under WSLg: the window renders and no
      GTK/WebKit init errors appear in the terminal.
- [ ] Settings → Connections shows **Media controls** (no "Windows" wording)
      and the platform-neutral description.
- [ ] Uncheck Media controls → `playerctl -l` no longer lists the player (or
      lists it Stopped with empty metadata); re-check → it returns without a
      restart. Playback is unaffected either way.

## MPRIS via playerctl (spec §4.3)

- [ ] `playerctl -l` lists `org.mpris.MediaPlayer2.OxiTide`.
- [ ] `playerctl status` reflects Playing/Paused; `playerctl metadata` shows
      `xesam:title`, `xesam:artist`, `xesam:album`, `mpris:length`, and an
      `mpris:artUrl` on `resources.tidal.com`.
- [ ] Track change (next or auto-advance): metadata flips to the new track;
      no stale title/art (Review Focus 1).
- [ ] `playerctl position` advances during playback; `playerctl position 30`
      seeks and the next reading is ~30 s (no backward jump). Then
      `playerctl position 60` while playing and let the track end: `Seeked`
      is not emitted during normal progress (Review Focus 2).
- [ ] `playerctl play-pause`, `playerctl play`, `playerctl pause`,
      `playerctl next`, `playerctl previous`, `playerctl stop` all drive the
      app. Rapid `play-pause` then `stop` does not desync the widget
      (Review Focus 3).
- [ ] Raise focuses from another WSLg window:
      `gdbus call --session --dest org.mpris.MediaPlayer2.OxiTide --object-path /org/mpris/MediaPlayer2 --method org.mpris.MediaPlayer2.Raise`
- [ ] Clearing the queue / stopping playback clears the metadata instead of
      leaving the last track.
- [ ] Quit while playing: no hang and the bus name disappears
      (`playerctl -l` no longer shows it).

## Deep links in WSLg (spec §4.4; requires Plan 8.3)

- [ ] After the first run, the runtime registration (`register_all()`,
      `crates/oxitide-app/src/main.rs:251`) leaves a desktop entry carrying
      `MimeType=x-scheme-handler/tidal;`:
      `grep -rl "x-scheme-handler/tidal" ~/.local/share/applications/*.desktop`
- [ ] Installed deb: `grep -qx 'MimeType=x-scheme-handler/tidal;' /usr/share/applications/OxiTide.desktop`
      after `sudo apt install ./target/release/bundle/deb/*.deb`.
- [ ] `xdg-mime query default x-scheme-handler/tidal` returns the OxiTide
      entry (Plan 8.3 requires `xdg-utils` + `desktop-file-utils`).
- [ ] Best effort: `xdg-open 'tidal://login/auth?code=smoke'` reaches the app
      under WSLg. Record the result either way; full browser-redirect +
      single-instance verification stays deferred.
- [ ] Windows snapshot untouched:
      `git diff -- crates/oxitide-app/src/scheme.rs` is empty (the HKCU
      registry guard is Windows-only; Linux uses the `.desktop` handler).

## Wayland geometry (spec §4.5)

- [ ] Record the session: `echo "$XDG_SESSION_TYPE $WAYLAND_DISPLAY"` (WSLg
      is Wayland).
- [ ] Size restore and entering/exiting mini still work. **If** mini exit
      shows an error toast (i.e. `set_position` rejects on Wayland and
      `apply_geometry` propagates it), apply the Task 4 guard in
      `window.rs` and re-run the Windows gates; record which happened.
- [ ] Saved-position restore degrades to a compositor-placed window; no
      crash, no off-screen window. Under X11 (bare metal) both restore fully.

## WebKit CSS findings (spec §4.6; Track 9 freeze; Plan Task 5)

- WebKitGTK version (`dpkg-query -W -f='${Version}\n' libwebkit2gtk-4.1-0`):
- `CSS.supports("color", "color-mix(in srgb, red 50%, blue)")`:
- `CSS.supports("backdrop-filter", "blur(2px)")` / `-webkit-` prefix:
- `CSSPropertyRule` present in `document.styleSheets` (i.e. `@property` parsed):
- Visual: dynamic-color crossfade (Now Playing), glass blur (sidebar/player
  bar/menu), accent washes (search focus, playing row): observed / broken.
- Fixes applied: none / list with `file:line`.
- The oldest-base WebKitGTK 4.1 floor check remains on the bare-metal
  checklist.

## Deferred to real hardware / real DE

- [ ] GNOME and KDE media widgets show metadata/status; their hardware media
      keys route to OxiTide (`2026-10-03-track8-bare-metal-checklist.md`).
- [ ] `xdg-open tidal://...` from a real browser flow launches/focuses the app
      and hits the single-instance path (bare-metal checklist).
- [ ] X11 restores window position fully; Wayland matches the documented
      degradation (bare-metal checklist).
- [ ] WebKitGTK 4.1 on the oldest base (Ubuntu 22.04 / Debian 12) renders
      both modes after the CSS pass (bare-metal checklist).
~~~

- [ ] **Step 2: Confirm the Windows scheme surface is untouched**

Run:

```powershell
git diff -- crates/oxitide-app/src/scheme.rs
```

Expected: empty output. Record the result in the note (the Linux handler is the `.desktop` entry; the HKCU snapshot/restore stays Windows-only).

- [ ] **Step 3: Run the WSLg deep-link checks**

In WSL2/WSLg, from the repository root:

```bash
grep -rl "x-scheme-handler/tidal" ~/.local/share/applications/*.desktop
xdg-mime query default x-scheme-handler/tidal
xdg-open 'tidal://login/auth?code=smoke'
```

Expected: at least one desktop entry matches, `xdg-mime` resolves it, and `xdg-open` launches/focuses the app. Record PASS/FAIL/BLOCKED in the note. If Plan 8.3 has not landed yet, record BLOCKED with that reason and move the items to the note's deferred section — the Task 2 MPRIS code does not depend on this step.

- [ ] **Step 4: Run the WSLg Wayland mini-player check**

Launch under WSLg, play a track, enter mini (Settings → Mini player or the player-bar control), then exit mini. Record `echo "$XDG_SESSION_TYPE $WAYLAND_DISPLAY"` and whether an error toast appeared. Expected: no toast; size changes work; only window positioning is compositor-controlled. If an error toast appears, continue to Step 5; otherwise record "guard not needed" and skip Step 5.

- [ ] **Step 5: Apply the Wayland guard only if Step 4 observed a failure**

In `crates/oxitide-app/src/window.rs`, make a rejected position non-fatal in `apply_geometry` so size/maximize restore still completes and the mini stash clears:

```rust
/// Apply a stashed geometry: size, position when known, then maximize.
/// Wayland forbids client-side positioning; a rejected position must not
/// fail the rest of the restore.
pub fn apply_geometry(window: &WebviewWindow, geometry: FullGeometry) -> tauri::Result<()> {
    window.set_size(PhysicalSize::new(geometry.width, geometry.height))?;
    if let (Some(x), Some(y)) = (geometry.x, geometry.y) {
        if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
            tracing::debug!("window position not applied: {e}");
        }
    }
    if geometry.maximized {
        window.maximize()?;
    }
    Ok(())
}
```

Run the Windows gates (`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`) and re-run the WSLg mini exit. Record the before/after result in the note.

- [ ] **Step 6: Record results and commit**

Fill in each note item with its PASS/FAIL/BLOCKED result, date, and evidence. Commit the note (and the guard if applied):

```bash
git add docs/superpowers/plans/notes/2026-10-03-track8-4-linux-desktop-smoke.md
git commit -m "docs(track8): add the Linux desktop integration smoke checklist"
```

If the guard was applied, commit it separately:

```bash
git add crates/oxitide-app/src/window.rs
git commit -m "fix(window): tolerate an unsupported window position"
```

---

### Task 5: WebKitGTK CSS pass (gated on the Track 9 UI freeze)

**Gate:** do not run this task until the owner declares the Track 9 UI freeze and the in-flight `ui/` changes are committed. If executed before the freeze, stop after Step 2 (environment + version recording) and leave Steps 3–6 for after the freeze — no UI file may be edited before it.

**Files:**
- Modify: `docs/superpowers/plans/notes/2026-10-03-track8-4-linux-desktop-smoke.md` (findings)
- Modify (only for observed failures): `ui/src/styles/tokens.css`, `ui/src/app.css`, and/or a component's `<style>` block

**Interfaces:**
- Consumes: the WSLg app from Task 4; the `@property` registrations and `transition` in `ui/src/styles/tokens.css:5-19,95-103`; the `color-mix`/`backdrop-filter` usage counted in `ui/src/app.css` and component styles.
- Produces: recorded feature-probe results and targeted fallbacks only where the floor breaks.

- [ ] **Step 1: Record the environment and confirm the gate**

In WSL2:

```bash
dpkg-query -W -f='${Version}\n' libwebkit2gtk-4.1-0
pkg-config --modversion webkit2gtk-4.1
```

Record both versions in the note's "WebKit CSS findings" section. Confirm in the note that the Track 9 UI freeze was declared (date).

- [ ] **Step 2: Probe the features in WSLg**

Launch the debug app (`npm --prefix ui run app:dev`) under WSLg. Open the WebView devtools (right-click → Inspect Element in a debug build; if unavailable, relaunch with `WEBKIT_INSPECTOR_SERVER=127.0.0.1:9222` and open `http://127.0.0.1:9222` in a browser), then evaluate in the Console:

```js
({
  colorMix: CSS.supports("color", "color-mix(in srgb, red 50%, blue)"),
  backdropFilter: CSS.supports("backdrop-filter", "blur(2px)"),
  webkitBackdropFilter: CSS.supports("-webkit-backdrop-filter", "blur(2px)"),
  propertyRule: [...document.styleSheets].some((sheet) => {
    try {
      return [...sheet.cssRules].some((rule) => rule.constructor.name === "CSSPropertyRule");
    } catch {
      return false;
    }
  }),
  dynPrimary: getComputedStyle(document.documentElement).getPropertyValue("--dyn-primary").trim(),
});
```

Record every value. Also view, with a cover-loaded track: the Now Playing dynamic-color crossfade (`--dyn-*`), the glass panels (sidebar, floating player bar, menus), and accent washes (search input focus, playing row). Note exactly what is missing vs Windows.

- [ ] **Step 3: Decide from the observations**

- `colorMix: true`, `backdropFilter`/`-webkit-` true, `propertyRule: true`, and no visual gaps → record "floor passes as-is; no fixes" and go to Step 6.
- `propertyRule: false` and `dynPrimary` empty → `@property` is not parsed; before the first cover analysis `--accent: var(--dyn-primary)` is invalid. Fix with literal fallbacks in `:root` (`ui/src/styles/tokens.css`) using the same values as the `@property` initial-values:

  ```css
  :root {
    /* Literal fallbacks for engines without @property: the registered
       initial-values do not exist there, and dynamicColor.svelte.ts only
       sets these after the first cover analysis. */
    --dyn-primary: #2ee2c4;
    --dyn-on-primary: #0b2420;
    --dyn-secondary: #17120e;
    --dyn-tertiary: #2ee2c4;
    --dyn-backdrop: #1a1510;
  }
  ```

  (`secondary`/`tertiary` mirror lines 7-8 of the existing registrations; `initial-value` is the source of truth.) Crossfades remain instant where unregistered — an accepted degradation, recorded in the note.
- `colorMix: false` and an observed missing accent wash → add a static declaration before each broken `color-mix` rule, e.g. in `ui/src/app.css`:

  ```css
  .search-input:focus {
    border-color: var(--line);
    border-color: color-mix(in srgb, var(--accent) 28%, var(--line));
    background: var(--surface-3);
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-2));
    box-shadow: 0 0 10px color-mix(in srgb, var(--accent) 16%, transparent);
  }
  ```

  Apply this pattern only to rules observed broken; do not sweep the codebase.
- `backdropFilter` false (or compositing disabled, e.g. `WEBKIT_DISABLE_COMPOSITING_MODE`) and an observed unreadable glass panel → add one `@supports` block in `ui/src/app.css`:

  ```css
  @supports not ((backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: blur(1px))) {
    .sidebar,
    .queue {
      background: var(--surface-2);
    }
    .player::before {
      background: var(--surface-3);
    }
  }
  ```

  Extend the selector list only for panels observed broken.

- [ ] **Step 4: Re-verify the fixes in WSLg**

Reload the app and repeat the Step 2 observations for each fixed area. Only fixed failures may be marked resolved; record the rest as accepted/deferred.

- [ ] **Step 5: Run the UI gates**

Run:

```powershell
npm --prefix ui run check
npm --prefix ui test
```

Expected: 0 errors, all tests pass.

- [ ] **Step 6: Record and commit**

Update the note's "WebKit CSS findings" section with the probe results, what was broken, the exact fixes (with `file:line`), and what remains deferred to the oldest-base check. Commit only the files actually changed:

```bash
git add docs/superpowers/plans/notes/2026-10-03-track8-4-linux-desktop-smoke.md
git commit -m "docs(track8): record the WebKitGTK CSS pass"
```

If UI fallbacks were added, commit them separately:

```bash
git add ui/src/styles/tokens.css ui/src/app.css
git commit -m "fix(ui): add WebKitGTK fallbacks for observed CSS gaps"
```

Do not edit `AGENTS.md`; a sibling Track 8 plan owns those updates.

---

## Execution order

Tasks 1 → 2 (Rust, any time) → 4 (docs/verification, any time) → 3 and 5 after the Track 9 UI freeze. Task 5 additionally requires Task 4's WSLg run. Keep the Windows gates green after every task; run the WSL2 gates after Tasks 2, 4 (if the guard lands), and 5.
