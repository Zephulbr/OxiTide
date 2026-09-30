# OxiTide — Track 6b: Discord Rich Presence

- **Status:** Draft for review
- **Date:** 2026-09-30
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` §3 Track 6
- **Process note:** scope, presence content, paused behavior, default state, transport, and
  worker approach were brainstormed live with the owner (2026-09-30). This document is the
  standalone source of truth for sub-track 6b; sub-track 6a (SMTC + media keys) is shipped.

---

## 1. Purpose & Outcome

Show what OxiTide is playing on the owner's Discord profile — the last remaining piece of
Track 6's integrations.

**Outcome:** while playback is active, the Discord profile card shows the track title,
artist, album art, a live progress bar, and a **Listen on TIDAL** button, under a
"Listening to OxiTide" status. Pausing swaps the state line to "Paused" and drops the
progress bar; resuming or seeking re-anchors it. Stopping clears the presence. The feature
is **off by default**, toggled in Settings → Connections, needs no Discord account linking
(local IPC only), and is **entirely inert** when Discord is not running, not installed, or
before the Discord application is registered.

Success = the profile card renders art/progress/button correctly, state changes track
within seconds, the toggle works live without a restart, and the integration never blocks
or affects playback.

## 2. Scope

**In scope:** a `discord-rich-presence` transport dependency; a `discord/` module in
`oxitide-app` with a pure, unit-tested presence assembler; a dedicated worker thread that
connects with backoff, coalesces/throttles updates, reconnects across Discord restarts, and
clears on disable/exit; reuse of the existing `media_controls` engine projection; the
`integrations.discord` config flag end-to-end (Rust config → `set_discord` command →
handler registration → `api.ts` → Settings toggle); the Discord application registration
checklist (owner task, Task 0 of the plan); a human GUI smoke checklist.

**Out of scope:** SMTC/media keys (6a, shipped); hand-rolling the Discord IPC protocol;
OAuth/Gateway presence or the official Social SDK; party/join/spectate/ask-to-join;
per-track privacy or listening-history features; rich presence for non-TIDAL sources;
non-Windows smoke testing; any change to playback, queue, catalog, auth, scrobbling, or UI
outside the one Settings row.

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| D1 | **Sub-track split:** this spec covers Discord Rich Presence only; 6a (SMTC) is a separate shipped spec. | Roadmap §1: each sub-track gets its own spec → plan → implementation cycle. |
| D2 | **Content: full + one button.** Details = track title, state = artist, large image = album art with album hover, small image = OxiTide logo, live progress bar, and one **Listen on TIDAL** button. | Owner choice; matches the official-client feel. |
| D3 | **Paused shows the track.** State becomes "Paused — {artist}" and timestamps are dropped; stopped/idle clears the presence. | Owner choice. "Frozen progress" is not representable over IPC — Discord ticks the bar client-side from timestamps — so pausing removes the bar instead. |
| D4 | **Off by default.** `integrations.discord = false`; the Settings toggle opts in. | Presence is visible to friends; opt-in is the least surprising. |
| D5 | **Transport: `discord-rich-presence` crate + our worker.** No hand-rolled IPC. | The maintained crate owns pipe discovery/framing/handshake/activity fields; our worker owns lifecycle, reconnect, and pacing. |
| D6 | **One worker thread fed by the existing projection.** Reuse `media_controls::{plan, SessionUpdate, TrackMeta, PlayState}`; one engine subscription fans out to SMTC and Discord. | Avoids a second mapping and duplicated `last_track` state; matches the SMTC worker pattern. |
| D7 | **Client ID is an embedded const.** The plan's Task 0 registers the Discord application and fills it in; not a config field. | The ID is public, non-secret, and single-tenant; Discord clients embed theirs too. |
| D8 | **Failures are silent and log-only.** Discord absent/unconfigured → inert worker; any IPC error disables the session quietly; playback is never affected. | Same posture as 6a D8; optional polish, never a startup dependency. |
| D9 | **Coalesce to newest; transitions send promptly; rebases throttled.** Track/playback changes send immediately, position-only rebases at most once per 5 s, and a 30 s refresh re-asserts the current activity. | Discord documents a ~1/15 s update guidance; timestamps make the bar tick without updates. |
| D10 | **External album art via URL; `logo` asset fallback.** Large image is the TIDAL CDN cover at 640 px; a track without a cover falls back to the uploaded `logo` asset. Only one art asset is required. | The crate accepts external URLs; the CDN is public; one upload keeps registration minimal. |
| D11 | **Listening activity type.** `ActivityType::Listening`, default status display (app name). | Listening gets Discord's time bar; "Listening to OxiTide" matches convention. |

## 4. Research summary

**Crate (`discord-rich-presence` 1.1.0, MIT, released 2026-01).** Cross-platform IPC
client (Windows named pipes / Unix sockets) with:
- `DiscordIpcClient::{new, connect, reconnect, close}` and the `DiscordIpc` trait's
  `set_activity`, `clear_activity`, `send`, `recv`.
- `activity::Activity` builders for `details`, `state`, `timestamps`, `assets`, `party`,
  `secrets`, `buttons` (max 2), `activity_type`, `status_display_type`.
- `activity::Assets::{large_image, large_text, small_image, small_text}` — the docs state
  "asset name **or URL**", and `activity::Timestamps::{start, end}` takes `i64`.
- `ActivityType::Listening` + both timestamps = the time bar.

**Discord IPC protocol (docs.discord.com/developers/topics/rpc + hard-mode.md).**
- Pipes `\\.\pipe\discord-ipc-0` … `-9`; clients try each and keep the first.
- Frames: little-endian `u32` opcode + `u32` length + JSON; handshake opcode 0 with
  `{ "v": 1, "client_id": "…" }`; `SET_ACTIVITY` on opcode 1 with
  `{ cmd, args: { pid, activity }, nonce }`; `PING` expects `PONG`; the client echoes
  every command back as a response.
- Update guidance is approximately one presence update per 15 s (SDK-era, still cited);
  timestamps make progress tick client-side, so we only send on change.
- External `large_image` URLs and buttons are documented fields; button clicks open the
  browser.

**In-repo integration points verified:**
- One engine fan-out in `build_player` (`crates/oxitide-app/src/commands.rs:398`) already
  forwards events; the SMTC sender is wired at `commands.rs:366-394` and its `set_smtc`
  command at `commands.rs:1287`.
- The cross-platform projection `media_controls::{PlayState, TrackMeta, SessionUpdate,
  cover_url, plan}` lives in `crates/oxitide-app/src/media_controls.rs:16-135` (the Windows
  WinRT worker is the separate `media_controls_win.rs`).
- `AppState.media: Mutex<Option<MediaSession>>` (`state.rs:42`, accessors `:136-142`,
  logout `:180`); `IntegrationsConfig { smtc }` (`config.rs:179-190`).
- Settings Connections row `Settings.svelte:634` (planned placeholder) and header copy
  `:554`; `api.ts:152` (`integrations`) and `:293` (`setSmtc`); handler list
  `main.rs:324`.

## 5. Architecture & components

### 5.1 Pure assembly — `crates/oxitide-app/src/discord/presence.rs` (new)

No I/O, no Discord types, unit-tested.

```rust
use crate::media_controls::{PlayState, TrackMeta};

pub struct ActivitySpec {
    pub details: String,            // track title, clipped
    pub state: String,              // artist, or "Paused — artist"
    pub large_image: String,        // CDN cover URL or "logo"
    pub large_text: String,         // album, or "OxiTide"
    pub small_image: &'static str,  // "logo"
    pub small_text: &'static str,   // "OxiTide"
    pub timestamps: Option<(i64, i64)>, // (start_ms, end_ms), unix epoch ms
    pub button_url: String,         // https://tidal.com/track/{track_id}
}

/// `None` means nothing is showing (no track or stopped).
pub fn assemble(
    track: Option<&TrackMeta>,
    play: PlayState,
    position_ms: u64,
    now_ms: i64,
) -> Option<ActivitySpec>;

/// True when the engine position has drifted from where the last sent
/// timestamps would have it (a seek).
pub fn should_rebase(
    last_start_ms: i64,
    last_sent_at_ms: i64,
    now_ms: i64,
    position_ms: u64,
) -> bool;

/// Clip to Discord's 128-character limit (char-boundary safe, ellipsis).
pub fn clip(text: &str) -> String;

pub fn track_url(track_id: u64) -> String;
```

Mapping rules:

- `track == None` or `play == Stopped` → `None`.
- `details` = `clip(&track.title)`; empty title → "Unknown track".
- `state` = playing: `clip(&track.artist)` (empty → "Unknown artist"); paused:
  `clip(&format!("Paused — {artist}"))` (empty artist → "Paused").
- `large_image` = `cover_url(&track.cover, 640)` when the cover resolves, else the asset
  key `"logo"`; `large_text` = album when non-empty, else `"OxiTide"`.
- `small_image = "logo"`, `small_text = "OxiTide"`.
- `timestamps` = `None` when paused or `duration_ms == 0`; otherwise
  `(now_ms - position_ms, now_ms - position_ms + duration_ms)` with `position_ms` clamped
  to `duration_ms` (the projection already clamps, but the assembler re-clamps).
- `button_url` = `https://tidal.com/track/{track_id}`.

`should_rebase` compares `position_ms` against
`last_start_ms + (now_ms - last_sent_at_ms)` and fires past a **2 s** drift. The worker
calls it only while playing.

### 5.2 Worker — `crates/oxitide-app/src/discord/mod.rs` (new)

Owns the connection lifecycle. Blocking crate, so a dedicated thread like the SMTC worker.

```rust
/// Filled by the plan's Task 0 after the Discord application is registered.
pub const DISCORD_CLIENT_ID: &str = "";

pub struct DiscordPresence { /* join handle, update sender, enable flag */ }

impl DiscordPresence {
    /// Spawns the worker. Never fails because Discord is absent or the client
    /// ID is unset — those degrade to an inert worker.
    pub fn spawn(enabled: bool) -> Result<Self, String>;
    /// Turns presence on/off without stopping the worker.
    pub fn set_enabled(&self, enabled: bool);
    /// Non-blocking update sender (bounded channel, drops when full).
    pub fn sender(&self) -> SessionSender;
}

impl Drop for DiscordPresence { /* clear + close + join */ }
```

`SessionSender` (`media_controls.rs:49`) is reused as the sender type; its constructor is
crate-visible.

Worker algorithm:

1. If `DISCORD_CLIENT_ID` is empty, log one warning ("Discord presence: client ID not
   configured") and park until shutdown.
2. While disabled: make no connection attempt; if previously connected, clear once and
   park.
3. Connect with `DiscordIpcClient::new(DISCORD_CLIENT_ID)` + `connect()`; failures back
   off 5 s → doubling → 60 s cap, retried; success resets the backoff and marks the
   desired activity unsent.
4. Loop: wake on the bounded channel or at least every 500 ms (which also bounds how long
   a toggle or shutdown waits). Drain all pending `SessionUpdate`s into local desired
   state (`track`, `play`, `position_ms`, `duration_ms`).
5. While enabled and connected, send when any of:
   - a track or play-state transition happened since the last successful send;
   - playing and `should_rebase(..)` and ≥5 s since the last send;
   - no successful send yet (connect/first/enable), or ≥30 s since the last send
     (periodic refresh that also self-heals a missed or dropped update).
6. A send builds `presence::assemble(.., now_ms)`. `Some(spec)` maps to
   `discord_rich_presence::activity::Activity` (`ActivityType::Listening`, assets,
   timestamps via `.start`/`.end`, one `Button`) and `set_activity`; `None` sends
   `clear_activity` only when an activity was previously set (tracked, so idling does not
   re-send clears). On IPC error: clear the client, reset send bookkeeping, and return to
   the connect loop; on success record `last_sent_at_ms`, `last_start_ms`, `last_track`,
   `last_play`, and whether an activity is currently set.
7. Enable→disable clears immediately; disable→enable marks unsent so the current track is
   re-sent.
8. Teardown: best-effort `clear_activity()`, then `close()`. Backoff sleeps are sliced so
   shutdown is prompt.

Response/PING handling (echoed responses, `PING`→`PONG`) is validated by Task 0; if the
crate's `set_activity` does not drain the inbound side, the worker adds a bounded drain of
one frame after each send (or a reader step), so the pipe cannot fill. This stays inside
`mod.rs`.

### 5.3 Engine projection & wiring — `commands.rs`

`build_player` keeps one `engine.subscribe` and computes `media_controls::plan` once for
all consumers:

- Create the SMTC sender (Windows, as today) and a Discord sender: spawn
  `DiscordPresence::spawn(config.integrations.discord)`, store the service via
  `state.set_discord(..)`, and keep its `SessionSender`.
- The subscription calls `plan(&event, *last)` unconditionally — even on non-Windows,
  where only Discord consumes it — and sends each update to the SMTC sender and/or the
  Discord sender (`try_send`, drops when full). It still emits `playback_event`.
- `replace_player`/`logout` drop the Discord service (`take_discord`) alongside
  `take_media`; a rebuild respawns it, so re-login works.

### 5.4 Config & IPC

`crates/oxitide-app/src/config.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationsConfig {
    /// Windows System Media Transport Controls (media keys + now-playing).
    pub smtc: bool,
    /// Discord Rich Presence (profile "Listening to OxiTide" activity).
    pub discord: bool,
}

impl Default for IntegrationsConfig {
    fn default() -> Self { Self { smtc: true, discord: false } }
}
```

Update the `Config { .. }` literal in `save_and_load_round_trip`; assert the default in
`partial_file_fills_defaults`; add `discord_defaults_false_and_round_trips`.

`crates/oxitide-app/src/state.rs`: `pub discord: Mutex<Option<DiscordPresence>>` with
`set_discord`/`take_discord`; `logout()` takes it.

`crates/oxitide-app/src/commands.rs`: `#[tauri::command] set_discord(state, enabled)` —
persist `config.integrations.discord`, save, `set_config`, then call `set_enabled` on the
live worker when present. Enabling before login takes effect at the next `build_player`.

`crates/oxitide-app/src/main.rs`: register `commands::set_discord` in
`generate_handler![]`, and declare `mod discord;`.

`crates/oxitide-app/Cargo.toml`: `discord-rich-presence = "1.1"` (cross-platform, no
target section). Its serde/serde_json/thiserror deps are already in the tree; it adds a
small transitive `uuid 0.8`.

### 5.5 Frontend

- `ui/src/lib/api.ts`: `Config.integrations.discord: boolean`;
  `api.setDiscord(enabled)` → `invoke<void>("set_discord", { enabled })`.
- `ui/src/views/Settings.svelte` (Connections): add `setDiscord` mirroring `setSmtc`
  (`:304-308`); replace the planned Discord row (`:634`) with a live checkbox row titled
  "Discord Rich Presence" and helper text ("Shows the current track on your Discord
  profile. Requires the Discord desktop client."); reword the header copy (`:554`) to
  "Scrobbling, media controls, and Discord Rich Presence are configurable below."; remove
  the now-unused `planned()` snippet (the SMTC row keeps the `.planned` container class).

## 6. Behavior specification

- **Track start/change:** the card shows title, artist, album art (album on hover), the
  OxiTide small logo, a live progress bar, and the Listen on TIDAL button; the status
  reads "Listening to OxiTide".
- **Pause:** state line becomes "Paused — {artist}", the progress bar disappears; resume
  re-anchors the bar from the current position.
- **Seek:** within ≤5 s of the next position update the bar is re-anchored; it never
  shows a large offset.
- **Skip / rapid skips:** the card converges to the latest track (updates coalesce; only
  the newest state is sent).
- **Stop / queue end:** the presence is cleared.
- **Enable/disable:** toggling off clears the presence immediately and stops updating;
  toggling on re-asserts the current state. No restart or re-login.
- **Restart:** the flag is read from config; on login the worker starts if enabled.
- **Logout / exit:** presence is cleared and the connection closed.
- **Discord absent:** nothing happens until Discord runs; once it does, presence appears.
- **Discord restart:** detected on the next send; the worker reconnects and re-asserts,
  and the 30 s refresh guarantees recovery even for a long paused/playing track.
- **No cover / no album:** `logo` asset fallback and "OxiTide" hover text.
- **Long fields:** title/artist/album clip at 128 characters.

## 7. Error handling & degradation

- Any connect/write/response error → `tracing` warn (connect failures at debug), retry
  with backoff; never panic, never surface to the UI, never touch playback.
- Empty client ID and Discord not installed are normal inert states (one startup log).
- An invalid asset key shows a blank image in Discord; the rest of the card still
  renders.
- Enable toggling persists through the existing config path; a save failure surfaces via
  the Settings `apply()` toast, as with every setting.
- The bounded update channel drops stale updates under pressure (`try_send`); the newest
  state always wins because the worker rebuilds the activity from its local state, not
  from the channel.

## 8. Performance (PM-D1)

- One background thread; parks on the channel with a 500 ms ceiling when idle; when
  disabled it does no network or IPC work.
- No polling loops and no per-second updates: IPC writes happen only on transitions,
  throttled rebases (≥5 s), and the 30 s refresh. Every send allocates a handful of short
  strings and the serialized frame.
- Startup adds a thread spawn and an off-main connect attempt; it never blocks the UI or
  audio path. No new allocations on the audio thread. No expected regression to cold
  start, time-to-first-sound, or scroll smoothness.

## 9. Testing & verification

**Unit (Rust, pure — `discord/presence.rs`):**
- Playing: details/state fields, `logo` small image, 640 cover URL from a dashed UUID,
  album hover, button URL, `(now − position, now − position + duration)` timestamps.
- Paused: "Paused — {artist}", no timestamps; empty artist → "Paused".
- Stopped/`None` track → `None`; duration 0 → no timestamps.
- No cover → `logo` large image; empty album → "OxiTide" hover.
- `clip`: under/over 128 chars, multibyte-safe.
- `should_rebase`: within 2 s → false; seek beyond 2 s → true; forward and backward.
- `track_url` shape.

**Rust (config):** `discord_defaults_false_and_round_trips`; `partial_file_fills_defaults`
asserts `discord == false`.

**Checks:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`;
`npm --prefix ui run check`; `npm --prefix ui test`.

**Owner Task 0 spike (in the plan), against real Discord:** register the application and
upload `logo`; run a scratch activity with an external TIDAL cover URL, one button, and
Listening timestamps; confirm the card, progress bar, button click, small image, and that
rapid updates and Discord restarts behave; confirm response/PING handling. Findings feed
Task 1 (no spec change expected).

**Human GUI smoke** (`docs/superpowers/plans/notes/2026-09-30-discord-rpc-smoke.md`):
toggle on with Discord running; card content while playing; pause/resume; seek; rapid
skips; stop/queue end; toggle off clears; toggle on restores without restart; app restart
honors the flag; logout and exit clear; Discord closed → started later; Discord restart
mid-track; no-cover track; long-title clipping; button opens TIDAL; playback and UI
unaffected.

## 10. Risks & open verifications

- **Update-rate limit.** The frequently cited guidance is ~1/15 s. Our coalescing, 5 s
  rebase floor, and 30 s refresh stay conservative; transitions send immediately, which is
  standard for music clients. If Discord drops bursts, the refresh converges and the plan
  can raise the floor — smoke verifies rapid skipping.
- **Crate maintenance.** 1.1.0 is recent (Jan 2026), MIT, cross-platform, and its feature
  surface exactly covers the card. It is pinned `"1.1"` and isolated inside `mod.rs`, so a
  future transport swap is contained.
- **External art URLs.** The crate documents URL support and Discord proxies external
  images; Task 0 confirms TIDAL CDN art renders. Fallback: `logo`-only (one-line change,
  already the no-cover path).
- **Response/PING handling.** The crate exposes `recv` but may not drain responses inside
  `set_activity`; Task 0 confirms. Any gap is a bounded drain in our worker.
- **Timestamps units.** Unix epoch **milliseconds** is the IPC convention; Task 0
  confirms the bar's accuracy before wiring.
- **Multi-client setups** (Discord stable/PTB/Canary): the first connectable pipe wins;
  presence lands on that client. OS-standard behavior, documented in the smoke.
- **Pre-registration:** an empty client ID keeps the worker inert with a single log; no
  errors, no user-visible noise.

## 11. Future / explicitly deferred

- A paused-overlay small image asset (a second upload; the text line covers it today).
- Party/join/spectate/ask-to-join and rich secrets — not meaningful for a local player.
- Per-track opt-out / private listening (the single toggle covers privacy for now).
- Non-Windows smoke (the transport is cross-platform; only Windows is verified now).
- Additional buttons (Discord caps at two).
- On completion, update the roadmap's Track 6 status to "6a + 6b shipped".
