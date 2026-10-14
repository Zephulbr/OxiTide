# Listen Along / group listening — assessment (2026-10-02)

**Shelved 2026-10-13** (owner decision; see "Re-assessment" at the end). Originally: potential,
not committed; revisit only after casting / TIDAL Connect, because both need the same
playback-session/transport primitive.

## What it is

Synchronized playback across two or more accounts so people can listen together, optionally
launched from Discord. No audio is streamed: every participant plays from their own TIDAL
account and only transport state (track + position + play/pause) is synchronized.

## How the existing options work

- **Discord "Listen Along"** — Spotify-exclusive partnership. Discord relays track ID,
  timestamp, and play state; each friend's Spotify client independently loads/seeks/plays.
  Not an open API, and TIDAL has no Discord integration.
- **TIDAL "Live"** (formerly "DJ") — host broadcasts their now-playing/queue, listeners follow
  via a link. Mobile-only, region-locked, HiFi/HiFi+ only, lossy for listeners, host-only
  control; reportedly removed in late 2025.
- **Community `/listen-together`** (TidaLuna plugin + Discord bot) — each participant's local
  TIDAL client plays from their own account, synced through the bot; no audio through Discord.
  The closest working precedent.

## Feasibility in OxiTide

- Playback is cleanly drivable/observable: `EngineEvent` (state/track/position) out,
  `EngineCmd` (play/seek/play_pause/…) in — driving a follower is straightforward.
- Discord is presence-only today (`discord-rich-presence`, `crates/oxitide-app/src/discord`);
  it never subscribes to `ACTIVITY_JOIN`. A join/invite surface would need an event-listening
  crate (e.g. `presenceforge`), a Discord bot, or the Discord Social SDK.
- **Main gap:** OxiTide has no networking subsystem at all (no server, sockets, WebSocket,
  P2P, or clock sync). Position is frame-count-derived and published every 500 ms, so honest
  sync needs a time-offset handshake and periodic drift correction.
- Remote friends need a relay or the Discord Social SDK (lobbies/NAT traversal); LAN /
  same-network is much simpler (mDNS + direct socket).

## Overlap with Track 7 casting / TIDAL Connect

| Area | Overlap |
|---|---|
| Peer/device discovery + control channel | High |
| Remote transport mapped to `EngineCmd`/`EngineEvent` | High |
| Position/clock authority | Medium |
| Controller/renderer session model | Medium |
| Audio data plane | None (listen-along streams no audio) |
| DSP/EQ, crossfade/automix, Atmos, 360 RA, ASIO | None |

## Recommendation

Build a generic **playback-session transport** (discovery + control channel + engine remote
mapping + optional clock sync) once; casting and listen-along are then thin consumers. Assess
listen-along only after casting, and prefer the smallest honest slice first (LAN,
host-broadcast, invite link) before any Discord SDK work.

## Constraints

- Each listener streams from their own account (as TIDAL Live / the community bot do); avoid
  routing audio through Discord. Self-bots violate Discord ToS, and only bots may use the
  "Listening to" activity type.
- Feasibility and overlap only — no design or scope committed. A spec would be its own
  spec → plan cycle once the shared transport exists.

## Re-assessment (2026-10-13) — shelved

Re-checked after Track 11 (phone remote) closed. Decision: **not building**; effort is not
justified for a personal-use app with no known second OxiTide user. Findings kept for a revival:

- **What Track 11 provides:** the slim `RemoteState` + SSE hub (`remote/events.rs`), axum
  server, QR/token pairing, LAN-only peer filter (already admits Tailscale `100.64/10`), and the
  `RemoteSession` seam (RM-D12). Roughly a third of the work, not most of it.
- **What it does not:** the direction is inverted (listen-along needs a *follower controller*
  that consumes a host's state and issues its own `EngineCmd`s); the host side should be a
  narrower read-only stream, not the RPC allowlist, with its own guest tokens; internet/relay/
  NAT traversal are explicit Track 11 non-goals (plain HTTP, home network only); listeners must
  run OxiTide (a web page cannot play TIDAL audio).
- **Wire format:** model Spotify Jam / Connect — state is an *anchor* (position at a host
  timestamp + play state), re-sent on seek/pause/track change; followers compute
  `position = anchor + (now - anchor_time)` after an NTP-style clock-offset handshake. The
  remote's 1 s position events carry no timestamp and would not suffice. Spotify-grade sync
  (~1 s) is acceptable; a ±100–250 ms target is realistic. The engine has no varispeed, so
  drift correction is thresholded seeks (>~300 ms). Engine seek latency is **unmeasured** and is
  the first thing to spike.
- **Other risks:** followers need the next track ID for gapless prefetch (not in the slim
  state); crossfade shifts boundaries per client; per-account region/quality availability needs
  a skip policy; smoke testing likely needs a second TIDAL account (unverified).
- **Smallest slice if revived:** LAN or Tailscale, invite link/QR, host-only control, follow by
  track (not queue), followers can detach. A hosted room relay (the analogue of Spotify's cloud)
  is the only way to reach friends off-LAN without Tailscale; Discord SDK work stays last.
