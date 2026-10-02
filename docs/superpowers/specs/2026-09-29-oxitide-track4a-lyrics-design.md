# OxiTide — Track 4a Lyrics

- **Status:** Draft for review
- **Date:** 2026-09-29
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 4, PM-D1) ·
  `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (§7.2 reserved slot) ·
  `2026-09-25-oxitide-2b-visual-convergence.md` (deferred lyrics item)
- **Research:** `docs/superpowers/plans/notes/2026-09-29-track4-api-research.md` (spike)
- **Process note:** scope and decisions were brainstormed live with the owner
  (2026-09-29). Track 4 is split: this spec is **4a lyrics**; discovery is a separate
  4b cycle. This document is the standalone source of truth for 4a.

---

## 1. Purpose & Outcome

Ship **synced lyrics** in the expanded Now Playing view — the Track 4 lyrics half,
and the payoff for the slot reserved in foundation §7.2.

**Outcome:** when the current track has lyrics, the right meta column becomes
lyrics-first: the current line highlights and auto-scrolls in time with playback,
clicking a line seeks, manual scrolling pauses following with a jump-to-current
affordance, and a quiet tab flips back to Next Up. Lyrics come from TIDAL's legacy
endpoint first, with a silent `lrclib.net` fallback; results are cached on disk per
track. Tracks without lyrics keep today's Next Up column unchanged.

Success = opening Now Playing on a track with lyrics shows them promptly, the
highlight stays in sync through seek/pause/resume, nothing new polls or blocks, and
the whole feature degrades quietly to Next Up whenever lyrics can't be produced.

## 2. Scope

**In scope:** the legacy TIDAL lyrics fetch; the lrclib fallback (exact + search);
a pure Rust LRC parser; a `LyricsService` with an on-disk per-track cache; one Tauri
command; a thin UI store and pure active-line helper; the Now Playing column layout,
tab toggle, sync/scroll/seek behavior, and loading/empty states; tests and a manual
smoke checklist.

**Out of scope:** discovery (Track 4b); lyrics in the mini player or player bar;
word-level/karaoke timing; lyrics editing, reporting, or contributions; offline
queueing of lyrics fetches; any change to playback, queue, or catalog behavior;
performance-harness work (Track 3; PM-D1 still applies here as stated in §9).

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| L-D1 | **Surface:** in expanded Now Playing, lyrics replace Next Up when the track has lyrics, with a quiet **Lyrics / Next up** tab; lyrics-first default, the chosen tab persists across track changes (amended 2026-10-02 hotfix; originally reset per track). No lyrics = today's Next Up column unchanged. | Keeps one surface and gives lyrics real room to scroll (owner choice). |
| L-D2 | **Interaction:** active-line highlight + smooth auto-scroll + **click any timed line to seek**; manual scroll pauses following and reveals a jump-to-current glyph. Untimed lyrics are plain scrollable text. | Sampled this from the interactive options; matches how the owner listens. |
| L-D3 | **Sources: TIDAL-first, silent lrclib fallback.** Legacy `tracks/{id}/lyrics` first; on a confirmed miss, lrclib exact match, then one `/api/search` attempt. The official JSON:API lyrics resource is `required-access-tier: INTERNAL` and is out. | Spike findings; no new dependency for tracks TIDAL covers, silent coverage extension for the rest. |
| L-D4 | **Cache: disk `cache/lyrics.json`** keyed by track id, memory + disk, **positives and negatives**; only confirmed misses are negative-cached; pruned to the most recent ~2,000 entries by `fetched_at` on save. | Survives restarts, keeps lrclib traffic minimal, bounded growth. |
| L-D5 | **Shape: Rust `LyricsService` + command.** A `track_lyrics()` on the tidal crate, a service in `oxitide-app` mirroring `ArtworkService`, LRC parsed by a pure Rust module, one `get_lyrics` command; the UI only stores and renders. | All HTTP stays in Rust; parsing is unit-testable; no logic in the webview. |
| L-D6 | **Timing: reuse the existing position events.** Highlight derives from `player.state.position_ms`; no polling and no background prefetch — lyrics load only while Now Playing is open. | PM-D1; lrclib stays untouched for tracks the user never opens. |
| L-D7 | **Instrumental = no lyrics.** An lrclib `instrumental: true` record counts as a confirmed miss; no "Instrumental" label. | Keeps the surface contract simple: lyrics or Next Up. |
| L-D8 | **No new auth scopes.** Lyrics and (later) discovery run on the existing `r_usr w_usr w_sub` token. | Spike finding; no login-flow change. |
| L-D9 | **Presentation follows tokens only:** reduced motion swaps smooth scrolling for jumps, RTL via `dir`, density tokens control leading, icons via `Icon.svelte` (no unicode). | Existing 2b obligations; no mode branching. |

## 4. Research summary

Full findings: `docs/superpowers/plans/notes/2026-09-29-track4-api-research.md`.

- Legacy `api.tidal.com/v1/tracks/{id}/lyrics` (existing token; 404 = none) returns
  `lyrics` (plain), `subtitles` (synced LRC), `lyricsProvider`, `providerLyricsId`,
  `isRightToLeft`.
- `lrclib.net/api/get` (public, no auth) matches on
  track/artist/album/duration (±2 s) and returns `syncedLyrics` (LRC), `plainLyrics`,
  `instrumental`; `/api/search` is the mismatch fallback. Docs require honoring
  `429` + `Retry-After`, sequential requests, and a descriptive User-Agent.
- Official `openapi.tidal.com/v2` lyrics paths are `required-access-tier: INTERNAL`;
  official discovery mixes need scopes OxiTide's token does not carry. Both are out.

## 5. Architecture & components

### 5.1 Tidal crate — `crates/oxitide-tidal/src/lyrics.rs`

- `TidalLyrics` raw DTO (serde) for the legacy response.
- `TidalClient::track_lyrics(&self, track_id: u64) -> Result<Option<TidalLyrics>>`:
  `GET tracks/{id}/lyrics` through the existing `get` (country code + refresh-on-401
  for free); `Error::NotFound` maps to `Ok(None)`; every other error propagates.

### 5.2 Lyrics service — `crates/oxitide-app/src/lyrics.rs`

Mirrors `artwork.rs`:

- Owns a reqwest client for lrclib (base URL injectable for wiremock), a
  `Mutex<HashMap<u64, CacheEntry>>` loaded from `cache/lyrics.json`, and the disk
  path; saves atomically (tmp + rename) like `ArtworkService::save_disk`.
- `CacheEntry { fetched_at: u64, lyrics: Option<LyricsDto> }`; prune keeps the 2,000
  newest by `fetched_at`.
- `pub async fn get(&self, client: &TidalClient, req: &LyricsRequest) -> Option<LyricsDto>`
  implements the order: memory/disk cache → TIDAL → lrclib exact → lrclib search →
  negative. It never returns an error: failure modes degrade to `None` (logged) and
  are **not** cached, so a later attempt can retry.
- `LyricsRequest { track_id, title, artist, album: Option<String>, duration_secs }`
  comes from the UI's `QueueItem` (already carries all of it).
- lrclib requests send `User-Agent: OxiTide/<CARGO_PKG_VERSION>` and honor `429`
  `Retry-After` with one retry.

### 5.3 Parser — pure function(s) in the same module

- `parse_lrc(text: &str) -> Vec<LyricLine>`: multiple `[stamp]`s per line expand;
  `[offset:±ms]` applied to every line; `[mm:ss.xx]` and `[mm:ss.xxx]` accepted;
  metadata tags ignored; blank timed lines kept (visual gaps); malformed lines
  skipped, never fatal.
- `split_plain(text: &str) -> Vec<LyricLine>`: newline split, untimed lines
  (`time_ms: None`), trailing empties trimmed.

### 5.4 IPC — `crates/oxitide-app/src/commands.rs`

- `#[tauri::command] get_lyrics(track_id) -> Result<Option<LyricsDto>, String>`
  using `require_client` + `state.lyrics()`; registered in `generate_handler![]`
  in `main.rs` (AGENTS.md rule).
- `LyricsService` is constructed in `main()` beside `ArtworkService`, at
  `default_cache_dir()/lyrics.json`.
- DTO (serde defaults; snake_case field names go over IPC verbatim by the same
  convention as the other DTOs; the UI mirrors them in `api.ts` by hand):

```ts
interface LyricLine { time_ms: number | null; text: string }
interface LyricsDto {
  track_id: number;
  source: "tidal" | "lrclib";
  synced: boolean;
  rtl: boolean;
  lines: LyricLine[];
}
```

### 5.5 UI — `ui/src/lib/lyrics.svelte.ts` + `ui/src/lib/lyrics.ts` + view

- `lyrics.svelte.ts`: a small singleton-style store with `open(trackId)` /
  `setTab()` and `{ loading, lyrics, tab }` state. It drops responses whose track id
  no longer matches the current one (rapid skipping never renders stale lyrics).
- `lyrics.ts` (pure, tested): `activeLine(lines, position_ms, lookahead_ms)` returns
  the index of the last timed line at or before `position_ms + 150`, or `-1`.
- `NowPlaying.svelte` consumes the store; the lyrics block replaces the reserved
  `div.lyrics-slot` and the existing Next Up markup moves under the tab.

## 6. Now Playing behavior

- **Layout:** with lyrics, the meta column becomes a flex column — header fixed,
  lyrics area `flex: 1`, scrolls internally (hidden scrollbar) with soft top/bottom
  fade masks; the column top-aligns instead of today's vertical centering. Without
  lyrics the layout is untouched.
- **Tabs:** the "Next up" eyebrow becomes a quiet two-tab row (**Lyrics / Next up**)
  only when lyrics exist; the choice persists across tracks (cleared when playback stops).
- **Sync:** active line at full text color, others muted; the active line sits ~40 %
  from the top via a transform transition; blank timed lines never highlight.
  `appearance.reducedMotion` replaces smooth movement with instant positioning.
- **Click-to-seek:** clicking a timed line calls `player.seek(time_ms)` and resumes
  following. Untimed lyrics are not clickable.
- **Manual scroll:** wheel/drag pauses auto-follow and shows a jump-to-current icon;
  clicking it (or clicking a line, or a new track) resumes following. Scrolling back
  manually does not auto-resume.
- **Loading:** while a fetch for a new track is pending and the Lyrics tab is active, the
  lyrics column stays in place with a muted "Loading lyrics…" placeholder (the spine
  stays mounted too); the Next Up column only returns on a confirmed miss. Cached
  results render immediately.
- **RTL:** `dir="rtl"` on the lyrics container when the DTO says so.
- **Density/modes:** leading and text sizes follow existing density tokens; no
  mode-specific branching.

## 7. Source mapping, fallback, and cache rules

- **TIDAL mapping:** non-empty `subtitles` → `parse_lrc` (synced); else non-empty
  `lyrics` → `split_plain`; `isRightToLeft` → `rtl`; both empty → confirmed miss.
- **lrclib exact:** `GET /api/get` with `track_name`, `artist_name`, album when
  known, and integer `duration` seconds. `syncedLyrics` wins over `plainLyrics`.
- **lrclib search:** only after an exact miss: `GET /api/search` with `track_name`
  + `artist_name`; the first result within ±2 s of our duration and not instrumental
  wins; otherwise confirmed miss.
- **Cache:** reads hit memory, then disk-at-startup (single load). Writes update
  memory and re-save atomically. Positives store the parsed DTO; negatives store
  `null`. Network errors, 429 exhaustion, and parse failures cache nothing.
- **Duration:** `QueueItem.duration_secs` (TIDAL seconds) is sent verbatim; no
  millisecond conversion.

## 8. Error handling & degradation

- TIDAL error other than 404 → fall through to lrclib; if lrclib also fails → `null`
  for this attempt (no cache), logged with `tracing::warn`.
- lrclib 404/empty → confirmed miss → negative cache.
- Parser yields zero lines from a non-empty body → treated as a miss (fall through /
  negative), never an error.
- UI: any `null` renders Next Up; invoke failures are swallowed by the store (the
  column is not an error surface; no toast).

## 9. Performance impact (PM-D1)

- **Startup:** one small JSON read (bounded by the 2,000-entry prune); no network.
- **Per track:** at most one `get_lyrics` invoke while Now Playing is open; cache
  hits avoid all HTTP. No polling — highlight reuses existing position events.
- **Fetching:** at most two lrclib requests per confirmed miss (exact + search) and
  only for tracks TIDAL lacks; sequential by construction, `Retry-After` honored.
- **Rendering:** one list keyed by track id, one transform per position event;
  parsing is linear in lyric size and stays in Rust.
- No regression is expected in cold start, time-to-first-sound, or scroll
  smoothness; the spec adds no new background work.

## 10. Testing & verification

**Rust (wiremock only; never live TIDAL/lrclib):**

- tidal crate: legacy response parses; 404 → `Ok(None)`; auth refresh reused.
- parser: centiseconds and milliseconds, multi-stamps, `[offset:]`, metadata tags,
  blank lines, malformed lines, plain split.
- service: TIDAL hit → parsed DTO cached; TIDAL miss → exact → search ordering;
  duration filter ±2 s; instrumental search hit treated as a miss; confirmed miss
  negative-cached (second call makes no HTTP); network failure not cached (retried);
  disk cache survives a restart; prune keeps the 2,000 newest; 429/`Retry-After`
  retried once.
- the `get_lyrics` command must be listed in `generate_handler![]` in `main.rs`; the
  manual smoke catches a missed registration (the invoke would fail).

**UI (vitest):**

- `activeLine`: before the first line, exactly on a boundary, between lines, after
  the last, lookahead window, blank lines, empty input.
- store: stale response for a previous track id is dropped; loading/complete
  transitions.

**Manual (recorded as a checklist in `docs/superpowers/plans/notes/`):**

- **Fixture-first:** during development, capture one real TIDAL lyrics response with
  a real account (temporary log or dev probe; responses are not secrets) and turn it
  into the wiremock fixture — this retires the linchpin risk.
- Then, with `npm --prefix ui run app:dev`: synced lyrics highlight/scroll on a
  known track; click-to-seek; manual scroll pause + jump-to-current; unsynced track;
  no-lyrics track keeps Next Up; instrumental behaves as no lyrics; an
  lrclib-only track (one TIDAL lacks); restart keeps lyrics cached; reduced motion
  jumps; RTL track if one can be found; both modes and densities; window resize to
  min size.

## 11. Risks & open verifications

- **Legacy endpoint under our token** — shape/availability verified only from the
  community client; the fixture-first step in §10 retires this first.
- **lrclib matching quality** — featuring suffixes and duration rounding can cause
  misses; the ±2 s search fallback mitigates; confirmed misses are cached so the
  cost is paid once.
- **Rapid skimming** — each track change fires at most one invoke while Now Playing
  is open; there is no batch or prefetch, so lrclib sees only deliberate plays.
- **RTL rendering** — honored via `dir`; no fonts work is planned.

## 12. Future / explicitly deferred

- Track 4b discovery (for-you feed repointed to `pages/for_you`, mixes, radio,
  similar artists) — separate spec next.
- Lyrics in mini/player bar; word-level timing; user-supplied lyrics; offline
  prefetch of lyrics for the queue.
