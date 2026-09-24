# OxiTide — Post-MVP Roadmap (Plan 6+)

- **Status:** Approved (owner-confirmed 2026-09-24)
- **Date:** 2026-09-24
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md` (§9 Phasing)
- **Supersedes:** the parent's Phase 2–4 ordering. The shipped Phase 1 MVP (v0.2.0) is
  described in `2026-09-21-oxitide-phase1-design.md`.

---

## 1. Purpose

Fix the order and framing of post-MVP work. Each track (or sub-track) still gets its own
dated spec → plan → implementation cycle; this document only fixes scope, order, decisions,
and the research each track needs. Plan numbers continue the existing sequence (Plan 6
onward), one plan per track slice.

Owner-confirmed ordering (2026-09-24):

| # | Track | Outcome |
|---|---|---|
| 1 | Library editing & favorites | Favorite/unfavorite tracks, albums, artists; playlist create/rename/delete; add/remove/reorder tracks |
| 2 | UX pack + UI/layout overhaul | **2a behavior:** shortcuts, context menus, sorting/filtering, virtualization, nav history, toasts, search-as-you-type. **2b visual:** design language, layout/IA, expanded now-playing, responsive window fit |
| 3 | Performance & stability wave | Profile and fix startup, time-to-first-sound, UI jank, memory, cache/eviction churn, large-library handling; build a measurement harness |
| 4 | Lyrics + discovery | Synced lyrics view; discovery feeds/mixes; track radio / similar artists |
| 5 | Scrobbling | Last.fm and ListenBrainz in parallel, independent toggles; now-playing, offline queue, retry |
| 6 | Remaining integrations | Discord RPC; SMTC + hardware media keys |
| 7 | Advanced audio | DSP/EQ (opt-in, mutually exclusive with bit-perfect); casting/TIDAL Connect; Dolby Atmos |

## 2. Carryover (before Plan 6)

- Merge `fix/plan5-pager-short-pages`; re-run the two pending Plan 5 paging smoke items
  (favorites/playlist counts, queue-all).
- Re-check Plan 4's two manual items (cache usage display, HE-AAC Low retirement message).

## 3. Track notes and known constraints

### Track 1 — Library editing & favorites

- The community client (`tidalapi`) confirms playlist create/delete, playlist track
  add/remove, favorites add/remove for tracks/albums/artists, and batch additions;
  playlist create/get/add/delete moved to TIDAL's **v2 endpoints**.
- `TidalClient` is currently GET-only: this track adds POST/DELETE verbs reusing the
  single-flight refresh-on-401 path, plus 429/`Retry-After` handling.
- **Unverified:** explicit track reorder semantics (dedicated endpoint vs delete+re-add) —
  verify before the spec is written.
- Interaction decisions (optimistic updates vs refetch, error reconciliation) belong to
  the plan.

### Track 2 — UX pack + UI/IA overhaul

- Two specs: **2a behavior** (keyboard shortcuts, context menus, sortable/filterable
  tables, list virtualization, back/forward navigation, toasts, search-as-you-type,
  consistent loading/empty/error states) and **2b visual identity + layout/IA**
  (design language, color/typography/icons, library sidebar tree, expanded now-playing,
  queue presentation, density, window fit/responsiveness).
- All five aspects are in scope: visual identity, layout & information architecture,
  player/queue experience, data presentation & interaction polish, window fit.
- The look-and-feel direction (references, light/dark, mockups) is decided during the 2b
  brainstorm; a visual pass is expected.
- Virtualization overlaps the Track 3 budgets; the measurement harness lands in Track 3
  even if the lists ship earlier.

### Track 3 — Performance & stability wave

- This operationalizes the project's core promise: beat the official client on
  responsiveness and reliability, not just match its features.
- Deliver a measurement harness plus fixes; candidate budgets: cold start,
  time-to-first-sound, 2k+ row scroll smoothness, memory ceiling, no UI stalls,
  cache/eviction churn, large-library load.
- Standing rule (see §4 PM-D1): every later spec states its performance impact.

### Track 4 — Lyrics + discovery

- **Lyrics:** TIDAL's official API exposes a lyrics resource (`lrcText` synced, plain
  `text`, provider, `technicalStatus`) on the new JSON:API surface; the legacy community
  path exists; `lrclib.net` (free, no auth, synced LRC) is a fallback used by other
  clients. The source choice (TIDAL-first vs lrclib-first vs both) is a Track 4 decision.
- **Discovery:** the official `userDiscoveryMixes` resource and community mix endpoints
  (daily/history/radio) plus artist radio / similar artists.
- Sync availability varies by track and region; handle "no lyrics" cleanly.

### Track 5 — Scrobbling

- Both services in parallel, independent toggles; one listen-event feed fans out.
- Last.fm: register an OxiTide API key+secret (owner prerequisite); web auth → session
  key in the OS keyring. ListenBrainz: user token only.
- Scrobble rules: tracks >30 s; scrobble at 50% or 4 min; now-playing pings; offline
  queue + retry per service; privacy toggle.

### Track 6 — Remaining integrations

- Discord RPC: requires registering a Discord application; decide what presence shows
  (track, artist, progress, buttons).
- SMTC + hardware media keys: Windows System Media Transport Controls via the `windows`
  crate; verify key handling and coexistence with the WebView.

### Track 7 — Advanced audio

- DSP/EQ (opt-in, mutually exclusive with bit-perfect; the UI must make the trade-off
  explicit), casting / TIDAL Connect (protocol research first), Dolby Atmos
  (Phase 0-style DRM spike before any commitment).
- Hold until Tracks 1–6 land unless demand changes.

## 4. Decisions (post-MVP)

| # | Decision | Rationale |
|---|---|---|
| PM-D1 | Performance is both a dedicated wave (Track 3) and a standing consideration in every track's design decisions; regressions block a track's completion. | Responsiveness/reliability is the product's core promise. |
| PM-D2 | **No explicit downloads.** Offline stays as the existing transient LRU cache; no download-manager UI. Revisit only if the owner changes stance. | TIDAL's terms prohibit downloading outside the official player; a deliberate plain-file library crosses a different line than transient caching, and the cache already covers replays. |
| PM-D3 | Scrobbling feeds Last.fm and ListenBrainz in parallel with independent toggles. | No reason to serialize; both may be wanted. |
| PM-D4 | Track 2 splits into behavior (2a) and visual/IA (2b) specs. | Keeps each plan reviewable and gives the visual direction its own pass. |

## 5. Open research questions

- Playlist reorder semantics (dedicated endpoint vs delete+re-add).
- Lyrics source decision (TIDAL official vs legacy vs lrclib) and cache strategy.
- Discovery endpoints to adopt (mixes, radio, similar artists) and their auth scopes.
- Last.fm API key registration (owner action).
- Dolby Atmos DRM status (spike) and casting/TIDAL Connect protocol.
- Last.fm / ListenBrainz offline-queue dedupe and failure behavior.

## 6. Cross-cutting backlog (fold into whichever plan fits)

- Performance budgets as a standing gate on every plan: cold start,
  time-to-first-sound, 2k+ row scroll smoothness, memory ceiling, no UI stalls —
  measured by the Track 3 harness.
- Performance considered in every track's design decisions (allocations, blocking calls,
  re-renders, API round-trips), not only Track 3; regressions block a track's completion.
- Queue/session persistence across app restarts.
- API-drift resilience: version-tolerant parsing, fixture refresh, error taxonomy.
- Packaging: code signing, auto-update (deferred; required only if shared beyond
  friends); CI (none today).
- Logging/diagnostics improvements as needed.

## 7. Notes

- Plan numbering continues from Plan 5; one plan per track slice, so Track 2 is expected
  to span two plans.
- The parent's Phase 4 ordering (DSP → casting → Atmos) is replaced by Track 7; Atmos
  and casting stay parked there.
- Track 2's visual direction and Track 4's lyrics source are deliberately left open
  here; they are brainstorm inputs for their own specs.
