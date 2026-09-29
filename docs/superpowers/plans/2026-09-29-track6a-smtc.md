# Windows SMTC + Hardware Media Keys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose OxiTide playback to the Windows System Media Transport Controls — flyout metadata, album art, a live timeline with scrub-to-seek, and Play/Pause/Next/Prev plus hardware media keys — enabled by default and toggleable from Settings.

**Architecture:** A Windows-only worker thread in `oxitide-app` owns all WinRT objects and pumps messages; the existing engine event fan-out feeds it normalized `SessionUpdate`s over a channel; a pure mapping module is unit-tested. One session per player, toggled through a shared atomic flag.

**Tech Stack:** Rust (windows 0.62, Tauri 2 commands, serde TOML), TypeScript, Svelte 5 runes, PowerShell 5.1.

**Spec:** `docs/superpowers/specs/2026-09-29-oxitide-track6a-smtc-design.md`

## Global Constraints

- **Do not commit.** The owner commits manually; each task ends with its verification commands instead of a commit step.
- No new third-party dependencies: only the `windows` crate already in the workspace (0.62).
- No comments in new source code.
- Windows-first: SMTC code is `#[cfg(windows)]`, with a no-op stub elsewhere.
- The engine listener is `Fn(EngineEvent) + Send + Sync`; it must only ever `try_send` (never block) and must not capture `!Sync` values.
- IPC args are camelCase from the UI; Rust command params stay snake_case; `ui/src/lib/api.ts` interfaces are kept in sync by hand.
- Styling consumes semantic tokens only; no `data-mode` branching (AGENTS.md).
- Windows PowerShell 5.1; run all commands from the repo root `C:\Users\V\Documents\Opencode\OxiTide`.
- Never log token/secret values.
- The integration is a leaf: it must never affect the audio path; failures degrade to a `tracing::warn!` and playback continues.

## Review Focus

The spec implies these inputs/failure modes; each is pinned to a test in the task that owns it.

1. Toggling SMTC off mid-track must not touch playback or block the UI, and the worker must not panic on shutdown (Task 4 `set_smtc`; Task 6 smoke).
2. `NowPlaying(None)` (queue cleared) must clear the flyout instead of leaving stale metadata (Task 1 `plan` test).
3. A track with no cover must still show title/artist/album, not blank the flyout (Task 1 `plan` test + Task 3 `apply_track` art guard).
4. A position past the duration must never make the OS scrubber overshoot (Task 1 `plan` clamp test).
5. Logout/exit while playing must join the worker without deadlock (Task 4 `take_media`/`Drop`; Task 6 smoke).

---

### Task 0: Spike — confirm GetForWindow, event delivery, and thumbnail

Throwaway. The deliverable is a findings note, not shipped code. The example probe is deleted in Task 4.

**Files:**
- Create: `crates/oxitide-app/examples/smtc_probe.rs` (throwaway)
- Create: `docs/superpowers/plans/notes/2026-09-29-smtc-spike.md`
- Modify: `crates/oxitide-app/Cargo.toml`

**Interfaces:**
- Produces: the confirmed interop call shape, HWND construction, whether a message pump is needed, and the working thumbnail path — recorded in the note and relied on by Task 3.

- [ ] **Step 1: Add the Windows dependency and features**

In `crates/oxitide-app/Cargo.toml`, after the `[dependencies]` table, append:

```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.62", features = [
    "Media",
    "Foundation",
    "Storage_Streams",
    "Win32_Foundation",
    "Win32_System_WinRT",
    "Win32_System_Com",
    "Win32_System_LibraryLoader",
    "Win32_UI_WindowsAndMessaging",
] }
```

- [ ] **Step 2: Write the throwaway probe**

Create `crates/oxitide-app/examples/smtc_probe.rs`:

```rust
#![cfg(windows)]

use std::time::{Duration, Instant};

use windows::core::{factory, w, HSTRING};
use windows::Foundation::{TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButtonPressedEventArgs,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, PeekMessageW, RegisterClassW,
    TranslateMessage, CW_USEDEFAULT, MSG, PM_REMOVE, WNDCLASSW, WS_OVERLAPPED,
};

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

fn main() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let hinstance = GetModuleHandleW(None).expect("module handle");
        let class = w!("OxiTideSmtcProbe");
        let wc = WNDCLASSW {
            hInstance: hinstance.into(),
            lpszClassName: class,
            lpfnWndProc: Some(wndproc),
            ..Default::default()
        };
        let _ = RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            Default::default(),
            class,
            w!("probe"),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            400,
            300,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .expect("create window");

        let interop: ISystemMediaTransportControlsInterop =
            factory::<SystemMediaTransportControls, _>().expect("interop factory");
        let controls: SystemMediaTransportControls =
            interop.GetForWindow(hwnd).expect("GetForWindow");
        controls.SetIsEnabled(true).unwrap();
        controls.SetIsPlayEnabled(true).unwrap();
        controls.SetIsPauseEnabled(true).unwrap();

        let handler = TypedEventHandler::new(
            |_, args: windows::core::Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
                let args = args.ok()?;
                println!("BUTTON: {:?} on {:?}", args.Button()?, std::thread::current().id());
                Ok(())
            },
        );
        controls.ButtonPressed(&handler).unwrap();

        let updater = controls.DisplayUpdater().unwrap();
        updater.SetType(MediaPlaybackType::Music).unwrap();
        let props = updater.MusicProperties().unwrap();
        props.SetTitle(&HSTRING::from("Probe Title")).unwrap();
        props.SetArtist(&HSTRING::from("Probe Artist")).unwrap();
        props.SetAlbumTitle(&HSTRING::from("Probe Album")).unwrap();
        let url = "https://resources.tidal.com/images/2fa8b7a6/9d5b/4a1e/9f0a/4a1e9f0a4a1e/320x320.jpg";
        match Uri::CreateUri(&HSTRING::from(url)) {
            Ok(uri) => match RandomAccessStreamReference::CreateFromUri(&uri) {
                Ok(stream) => {
                    updater.SetThumbnail(&stream).unwrap();
                    println!("THUMBNAIL: URI accepted");
                }
                Err(e) => println!("THUMBNAIL: CreateFromUri failed: {e}"),
            },
            Err(e) => println!("THUMBNAIL: CreateUri failed: {e}"),
        }
        updater.Update().unwrap();
        controls.SetPlaybackStatus(MediaPlaybackStatus(3)).unwrap();
        println!("SMTC probe ready: open the flyout and press media keys for 30s");

        let start = Instant::now();
        let mut msg = MSG::default();
        while start.elapsed() < Duration::from_secs(30) {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        controls.SetIsEnabled(false).unwrap();
    }
}
```

- [ ] **Step 3: Run the probe and answer three questions**

Run: `cargo run -p oxitide-app --example smtc_probe`
Expected: it compiles and runs; the Windows media flyout shows "Probe Title / Probe Artist / Probe Album"; pressing media keys prints `BUTTON: …`.

Some exact Win32 signatures (`CreateWindowExW` argument types, `HWND(...)` construction, `MSG::default()`, `PeekMessageW(&mut msg, None, …)`) may differ in this `windows` version — adjust until it compiles. Observe:
1. Does `factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()` + `GetForWindow` succeed? (If not, capture the HRESULT.)
2. Do `ButtonPressed` callbacks arrive, and on which thread (the printed thread id)? Is the custom `PeekMessageW` pump sufficient, or do they arrive on the main thread?
3. Does the thumbnail render in the flyout from `CreateFromUri(https…)`, or is a file/stream fallback required?

- [ ] **Step 4: Record findings**

Create `docs/superpowers/plans/notes/2026-09-29-smtc-spike.md`:

```markdown
# SMTC spike findings (2026-09-29)

- GetForWindow: <success | HRESULT …>
- HWND construction that compiled: <…>
- ButtonPressed delivery: <arrives via own pump | arrives on main thread | not delivered> (thread id: …)
- Thumbnail via CreateFromUri: <rendered | blank> — fallback needed: <no | describe>
- Windows version on the test machine: <…>
- Notes for Task 3: <exact interop/HWND/thumbnail code that worked>
```

Run: `cargo build -p oxitide-app --example smtc_probe`
Expected: success (keep the example until Task 4).

---

### Task 1: Pure mapping module

**Files:**
- Create: `crates/oxitide-app/src/media_controls.rs`
- Modify: `crates/oxitide-app/src/main.rs` (module list, ~L3-12)

**Interfaces:**
- Consumes: `oxitide_audio::engine::{EngineCmd, EngineEvent}`, `oxitide_audio::queue::QueueItem`.
- Produces: `PlayState`, `TrackMeta`, `SessionUpdate`, `ControlCommand`, `SessionSender`, `cover_url(&str, u32) -> Option<String>`, `plan(&EngineEvent, Option<u64>) -> (Vec<SessionUpdate>, Option<u64>)`, `to_engine_cmd(ControlCommand) -> EngineCmd`, and (behind `#[cfg(not(windows))]`) a `MediaSession` stub. `SessionSender` is `Clone + Send + Sync`; its `send` is non-blocking (`try_send`).

The file starts with a temporary `#![allow(dead_code)]` because nothing consumes it until Task 4; Task 4 removes it.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-app/src/media_controls.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_audio::engine::PlaybackState;
    use oxitide_audio::output::OutputMode;
    use oxitide_audio::queue::{QueueItem, RepeatMode};
    use oxitide_tidal::stream::Quality;

    fn item(track_id: u64, cover: Option<&str>) -> QueueItem {
        QueueItem {
            track_id,
            title: format!("Title {track_id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_secs: 200,
            cover: cover.map(str::to_string),
        }
    }

    fn state(playing: bool, current: Option<QueueItem>) -> EngineEvent {
        EngineEvent::State(PlaybackState {
            playing,
            buffering: false,
            current,
            position_ms: 0,
            duration_ms: 0,
            volume: 1.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            preferred_quality: Quality::Lossless,
            output_mode: OutputMode::Shared,
            bit_perfect: false,
            bit_perfect_notice: None,
            queue: Vec::new(),
            stream: None,
        })
    }

    #[test]
    fn cover_url_handles_uuid_and_absolute() {
        assert_eq!(cover_url("", 320), None);
        assert_eq!(
            cover_url("2fa8b7a6-9d5b-4a1e-9f0a-4a1e9f0a4a1e", 320).unwrap(),
            "https://resources.tidal.com/images/2fa8b7a6/9d5b/4a1e/9f0a/4a1e9f0a4a1e/320x320.jpg"
        );
        assert_eq!(
            cover_url("https://example.com/a.jpg", 320).unwrap(),
            "https://example.com/a.jpg"
        );
    }

    #[test]
    fn now_playing_emits_track_once() {
        let (updates, last) = plan(&EngineEvent::NowPlaying(Some(item(7, Some("x")))), None);
        assert_eq!(last, Some(7));
        assert_eq!(
            updates,
            vec![SessionUpdate::Track(Some(TrackMeta {
                track_id: 7,
                title: "Title 7".into(),
                artist: "Artist".into(),
                album: "Album".into(),
                duration_ms: 200_000,
                cover: Some("x".into()),
            }))]
        );
        let (again, last) = plan(&EngineEvent::NowPlaying(Some(item(7, Some("x")))), last);
        assert!(again.is_empty());
        assert_eq!(last, Some(7));
    }

    #[test]
    fn now_playing_none_clears_and_stops() {
        let (updates, last) = plan(&EngineEvent::NowPlaying(None), Some(7));
        assert_eq!(
            updates,
            vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)]
        );
        assert_eq!(last, None);
    }

    #[test]
    fn state_sets_playback_and_new_track_only() {
        let (updates, last) = plan(&state(true, Some(item(3, None))), None);
        assert_eq!(
            updates,
            vec![
                SessionUpdate::Track(Some(TrackMeta {
                    track_id: 3,
                    title: "Title 3".into(),
                    artist: "Artist".into(),
                    album: "Album".into(),
                    duration_ms: 200_000,
                    cover: None,
                })),
                SessionUpdate::Playback(PlayState::Playing),
            ]
        );
        let (again, _) = plan(&state(false, Some(item(3, None))), last);
        assert_eq!(again, vec![SessionUpdate::Playback(PlayState::Paused)]);
    }

    #[test]
    fn state_without_current_is_stopped() {
        let (updates, last) = plan(&state(false, None), Some(3));
        assert_eq!(
            updates,
            vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)]
        );
        assert_eq!(last, None);
    }

    #[test]
    fn position_is_clamped_to_duration() {
        let (updates, _) =
            plan(&EngineEvent::Position { position_ms: 250_000, duration_ms: 200_000 }, None);
        assert_eq!(
            updates,
            vec![SessionUpdate::Position { position_ms: 200_000, duration_ms: 200_000 }]
        );
        let (updates, _) = plan(&EngineEvent::Position { position_ms: 5, duration_ms: 0 }, None);
        assert_eq!(updates, vec![SessionUpdate::Position { position_ms: 5, duration_ms: 0 }]);
    }

    #[test]
    fn control_commands_map_to_engine_commands() {
        assert!(matches!(to_engine_cmd(ControlCommand::PlayPause), EngineCmd::PlayPause));
        assert!(matches!(to_engine_cmd(ControlCommand::Next), EngineCmd::Next));
        assert!(matches!(to_engine_cmd(ControlCommand::Prev), EngineCmd::Prev));
        assert!(matches!(to_engine_cmd(ControlCommand::Seek(42)), EngineCmd::Seek(42)));
    }
}
```

- [ ] **Step 2: Register the module, then run the tests to verify they fail**

In `main.rs`, add to the module list (after `mod lyrics;`):

```rust
mod media_controls;
```

Run: `cargo test -p oxitide-app media_controls`
Expected: compile error — `cover_url`/`plan`/`TrackMeta`/`SessionUpdate`/`PlayState`/`ControlCommand`/`to_engine_cmd` not found.

- [ ] **Step 3: Write the implementation**

Prepend to `crates/oxitide-app/src/media_controls.rs`, above the `#[cfg(test)]` module:

```rust
#![allow(dead_code)]

use std::sync::mpsc::SyncSender;

use oxitide_audio::engine::{EngineCmd, EngineEvent};
use oxitide_audio::queue::QueueItem;

#[cfg(not(windows))]
pub use stub::MediaSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackMeta {
    pub track_id: u64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionUpdate {
    Track(Option<TrackMeta>),
    Playback(PlayState),
    Position { position_ms: u64, duration_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    PlayPause,
    Next,
    Prev,
    Seek(u64),
}

#[derive(Clone)]
pub struct SessionSender {
    tx: SyncSender<SessionUpdate>,
}

impl SessionSender {
    pub(crate) fn new(tx: SyncSender<SessionUpdate>) -> Self {
        Self { tx }
    }

    pub fn send(&self, update: SessionUpdate) {
        let _ = self.tx.try_send(update);
    }
}

pub fn cover_url(cover: &str, size: u32) -> Option<String> {
    if cover.is_empty() {
        return None;
    }
    if cover.starts_with("http") {
        return Some(cover.to_string());
    }
    Some(format!(
        "https://resources.tidal.com/images/{}/{size}x{size}.jpg",
        cover.replace('-', "/")
    ))
}

fn track_meta(item: &QueueItem) -> TrackMeta {
    TrackMeta {
        track_id: item.track_id,
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_secs as u64 * 1000,
        cover: item.cover.clone(),
    }
}

pub fn plan(event: &EngineEvent, last_track: Option<u64>) -> (Vec<SessionUpdate>, Option<u64>) {
    match event {
        EngineEvent::NowPlaying(Some(item)) => {
            if last_track == Some(item.track_id) {
                (Vec::new(), last_track)
            } else {
                (vec![SessionUpdate::Track(Some(track_meta(item)))], Some(item.track_id))
            }
        }
        EngineEvent::NowPlaying(None) => (
            vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)],
            None,
        ),
        EngineEvent::State(snapshot) => match &snapshot.current {
            Some(item) => {
                let play = if snapshot.playing { PlayState::Playing } else { PlayState::Paused };
                let mut updates = vec![SessionUpdate::Playback(play)];
                if last_track == Some(item.track_id) {
                    (updates, last_track)
                } else {
                    updates.insert(0, SessionUpdate::Track(Some(track_meta(item))));
                    (updates, Some(item.track_id))
                }
            }
            None => (
                vec![SessionUpdate::Track(None), SessionUpdate::Playback(PlayState::Stopped)],
                None,
            ),
        },
        EngineEvent::Position { position_ms, duration_ms } => {
            let position =
                if *duration_ms > 0 { (*position_ms).min(*duration_ms) } else { *position_ms };
            (
                vec![SessionUpdate::Position { position_ms: position, duration_ms: *duration_ms }],
                last_track,
            )
        }
        EngineEvent::Error { .. } => (Vec::new(), last_track),
    }
}

pub fn to_engine_cmd(cmd: ControlCommand) -> EngineCmd {
    match cmd {
        ControlCommand::PlayPause => EngineCmd::PlayPause,
        ControlCommand::Next => EngineCmd::Next,
        ControlCommand::Prev => EngineCmd::Prev,
        ControlCommand::Seek(ms) => EngineCmd::Seek(ms),
    }
}

#[cfg(not(windows))]
mod stub {
    use std::sync::mpsc;

    use oxitide_audio::engine::Engine;

    use super::{SessionSender, SessionUpdate};

    pub struct MediaSession {
        tx: mpsc::SyncSender<SessionUpdate>,
    }

    impl MediaSession {
        pub fn spawn(_engine: Engine, _hwnd: isize, _enabled: bool) -> Result<Self, String> {
            let (tx, _rx) = mpsc::sync_channel(1);
            Ok(MediaSession { tx })
        }

        pub fn sender(&self) -> SessionSender {
            SessionSender::new(self.tx.clone())
        }

        pub fn set_enabled(&self, _enabled: bool) {}
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app media_controls`
Expected: PASS (7 tests).

---

### Task 2: `integrations.smtc` config

**Files:**
- Modify: `crates/oxitide-app/src/config.rs`

**Interfaces:**
- Produces: `Config.integrations: IntegrationsConfig` with `pub smtc: bool` (default `true`); TOML key `integrations.smtc`.

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `config.rs` (before the final `}`):

```rust
    #[test]
    fn smtc_defaults_true_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(Config::default().integrations.smtc);
        let mut config = Config::default();
        config.integrations.smtc = false;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[integrations]"), "{text}");
        assert!(text.contains("smtc = false"), "{text}");
        assert!(!Config::load_from(&path).integrations.smtc);
        let _ = fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app smtc_defaults_true_and_round_trips`
Expected: compile error — no field `integrations`.

- [ ] **Step 3: Add the section, field, and fixtures**

Add the struct after `CacheConfig`'s `impl Default` (near `config.rs:175`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationsConfig {
    /// Windows System Media Transport Controls (media keys + now-playing).
    pub smtc: bool,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self { smtc: true }
    }
}
```

(The doc comment on `smtc` is allowed; the no-comments rule is about implementation commentary.)

Add the field to `Config` (after `pub cache: CacheConfig,`):

```rust
    pub integrations: IntegrationsConfig,
```

In `save_and_load_round_trip`, add to the `Config { .. }` literal after the `cache: CacheConfig { .. },` entry:

```rust
            integrations: IntegrationsConfig { smtc: false },
```

In `partial_file_fills_defaults`, after `assert!(!config.cache.clear_on_exit);`:

```rust
        assert!(config.integrations.smtc);
```

- [ ] **Step 4: Run the tests and clippy**

Run: `cargo test -p oxitide-app config`
Expected: all config tests PASS.

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings (the temporary `#![allow(dead_code)]` in `media_controls.rs` covers its not-yet-used items).

---

### Task 3: Windows worker thread

**Files:**
- Create: `crates/oxitide-app/src/media_controls_win.rs`
- Modify: `crates/oxitide-app/src/media_controls.rs` (add the Windows module declaration)

**Interfaces:**
- Consumes: `super::{cover_url, to_engine_cmd, ControlCommand, PlayState, SessionSender, SessionUpdate, TrackMeta}`; the interop/HWND/thumbnail shapes confirmed in Task 0.
- Produces: `pub struct MediaSession` with `spawn(engine: Engine, hwnd: isize, enabled: bool) -> Result<Self, String>`, `sender() -> SessionSender`, `set_enabled(&self, bool)`, and a `Drop` that drops the sender and joins the worker.

- [ ] **Step 1: Declare the Windows module**

In `crates/oxitide-app/src/media_controls.rs`, just below the `use` block, add:

```rust
#[cfg(windows)]
#[path = "media_controls_win.rs"]
mod win;

#[cfg(windows)]
pub use win::MediaSession;
```

- [ ] **Step 2: Write the worker**

Create `crates/oxitide-app/src/media_controls_win.rs`:

```rust
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use oxitide_audio::engine::{Engine, EngineCmd};
use windows::core::{factory, HSTRING};
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, PlaybackPositionChangeRequestedEventArgs,
    SystemMediaTransportControls, SystemMediaTransportControlsButton,
    SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

use super::{cover_url, to_engine_cmd, ControlCommand, PlayState, SessionSender, SessionUpdate, TrackMeta};

const THUMBNAIL_SIZE: u32 = 320;
const CHANNEL_CAPACITY: usize = 64;

pub struct MediaSession {
    tx: Option<SyncSender<SessionUpdate>>,
    enabled: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl MediaSession {
    pub fn spawn(engine: Engine, hwnd: isize, enabled: bool) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<SessionUpdate>(CHANNEL_CAPACITY);
        let flag = Arc::new(AtomicBool::new(enabled));
        let worker_flag = flag.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-smtc".into())
            .spawn(move || worker(engine, hwnd, rx, worker_flag))
            .map_err(|e| e.to_string())?;
        Ok(Self { tx: Some(tx), enabled: flag, handle: Some(handle) })
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
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct Smtc {
    controls: SystemMediaTransportControls,
    updater: windows::Media::SystemMediaTransportControlsDisplayUpdater,
    timeline: SystemMediaTransportControlsTimelineProperties,
}

fn setup(engine: &Engine, hwnd: isize) -> windows::core::Result<Smtc> {
    let interop: ISystemMediaTransportControlsInterop =
        factory::<SystemMediaTransportControls, _>()?;
    let controls: SystemMediaTransportControls = unsafe { interop.GetForWindow(HWND(hwnd as _))? };
    controls.SetIsPlayEnabled(true)?;
    controls.SetIsPauseEnabled(true)?;
    controls.SetIsNextEnabled(true)?;
    controls.SetIsPreviousEnabled(true)?;
    let updater = controls.DisplayUpdater()?;
    updater.SetType(MediaPlaybackType::Music)?;

    let buttons = engine.clone();
    let button_handler = TypedEventHandler::new(
        move |_, args: windows::core::Ref<'_, SystemMediaTransportControlsButtonPressedEventArgs>| {
            let args = args.ok()?;
            dispatch_button(&buttons, args.Button()?);
            Ok(())
        },
    );
    controls.ButtonPressed(&button_handler)?;

    let seeker = engine.clone();
    let position_handler = TypedEventHandler::new(
        move |_, args: windows::core::Ref<'_, PlaybackPositionChangeRequestedEventArgs>| {
            let args = args.ok()?;
            let requested = Duration::from(args.RequestedPlaybackPosition()?);
            let _ = seeker.handle(EngineCmd::Seek(requested.as_millis() as u64));
            Ok(())
        },
    );
    controls.PlaybackPositionChangeRequested(&position_handler)?;

    Ok(Smtc {
        controls,
        updater,
        timeline: SystemMediaTransportControlsTimelineProperties::new()?,
    })
}

fn dispatch_button(engine: &Engine, button: SystemMediaTransportControlsButton) {
    let command = if button == SystemMediaTransportControlsButton::Play
        || button == SystemMediaTransportControlsButton::Pause
    {
        ControlCommand::PlayPause
    } else if button == SystemMediaTransportControlsButton::Next {
        ControlCommand::Next
    } else if button == SystemMediaTransportControlsButton::Previous {
        ControlCommand::Prev
    } else {
        return;
    };
    let _ = engine.handle(to_engine_cmd(command));
}

fn worker(engine: Engine, hwnd: isize, rx: Receiver<SessionUpdate>, enabled: Arc<AtomicBool>) {
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_MULTITHREADED,
        );
    }
    let mut smtc: Option<Smtc> = None;
    let mut shown = false;
    let mut last_track: Option<TrackMeta> = None;
    let mut last_play = PlayState::Stopped;
    let mut last_position = (0u64, 0u64);
    let mut msg = MSG::default();

    loop {
        let want = enabled.load(Ordering::Relaxed);
        if want && smtc.is_none() {
            match setup(&engine, hwnd) {
                Ok(created) => smtc = Some(created),
                Err(e) => {
                    tracing::warn!("SMTC unavailable: {e}");
                    enabled.store(false, Ordering::Relaxed);
                }
            }
        }
        if let Some(s) = smtc.as_mut() {
            if want != shown {
                if let Err(e) = s.controls.SetIsEnabled(want) {
                    tracing::warn!("SMTC enable failed: {e}");
                }
                shown = want;
                if want {
                    if let Some(track) = &last_track {
                        let _ = apply_track(s, track);
                    }
                    let _ = s.controls.SetPlaybackStatus(playback_status(last_play));
                    let _ = apply_position(s, last_position.0, last_position.1);
                } else {
                    let _ = s.updater.ClearAll();
                }
            }
        }

        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(update) => {
                match &update {
                    SessionUpdate::Track(track) => last_track = track.clone(),
                    SessionUpdate::Playback(state) => last_play = *state,
                    SessionUpdate::Position { position_ms, duration_ms } => {
                        last_position = (*position_ms, *duration_ms);
                    }
                }
                if let (Some(s), true) = (smtc.as_mut(), want) {
                    apply_update(s, last_track.as_ref(), last_play, last_position);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    if let Some(s) = smtc.as_ref() {
        let _ = s.controls.SetIsEnabled(false);
        let _ = s.updater.ClearAll();
    }
}

fn apply_update(smtc: &mut Smtc, track: Option<&TrackMeta>, play: PlayState, position: (u64, u64)) {
    if let Some(track) = track {
        if let Err(e) = apply_track(smtc, track) {
            tracing::warn!("SMTC metadata failed: {e}");
        }
    }
    if let Err(e) = smtc.controls.SetPlaybackStatus(playback_status(play)) {
        tracing::warn!("SMTC status failed: {e}");
    }
    if let Err(e) = apply_position(smtc, position.0, position.1) {
        tracing::warn!("SMTC timeline failed: {e}");
    }
}

fn apply_track(smtc: &mut Smtc, track: &TrackMeta) -> windows::core::Result<()> {
    let properties = smtc.updater.MusicProperties()?;
    properties.SetTitle(&HSTRING::from(track.title.as_str()))?;
    properties.SetArtist(&HSTRING::from(track.artist.as_str()))?;
    properties.SetAlbumTitle(&HSTRING::from(track.album.as_str()))?;
    if let Some(url) = track.cover.as_deref().and_then(|cover| cover_url(cover, THUMBNAIL_SIZE)) {
        if let Ok(uri) = Uri::CreateUri(&HSTRING::from(url.as_str())) {
            if let Ok(stream) = RandomAccessStreamReference::CreateFromUri(&uri) {
                let _ = smtc.updater.SetThumbnail(&stream);
            }
        }
    }
    smtc.updater.Update()
}

fn apply_position(smtc: &mut Smtc, position_ms: u64, duration_ms: u64) -> windows::core::Result<()> {
    let start = TimeSpan::default();
    let end = TimeSpan::from(Duration::from_millis(duration_ms));
    let position = TimeSpan::from(Duration::from_millis(position_ms));
    smtc.timeline.SetStartTime(start)?;
    smtc.timeline.SetMinSeekTime(start)?;
    smtc.timeline.SetEndTime(end)?;
    smtc.timeline.SetMaxSeekTime(end)?;
    smtc.timeline.SetPosition(position)?;
    smtc.controls.UpdateTimelineProperties(&smtc.timeline)
}

fn playback_status(state: PlayState) -> MediaPlaybackStatus {
    match state {
        PlayState::Playing => MediaPlaybackStatus(3),
        PlayState::Paused => MediaPlaybackStatus(4),
        PlayState::Stopped => MediaPlaybackStatus(2),
    }
}
```

- [ ] **Step 3: Compile and lint**

Run: `cargo check -p oxitide-app`
Expected: success. If a `windows` signature differs, adjust to the shape recorded in Task 0 (in particular `HWND(hwnd as _)`, `PeekMessageW(&mut msg, None, …)`, and `MediaPlaybackStatus(3)` vs the crate's associated constants).

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings.

---

### Task 4: Lifecycle wiring

**Files:**
- Modify: `crates/oxitide-app/src/state.rs` (AppState, logout)
- Modify: `crates/oxitide-app/src/commands.rs` (`build_player`, `set_smtc`)
- Modify: `crates/oxitide-app/src/main.rs` (handler list)
- Modify: `crates/oxitide-app/src/media_controls.rs` (remove the temporary `#![allow(dead_code)]`)
- Modify: `crates/oxitide-app/src/media_controls_win.rs` (remove the temporary `#![allow(dead_code)]`)
- Delete: `crates/oxitide-app/examples/smtc_probe.rs`

**Interfaces:**
- Consumes: `crate::media_controls::{MediaSession, SessionSender, plan}` (Tasks 1, 3).
- Produces: `AppState::set_media`/`take_media`; Tauri command `set_smtc(enabled: bool)`; SMTC fed from the engine fan-out.

- [ ] **Step 1: Add the AppState field and accessors**

In `state.rs`, add to `AppState` (after `pub lyrics: Mutex<Option<Arc<LyricsService>>>,`):

```rust
    pub media: Mutex<Option<crate::media_controls::MediaSession>>,
```

Add accessors after `lyrics(&self)`:

```rust
    pub fn set_media(&self, session: crate::media_controls::MediaSession) {
        *self.media.lock().unwrap() = Some(session);
    }

    pub fn take_media(&self) -> Option<crate::media_controls::MediaSession> {
        self.media.lock().unwrap().take()
    }
```

Change `logout()`:

```rust
    pub fn logout(&self) {
        let _ = self.take_media();
        *self.client.write().unwrap() = None;
        *self.player.write().unwrap() = None;
        self.clear_session_expired();
    }
```

- [ ] **Step 2: Feed the session from `build_player`**

In `commands.rs`, replace the block from `let handle = app.clone();` through the closing `Ok(Player { engine })` (currently `commands.rs:366-376`) with:

```rust
    let _ = app.state::<AppState>().take_media();
    let mut sender: Option<crate::media_controls::SessionSender> = None;
    #[cfg(windows)]
    if config.integrations.smtc {
        let state = app.state::<AppState>();
        match app.get_webview_window("main").map(|window| window.hwnd()) {
            Some(Ok(hwnd)) => {
                match crate::media_controls::MediaSession::spawn(
                    engine.clone(),
                    hwnd.0 as isize,
                    true,
                ) {
                    Ok(session) => {
                        sender = Some(session.sender());
                        state.set_media(session);
                    }
                    Err(e) => tracing::warn!("SMTC disabled: {e}"),
                }
            }
            Some(Err(e)) => tracing::warn!("SMTC disabled (no HWND): {e}"),
            None => tracing::warn!("SMTC disabled (no main window)"),
        }
    }

    let handle = app.clone();
    let last_track = std::sync::Mutex::new(None);
    engine.subscribe(Arc::new(move |event| {
        if let EngineEvent::Error { code, .. } = &event {
            if code == "auth" {
                handle.state::<AppState>().notify_session_expired();
            }
        }
        if let Some(sender) = &sender {
            let mut last = last_track.lock().unwrap();
            let (updates, next) = crate::media_controls::plan(&event, *last);
            *last = next;
            for update in updates {
                sender.send(update);
            }
        }
        let _ = tauri::Emitter::emit(&handle, "playback_event", crate::player::to_dto(event));
    }));
    Ok(Player { engine })
```

- [ ] **Step 3: Add the `set_smtc` command**

In `commands.rs`, after `set_oneko` (`commands.rs:1119`):

```rust
#[tauri::command]
pub fn set_smtc(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.integrations.smtc = enabled;
    config.save()?;
    state.set_config(config);
    if let Some(session) = state.media.lock().unwrap().as_ref() {
        session.set_enabled(enabled);
    }
    Ok(())
}
```

- [ ] **Step 4: Register the command**

In `main.rs`, in `generate_handler![...]` after `commands::set_oneko,`:

```rust
            commands::set_smtc,
```

- [ ] **Step 5: Remove the temporary lints and the throwaway probe**

Delete the first line (`#![allow(dead_code)]`) from both `crates/oxitide-app/src/media_controls.rs` and `crates/oxitide-app/src/media_controls_win.rs`.

Run: `Remove-Item -LiteralPath "crates\oxitide-app\examples\smtc_probe.rs"`
Then remove the `Win32_System_LibraryLoader` feature (used only by the probe) from `crates/oxitide-app/Cargo.toml`, keeping the rest.

- [ ] **Step 6: Verify**

Run: `cargo test -p oxitide-app`
Expected: all tests PASS.

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings (if a dead-code warning appears, it names a genuinely unused item — remove it rather than re-adding the allow).

Run: `cargo build -p oxitide-app`
Expected: success.

---

### Task 5: Frontend toggle

**Files:**
- Modify: `ui/src/lib/api.ts` (`Config` ~L110-136, `api` object ~L250)
- Modify: `ui/src/views/Settings.svelte` (handlers ~L187; Connections panel ~L419-444)

**Interfaces:**
- Consumes: `set_smtc` command and `integrations.smtc` (Task 4).
- Produces: `Config.integrations.smtc`; `api.setSmtc(enabled)`; the Settings checkbox.

- [ ] **Step 1: Add the API type and wrapper**

In `api.ts`, add to the `Config` interface after the `ui` object closes:

```ts
  integrations: {
    smtc: boolean;
  };
```

In the `api` object, after `setOneko` (`api.ts:250`):

```ts
  setSmtc: (enabled: boolean) => invoke<void>("set_smtc", { enabled }),
```

- [ ] **Step 2: Add the Settings handler**

In `Settings.svelte`, after `setOneko` (L191):

```ts
  function setSmtc(enabled: boolean) {
    if (!config) return;
    config = { ...config, integrations: { ...config.integrations, smtc: enabled } };
    api.setSmtc(enabled).catch(apply);
  }
```

- [ ] **Step 3: Replace the planned row with a real toggle**

In the Connections panel (`Settings.svelte:437-443`), reword the header and replace the `Windows media controls` planned row:

```svelte
      <p class="muted">Integrations.</p>
      <h3>Scrobbling</h3>
      {@render planned("Last.fm", "Scrobble plays and publish now-playing to Last.fm.")}
      {@render planned("ListenBrainz", "Scrobble plays and publish now-playing to ListenBrainz.")}
      <h3>Presence & controls</h3>
      {@render planned("Discord Rich Presence", "Show what you're listening to on your Discord profile.")}
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.integrations.smtc}
              onchange={(e) => setSmtc((e.target as HTMLInputElement).checked)}
            />
            <span>Windows media controls</span>
          </label>
          <span class="muted">Hardware media keys and the Windows media overlay (SMTC).</span>
        </div>
      </div>
```

- [ ] **Step 4: Verify types and tests**

Run: `npm --prefix ui run check`
Expected: 0 errors.

Run: `npm --prefix ui test`
Expected: full suite PASS.

---

### Task 6: Smoke checklist and full verification

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-29-smtc-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the human verification record and the full check run.

- [ ] **Step 1: Write the human smoke checklist**

Create `docs/superpowers/plans/notes/2026-09-29-smtc-smoke.md`:

```markdown
# Windows SMTC + media keys — manual smoke checklist

Requires a real account and a real Windows session; record date and result per item.

- [ ] Play a track with the window focused: the media flyout shows title / artist / album and
      the album art; the play/pause icon matches the state.
- [ ] Hardware media keys (focus another app first): play/pause, next, and previous all drive
      OxiTide while its window is unfocused.
- [ ] The flyout scrubber advances during playback and never overshoots the track end.
- [ ] Dragging the flyout scrubber seeks OxiTide to that position.
- [ ] Pause: the flyout shows paused; media-key play resumes.
- [ ] A track with no album art still shows title / artist / album (flyout not blank).
- [ ] A track with unusual metadata does not break the flyout.
- [ ] Mini player mode: metadata and media keys still work.
- [ ] Queue clears / playback stops: the flyout clears instead of showing stale metadata.
- [ ] Settings → Connections: unchecking "Windows media controls" removes it from the flyout
      and media keys stop controlling OxiTide; playback is unaffected. Re-checking restores it
      without a restart.
- [ ] Restart the app: the flag is honored; a disabled setting stays disabled.
- [ ] Log out: the flyout entry is cleared and the app does not hang.
- [ ] Quit while playing: no hang, no orphaned media session.
- [ ] WebView interaction (typing, clicking, scrolling) is unaffected throughout.
```

- [ ] **Step 2: Run the full verification suite**

Run: `cargo test --workspace`
Expected: all tests PASS.

Run: `cargo clippy --workspace -- -D warnings`
Expected: no warnings.

Run: `npm --prefix ui run check`
Expected: 0 errors.

Run: `npm --prefix ui test`
Expected: full suite PASS.

- [ ] **Step 3: Manual GUI run (human, against the checklist)**

Run: `npm --prefix ui run app:dev`
Expected: the window opens; walk the checklist above and record the results. Any interop or
thumbnail failure is triaged against the Task 0 findings before being called a defect.
```

---

## Roadmap follow-up

After this ships, set Track 6's Status in `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` to note "6a (SMTC/media keys) done; 6b (Discord RPC) next". Do this only when the owner confirms the smoke checklist passed.

---

## Shipped deviations (post-execution review)

The shipped code diverged from the snippets above; recorded here so the plan matches reality. Snippets below are the as-built versions for the two bug-prone spots.

- **R6:** Task 1 removed a duplicate `mod stub` declaration — the plan text was wrong.
- **R7:** `MediaSession` gained `shutdown: Arc<AtomicBool>`; `Drop` sets it before joining so a lingering `SessionSender` clone cannot deadlock the worker (which would otherwise block forever in `recv_timeout`).
- **R8:** `apply_track` takes `Option<&TrackMeta>` and `ClearAll`s on `None`; each `SessionUpdate` applies only the aspect it carries; `Track(Some)` also resets the timeline from `duration_ms`.
- **R9:** `build_player` spawns the worker UNCONDITIONALLY (passing `config.integrations.smtc` as `enabled`); the seek handler routes through `to_engine_cmd(ControlCommand::Seek(ms))`.

Corrected `apply_track` (clears first so a track with no cover does not retain the previous thumbnail):

```rust
fn apply_track(smtc: &mut Smtc, track: Option<&TrackMeta>) -> windows::core::Result<()> {
    smtc.updater.ClearAll()?;
    let Some(track) = track else {
        return smtc.updater.Update();
    };
    smtc.updater.SetType(MediaPlaybackType::Music)?;
    let properties = smtc.updater.MusicProperties()?;
    properties.SetTitle(&HSTRING::from(track.title.as_str()))?;
    properties.SetArtist(&HSTRING::from(track.artist.as_str()))?;
    properties.SetAlbumTitle(&HSTRING::from(track.album.as_str()))?;
    if let Some(url) = track.cover.as_deref().and_then(|cover| cover_url(cover, THUMBNAIL_SIZE)) {
        if let Ok(uri) = Uri::CreateUri(&HSTRING::from(url.as_str())) {
            if let Ok(stream) = RandomAccessStreamReference::CreateFromUri(&uri) {
                let _ = smtc.updater.SetThumbnail(&stream);
            }
        }
    }
    smtc.updater.Update()
}
```

Corrected `build_player` spawn (UNCONDITIONAL — do not wrap in `if config.integrations.smtc`, or a session created while the setting is on would strand when later build paths skip the spawn):

```rust
    #[cfg(windows)]
    let sender = {
        let state = app.state::<AppState>();
        let _ = state.take_media();
        let mut found = None;
        match app.get_webview_window("main").map(|window| window.hwnd()) {
            Some(Ok(hwnd)) => {
                match crate::media_controls::MediaSession::spawn(
                    engine.clone(),
                    hwnd.0 as isize,
                    config.integrations.smtc,
                ) {
                    Ok(session) => {
                        found = Some(session.sender());
                        state.set_media(session);
                    }
                    Err(e) => tracing::warn!("SMTC disabled: {e}"),
                }
            }
            Some(Err(e)) => tracing::warn!("SMTC disabled (no HWND): {e}"),
            None => tracing::warn!("SMTC disabled (no main window)"),
        }
        found
    };
```
