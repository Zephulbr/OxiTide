# Track 10 (Daily-driver QoL) — research questions (2026-10-01)

Collected before the Track 10 spec so it starts with facts. Nothing here is
committed scope detail; each question needs a spike/answer first.

## Confirmed (desk research)

- **AI content controls are real TIDAL features.** TIDAL announced its AI policy
  on 2026-06-29; from mid-July 2026 wholly AI-generated recordings carry an "AI"
  label, and Settings offers a filter ("Turn off AI music"): when off, AI-labeled
  recordings are grayed out and skipped and are removed from personalized
  recommendations as they refresh.
  <https://support.tidal.com/hc/en-us/articles/48031883413521-AI-Policy>
  <https://tidal.com/ai-policy>
- **Explicit-content filter is real too.** The official app allows/block explicit
  content; blocking removes explicit content from browse and search results and
  clears the current queue when the toggle changes.
  <https://support.tidal.com/hc/en-us/articles/9936639051153-Explicit-Content>
- `explicit: bool` is already present in some API payloads this client parses
  (e.g. `crates/oxitide-tidal/src/feed.rs`, `models.rs`); a Track/Album DTO audit
  is still needed.

## Open questions (spike before spec)

1. **AI flag:** does the unofficial API expose the AI label (field name? in
   `mediaMetadata.tags`, a top-level flag, or album-level only)? If not exposed,
   the feature is infeasible for this client and should be recorded as such.
2. **Settings endpoints:** is there a user-settings endpoint that flips the AI
   and explicit filters server-side (so catalog/search/mixes respect it), or must
   OxiTide filter client-side? Client-side filtering must apply consistently to
   catalog, search, mixes, queue, and playback (skip + gray out).
3. **Playback history:** is there a recently-played endpoint, or does the history
   view run on a local play log? Recommendation: local-first log (private,
   offline, retention-controlled), optionally enriched by API data.
4. **Stats page:** scope (top artists/tracks, listening time, streaks?) and data
   source (local log vs scrobble services). Keep it a phase, not a blocker.
5. **Credits:** is there a track-credits endpoint? (Artist page/credits/album
   info is Track 9 surface work; the API question belongs here too.)
6. **Love:** Last.fm `track.love`/`track.unlove` with the existing session key is
   straightforward. ListenBrainz feedback support is unverified — check its API
   before promising parity. Design: separate scrobble "love" action, distinct
   from the TIDAL heart (favorites); do not auto-mirror without an explicit
   setting.
7. **Session/queue persistence shape:** what persists (queue items, current
   index, position, shuffle/repeat, volume, and the playlist play-source uuid
   so the sidebar's playing-playlist marker returns with the queue), where
   (config vs a data file), migration/versioning, and behavior when cached
   media is evicted. The resume sequence already exists (`player.restore()`
   in `ui/src/lib/player.svelte.ts`: playTracks, seek, re-pause); missing are
   a disk store, a save policy, and an auth-gated boot load. Decide whether a
   restored session starts paused or playing.
8. **Tray + hotkeys:** Windows-first behavior (close-to-tray vs minimize-to-tray,
   start minimized), conflict handling with SMTC media keys; Linux specifics ride
   on Track 8 (MPRIS, AppIndicator, Wayland global-shortcut portal).
9. **Sleep timer semantics:** stop at end of track vs immediate fade, resume
   behavior, persistence.

## Suggested spike order

Item 1–2 gate the AI/explicit feature; items 3–5 gate the history/credits parts;
items 6–9 are design decisions that can be settled in the spec once the API
questions are answered.
