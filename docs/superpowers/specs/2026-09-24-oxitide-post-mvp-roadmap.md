# OxiTide — Post-MVP Roadmap (Plan 6+)

- **Status:** Approved (owner-confirmed 2026-09-24); execution order revised 2026-09-28
- **Date:** 2026-09-24
- **Revised:** 2026-09-28 — execution order reordered to Tracks 4 → 5 → 6 → 3 → 7 after Plan 10; Track labels stay stable (see PM-D5).
- **Owner:** V
- **Parent:** `2026-09-21-oxitide-design.md` (§9 Phasing)
- **Supersedes:** the parent's Phase 2–4 ordering. The shipped Phase 1 MVP (v0.2.0) is
  described in `2026-09-21-oxitide-phase1-design.md`.

---

## 1. Purpose

Fix the order and framing of post-MVP work. Each track (or sub-track) still gets its own
dated spec → plan → implementation cycle; this document only fixes scope, execution order,
decisions, and the research each track needs. Plan numbers continue the existing sequence
(Plan 6 onward) in execution order; a track may span one or more plans.

Owner-confirmed ordering (2026-09-24), execution order revised 2026-09-28 (PM-D5).

**Track labels are stable; the execution order below is not track-number order.** Track 3 is
executed fourth in the sequence.

| Exec | Track | Outcome | Status |
|---|---|---|---|
| — | Track 1 | Library editing & favorites: favorite/unfavorite tracks, albums, artists; playlist create/rename/delete; add/remove/reorder tracks | Done (Plans 6–7 + write hotfix) |
| — | Track 2a | UX behavior: shortcuts, context menus, sorting/filtering, virtualization, nav history, toasts, search-as-you-type | Done (Plans 8–9) |
| now | Track 2b | Visual: design language, layout/IA, expanded now-playing, density, responsive window fit | In progress (Plans 10–13 + follow-up polish) — mostly implemented, kept **open** for bug fixes and additional polish |
| 1 | Track 4 | Lyrics + discovery: synced lyrics view; discovery feeds/mixes; track radio / similar artists | Next |
| 2 | Track 5 | Scrobbling: Last.fm and ListenBrainz in parallel, independent toggles; now-playing, offline queue, retry | Owner prerequisite: register Last.fm API key (start now) |
| 3 | Track 6 | Remaining integrations: Discord RPC; SMTC + hardware media keys | Done (6a SMTC/media keys + 6b Discord Rich Presence) |
| 4 | Track 3 | Performance & stability wave: profile and fix startup, time-to-first-sound, UI jank, memory, cache/eviction churn, large-library handling; build a measurement harness | Deferred wholesale (no early harness) — PM-D1 standing gate remains |
| 5 | Track 7 | Advanced audio: DSP/EQ (opt-in, mutually exclusive with bit-perfect); casting/TIDAL Connect; Dolby Atmos | Parked last |

Track 2b has no hard "done" gate before Track 4 starts; it may still receive bug fixes and
polish in parallel.

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
- **Status:** 2a is done (Plans 8–9). 2b is in progress (Plans 10–13 plus follow-up polish) —
  mostly implemented but deliberately left **open** for bug fixes and additional polish, with
  no hard completion gate before Track 4 starts.
- Virtualization overlaps the Track 3 budgets; the measurement harness now lands with the
  deferred Track 3 wave (see PM-D5).

### Track 3 — Performance & stability wave

- This operationalizes the project's core promise: beat the official client on
  responsiveness and reliability, not just match its features.
- Deliver a measurement harness plus fixes; candidate budgets: cold start,
  time-to-first-sound, 2k+ row scroll smoothness, memory ceiling, no UI stalls,
  cache/eviction churn, large-library load.
- Standing rule (see §4 PM-D1): every later spec states its performance impact.
- **Execution moved:** this wave now runs fourth (after Tracks 4, 5, 6) and is deferred as a
  whole, including the harness — there is no early baseline (PM-D5). The PM-D1 standing gate
  still applies: Tracks 4–6 must state their performance impact and must not regress it.

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
| PM-D5 | Execution order is revised to Tracks 4 → 5 → 6 → 3 → 7 after Plan 10 (labelled order unchanged). Track 3 is deferred wholesale — no early harness. | Tracks 4–6 are additive/user-facing and Track 7 is optional, so the dedicated optimization wave targets a stable feature set. PM-D1's standing gate keeps regressions honest meanwhile; Track 5's Last.fm key registration starts now (long lead). |

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

- Plan numbering continues from Plan 5 and follows the **execution order** (PM-D5): Track 2b
  spans Plans 10–13, then Tracks 4, 5, 6, then the deferred Track 3, then Track 7. **Track labels
  do not change** — "Track 3" still names Performance & stability even though it runs fourth.
  Track 2 has spanned Plans 8–9 (2a) and Plans 10–13 (2b); later tracks may similarly span
  more than one plan.
- The parent's Phase 4 ordering (DSP → casting → Atmos) is replaced by Track 7; Atmos
  and casting stay parked there.
- Track 2's visual direction and Track 4's lyrics source are deliberately left open
  here; they are brainstorm inputs for their own specs.
