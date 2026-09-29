# OxiTide — Track 5 Scrobbling

- **Status:** Draft for review
- **Date:** 2026-09-29
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 5, PM-D1, PM-D3)
- **Process note:** scope and decisions were brainstormed live with the owner
  (2026-09-29). This document is the standalone source of truth for Track 5.

---

## 1. Purpose & Outcome

Ship **scrobbling** — the Track 5 payoff and the first real use of the Connections
settings tab: one listen-event feed fans out to **Last.fm** and **ListenBrainz**,
independently connectable, toggleable, and retryable.

**Outcome:** after connecting a service in Settings → Connections, real plays appear
there. A track longer than 30 s scrobbles once it has actually been listened to for
half its length or 4 minutes; a now-playing ping goes out when a track starts and is
refreshed after a pause. Each service can be turned off without affecting the other.
A dropped network queues scrobbles for the rest of the session and drains them once
the service is reachable again. Missing metadata, exhausted retries, and invalid
credentials degrade quietly, except for one toast when a service needs reconnecting.

Success = both services scrobble the same plays independently, nothing new blocks or
polls playback, secrets stay in the Windows Credential Manager, and the whole feature
is inert until the user connects a service.

## 2. Scope

**In scope:** a Rust `ScrobblerService` fed by engine events; a pure `ScrobbleTracker`;
Last.fm and ListenBrainz clients with signing/payload tests; Last.fm browser auth +
session key; ListenBrainz token validation; keyring-backed credentials; an in-memory
per-service scrobble queue with backoff; a `[scrobbling]` config section and
`OXITIDE_LASTFM_*` env overrides; status/connect/disconnect/toggle commands; the
Connections-tab rows and a status event; tests and a manual smoke checklist.

**Out of scope:** queue persistence across restarts or backfill of past listens;
love/rating, recommendations, or any read API; per-track opt-out or private sessions;
a "scrobbled" indicator in Now Playing; Discord/SMTC presence (Track 6); any change to
playback, queue, catalog, or TIDAL auth; the Track 3 performance harness (PM-D1 still
applies as stated in §10).

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| S-D1 | Both services in one cycle, independent toggles, one shared feed. | PM-D3; no reason to serialize. |
| S-D2 | A Rust `ScrobblerService` in `oxitide-app`, fed by engine events. | All HTTP and secrets stay in Rust; the tracker is unit-testable; the service outlives player rebuilds. |
| S-D3 | The Last.fm app key/secret come from `OXITIDE_LASTFM_API_KEY`/`_SECRET` env vars, else `config.toml`; never committed. | Owner choice; personal use. |
| S-D4 | **Memory-only** per-service queue; retry within the session with backoff. | Owner choice; simplest, no queue file to manage. |
| S-D5 | No separate privacy toggle; the per-service toggles cover it. | Owner choice; fewer surfaces. |
| S-D6 | The Connections tab shows per-service status; a single toast on auth failure. | Owner choice; quiet by default. |
| S-D7 | Scrobble when the track is >30 s **and** accumulated listened time ≥ 50% or 4 min; now-playing at start and on resume after a pause. | Roadmap rules; accumulated (not raw) time resists seek over-counting. |
| S-D8 | Last.fm auth = browser authorize + session key; ListenBrainz = token paste. Secrets in the Windows Credential Manager; never logged. | Matches TIDAL's browser/paste auth; no embedded webview. |
| S-D9 | No queue persistence and no backfill of past listens. | Follows S-D4; YAGNI. |
| S-D10 | Icons via `Icon.svelte`, semantic tokens only, no mode branching. | Plan 10 constraints. |

## 4. Research summary

Verified against the providers' public documentation; the wiremock fixtures in §11.2
and the manual smoke are the confirmation gates.

**Last.fm** (`https://ws.audioscrobbler.com/2.0/`):

- All calls are signed. `api_sig` = MD5 hex of every parameter (excluding `format`
  and `callback`) sorted by name and concatenated as `name+value`, with the shared
  secret appended. Sent as a URL query for GET and form fields for POST.
- Auth: `auth.getToken` (signed) yields a token; the user authorizes at
  `https://www.last.fm/api/auth/?api_key={key}&token={token}`; `auth.getSession`
  (signed, with the token) returns `session.name` (username) and `session.key`
  (session key). Error `14` means the user has not authorized yet.
- Writes: `track.updateNowPlaying` and `track.scrobble` (form POST, session key,
  `artist`, `track`, `timestamp`, optional `album`, `duration`). `track.scrobble`
  batches up to 50 listens via indexed parameters. Timestamps are UTC epoch seconds of
  when the track *started*.
- Error codes: `4` auth failed, `6` invalid parameters, `9` invalid session key,
  `10` invalid API key, `16` temporarily unavailable, `26` suspended, `29` rate limit.

**ListenBrainz** (`https://api.listenbrainz.org/1/`):

- Auth is `Authorization: Token <token>`; the token is read from the user's
  `listenbrainz.org/profile/` page. `GET /1/validate-token` returns the username.
- Writes: `POST /1/submit-listens` with `listen_type` of `playing_now` (no timestamp)
  or `single` (with `listened_at`, UTC epoch seconds) and a payload of
  `track_metadata { artist_name, track_name, release_name?, additional_info }`.
- Rate limits are enforced; the worker serializes and paces requests and honors `429`.

No new TIDAL auth scopes are required.

## 5. Architecture & components

New dir module `crates/oxitide-app/src/scrobble/`:

### 5.1 `mod.rs` — `ScrobblerService`

Owns an injectable-base-URL `reqwest::Client`, a `LastFmClient` and a
`ListenBrainzClient`, the loaded credentials, per-service enable flags, queues, and
last-error/reconnect state — all behind mutexes. Construction starts a worker task
that owns both an `mpsc::Receiver<ScrobbleIntent>` and a 30 s tick.

- `on_player_event(&self, event: &EngineEvent)` is cheap: it feeds the tracker under a
  mutex and, on a `NowPlaying`/`Scrobble` intent, sends on the channel. It never does
  I/O, so the engine callback stays non-blocking.
- Async methods: `status()`, `lastfm_begin_auth()`, `lastfm_complete_auth()`,
  `lastfm_disconnect()`, `listenbrainz_connect(token)`, `listenbrainz_disconnect()`,
  `set_enabled(service, enabled)`.
- `emit_status()` sends a `scrobble://status` event on meaningful transitions only.

### 5.2 `tracker.rs` — pure `ScrobbleTracker`

No I/O. Consumes playback events and returns intents (`NowPlaying`, `Scrobble
{ item, started_at }`). Constants and rules in §6. Unit-tested exhaustively.

### 5.3 `lastfm.rs`

`api_sig(params, secret)` (pure MD5), serde DTOs, and `now_playing`, `scrobble`,
`get_token`, `get_session` methods. Base URL injectable for wiremock.

### 5.4 `listenbrainz.rs`

`validate`, `playing_now`, `submit_listen` (Bearer token). Base URL injectable.

### 5.5 `credentials.rs`

`CredentialStore` trait with `KeyringCredentialStore` (service `oxitide`, users
`lastfm` / `listenbrainz`) and `MemoryCredentialStore` for tests. Stores one JSON blob
per service (`{ "username": …, "secret": … }`) with a redacting `Debug`, mirroring
`TokenSet`.

### 5.6 `queue.rs`

Bounded per-service `VecDeque<QueuedScrobble { item, started_at }>`, deduped by
`(track_id, started_at)`, cap 1000 (overflow drops oldest, logged).

### 5.7 Integration

- `AppState` gains `scrobbler: Mutex<Option<Arc<ScrobblerService>>>`, set in `main()`
  beside `LyricsService`, so it persists across `build_player` rebuilds.
- `build_player` (`crates/oxitide-app/src/commands.rs:367`) adds one subscription that
  forwards each `EngineEvent` to `scrobbler.on_player_event`.
- `commands.rs` adds the status/connect/disconnect/toggle commands; all are listed in
  `generate_handler![]` in `main.rs` (AGENTS.md rule).

```
Engine worker ─EngineEvent─▶ subscribe ─on_player_event─▶ ScrobbleTracker (mutex)
                                                            │ NowPlaying / Scrobble
                                                            ▼
                                                   mpsc ─▶ scrobbler worker (tokio)
                                                            │ fans out to enabled services
                                                            ▼
                                                  Last.fm / ListenBrainz HTTP
                                                  failures → memory queue + backoff
UI ─invoke─▶ commands ─▶ service (auth, toggles, status)
```

### 5.8 Config, keyring, dependencies

`Config` gains a `#[serde(default)]` section so old configs still load:

```toml
[scrobbling]
lastfm_enabled = false
listenbrainz_enabled = false
lastfm_api_key = ""
lastfm_api_secret = ""
```

`main()` resolves the Last.fm key/secret as `env var` else config field. Usernames and
secrets live in the keyring, not the config. New dependencies in
`crates/oxitide-app/Cargo.toml`: `keyring = { version = "3", features =
["windows-native"] }` and an MD5 crate (`md-5`, consistent with the existing
RustCrypto `sha2`).

## 6. Listen-event semantics

The tracker is fed raw `EngineEvent`s.

- **`NowPlaying(Some(item))`** resets the tracker — even for the same track id, so
  `repeat=one` produces a fresh scrobble per loop. Records `started_at` (UTC epoch
  seconds), the item, `last_pos_ms = 0`, `listened_ms = 0`, `scrobbled = false`. Items
  with an empty artist or title are marked non-scrobblable (debug-logged).
- **`NowPlaying(None)` / stopped** ends tracking and discards an un-scrobbled play.
  Skipping below the threshold never scrobbles.
- **`Position { position_ms, duration_ms }`**: `delta = position_ms - last_pos_ms`. A
  forward step `0 < delta ≤ 3000 ms` is contiguous playback → `listened_ms += delta`.
  Anything else (backward seek, jump > 3 s) adds nothing and re-bases `last_pos_ms`.
  When `duration_secs > 30` and `listened_ms ≥ min(duration/2, 240_000 ms)` and the
  play is not yet scrobbled → emit `Scrobble { item, started_at }` once. (Half the
  duration or 4 minutes, **whichever comes first**, so the shorter bound wins.)
  Duration uses
  the engine's `duration_ms`, falling back to `item.duration_secs`.
- **`State` (pause/resume)**: nothing accumulates while paused; resuming after a pause
  ≥ 30 s re-emits `NowPlaying` (throttled) so Last.fm's live status stays fresh.
- Constants: `MIN_TRACK_SECS = 30`, `SCROBBLE_FRACTION = 0.5`, `SCROBBLE_MIN_MS =
  240_000`, `SEEK_MAX_MS = 3000`, `PAUSE_RESEND_MS = 30_000`.

## 7. Auth & credentials

**Last.fm** (key/secret per S-D3; when absent the row shows "API key not configured"
and Connect is disabled):

1. `auth.getToken` (signed) → token kept in memory; the command returns the authorize
   URL.
2. The UI opens `https://www.last.fm/api/auth/?api_key=…&token=…` through the existing
   `openUrl` helper.
3. `auth.getSession` (signed, with the token) is polled every ~3 s for ≤2 min; error
   `14` means "keep waiting". Success saves `{username, session_key}` to the keyring
   and flips the service to connected + enabled.

**ListenBrainz**: the UI submits a token; Rust calls `GET /1/validate-token` with the
Bearer token; success saves `{username, token}` and marks it connected + enabled; an
invalid token is an inline error, not a toast.

**Credentials**: loaded once at startup, redacting `Debug`, never logged. Disconnect
deletes the keyring entry, clears the username and queue, and turns the toggle off. If
the keyring is unavailable (non-Windows dev machine), connect fails with a clear
message and the service stays inert.

## 8. Queue, retry, and error handling

- Scrobbles that fail retryably are enqueued; the worker flushes oldest-first when the
  service is enabled, connected, and queued. Retryable failures back off exponentially
  (5 s → cap 5 min). Last.fm batches up to 50 scrobbles per request; ListenBrainz sends
  one `single` listen per request.
- Error taxonomy per client:
  - **Retryable** — network errors, 5xx, `429` (honoring `Retry-After`), Last.fm `16`.
  - **Auth** — Last.fm `9`/`4`/`10`, ListenBrainz 401/403.
  - **Permanent** — Last.fm `6`/`26`, ListenBrainz 400.
- Retryable → queue + backoff. Auth → stop that service, mark `needs_reconnect`, and
  emit **one** toast; the other service keeps working. Permanent → log and drop the
  item so it cannot poison the queue.
- Now-playing is best-effort: failures are logged but never queued or toasted (a stale
  ping is useless).
- Connect/validate commands return `Result<_, String>` for inline UI errors; runtime
  failures surface only through the toast (auth) and status fields.

## 9. UI — Connections tab

Replace the two `planned(...)` scrobble rows in `ui/src/views/Settings.svelte` with
live rows; Discord/SMTC stay "Planned".

- Each row: an enable toggle (disabled until connected), the service name, and status.
  Not connected → Connect; connected → "Connected as {username}", a pending count, a
  muted last-error line when present, and Disconnect.
- **Last.fm** connect shows "Waiting for authorization in your browser…" with a
  spinner, an "I've authorized" retry button, and Cancel.
- **ListenBrainz** connect is a password-style token input plus a link to
  `listenbrainz.org/profile/`; invalid tokens show inline.
- Status comes from `get_scrobble_status`, refetched on tab open and after actions; the
  `scrobble://status` event keeps an open tab fresh without polling.
- An auth failure raises one toast through the existing `toasts` store.
- New `ui/src/lib/scrobble.ts` holds pure status→label/disabled helpers for vitest.
- Icons via `Icon.svelte`; semantic tokens only; no mode branching.

### 9.1 IPC contract

```ts
type ScrobbleServiceId = "lastfm" | "listenbrainz";

interface ScrobbleServiceStatus {
  id: ScrobbleServiceId;
  enabled: boolean;
  connected: boolean;
  username: string | null;
  queued: number;
  needs_reconnect: boolean;
  last_error: string | null;
  configured: boolean; // Last.fm: key/secret present
}

interface ScrobbleStatusDto {
  lastfm: ScrobbleServiceStatus;
  listenbrainz: ScrobbleServiceStatus;
}
```

Commands (registered in `main.rs`): `get_scrobble_status`,
`lastfm_begin_auth`, `lastfm_complete_auth`, `lastfm_cancel_auth`,
`lastfm_disconnect`, `listenbrainz_connect(token)`, `listenbrainz_disconnect`,
`set_scrobble_enabled(service, enabled)`. Event: `scrobble://status` carrying
`ScrobbleStatusDto`.

## 10. Performance impact (PM-D1)

- Tracker: a few integer operations per ~1 Hz position event; allocates only on track
  change.
- No network when queues are empty — the worker sleeps on the channel and a 30 s tick;
  each now-playing/scrobble intent is one request.
- Startup: config parse plus one keyring read per configured service; no network.
- The queue is memory-only (S-D4), so no disk writes; toggles persist through the
  existing config save.
- The status event fires on transitions only, never per scrobble.
- No expected regression to cold start, time-to-first-sound, or scroll smoothness.

## 11. Testing & verification

### 11.1 Rust — pure

- `api_sig` against a known Last.fm signature vector.
- Tracker: 30 s minimum, the 50% and 4-min paths, forward and backward seeks ignored,
  repeat reset, no accumulation while paused, skip below threshold, empty-artist
  metadata skipped, and resume re-ping throttling.
- Queue: dedupe, cap/oldest-drop, backoff.
- Config: defaults fill old files, round-trip, and env-over-config precedence.
- Credentials: `MemoryCredentialStore` round-trip; redacting `Debug` hides secrets.

### 11.2 Rust — wiremock only (never live)

- Last.fm `getToken`/`getSession` including the `14` then success sequence; signed
  now-playing and scrobble requests asserting the exact `api_sig` and form fields; a
  50-item batch; error `9` → `needs_reconnect`; `16`/`429` → retry.
- ListenBrainz `validate-token` valid and invalid; `playing_now` and `single` payload
  shapes; 401 → auth; 5xx → retry.

### 11.3 UI — vitest

- `scrobble.ts` label/disabled logic across status combinations.

### 11.4 Manual (checklist in `docs/superpowers/plans/notes/`)

- Key/secret via env; Last.fm browser connect shows the username; ListenBrainz token
  connect succeeds.
- A >30 s track played past half appears on both services; now-playing is live and
  refreshes after a pause.
- Turning one service off stops only that service; the other still receives.
- Going offline mid-track queues the scrobble, and restoring the network drains it
  (within the session — memory-only).
- A revoked ListenBrainz token raises one auth toast and `needs_reconnect` without
  affecting Last.fm.
- Disconnect clears the keyring, flips the toggle off, and returns the row to Connect.
- A <30 s track and a pre-half skip never scrobble; `repeat=one` scrobbles each loop.
- Both modes and densities, reduced motion, and the minimum window size.

## 12. Risks & open verifications

- **Last.fm signing is the linchpin** — confirmed by the §11.2 fixture before any live
  run; the live smoke is the final gate and needs the key/secret.
- Browser approval is manual; the poll timeout and Cancel path must be clean.
- Seek accounting is not perfect (repeated sub-3 s seeks can accumulate slowly) —
  accepted and documented.
- ListenBrainz rate limits may delay a large backlog; the worker serializes and paces.
- The keyring is Windows-first; a dev machine without it fails connect cleanly.
- API drift: both providers are stable; parsing is version-tolerant with typed errors.

## 13. Future / explicitly deferred

- Queue persistence across restarts and Last.fm's 14-day backfill.
- Love/rating, recommendations, ListenBrainz feedback, and Last.fm read APIs.
- Discord Rich Presence and SMTC/hardware media keys (Track 6).
- Per-track opt-out, private sessions, and a "scrobbled" indicator in Now Playing.
