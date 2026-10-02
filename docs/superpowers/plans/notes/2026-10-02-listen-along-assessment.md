# Listen Along / group listening — assessment (2026-10-02)

Potential, **not committed**. Revisit only **after** casting / TIDAL Connect (Track 7) lands,
because both would need the same playback-session/transport primitive.

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
