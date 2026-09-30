# Discord Rich Presence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show what OxiTide is playing on the owner's Discord profile — track, artist, album art, live progress, small logo, and a "Listen on TIDAL" button — off by default, toggled in Settings, inert when Discord is absent.

**Architecture:** A `discord-rich-presence` IPC client is owned by a dedicated worker thread in `oxitide-app`; one pure module computes both the activity fields and the send schedule (coalesce/throttle/rebase), and the worker turns those into IPC calls with reconnect/backoff. The engine's existing single subscription feeds one shared `media_controls::plan` projection to SMTC and Discord.

**Tech Stack:** Rust (discord-rich-presence 1.1, Tauri 2 commands, serde TOML), TypeScript, Svelte 5 runes, PowerShell 5.1.

**Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track6b-discord-presence-design.md`

## Global Constraints

- **Do not commit.** The owner commits manually; each task ends with its verification commands instead of a commit step.
- Only new dependency: `discord-rich-presence = "1.1"` in `crates/oxitide-app/Cargo.toml`. No target-cfg section for it (cross-platform).
- No comments in new source code.
- The Discord module is cross-platform (no `cfg(windows)`); Windows is the only platform smoke-tested.
- The engine listener is `Fn(EngineEvent) + Send + Sync`; it must only ever `try_send` (never block) and must not capture `!Sync` values.
- IPC args are camelCase from the UI; Rust command params stay snake_case; `ui/src/lib/api.ts` interfaces are kept in sync by hand.
- Styling consumes semantic tokens only; no `data-mode` branching (AGENTS.md).
- The integration is a leaf: it must never affect the audio path; lifecycle failures log at `warn`, retry failures at `debug`, and playback continues.
- Exact values: channel capacity 64, worker wake 500 ms, rebase floor 5 s, refresh 30 s, seek drift 2 s, backoff 5 s → 60 s, field clip 128 chars, cover size 640, asset key `logo`, button label `Listen on TIDAL`, button URL `https://tidal.com/track/{id}`, activity type Listening, `integrations.discord` default `false`.
- Windows PowerShell 5.1; run all commands from the repo root `C:\Users\V\Documents\Opencode\OxiTide`.
- The throwaway probe from Task 0 must still pass `cargo clippy --workspace -- -D warnings`; it is deleted in Task 4.

## Review Focus

The spec implies these inputs/failure modes; each is pinned to a test in the task that owns it.

1. Rapid skips must converge to the latest track and never flood Discord (Task 1 `plan_action` tests + Task 6 smoke).
2. Discord absent at startup or restarted mid-track must recover with no user action (reconnect loop + 30 s refresh) (Task 1 refresh test + Task 6 smoke).
3. Pause must drop the progress bar and show "Paused — {artist}"; a stale/advancing bar is a defect (Task 1 `assemble` tests + Task 6 smoke).
4. A seek must re-anchor the bar within ~5 s and never leave a large offset (Task 1 `should_rebase`/`plan_action` tests + Task 6 smoke).
5. A track with no cover or missing metadata must still render a card (`logo` fallback, "Unknown track"/"Unknown artist") instead of blanking (Task 1 `assemble` tests + Task 6 smoke).

---

### Task 0: Spike — register the Discord app and confirm crate behavior

Throwaway. The deliverable is a findings note plus a registered application, not shipped code. The probe is deleted in Task 4.

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml`
- Create: `crates/oxitide-app/examples/discord_probe.rs` (throwaway)
- Create: `docs/superpowers/plans/notes/2026-09-30-discord-rpc-spike.md`

**Interfaces:**
- Produces: the confirmed client ID (`DISCORD_CLIENT_ID` value for Task 2), whether the crate echoes SET_ACTIVITY responses (drain requirement), external-art/button rendering, burst behavior, and whether a fresh `connect()` works after a Discord restart. Task 2's `drain_one` and reconnect-by-recreate approach assume these; if the findings contradict them, adapt Task 2 inside `discord/mod.rs` only.

- [ ] **Step 1: Owner registers the Discord application**

Owner actions, done once:

1. Open https://discord.com/developers/applications → **New Application** → name it `OxiTide`.
2. **General Information** → copy the **Application ID** (not secret).
3. **Rich Presence → Art Assets** → upload `logo` (512×512 PNG, transparent) and save.
4. Keep the Discord desktop client running and signed in.

Record the Application ID in the findings note (Step 4).

- [ ] **Step 2: Add the dependency and write the probe**

In `crates/oxitide-app/Cargo.toml`, after the `image = ...` line in `[dependencies]`:

```toml
discord-rich-presence = "1.1"
```

Create `crates/oxitide-app/examples/discord_probe.rs`:

```rust
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64
}

fn main() {
    let client_id = std::env::args().nth(1).expect("usage: discord_probe <client-id>");
    let cover =
        "https://resources.tidal.com/images/2fa8b7a6/9d5b/4a1e/9f0a/4a1e9f0a4a1e/640x640.jpg";

    let mut client = DiscordIpcClient::new(client_id.as_str());
    client.connect().expect("connect to Discord");
    println!("connected; open your Discord profile now (60s)");

    let start = now_ms() - 30_000;
    let activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .details("Probe Title")
        .state("Probe Artist")
        .assets(
            Assets::new()
                .large_image(cover)
                .large_text("Probe Album")
                .small_image("logo")
                .small_text("OxiTide"),
        )
        .timestamps(Timestamps::new().start(start).end(start + 240_000))
        .buttons(vec![Button::new("Listen on TIDAL", "https://tidal.com/track/373314259")]);
    client.set_activity(activity).expect("set activity");
    std::thread::sleep(Duration::from_secs(60));

    match client.recv() {
        Ok((op, value)) => println!("echo after set_activity: opcode {op}, {value}"),
        Err(e) => println!("recv after set_activity failed: {e}"),
    }

    for i in 0..10 {
        let start = now_ms();
        let burst = Activity::new()
            .activity_type(ActivityType::Listening)
            .details(format!("Burst {i}"))
            .state("Probe Artist")
            .timestamps(Timestamps::new().start(start).end(start + 60_000));
        if let Err(e) = client.set_activity(burst) {
            println!("burst {i} failed: {e}");
            break;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    for _ in 0..5 {
        match client.recv() {
            Ok((op, value)) => println!("burst echo: opcode {op}, {value}"),
            Err(e) => {
                println!("burst recv failed: {e}");
                break;
            }
        }
    }

    println!("quit and restart Discord now, then press Enter");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    let mut fresh = DiscordIpcClient::new(client_id.as_str());
    match fresh.connect() {
        Ok(()) => {
            let activity = Activity::new()
                .activity_type(ActivityType::Listening)
                .details("Reconnected")
                .state("Probe Artist");
            let _ = fresh.set_activity(activity);
            println!("fresh connect + resend OK (15s)");
            std::thread::sleep(Duration::from_secs(15));
            let _ = fresh.clear_activity();
            let _ = fresh.close();
        }
        Err(e) => println!("fresh connect failed: {e}"),
    }
}
```

- [ ] **Step 3: Run the probe and answer five questions**

Run: `cargo run -p oxitide-app --example discord_probe -- <application-id>`

While the probe is running, watch your Discord profile card, then observe the console. Answer:

1. Does the card show **Probe Title / Probe Artist**, the TIDAL album art, the `logo` small image, a progress bar that starts ~30 s in, and a working **Listen on TIDAL** button?
2. Does the post-sleep `recv()` print a SET_ACTIVITY echo (opcode + JSON), proving responses must be drained? Do any PINGs (opcode 3) arrive?
3. During the 10-update burst, which presences actually render in order? (Rate-limit behavior.)
4. After quitting and restarting Discord, does a fresh `DiscordIpcClient::connect()` succeed and re-show the card? Does the old client's next write error?
5. Does the progress bar show elapsed/total matching the unix-millisecond timestamps?

If the probe hangs at a `recv()` (no echo arrives), that is itself the answer: Task 2's `drain_one` must then be removed and sends left undrained; record it.

- [ ] **Step 4: Record findings**

Create `docs/superpowers/plans/notes/2026-09-30-discord-rpc-spike.md`:

```markdown
# Discord RPC spike findings (2026-09-30)

- Application ID (public, embed in Task 2): <…>
- `logo` asset uploaded: <yes | no>
- Card render (external art / small image / button / progress): <…>
- SET_ACTIVITY echo via recv: <opcode + sample | none — drain not needed | hang>
- PING frames seen: <yes | no>
- Burst of 10 updates 300 ms apart, rendered sequence: <…>
- Fresh connect after Discord restart: <works | fails — error>
- Timestamps unit confirmed (unix ms): <yes | no>
- Notes for Task 2: <anything that changes drain/reconnect code, or "none">
```

Run: `cargo clippy -p oxitide-app --example discord_probe -- -D warnings`
Expected: no warnings (fix the probe if it warns; keep it until Task 4).

---

### Task 1: Pure presence module (activity fields + send schedule)

**Files:**
- Create: `crates/oxitide-app/src/discord/mod.rs`
- Create: `crates/oxitide-app/src/discord/presence.rs`
- Modify: `crates/oxitide-app/src/main.rs` (module list, ~L3-14)

**Interfaces:**
- Consumes: `crate::media_controls::{cover_url, PlayState, TrackMeta}`.
- Produces (all in `discord::presence`): `ActivitySpec` (details/state/large_image/large_text/small_image/small_text/timestamps/button_url), `Desired { track: Option<TrackMeta>, play: PlayState, position_ms: u64 }`, `Sent { track_id: Option<u64>, play: PlayState, start_ms: i64, at_ms: i64, shown: bool }` with `Sent::NEVER`, `Action { None, Clear, Send(ActivitySpec) }`, `clip(&str) -> String`, `track_url(u64) -> String`, `assemble(Option<&TrackMeta>, PlayState, u64, i64) -> Option<ActivitySpec>`, `should_rebase(i64, i64, i64, u64) -> bool`, `plan_action(&Desired, &Sent, i64) -> Action`. Task 2 consumes `plan_action`, `Action`, `ActivitySpec`, `Desired`, `Sent`.

Both files start with `#![allow(dead_code)]` because nothing consumes them until Task 4; Task 4 removes those lines.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-app/src/discord/presence.rs`:

```rust
#![allow(dead_code)]

use crate::media_controls::{cover_url, PlayState, TrackMeta};

pub const COVER_SIZE: u32 = 640;
const FIELD_LIMIT: usize = 128;
const SEEK_DRIFT_MS: u64 = 2_000;
pub const REBASE_MIN_MS: i64 = 5_000;
pub const REFRESH_MS: i64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivitySpec {
    pub details: String,
    pub state: String,
    pub large_image: String,
    pub large_text: String,
    pub small_image: &'static str,
    pub small_text: &'static str,
    pub timestamps: Option<(i64, i64)>,
    pub button_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desired {
    pub track: Option<TrackMeta>,
    pub play: PlayState,
    pub position_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sent {
    pub track_id: Option<u64>,
    pub play: PlayState,
    pub start_ms: i64,
    pub at_ms: i64,
    pub shown: bool,
}

impl Sent {
    pub const NEVER: Sent =
        Sent { track_id: None, play: PlayState::Stopped, start_ms: 0, at_ms: 0, shown: false };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Clear,
    Send(ActivitySpec),
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000;

    fn meta(track_id: u64, cover: Option<&str>) -> TrackMeta {
        TrackMeta {
            track_id,
            title: format!("Title {track_id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            duration_ms: 200_000,
            cover: cover.map(str::to_string),
        }
    }

    fn desired(track: Option<TrackMeta>, play: PlayState, position_ms: u64) -> Desired {
        Desired { track, play, position_ms }
    }

    fn shown(track_id: u64, play: PlayState, start_ms: i64, at_ms: i64) -> Sent {
        Sent { track_id: Some(track_id), play, start_ms, at_ms, shown: true }
    }

    #[test]
    fn playing_maps_all_fields() {
        let spec = assemble(
            Some(&meta(7, Some("2fa8b7a6-9d5b-4a1e-9f0a-4a1e9f0a4a1e"))),
            PlayState::Playing,
            30_000,
            NOW,
        )
        .unwrap();
        assert_eq!(spec.details, "Title 7");
        assert_eq!(spec.state, "Artist");
        assert_eq!(
            spec.large_image,
            "https://resources.tidal.com/images/2fa8b7a6/9d5b/4a1e/9f0a/4a1e9f0a4a1e/640x640.jpg"
        );
        assert_eq!(spec.large_text, "Album");
        assert_eq!(spec.small_image, "logo");
        assert_eq!(spec.small_text, "OxiTide");
        assert_eq!(spec.timestamps, Some((NOW - 30_000, NOW - 30_000 + 200_000)));
        assert_eq!(spec.button_url, "https://tidal.com/track/7");
    }

    #[test]
    fn paused_prefixes_state_and_drops_timestamps() {
        let spec = assemble(Some(&meta(7, None)), PlayState::Paused, 30_000, NOW).unwrap();
        assert_eq!(spec.state, "Paused — Artist");
        assert_eq!(spec.timestamps, None);
        assert_eq!(spec.large_image, "logo");
        assert_eq!(spec.large_text, "OxiTide");
    }

    #[test]
    fn stopped_or_missing_track_is_none() {
        assert_eq!(assemble(None, PlayState::Stopped, 0, NOW), None);
        assert_eq!(assemble(None, PlayState::Playing, 0, NOW), None);
        assert_eq!(assemble(Some(&meta(7, None)), PlayState::Stopped, 0, NOW), None);
    }

    #[test]
    fn zero_duration_omits_timestamps() {
        let mut track = meta(7, None);
        track.duration_ms = 0;
        let spec = assemble(Some(&track), PlayState::Playing, 0, NOW).unwrap();
        assert_eq!(spec.timestamps, None);
    }

    #[test]
    fn position_past_duration_is_clamped() {
        let spec = assemble(Some(&meta(7, None)), PlayState::Playing, 250_000, NOW).unwrap();
        assert_eq!(spec.timestamps, Some((NOW - 200_000, NOW)));
    }

    #[test]
    fn empty_metadata_falls_back() {
        let mut track = meta(7, None);
        track.title = String::new();
        track.artist = String::new();
        track.album = String::new();
        let spec = assemble(Some(&track), PlayState::Playing, 0, NOW).unwrap();
        assert_eq!(spec.details, "Unknown track");
        assert_eq!(spec.state, "Unknown artist");
        assert_eq!(spec.large_text, "OxiTide");
        let paused = assemble(Some(&track), PlayState::Paused, 0, NOW).unwrap();
        assert_eq!(paused.state, "Paused");
    }

    #[test]
    fn clip_keeps_128_and_truncates_with_ellipsis() {
        let at_limit = "a".repeat(128);
        assert_eq!(clip(&at_limit), at_limit);
        let clipped = clip(&"a".repeat(129));
        assert_eq!(clipped.chars().count(), 128);
        assert!(clipped.ends_with('…'));
        assert_eq!(clip(&"é".repeat(200)).chars().count(), 128);
    }

    #[test]
    fn track_url_shape() {
        assert_eq!(track_url(42), "https://tidal.com/track/42");
    }

    #[test]
    fn should_rebase_threshold() {
        let start = NOW - 30_000;
        assert!(!should_rebase(start, NOW, NOW, 30_000));
        assert!(!should_rebase(start, NOW, NOW, 32_000));
        assert!(should_rebase(start, NOW, NOW, 32_001));
        assert!(should_rebase(start, NOW, NOW, 27_000));
    }

    #[test]
    fn plan_action_shows_first_and_suppresses_repeats() {
        let d = desired(Some(meta(7, None)), PlayState::Playing, 0);
        assert!(matches!(plan_action(&d, &Sent::NEVER, NOW), Action::Send(_)));
        let s = shown(7, PlayState::Playing, NOW, NOW);
        assert_eq!(plan_action(&d, &s, NOW + 1_000), Action::None);
    }

    #[test]
    fn plan_action_clears_when_idle() {
        let none = desired(None, PlayState::Stopped, 0);
        assert_eq!(plan_action(&none, &Sent::NEVER, NOW), Action::None);
        let s = shown(7, PlayState::Playing, NOW, NOW);
        assert_eq!(plan_action(&none, &s, NOW), Action::Clear);
    }

    #[test]
    fn plan_action_sends_on_track_and_play_changes() {
        let s = shown(7, PlayState::Playing, NOW, NOW);
        let next_track = desired(Some(meta(8, None)), PlayState::Playing, 0);
        assert!(matches!(plan_action(&next_track, &s, NOW + 1_000), Action::Send(_)));
        let paused = desired(Some(meta(7, None)), PlayState::Paused, 0);
        assert!(matches!(plan_action(&paused, &s, NOW + 1_000), Action::Send(_)));
    }

    #[test]
    fn plan_action_refreshes_after_thirty_seconds() {
        let d = desired(Some(meta(7, None)), PlayState::Playing, 998_999);
        let s = shown(7, PlayState::Playing, NOW - 31_000, NOW);
        assert_eq!(plan_action(&d, &s, NOW + 29_999), Action::None);
        assert!(matches!(plan_action(&d, &s, NOW + 30_000), Action::Send(_)));
    }

    #[test]
    fn plan_action_rebases_a_seek_but_not_faster_than_five_seconds() {
        let s = shown(7, PlayState::Playing, NOW - 30_000, NOW);
        let seeked = desired(Some(meta(7, None)), PlayState::Playing, 90_000);
        assert_eq!(plan_action(&seeked, &s, NOW + 1_000), Action::None);
        assert!(matches!(plan_action(&seeked, &s, NOW + 5_000), Action::Send(_)));
    }

    #[test]
    fn plan_action_never_rebases_while_paused() {
        let s = shown(7, PlayState::Paused, NOW, NOW);
        let d = desired(Some(meta(7, None)), PlayState::Paused, 90_000);
        assert_eq!(plan_action(&d, &s, NOW + 6_000), Action::None);
    }
}
```

Create `crates/oxitide-app/src/discord/mod.rs`:

```rust
#![allow(dead_code)]

mod presence;
```

Add to `crates/oxitide-app/src/main.rs` module list (after `mod config;`):

```rust
mod discord;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app discord`
Expected: FAIL to compile — `clip`, `track_url`, `assemble`, `should_rebase`, `plan_action` are not defined.

- [ ] **Step 3: Implement the pure module**

In `crates/oxitide-app/src/discord/presence.rs`, insert the implementation between the `Action` enum and the `#[cfg(test)]` module:

```rust
pub fn clip(text: &str) -> String {
    if text.chars().count() <= FIELD_LIMIT {
        return text.to_string();
    }
    let mut out: String = text.chars().take(FIELD_LIMIT - 1).collect();
    out.push('…');
    out
}

pub fn track_url(track_id: u64) -> String {
    format!("https://tidal.com/track/{track_id}")
}

pub fn assemble(
    track: Option<&TrackMeta>,
    play: PlayState,
    position_ms: u64,
    now_ms: i64,
) -> Option<ActivitySpec> {
    let track = track?;
    if play == PlayState::Stopped {
        return None;
    }
    let artist_empty = track.artist.trim().is_empty();
    let details =
        if track.title.trim().is_empty() { "Unknown track".to_string() } else { clip(&track.title) };
    let artist =
        if artist_empty { "Unknown artist".to_string() } else { clip(&track.artist) };
    let state = if play == PlayState::Paused {
        if artist_empty { "Paused".to_string() } else { clip(&format!("Paused — {artist}")) }
    } else {
        artist
    };
    let (large_image, large_text) =
        match track.cover.as_deref().and_then(|cover| cover_url(cover, COVER_SIZE)) {
            Some(url) => (
                url,
                if track.album.trim().is_empty() {
                    "OxiTide".to_string()
                } else {
                    clip(&track.album)
                },
            ),
            None => ("logo".to_string(), "OxiTide".to_string()),
        };
    let timestamps = if play == PlayState::Playing && track.duration_ms > 0 {
        let position = position_ms.min(track.duration_ms);
        let start = now_ms - position as i64;
        Some((start, start + track.duration_ms as i64))
    } else {
        None
    };
    Some(ActivitySpec {
        details,
        state,
        large_image,
        large_text,
        small_image: "logo",
        small_text: "OxiTide",
        timestamps,
        button_url: track_url(track.track_id),
    })
}

pub fn should_rebase(last_start_ms: i64, last_sent_at_ms: i64, now_ms: i64, position_ms: u64) -> bool {
    let expected = last_start_ms + (now_ms - last_sent_at_ms);
    let drift = (position_ms as i64 - expected).unsigned_abs();
    drift > SEEK_DRIFT_MS
}

pub fn plan_action(desired: &Desired, sent: &Sent, now_ms: i64) -> Action {
    let Some(spec) = assemble(desired.track.as_ref(), desired.play, desired.position_ms, now_ms)
    else {
        return if sent.shown { Action::Clear } else { Action::None };
    };
    let track_changed = sent.track_id != desired.track.as_ref().map(|track| track.track_id);
    let play_changed = sent.play != desired.play;
    let refresh_due = now_ms - sent.at_ms >= REFRESH_MS;
    let rebase_due = desired.play == PlayState::Playing
        && sent.shown
        && now_ms - sent.at_ms >= REBASE_MIN_MS
        && should_rebase(sent.start_ms, sent.at_ms, now_ms, desired.position_ms);
    if !sent.shown || track_changed || play_changed || refresh_due || rebase_due {
        Action::Send(spec)
    } else {
        Action::None
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app discord`
Expected: all `discord::presence::tests` PASS.

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings (`#![allow(dead_code)]` is deliberate until Task 4).

---

### Task 2: Discord worker and service

**Files:**
- Modify: `crates/oxitide-app/src/discord/mod.rs`

**Interfaces:**
- Consumes: `discord_rich_presence::{DiscordIpc, DiscordIpcClient}` and `discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps}`; `crate::media_controls::{SessionSender, SessionUpdate, PlayState}`; `presence::{plan_action, Action, ActivitySpec, Desired, Sent}`.
- Produces: `discord::DISCORD_CLIENT_ID` (const, filled from Task 0), `discord::DiscordPresence::{spawn(bool) -> Result<Self, String>, sender() -> SessionSender, set_enabled(bool)}` with `Drop` = clear + close + join. Task 4 consumes all three.

- [ ] **Step 1: Write the worker module**

Replace the contents of `crates/oxitide-app/src/discord/mod.rs` with:

```rust
#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Button, Timestamps};
use discord_rich_presence::error::Error;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::media_controls::{PlayState, SessionSender, SessionUpdate};

mod presence;

use presence::{plan_action, Action, ActivitySpec, Desired, Sent};

pub const DISCORD_CLIENT_ID: &str = "";

const CHANNEL_CAPACITY: usize = 64;
const LOOP_TIMEOUT: Duration = Duration::from_millis(500);
const BACKOFF_START: Duration = Duration::from_secs(5);
const BACKOFF_CAP: Duration = Duration::from_secs(60);
const SEND_FAILURE_DELAY: Duration = Duration::from_secs(1);
const BUTTON_LABEL: &str = "Listen on TIDAL";

pub struct DiscordPresence {
    tx: Option<SyncSender<SessionUpdate>>,
    enabled: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl DiscordPresence {
    pub fn spawn(enabled: bool) -> Result<Self, String> {
        let (tx, rx) = mpsc::sync_channel::<SessionUpdate>(CHANNEL_CAPACITY);
        let flag = Arc::new(AtomicBool::new(enabled));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_flag = flag.clone();
        let worker_shutdown = shutdown.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-discord".into())
            .spawn(move || worker(rx, worker_flag, worker_shutdown))
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

impl Drop for DiscordPresence {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.tx.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn apply(desired: &mut Desired, update: SessionUpdate) {
    match update {
        SessionUpdate::Track(track) => {
            desired.track = track;
            desired.position_ms = 0;
            if desired.track.is_none() {
                desired.play = PlayState::Stopped;
            }
        }
        SessionUpdate::Playback(play) => desired.play = play,
        SessionUpdate::Position { position_ms, .. } => desired.position_ms = position_ms,
    }
}

fn drain_one(client: &mut DiscordIpcClient) -> Result<(), Error> {
    for _ in 0..3 {
        match client.recv()? {
            (3, payload) => client.send(payload, 4)?,
            _ => return Ok(()),
        }
    }
    Ok(())
}

fn set_activity(
    client: &mut DiscordIpcClient,
    spec: &ActivitySpec,
) -> Result<(), Error> {
    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .details(spec.details.as_str())
        .state(spec.state.as_str())
        .assets(
            Assets::new()
                .large_image(spec.large_image.as_str())
                .large_text(spec.large_text.as_str())
                .small_image(spec.small_image)
                .small_text(spec.small_text),
        )
        .buttons(vec![Button::new(BUTTON_LABEL, spec.button_url.as_str())]);
    if let Some((start, end)) = spec.timestamps {
        activity = activity.timestamps(Timestamps::new().start(start).end(end));
    }
    client.set_activity(activity)?;
    drain_one(client)
}

fn worker(rx: Receiver<SessionUpdate>, enabled: Arc<AtomicBool>, shutdown: Arc<AtomicBool>) {
    if DISCORD_CLIENT_ID.is_empty() {
        tracing::warn!("Discord presence disabled: client ID not configured");
        while !shutdown.load(Ordering::Relaxed) {
            std::thread::sleep(LOOP_TIMEOUT);
        }
        return;
    }

    let mut client: Option<DiscordIpcClient> = None;
    let mut desired = Desired { track: None, play: PlayState::Stopped, position_ms: 0 };
    let mut sent = Sent::NEVER;
    let mut backoff = BACKOFF_START;
    let mut next_connect = Instant::now();

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let want = enabled.load(Ordering::Relaxed);
        if !want && sent.shown {
            if let Some(c) = client.as_mut() {
                let _ = c.clear_activity();
            }
            sent = Sent::NEVER;
        }
        if want && client.is_none() && Instant::now() >= next_connect {
            let mut fresh = DiscordIpcClient::new(DISCORD_CLIENT_ID);
            match fresh.connect() {
                Ok(()) => {
                    tracing::info!("Discord presence connected");
                    client = Some(fresh);
                    sent = Sent::NEVER;
                    backoff = BACKOFF_START;
                }
                Err(e) => {
                    tracing::debug!("Discord presence unavailable: {e}");
                    next_connect = Instant::now() + backoff;
                    backoff = (backoff * 2).min(BACKOFF_CAP);
                }
            }
        }

        match rx.recv_timeout(LOOP_TIMEOUT) {
            Ok(update) => {
                apply(&mut desired, update);
                while let Ok(update) = rx.try_recv() {
                    apply(&mut desired, update);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if want && client.is_some() {
            let now = now_ms();
            match plan_action(&desired, &sent, now) {
                Action::None => {}
                Action::Clear => {
                    match client.as_mut().map(|c| c.clear_activity()) {
                        Some(Ok(())) => sent = Sent::NEVER,
                        Some(Err(e)) => {
                            tracing::debug!("Discord presence clear failed: {e}");
                            client = None;
                            next_connect = Instant::now() + SEND_FAILURE_DELAY;
                        }
                        None => {}
                    }
                }
                Action::Send(spec) => {
                    match client.as_mut().map(|c| set_activity(c, &spec)) {
                        Some(Ok(())) => {
                            sent = Sent {
                                track_id: desired.track.as_ref().map(|track| track.track_id),
                                play: desired.play,
                                start_ms: spec.timestamps.map(|(start, _)| start).unwrap_or(now),
                                at_ms: now,
                                shown: true,
                            };
                        }
                        Some(Err(e)) => {
                            tracing::debug!("Discord presence update failed: {e}");
                            client = None;
                            next_connect = Instant::now() + SEND_FAILURE_DELAY;
                        }
                        None => {}
                    }
                }
            }
        }
    }

    if let Some(client) = client.as_mut() {
        if sent.shown {
            let _ = client.clear_activity();
        }
        let _ = client.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_service_spawns_sends_and_drops() {
        let service = DiscordPresence::spawn(false).expect("spawn");
        service.sender().send(SessionUpdate::Playback(PlayState::Paused));
        drop(service);
    }
}
```

- [ ] **Step 2: Fill in the client ID**

Set `pub const DISCORD_CLIENT_ID` to the Application ID recorded in Task 0 (a public, non-secret ID string). If the spike has not run, leave it `""`; the worker logs one warning and stays inert.

- [ ] **Step 3: Verify**

Run: `cargo test -p oxitide-app discord`
Expected: `discord::presence::tests` and `discord::tests` PASS (the disabled service test takes ≤ ~500 ms).

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings.

---

### Task 3: `integrations.discord` config flag

**Files:**
- Modify: `crates/oxitide-app/src/config.rs` (`IntegrationsConfig` ~L179-190; round-trip literal ~L357; partial defaults ~L402; new test after `smtc_defaults_true_and_round_trips` ~L644)

**Interfaces:**
- Consumes: nothing.
- Produces: `IntegrationsConfig.discord: bool` (default `false`), serialized as `discord = false` under `[integrations]`. Task 4 and Task 5 consume it.

- [ ] **Step 1: Write the failing test**

Append after `smtc_defaults_true_and_round_trips` in `crates/oxitide-app/src/config.rs`:

```rust
    #[test]
    fn discord_defaults_false_and_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        assert!(!Config::default().integrations.discord);
        let mut config = Config::default();
        config.integrations.discord = true;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("[integrations]"), "{text}");
        assert!(text.contains("discord = true"), "{text}");
        assert!(Config::load_from(&path).integrations.discord);
        let _ = fs::remove_dir_all(dir);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app discord_defaults_false_and_round_trips`
Expected: FAIL to compile — no field `discord` on `IntegrationsConfig`.

- [ ] **Step 3: Implement the flag**

In `crates/oxitide-app/src/config.rs`, replace `IntegrationsConfig` with:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationsConfig {
    /// Windows System Media Transport Controls (media keys + now-playing).
    pub smtc: bool,
    /// Discord Rich Presence ("Listening to OxiTide" profile activity).
    pub discord: bool,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self { smtc: true, discord: false }
    }
}
```

Update the `save_and_load_round_trip` literal (`integrations: IntegrationsConfig { smtc: false },`) to:

```rust
            integrations: IntegrationsConfig { smtc: false, discord: true },
```

Add to `partial_file_fills_defaults`, after `assert!(config.integrations.smtc);`:

```rust
        assert!(!config.integrations.discord);
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app config`
Expected: all config tests PASS, including the new one.

---

### Task 4: AppState, build_player wiring, and the `set_discord` command

**Files:**
- Modify: `crates/oxitide-app/src/state.rs` (field ~L42, accessors ~L136-142, `logout` ~L180)
- Modify: `crates/oxitide-app/src/commands.rs` (`build_player` ~L366-413, new command after `set_smtc` ~L1296)
- Modify: `crates/oxitide-app/src/main.rs` (handler list ~L324)
- Modify: `crates/oxitide-app/src/discord/mod.rs` and `presence.rs` (remove the first `#![allow(dead_code)]` line from each)
- Delete: `crates/oxitide-app/examples/discord_probe.rs`

**Interfaces:**
- Consumes: `discord::{DiscordPresence, DISCORD_CLIENT_ID}`, `IntegrationsConfig.discord`, `media_controls::{plan, SessionSender, SessionUpdate}`.
- Produces: `AppState::{set_discord, take_discord}`; `commands::set_discord`; the fan-out from one `plan` call to both senders. Task 5 consumes the command name and config field.

- [ ] **Step 1: Add the state field and accessors**

In `crates/oxitide-app/src/state.rs`, after `pub media: Mutex<Option<crate::media_controls::MediaSession>>,`:

```rust
    pub discord: Mutex<Option<crate::discord::DiscordPresence>>,
```

After `take_media`, add:

```rust
    pub fn set_discord(&self, service: crate::discord::DiscordPresence) {
        *self.discord.lock().unwrap() = Some(service);
    }

    pub fn take_discord(&self) -> Option<crate::discord::DiscordPresence> {
        self.discord.lock().unwrap().take()
    }
```

In `logout`, next to `let _ = self.take_media();`:

```rust
        let _ = self.take_discord();
```

- [ ] **Step 2: Fan out the engine projection in `build_player`**

In `crates/oxitide-app/src/commands.rs`, replace the block from `#[cfg(windows)]\n    let sender = {` through the end of the first `engine.subscribe(...)` call (`}));`, ~L366-413) with:

```rust
    let state = app.state::<AppState>();
    let _ = state.take_media();
    let _ = state.take_discord();
    #[cfg(windows)]
    let smtc_sender = {
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
    #[cfg(not(windows))]
    let smtc_sender: Option<crate::media_controls::SessionSender> = None;
    let discord_sender = match crate::discord::DiscordPresence::spawn(config.integrations.discord) {
        Ok(service) => {
            let sender = service.sender();
            state.set_discord(service);
            Some(sender)
        }
        Err(e) => {
            tracing::warn!("Discord presence disabled: {e}");
            None
        }
    };

    let handle = app.clone();
    let last_track = std::sync::Mutex::new(None);
    engine.subscribe(Arc::new(move |event| {
        if let EngineEvent::Error { code, .. } = &event {
            if code == "auth" {
                handle.state::<AppState>().notify_session_expired();
            }
        }
        if smtc_sender.is_some() || discord_sender.is_some() {
            let mut last = last_track.lock().unwrap();
            let (updates, next) = crate::media_controls::plan(&event, *last);
            *last = next;
            for update in updates {
                if let Some(sender) = &smtc_sender {
                    sender.send(update.clone());
                }
                if let Some(sender) = &discord_sender {
                    sender.send(update);
                }
            }
        }
        let _ = tauri::Emitter::emit(&handle, "playback_event", crate::player::to_dto(event));
    }));
```

Note: the old code created a `state` guard inside each cfg arm; the new code hoists one guard and the non-Windows arm no longer takes `media` itself.

- [ ] **Step 3: Add the `set_discord` command**

In `crates/oxitide-app/src/commands.rs`, after `set_smtc`:

```rust
#[tauri::command]
pub fn set_discord(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.integrations.discord = enabled;
    config.save()?;
    state.set_config(config);
    if let Some(service) = state.discord.lock().unwrap().as_ref() {
        service.set_enabled(enabled);
    }
    Ok(())
}
```

- [ ] **Step 4: Register the command**

In `crates/oxitide-app/src/main.rs`, in `generate_handler![...]`, after `commands::set_smtc,`:

```rust
            commands::set_discord,
```

- [ ] **Step 5: Remove the temporary dead-code allows and the probe**

Remove the first line (`#![allow(dead_code)]`) from `crates/oxitide-app/src/discord/mod.rs` and `crates/oxitide-app/src/discord/presence.rs`.

Run: `Remove-Item -LiteralPath "crates\oxitide-app\examples\discord_probe.rs"`

- [ ] **Step 6: Verify**

Run: `cargo test -p oxitide-app`
Expected: all tests PASS.

Run: `cargo clippy -p oxitide-app -- -D warnings`
Expected: no warnings (if a dead-code warning appears it names a genuinely unused item — remove it rather than re-adding the allow).

Run: `cargo build -p oxitide-app`
Expected: success.

---

### Task 5: Frontend toggle

**Files:**
- Modify: `ui/src/lib/api.ts` (`Config.integrations` ~L152-154; `api` object ~L293)
- Modify: `ui/src/views/Settings.svelte` (handlers ~L304-308; `planned` snippet ~L542-553; header copy ~L554; Discord row ~L634)

**Interfaces:**
- Consumes: the `set_discord` command and `integrations.discord` (Task 4).
- Produces: `Config.integrations.discord`; `api.setDiscord(enabled)`; the Settings checkbox.

- [ ] **Step 1: Add the API type and wrapper**

In `ui/src/lib/api.ts`, extend the integrations object:

```ts
  integrations: {
    smtc: boolean;
    discord: boolean;
  };
```

In the `api` object, after `setSmtc`:

```ts
  setDiscord: (enabled: boolean) => invoke<void>("set_discord", { enabled }),
```

- [ ] **Step 2: Add the Settings handler**

In `ui/src/views/Settings.svelte`, after `setSmtc`:

```ts
  function setDiscord(enabled: boolean) {
    if (!config) return;
    config = { ...config, integrations: { ...config.integrations, discord: enabled } };
    api.setDiscord(enabled).catch(apply);
  }
```

- [ ] **Step 3: Replace the planned row with a live toggle**

In the Connections panel, delete the `planned` snippet (`{#snippet planned(name: string, description: string)} … {/snippet}`), reword the intro line, and replace the `{@render planned("Discord Rich Presence", …)}` line. The result around the "Presence & controls" heading:

```svelte
      <p class="muted">Scrobbling, media controls, and Discord Rich Presence are configurable below.</p>
```

```svelte
      <h3>Presence & controls</h3>
      <div class="planned">
        <div class="planned-main">
          <label class="planned-head">
            <input
              type="checkbox"
              checked={config.integrations.discord}
              onchange={(e) => setDiscord((e.target as HTMLInputElement).checked)}
            />
            <span>Discord Rich Presence</span>
          </label>
          <span class="muted">
            Shows the current track on your Discord profile. Requires the Discord desktop client.
          </span>
        </div>
      </div>
```

The SMTC row below keeps its existing `class="planned"` container unchanged. Confirm no other `planned(...)` render remains: `rg -n "planned\(" ui/src/views/Settings.svelte` (expect no matches; if `rg` is unavailable use `Select-String -Path "ui\src\views\Settings.svelte" -Pattern "planned\("`).

- [ ] **Step 4: Verify types and tests**

Run: `npm --prefix ui run check`
Expected: 0 errors.

Run: `npm --prefix ui test`
Expected: full suite PASS.

---

### Task 6: Smoke checklist and full verification

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-30-discord-rpc-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the human verification record and the full check run.

- [ ] **Step 1: Write the human smoke checklist**

Create `docs/superpowers/plans/notes/2026-09-30-discord-rpc-smoke.md`:

```markdown
# Discord Rich Presence — manual smoke checklist

Requires a real account, a running signed-in Discord desktop client, and the registered
application from Task 0. Record date and result per item.

- [ ] Settings → Connections → enable "Discord Rich Presence"; the choice persists after a
      restart and is honored on login.
- [ ] Play a track: the Discord profile card shows title, artist, album art (hover shows the
      album), the OxiTide small logo, a progress bar anchored to the real position, and a
      "Listen on TIDAL" button that opens the track in a browser.
- [ ] Status reads "Listening to OxiTide".
- [ ] Pause: the state line shows "Paused — {artist}" and the progress bar disappears; resume
      re-anchors the bar from the current position.
- [ ] Seek: within a few seconds the bar re-anchors and never shows a large offset.
- [ ] Rapid skips (5+ in a row): the card converges to the latest track.
- [ ] Stop playback / clear the queue: the presence clears.
- [ ] Toggle the setting off while playing: the presence clears; toggle on: it reappears with
      the current track, no restart needed.
- [ ] Discord closed at app start, then started: the presence appears without restarting
      OxiTide.
- [ ] Quit and restart Discord while a track plays: the presence returns within ~30 s.
- [ ] A track with no album art shows the OxiTide logo instead of a blank image.
- [ ] A track whose title/artist exceed 128 characters renders clipped, without errors.
- [ ] Logout and app exit: the presence clears; no hang.
- [ ] Playback, scrobbling, SMTC, and the UI are unaffected throughout.
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
Expected: the window opens; walk the checklist above and record the results. If the spike's
findings contradicted any assumption, note the deviation alongside the results.

---

## Roadmap follow-up

After this ships, update `docs/superpowers/specs/2026-09-24-oxitide-post-mvp-roadmap.md` §1:
set Track 6's Status to "6a (SMTC/media keys) + 6b (Discord RPC) done". Do this only when the
owner confirms the smoke checklist passed.
