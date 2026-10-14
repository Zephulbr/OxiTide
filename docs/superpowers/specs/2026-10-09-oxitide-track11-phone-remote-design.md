# OxiTide — Track 11 Phone Remote

- **Status:** **Closed 2026-10-11** — owner smoke passed (Linux item deferred); completion note
  `plans/notes/2026-10-10-track11-completion.md`; plan record (with refinements and the smoke
  round 1 changes) `plans/2026-10-09-track11-phone-remote.md`
- **Date:** 2026-10-09
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (casting / TIDAL Connect, parked in
  Track 7; this track replaces the "control OxiTide from the phone" goal of that item);
  `plans/notes/2026-10-09-tidal-connect-receiver-spike.md` (why the TIDAL-app route was not
  taken); `plans/notes/2026-10-02-listen-along-assessment.md` (shared session-transport
  primitive)
- **Process note:** the owner asked for casting / TIDAL Connect research on 2026-10-09. The
  spike proved a TIDAL-app receiver is only possible by running TIDAL's own binary (which
  needs TIDAL desktop installed) because the phone verifies a TIDAL-issued certificate. The
  owner chose a self-contained alternative. This document is the standalone source of truth
  for Track 11 and adds the track to the roadmap series.

---

## 1. Purpose & Outcome

Track 11 lets a phone control a running OxiTide over the local network, with nothing to
install on the phone and no dependency on TIDAL's software: OxiTide serves a small remote
control page, the phone opens it by scanning a QR code, and from then on the phone can
see what is playing and drive playback, the queue, search, and the library.

**Outcome:**

- Settings → Remote enables a local server (**off by default**) and shows a QR code that
  pairs a phone in one scan (§6.2).
- The phone shows now-playing (cover, title, artist, progress, quality) and controls
  play/pause, previous/next, seek, volume, shuffle, repeat, and fade-and-pause (§6.5).
- The phone shows and edits the queue: jump to an item, reorder, remove, clear (§6.5).
- The phone searches and browses the same catalogue as the desktop: Home, Explore, search,
  albums, artists, playlists, mixes, favorites, the user's playlists and folders, and the
  local history; it can start playback from any of them, add to the queue, toggle
  favorites, and add tracks to playlists (§6.5).
- Several phones can be paired; each can be revoked from the desktop at any time (§6.2).
- The feature adds **no cost when disabled**, no cloud relay, no account, and exposes no
  credentials or settings (§5, §6.3).

**Success =** every scripted gate stays green (`cargo test --workspace`,
`cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`,
`npm --prefix ui test`) on Windows and under the WSL2 Linux gates; the §7 smoke checklist
passes on a real phone against a real account; the performance budgets of PM-D1 do not
regress with the feature disabled **or** enabled and idle (§8); Track 11 closes through a
completion note with an explicit verified/exempted split (§10).

## 2. Scope

**In scope (committed):**

- The server: listener lifecycle, LAN-only admission, host/origin guards, port selection,
  interface selection for the QR URL.
- Pairing and the device store: one-time pairing token, per-device tokens (hashed at rest),
  optional desktop approval, list/revoke, rate limiting.
- The transport: one invoke-shaped RPC endpoint with an explicit method allowlist, and a
  server-sent-events stream with snapshot + deltas.
- The remote page: a second Vite entry (Svelte 5 runes) covering Now Playing, Queue,
  Search, Home/Explore, Library, detail pages, History, Lyrics.
- Settings UI (new "Remote" section), commands, DTOs, config block, tests, smoke checklist.
- Platform: Windows and Linux (the server is platform-neutral; Linux follows Track 8 seams).

**Out of scope (non-goals):**

- TIDAL Connect / appearing in the TIDAL phone app (spike note); casting to other devices.
- The phone as an audio output, or any audio streaming to the phone.
- Internet-facing access, relays, NAT traversal, accounts, TLS certificates managed by the
  app (users who want remote access use a VPN such as Tailscale, which the LAN-only filter
  admits, §6.1).
- Changing app settings from the phone: output device, DSP, quality, scrobbling, hotkeys,
  appearance, cache, login/logout, window/mini-player control (§6.3 excludes them).
- Playlist/folder management (create, rename, delete, move), sleep-timer countdowns
  (the timer lives in the desktop UI; only the immediate fade-and-pause is exposed),
  per-device permission levels, push notifications, a native phone app, PWA install features
  that require a secure context.
- Controlling OxiTide while it is closed (Wake-on-LAN, autostart). The tray keeps it
  resident (Track 10); that is the only background story.

## 3. Decisions

Owner-confirmed 2026-10-09: RM-D1 (web remote, not the TIDAL app), RM-D2 (off by default,
lightweight/unintrusive), and the full scope in §2. **All other rows are proposals for the
owner to confirm or change at review** (notably pairing, approval default, HTTP-only,
cover loading, port, and the dependency choices).

| # | Decision | Rationale |
|---|---|---|
| RM-D1 | **OxiTide's own web remote; not the TIDAL app.** The Connect route (run TIDAL's SDK binary as a sidecar) stays documented in the spike note and is not built. | The phone rejects any receiver certificate not issued by TIDAL; the only compliant path needs TIDAL desktop installed. Owner prefers no dependency. |
| RM-D2 | **Opt-in, off by default.** No listener, thread, or allocation when disabled; nothing on the startup path. | Unintrusive; PM-D1. |
| RM-D3 | **LAN-only admission.** Accept loopback, RFC 1918, link-local, CGNAT `100.64/10` (Tailscale), IPv6 ULA/link-local. Reject everything else at accept, before any HTTP parsing. | The server is never meant to be internet-reachable even if the user port-forwards by accident. |
| RM-D4 | **QR pairing with per-device tokens.** A pairing token (128-bit, single-use, 5-minute TTL) rides in the URL **fragment**; exchanging it yields a 256-bit device token stored as a SHA-256 hash and delivered as an `HttpOnly; SameSite=Strict` cookie. **Desktop approval defaults on** (`remote.approve_new_devices`). | Fragment keeps the token out of logs/Referer; hashing limits a leaked `remote.json`; approval blocks a shoulder-surfed QR. |
| RM-D5 | **One RPC endpoint + one SSE stream.** `POST /api/rpc` with `{method, params}` (camelCase params, as `invoke`), `GET /api/events` for state. No websockets. `axum` is the single new HTTP dependency (built on `hyper`/`tower`/`tokio`, already in `Cargo.lock`); `qrcode` renders the QR as SVG on the Rust side. Verify both with `cargo tree` in plan 11.1. | Smallest surface to audit; SSE needs no websocket crate and works through every browser/proxy. |
| RM-D6 | **Reuse the existing command functions.** RPC handlers call the `commands::*` functions with `AppHandle.state::<AppState>()`; no logic is duplicated, so content filters, track-flag enrichment, and `fail_tidal` session handling apply unchanged. | The same behaviour on every control surface; matches how SMTC/MPRIS/hotkeys reach the engine. |
| RM-D7 | **Explicit method allowlist** (§6.3). Anything not listed — auth, config setters, devices, DSP, cache, window, shell, logs — is unreachable from the remote. | A paired phone must not be able to change security-relevant state. |
| RM-D8 | **Slim state + paged queue.** `PlaybackState` embeds the whole queue; the remote protocol strips it, sends `queueLen` and `queueRev`, and serves queue pages on request. | A 10k-item queue (perf scenario S7) must never ride every event. |
| RM-D9 | **Frontend is a second Vite entry** (`ui/remote.html`, Svelte 5 runes, shared semantic tokens), embedded with the app and served by the same server. It imports API **types** only from `ui/src/lib/api.ts` and never `@tauri-apps/api`. | Same conventions/tests as the main UI; no second toolchain. |
| RM-D10 | **Covers load directly from `resources.tidal.com`** (the phone needs internet for artwork); no image proxy in v1. | Keeps the server thin; the CDN is already the CSP allowlist for the app. |
| RM-D11 | **Plain HTTP on the LAN** in v1. The token cookie is `HttpOnly`/`SameSite=Strict`, Host and Origin are validated, and the residual risk (a passive sniffer on the same Wi-Fi can read the cookie) is documented. HTTPS is an open item (§9). | Self-signed certificates produce browser warnings that defeat "scan and go"; the threat model is a home network. |
| RM-D12 | **Session seam, not a Connect plan.** The module keeps a transport-agnostic `RemoteSession` boundary (commands in, slim state out) so another front end (e.g. the TIDAL Connect sidecar, listen-along) can reuse it later. Nothing but the web front end is built now. | Costs nothing now; keeps the Connect option open. |
| RM-D13 | **Config:** `[remote]` in `config.toml` (`enabled`, `port`, `interface`, `approve_new_devices`); device records in `remote.json` (versioned, atomic tmp+rename). Tokens and pairing secrets are never logged, never in `config.toml`. | Preferences vs state split already used by Track 10 (QoL-D5). |

## 4. Research summary

1. **Why not the TIDAL app** (`plans/notes/2026-10-09-tidal-connect-receiver-spike.md`):
   the phone-facing Connect server is a TLS websocket whose certificate must chain to
   "TIDAL TS CA"; a self-signed receiver is rejected (`unknown ca`). The only compliant
   receiver is TIDAL's own binary, shipped inside TIDAL desktop, which cannot be bundled.
2. **Existing control seams.** `media_controls.rs` maps external transport events to
   `ControlCommand` → `to_engine_cmd` → `engine.handle(...)` (SMTC on Windows, MPRIS on
   Linux). `engine.subscribe(...)` in `commands.rs` fans every `EngineEvent` out to the
   UI, tray, session saver, Discord, scrobbler, and history; the remote is one more
   subscriber. `AppState` is managed by Tauri, so any thread holding an `AppHandle` can
   reach it with `handle.state::<AppState>()`.
3. **Tauri commands are ordinary functions.** `#[tauri::command] fn x(state: State<'_,
   AppState>, ...)` can be called directly with `handle.state::<AppState>()`; the RPC layer
   needs no refactor of command bodies.
4. **Networking baseline.** The workspace has `reqwest`/`hyper`/`tokio`/`tower` in
   `Cargo.lock`; `axum`, `tungstenite`, `qrcode`, and a mDNS crate are absent. The workspace
   `tokio` enables `macros`, `rt-multi-thread`, `sync`, `time` only — `net` must be added.
5. **Static assets.** `frontendDist` is `ui/dist`; Tauri's asset resolver can serve a second
   HTML entry from the embedded dist in release builds (verify; dev uses the Vite server on
   1420, §9).
6. **Windows firewall.** A listener on a new executable path triggers the inbound-rule
   prompt; denying creates a persistent Block rule (observed in the spike: a denied
   `node.exe` silently dropped mDNS). The UI must explain this (§6.6).
7. **Browser capabilities over HTTP.** Wake Lock, service workers, and Media Session
   lock-screen controls need a secure context, so the page cannot keep the phone screen on
   or install as a full PWA; "Add to Home Screen" with a standalone meta tag still works.

## 5. Architecture

```
phone browser ──LAN──▶ [peer filter] ─▶ axum (Host/Origin/cookie guards)
   ▲                                       ├─ GET  /remote…       embedded page assets
   │ SSE: snapshot + deltas                ├─ POST /api/pair      token → device cookie
   └───────────────────────────────────────├─ GET  /api/events    SSE hub
                                           └─ POST /api/rpc       allowlist ─▶ commands::*
                                                                        │
              Engine ──subscribe──▶ remote hub (slim state, queueRev) ◀─┘ AppState / Engine
```

### 5.1 Modules and files

New module tree `crates/oxitide-app/src/remote/`:

- `mod.rs` — `RemoteService` (start/stop, status), held in `AppState` as
  `Mutex<Option<Arc<RemoteService>>>` with `set_remote`/`remote` accessors like the other
  services. Disabled = `None`.
- `server.rs` — listener, peer filter, router, graceful shutdown, port fallback.
- `guard.rs` — pure helpers: `is_lan_peer(IpAddr)`, `host_allowed(&str, &[IpAddr])`,
  `origin_allowed`, cookie parsing, rate limiter. All unit-tested without sockets.
- `pairing.rs` — pairing tokens, device store (`remote.json`), hashing, approval flow.
- `rpc.rs` — the allowlist dispatcher and wire types.
- `events.rs` — the hub: engine subscriber → slim state → `tokio::sync::broadcast`.
- `qr.rs` — URL + QR SVG for the pairing flow.

State lives under the existing app-data root (`%APPDATA%\OxiTide\remote.json`) using the
`config.rs` atomic write pattern. No new files besides `remote.json`.

### 5.2 Frontend

- `ui/remote.html` and `ui/src/remote/**` (Svelte 5 runes only). A Vite multi-page build
  adds the second entry to `ui/dist`; `vite.config.ts` gains `build.rollupOptions.input`
  for `index.html` and `remote.html`.
- `ui/src/remote/rpc.ts` (fetch + SSE client, reconnect/resync), `store.svelte.ts`
  (playback state, queue pages), `pages/*.svelte`. Types are imported type-only from
  `ui/src/lib/api.ts`; a vitest guard fails if the remote entry imports `@tauri-apps/api`
  or any main-shell module.
- Styling consumes **semantic tokens only** (`tokens.css`); the remote follows the Atmosphere
  look with the default accent (it never reads `--dyn-*` or the Rack tokens). Update
  `ui/design/DESIGN.md` with a short "Phone remote" section (AGENTS.md rule).
- The page is served with its own CSP: `default-src 'self'; img-src 'self'
  https://resources.tidal.com data:; connect-src 'self'; script-src 'self'; style-src 'self'
  'unsafe-inline'; frame-ancestors 'none'; base-uri 'self'`. The webview CSP in
  `tauri.conf.json` is unaffected (the phone is not a webview), and no new remote origin is
  added to it.

### 5.3 Config, commands, events

- `[remote]` in `config.toml`: `enabled = false`, `port = 47820`, `interface = ""` (empty =
  auto), `approve_new_devices = true`.
- New `#[tauri::command]`s (also listed in `generate_handler!`): `get_remote_status`,
  `set_remote_enabled`, `set_remote_settings`, `begin_remote_pairing`,
  `cancel_remote_pairing`, `list_remote_devices`, `revoke_remote_device`,
  `resolve_remote_pair_request`.
- New Tauri events to the desktop UI: `remote_status` (listening address, connected count),
  `remote_pair_request` (a phone is waiting for approval).
- DTOs are Rust-first; mirror them in `ui/src/lib/api.ts` by hand (IPC contract).

## 6. Features

### 6.1 Server and lifecycle (Plan 11.1)

- `set_remote_enabled(true)` binds `[::]:port` (dual-stack; falls back to `0.0.0.0`),
  trying `port … port+9`; the chosen port is written back to status, not to config. Binding
  failure surfaces as a status error, never a panic.
- Peer filter on `accept` (RM-D3): drop non-admitted peers before reading a byte. Because
  Tailscale addresses (`100.64/10`) are admitted, a VPN gives remote access with no extra
  feature.
- `Host` must be an IP literal that belongs to this machine, or `localhost`; anything else is
  `403` (DNS-rebinding defence). `Origin`, when present, must equal the `Host` origin.
- At most 8 concurrent SSE connections and 32 in-flight RPC requests; excess get `503`.
- Request bodies are capped at 64 KB; responses at 4 MB (larger lists must use the paged
  methods, §6.3).
- The QR URL uses the address of the chosen interface (default: the interface with the
  default route; ties break toward `192.168/16`, `10/8`, `172.16/12`; virtual adapters such
  as WSL/Hyper-V `vEthernet` are deprioritised). Settings lets the user override from a list.
- Disable (or logout, or quit) stops accepting, closes all SSE streams with a final
  `revoked` event, and drops the hub. The device store persists.
- Startup: when `enabled`, the server starts **after** the window is shown and
  `player_built`, off the critical path (§8). While logged out the server runs but every
  RPC returns `not_logged_in` and the page shows "Sign in on the PC".

### 6.2 Pairing, devices, and security (Plan 11.2)

**Pairing flow**

1. Settings → Remote → **Pair a phone** (`begin_remote_pairing`) generates a 128-bit token
   (OS RNG), valid 5 minutes, single-use, one outstanding at a time, and renders
   `http://<ip>:<port>/remote#p=<token>` as text and a QR SVG.
2. The phone loads the page; the script reads the fragment, then `POST /api/pair`
   `{token, deviceName}` (`deviceName` is the phone-supplied label, sanitised, ≤ 40 chars).
3. If `approve_new_devices` is on, the desktop shows "Pair '<name>' from <ip>?" and
   `resolve_remote_pair_request` decides; the phone waits (60 s timeout). Off = automatic.
4. On approval the server consumes the token, creates the device (id, name, created,
   last-seen), stores the **hash** of a fresh 256-bit device token, and answers with
   `Set-Cookie: oxi_remote=<token>; HttpOnly; SameSite=Strict; Path=/; Max-Age=31536000`.
   The page then strips the fragment from the address bar.
5. Every later request must carry the cookie; the server compares hashes in constant time
   and updates `last_seen` at most once a minute.

**Rules**

- Failed pairings and failed cookie checks are rate limited (5 per minute per peer IP,
  global ceiling 30 per minute); lockouts return `429` with `Retry-After`.
- `POST` requests require `Content-Type: application/json` and the header
  `X-Oxi-Remote: 1` (a custom header cannot be sent cross-site without a preflight), on top
  of `SameSite=Strict` and the Origin check.
- Revoking a device (`revoke_remote_device`) deletes its hash and closes its SSE streams
  immediately. Settings lists name, last seen, and connected state.
- Secrets never appear in logs, panics, error strings, or `remote.json` in clear; the
  fragment token is never sent to a server log because fragments are not transmitted.
- What a paired phone can reach is exactly §6.3. It cannot read tokens, credentials, file
  paths, or configuration.

**Accepted residual risk (RM-D11):** on plain HTTP a passive sniffer on the same network can
read the device cookie. Mitigations: LAN-only admission, revocable devices, visible
connected/last-seen state. HTTPS with a certificate fingerprint carried in the QR is the
open item to revisit (§9).

### 6.3 RPC and the method allowlist (Plan 11.3)

Wire format: `POST /api/rpc` body `{"method": "<name>", "params": {…}}` → `{"ok": true,
"result": …}` or `{"ok": false, "error": "<code>: <message>"}`. Params use camelCase like
`invoke`; Rust parameter names stay snake_case and the dispatcher maps them. Unknown methods
return `unknown_method`; unauthenticated calls `401`; a logged-out desktop `not_logged_in`.

| Group | Methods (existing command functions) |
|---|---|
| Transport | `play_pause`, `next`, `prev`, `seek`, `set_volume` (never persists: `persist=false`), `set_shuffle`, `set_repeat`, `fade_and_pause`, `retry` |
| Playback start | `play_track_ids`, `play_page_items` |
| Queue | `queue_add`, `queue_remove`, `queue_reorder`, `queue_clear`, plus the remote-only `get_queue_page` (§6.4) and `queue_jump {index}` |
| State | the remote-only `get_remote_state` (slim snapshot) |
| Discovery | `search`, `get_home`, `get_explore`, `get_for_you`, `get_favorite_mixes`, `get_mix`, `get_track_radio`, `get_artist_radio`, `get_similar_artists` |
| Catalogue | `get_album`, `get_album_tracks`, `get_album_info`, `get_playlist`, `get_playlist_tracks_page`, `get_artist_page`, `get_artist_top_tracks_page`, `get_artist_albums_all`, `get_track`, `get_lyrics` |
| Library | `get_favorite_tracks_page`, `get_favorite_albums`, `get_favorite_artists`, `get_favorite_ids`, `get_user_playlists`, `get_collection_items`, `get_history_page` |
| Writes | `set_favorite_track`, `set_favorite_album`, `set_favorite_artist`, `add_playlist_tracks` |

`queue_jump {index}` mirrors what the desktop queue rail does (`play_tracks(queue, index)`),
but takes the queue from the engine's own snapshot on the server, so the phone never uploads
queue items and `play_tracks` itself stays excluded.

**Not exposed (examples):** all `auth_*`/`finish_login`, every `set_*` that changes
configuration or devices (`set_output_device`, `set_exclusive*`, `set_dsp_settings`,
`set_preferred_quality`, `set_ui_mode`, `set_hotkey`, `set_discord`, …), scrobble and cache
commands, `play_tracks` (client-built queue items), `open_external`, logs, perf, window and
mini-player commands, folder/playlist management, `clear_history`, `get_config`.
A test enumerates the `generate_handler!` list against the allowlist so a new command is
denied by default and a deliberate exposure is a reviewed change.

**Bounds.** Unbounded commands (`get_favorite_tracks`, `get_playlist_tracks`) are excluded in
favour of paged variants; the dispatcher enforces the 4 MB response cap and rejects
`limit > 200`. Playback start accepts at most 5000 ids per call.

### 6.4 State model and event stream (Plan 11.3)

**Slim state** (`RemoteState`, serialised camelCase): `loggedIn`, `playing`, `buffering`,
`current` (`trackId`, `title`, `artist`, `album`, `cover`, `durationSecs`, `explicit`,
`audioQuality`), `currentIndex`, `positionMs`, `durationMs`, `volume`, `shuffle`, `repeat`,
`queueLen`, `queueRev`, `stream` (`codec`, `sampleRate`, `bitDepth`, `bitrateKbps`),
`bitPerfect`, `dspStages`. The queue itself is never in it (RM-D8).

**`queueRev`** is a counter the hub increments when the queue contents change (compared by
length + an id hash computed only while at least one SSE client is connected). The phone
refetches pages with `get_queue_page {offset, limit}` when `queueRev` moves.

**SSE (`GET /api/events`)** — `text/event-stream`, one stream per phone:

| Event | Payload | Sent |
|---|---|---|
| `snapshot` | full `RemoteState` | on connect and after any resync |
| `state` | full `RemoteState` | on engine `State`/`NowPlaying` events (coalesced to ≤ 10/s) |
| `position` | `{positionMs, durationMs}` | at most 1 Hz while playing |
| `error` | `{code, message}` | engine errors |
| `session` | `loggedOut` \| `expired` \| `ok` | session changes |
| `revoked` | — | device revoked or remote disabled; the server then closes |

Each event carries an incrementing `id:`; on reconnect the client always receives a fresh
`snapshot` (no replay buffer). A 15-second `:keep-alive` comment keeps proxies and mobile
radios from idling the stream. A slow client is dropped (bounded `broadcast` lag) and
resyncs by reconnecting.

The hub subscribes with `engine.subscribe(...)` like the existing listeners and must never
block the engine thread: it only converts and `send`s on a `broadcast` channel, and does
nothing at all while no SSE client is connected.

### 6.5 The remote page (Plans 11.4, 11.5)

Single page app, mobile-first, no routing library (hash routes), light/dark follows
`prefers-color-scheme`.

- **Now Playing** (default): cover, title/artist (artist/album link to their pages),
  progress with seek, play/pause, previous/next, shuffle, repeat, volume, a "Pause with fade"
  action, quality / bit-perfect / DSP badges, a heart toggle for the current track, lyrics
  (synced highlight when available; plain otherwise), and a link to the queue. A compact
  mini bar persists on every other page.
- **Queue:** paged list with the current item marked; tap to jump (the remote-only
  `queue_jump {index}`, §6.3), button to remove, drag handle to reorder, Clear. Large queues
  load pages on scroll.
- **Search:** one box, results grouped as tracks/albums/artists/playlists (the existing
  `search` shape), tap-to-play with "Play from here", and a per-row menu with "Add to
  queue", "Add to playlist…" and Favorite. There is no "Play next": the engine only appends.
- **Home / Explore / For You / Mixes:** the existing page-feed sections rendered as
  horizontal shelves; items open the corresponding detail page or start playback.
- **Library:** Favorites (tracks, albums, artists), Playlists and folders (read-only
  browse), History (recently played, from `history.jsonl`).
- **Detail pages:** album, artist (top tracks, albums, similar), playlist, mix. Each offers
  Play, Shuffle, and per-row actions as above.
- **States:** connecting, reconnecting (with a visible banner), signed out on the PC
  ("Sign in on the PC"), device revoked (returns to the pairing screen), PC asleep or app
  closed (retry with backoff; never a blank page).
- **Accessibility:** semantic controls, visible focus, labelled icons, touch targets ≥ 44 px;
  Lucide icons only (no unicode glyphs), in line with the main UI rules.

### 6.6 Settings UI (Plan 11.2)

New **Remote** section in `ui/src/views/Settings.svelte`:

- Master toggle (shows the actual address and port while listening, and a clear error if
  binding failed).
- Interface picker (Auto + detected addresses), port, "Approve new devices on this PC".
- **Pair a phone** → dialog with QR + URL + countdown + cancel; the approval prompt
  surfaces here (and as a tray notification if the window is hidden).
- Device list: name, last seen, connected dot, Revoke.
- Windows guidance: "Windows may ask to allow OxiTide through the firewall — choose
  *Private networks*. If a phone cannot connect, check Windows Security → Firewall → Allow an
  app." Linux guidance names `ufw allow <port>/tcp`.
- A note that artwork loads from TIDAL's CDN so the phone needs internet access.

### 6.7 The session seam (RM-D12)

`remote/mod.rs` exposes a `RemoteSession` trait-shaped boundary — `command(RemoteCommand)`
in, `subscribe() -> RemoteState stream` out — implemented by the web front end only.
Documented purpose: a later TIDAL Connect sidecar or listen-along transport can map its
messages onto the same boundary without touching the engine. No other implementation or
configuration is added in this track.

## 7. Testing and verification

**Scripted (Windows and the WSL2 Linux gates):**

- `guard.rs`: peer admission table (loopback, RFC 1918, link-local, `100.64/10`, public,
  IPv6 ULA/link-local/global, IPv4-mapped IPv6), Host/Origin validation including
  DNS-rebinding cases, cookie parsing, rate limiter.
- `pairing.rs`: token TTL, single use, one outstanding token, approval accept/deny/timeout,
  device hash round-trip, constant-time compare, revoke, `remote.json` versioning and
  atomic write (the `config.rs` test pattern).
- `rpc.rs`: every allowlisted method dispatches; an unknown or excluded method is denied; a
  test compares the `generate_handler!` list to the allowlist so new commands default to
  denied; param mapping (camelCase), response cap, `limit` cap.
- `events.rs`: slim-state conversion strips the queue, `queueRev` changes iff the queue
  changes, coalescing, 1 Hz position throttle, nothing computed with zero clients, lag drop.
- Server integration tests on an ephemeral port using `reqwest`: pairing happy path, bad
  token, replay, missing cookie, wrong Host/Origin, oversize body, SSE snapshot then delta,
  revoke closes the stream, disable closes the stream, `not_logged_in` while logged out.
- UI (`vitest`, jsdom): the remote store (snapshot/delta merge, reconnect resync, queue page
  invalidation by `queueRev`), RPC client error mapping, the "no Tauri imports" guard, the
  Settings Remote section.
- `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `npm --prefix ui run check`, `npm --prefix ui test` all green.

**Human smoke (real phone, real account)** — recorded in
`docs/superpowers/plans/notes/`: pair via QR, approval prompt, second phone, revoke one;
every Now Playing control; seek while playing; queue jump/reorder/remove/clear on a long
queue; search → play, add to queue, add to playlist, favorite; Home/Library/detail pages;
lyrics; PC logout and re-login behaviour; PC sleep/wake; phone on cellular cannot reach the
server; app disabled mid-session; Windows firewall prompt both ways; Linux build smoke if
available.

## 8. Performance (PM-D1)

- **Disabled:** zero runtime cost, nothing on the startup path; S1/S2 budgets unchanged.
- **Enabled and idle:** one parked listener task; the engine subscriber exits immediately
  when no SSE client is connected (no queue clone, no hashing).
- **Active:** per-event work is a slim-state conversion; position sends are 1 Hz; queue
  hashing only on `State` events and only with a client connected.
- Server start runs after the window is shown on a runtime task, so S1 cold start stays
  within budget; the PM-D1 gate (S1–S3, S5–S7) is re-run with the feature **enabled and
  one phone connected** before closure.
- The main bundle is unaffected: the remote page is a separate entry and is not loaded by
  the shell.

## 9. Risks and open items

| Item | Handling |
|---|---|
| Serving the second entry from the embedded dist (`asset_resolver`) in release; dev mode serves from Vite | Verify in plan 11.1; fallback is `rust-embed`/`include_dir` of the built `remote` assets and a dev proxy to `localhost:1420` |
| `axum` + `qrcode` dependency size and compile time | `cargo tree`/build-time check in 11.1; the QR could move to the UI if `qrcode` is heavy |
| Windows firewall prompt UX; a denied prompt creates a persistent Block rule | Settings guidance (§6.6); optionally add an installer firewall rule (NSIS) — decide in 11.2 |
| Plain HTTP (cookie sniffable on the LAN) | Accepted for v1 (RM-D11); revisit HTTPS with a fingerprint-in-QR flow |
| iOS Safari backgrounds tabs and drops SSE | Reconnect + fresh `snapshot` handles it; document that the phone screen must stay on |
| Wrong interface in the QR on multi-adapter PCs (WSL/Hyper-V/VPN) | Route-based auto pick, deprioritise virtual adapters, manual override |
| Queue jump clones a long queue on the server | Same cost the desktop already pays over IPC (`PlayTracks { items, start }`); no engine change. If S7-scale queues show a hitch in the smoke, add a jump-to-index `EngineCmd` in a later plan (QoL-D12 spirit: engine stays minimal) |
| Phone-supplied device names | Sanitised and length-capped; rendered as text only |
| Clock-free design | No timestamps are trusted from the phone |
| mDNS name (`oxitide.local`) as a stable address | Out of scope; the QR carries the IP |
| Several controllers issuing conflicting commands | Last command wins; the state stream shows the result to everyone |

## 10. Plan sequence and closure

1. **11.1 Server core:** `remote/` module, config block, listener + guards, peer/Host/Origin
   checks, port fallback, interface selection, lifecycle (enable/disable/logout), hub stub,
   dependency and asset-serving verification.
2. **11.2 Pairing and Settings:** device store, token flow, approval, rate limits, QR, the
   Settings Remote section, `remote_status`/`remote_pair_request` events, firewall guidance.
3. **11.3 RPC and events:** allowlist dispatcher + coverage test, slim state, `queueRev`,
   SSE protocol, `get_queue_page`, `queue_jump`, tests.
4. **11.4 Remote page, core:** second Vite entry, shell, store, Now Playing, mini bar, Queue.
5. **11.5 Remote page, discovery:** Search, Home/Explore, Library, detail pages, History,
   Lyrics, row actions.
6. **11.6 Hardening and closure:** smoke checklists, PM-D1 re-runs, docs (`AGENTS.md`
   architecture bullet and IPC note, `ui/design/DESIGN.md`, this spec's status), completion
   note with the verified/exempted split.

Closure requires: scripted gates green on Windows and the Linux gates; the §7 smoke
checklist passed or explicitly exempted by the owner; PM-D1 numbers recorded for the
enabled case; roadmap entry added (Track 11) and the Track 7 casting line updated to point
here and to the spike note.
