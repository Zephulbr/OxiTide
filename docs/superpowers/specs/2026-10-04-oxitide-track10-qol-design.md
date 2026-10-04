# OxiTide — Track 10 Daily-driver QoL

- **Status:** Draft for review — design approved live with the owner (2026-10-04); plans pending
- **Date:** 2026-10-04
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 10; PM-D1–PM-D6);
  `plans/notes/2026-10-01-qol-research.md` (pre-spec research);
  `plans/notes/2026-10-04-track8-completion.md` (Linux seams and exemptions)
- **Process note:** scope, decisions, and architecture were brainstormed live with the
  owner on 2026-10-04. The desk spikes resolved the pre-spec open questions (§4). This
  document is the standalone source of truth for Track 10.

---

## 1. Purpose & Outcome

Track 10 turns OxiTide into a daily driver: the app remembers **where you were and what
was playing**, gives the library a **memory** (playback history + stats), adds the small
creature comforts (sleep timer, scrobble love, content blocking), and behaves like a
resident desktop app (tray, close-to-tray, global hotkeys).

**Outcome:**

- Restarting OxiTide brings back the queue, current track, position, shuffle/repeat, and
  the playing-playlist marker — **paused**, ready for play (§6.1).
- A sleep timer fades out and pauses after a countdown or at the end of the current
  track (§6.2).
- A local, private play log powers a Recently-played view and a stats page (§6.3).
- Loved tracks can be sent to Last.fm and ListenBrainz independently of the TIDAL heart
  (§6.4).
- AI-labeled and explicit content can be blocked with official-app-like behavior:
  grayed out, skipped, queue pruned/cleared on toggle (§6.5).
- The window closes to a system tray, and user-defined global hotkeys control playback
  from anywhere (§6.6).

**Success =** every scripted Windows gate stays green (`cargo test --workspace`,
`cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`,
`npm --prefix ui test`); the GUI smoke checklists in §7 pass with a real account;
`history.jsonl` writes never appear on the 500 ms position path; Track 10 closes through
the completion note in §10 with an explicit verified/exempted split.

## 2. Scope

**In scope:**

- Session/queue persistence: `session.json`, restore flow, logout/expiry lifecycle.
- Small audio-engine additions: `current_index` in `PlaybackState`, `start_paused` on
  play, `EngineCmd::FadeAndPause`.
- Sleep timer UI and semantics (duration presets/custom + end-of-track).
- Local playback history (`history.jsonl`), Recently-played view, stats page, retention
  and privacy controls.
- Scrobble love for Last.fm (`track.love`/`unlove`) and ListenBrainz
  (`recording-feedback`), with offline retry and pending-msid handling.
- AI/explicit content blocking: `track_flags` enrichment via public v2, gray-out UI,
  enqueue filtering, toggle behaviors.
- System tray, close-to-tray, the single-instance re-show fix, and global hotkeys.
- Config additions, commands, DTOs, tests, and smoke checklists for all of the above.

**Out of scope (non-goals):**

- Love→TIDAL-favorite mirroring (no auto-mirror; separate actions).
- Autostart at login and start-minimized; the tray is user-invoked only.
- ListenBrainz hate (`-1`); the love toggle is binary.
- Server-side personalization removal for AI content (official-app-only); OxiTide
  filters client-side.
- Scrobble backfill, scrobble-queue persistence, or any history sync to services.
- Restoring a session across an **explicit logout** (the session file is cleared).
- DSP/EQ/crossfade/casting/Atmos (Track 7), downloads (PM-D2), i18n (Track 9).

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| QoL-D1 | **Restore paused with full state.** Queue, current index, position, shuffle/repeat, and the play-source marker return; playback stays paused until play is pressed. | Owner call; no audio at login. |
| QoL-D2 | **Content blocking matches official behavior**, enforced client-side: AI + explicit toggles, gray-out + skip, queue cleared/pruned on toggle. | Owner call; the API exposes what is needed (§4 items 1–2). |
| QoL-D3 | **Close hides to tray** (`ui.close_to_tray` default `true`); Quit lives in the tray menu; the setting can restore close-to-exit. | Owner call; resident-app behavior. |
| QoL-D4 | **Sleep timer fades out (~5 s) then pauses**, queue kept; in-memory only, resets on restart. | Owner call; pause-over-quit keeps the morning resume. |
| QoL-D5 | **Rust owns the new state** (Approach A): versioned data files under `%APPDATA%\OxiTide\`; UI uses commands/events; `config.toml` stays preferences-only. | Testable without a window, survives WebView storage clears, no size ceiling. |
| QoL-D6 | **One spec for the whole track, staged plans (10.1–10.6).** | Mirrors Track 3/Track 8; each plan independently reviewable. |
| QoL-D7 | **Scrobble love is separate from the TIDAL heart and never auto-mirrors.** State is local-first; services are write targets. | Research note; avoids surprising account writes. |
| QoL-D8 | **ListenBrainz love resolves an msid at now-playing time** (`return_msid: true`), falls back to a metadata lookup, then stays pending until resolvable. No hate. | Only viable path to LB feedback parity. |
| QoL-D9 | **History is local-first JSONL**, default cap 10k entries, stats derived on read; per-track recording threshold follows listen semantics (≥30 s, or ≥50% under 60 s). | Private, offline, retention-controlled. |
| QoL-D10 | **Global hotkeys are user-defined with empty defaults**, registered via `tauri-plugin-global-shortcut`; conflicts keep the previous binding. | Avoids OS conflicts; media keys stay on SMTC/MPRIS. |
| QoL-D11 | **Flag lookups fail open.** Unknown `ai`/`explicit` values never block playback; enrichment is batched, cached, and asynchronous. | A network hiccup must never break playback. |
| QoL-D12 | **Engine changes stay minimal**: `current_index` on `PlaybackState`, `start_paused` on play commands, `FadeAndPause`. Nothing else moves into `oxitide-audio`. | The engine stays a pure playback crate. |

## 4. Research summary

Desk spikes (2026-10-04) resolved the pre-spec questions in
`plans/notes/2026-10-01-qol-research.md`:

1. **AI flag — exposed.** TIDAL's public v2 track attributes carry
   `ai: boolean` ("Whether the track is AI-generated") alongside `explicit`. The app
   already has the `get_openapi` seam (`crates/oxitide-tidal/src/client.rs`) used by
   `artist_profile`.
2. **Settings endpoints — none public.** No user-settings endpoint exposes the AI or
   explicit filters; enforcement must be client-side. v2 `searchResults` does offer
   `explicitFilter=INCLUDE|EXCLUDE`, but the app's search is v1-shaped and porting it is
   not warranted (considered and rejected, §6.5).
3. **Playback history — no endpoint.** Local-first log confirmed as the design (D9).
4. **Stats — local.** Derived from the log; scope in §6.3.
5. **Credits — v2 relationship exists** (`credits` on the public tracks API), but that
   work belongs to Track 9 surfaces; nothing in Track 10 depends on it.
6. **Love — feasible both sides.** Last.fm `track.love`/`track.unlove` are signed calls
   with the existing session key. ListenBrainz
   `POST /1/feedback/recording-feedback` takes `score` 1/0 and needs a
   `recording_mbid`/`msid`; `submit-listens` supports `return_msid: true` on
   `playing_now`, and `/1/metadata/lookup` can resolve artist+title.
7. **Session shape — designed in §6.1.** Existing seams: `player.restore()` in
   `ui/src/lib/player.svelte.ts`, `ExpiredSession` snapshot in `ui/src/App.svelte`,
   volume already persists via `config.audio.volume`.
8. **Tray/hotkeys — greenfield** on both platforms; Linux rides Track 8 seams and
   libappindicator (already a documented build prerequisite).
9. **Sleep timer — designed in §6.2.** No fade exists today; `FadeAndPause` adds it on
   the existing volume path.

## 5. Architecture

### 5.1 Data ownership and files

New modules in `crates/oxitide-app`: `session.rs`, `history.rs`, `loves.rs`,
`track_flags.rs`. All files live under the existing `ProjectDirs` app-data root
(`%APPDATA%\OxiTide\`), use atomic tmp+rename writes (the `config.rs` pattern), and
carry their own `version` where the format is a single object.

| File | Content | Lifecycle |
|---|---|---|
| `session.json` | `version`, `queue[]` (item fields + optional cached `explicit`/`ai` flags), `current_index`, `position_ms`, `shuffle`, `repeat`, `play_source`, `saved_at` | Written on a debounce; cleared on explicit logout; malformed → renamed `.bak`, treated as absent |
| `history.jsonl` | Append-only play records: `track_id, title, artist, album, duration_secs, started_at, listened_ms, completed` | Appended on play finalize; pruned oldest-first at `history.max_entries` |
| `loves.json` | `version`, `loves: {track_id → {lastfm, listenbrainz, loved_at}}`, `pending_listenbrainz[]` | Updated on love/unlove; malformed → `.bak` |
| `cache/track_flags.json` | `version`, `{track_id → {explicit?, ai?, fetched_at}}` | Written by enrichment lookups; pruned by count |

**Save policy.** A dedicated background task owns the session dirty-flag: flush at most
once per ~3 s while playing, immediately after track change / queue edit / pause /
shuffle / repeat / queue clear / play-source change, and on `RunEvent::Exit`. The 500 ms
`Position` tick only updates the in-memory dirty state — no disk I/O, no lock held
during events. The tray-hide path never exits, so the timer keeps flushing normally.

### 5.2 Engine additions (`oxitide-audio`)

- `PlaybackState` gains `current_index: Option<usize>` (from `Queue.current`), removing
  the duplicate-`track_id` ambiguity that `App.svelte`'s `findIndex` has today.
- `PlayTracks`/`PlayShuffled` gain `start_paused: bool`: the queue/current is set and
  `State` emits `playing = false` without opening output; `Seek` then positions the
  session. This removes the play→seek→pause audio blip in the existing restore path.
- `EngineCmd::FadeAndPause { duration_ms }`: ramps volume to 0 on the same path
  `SetVolume` uses (endpoint volume in exclusive/bit-perfect, sample scaling otherwise),
  pauses, then restores the pre-fade volume so the next play is not silent. Play/pause
  during a fade cancels it and restores volume. Emits `State` when done.

### 5.3 Commands, events, config

New Tauri commands (all registered in `main.rs`'s `generate_handler!`): session
`get_restored_session`, `set_play_source`, `clear_session`; history `get_history_page`,
`get_stats`, `clear_history`; love `get_love_state`, `set_love`; flags
`get_track_flags`; content setters write `config.content`; tray/hotkeys commands in
10.6. New DTOs mirror camelCase IPC conventions; `ui/src/lib/api.ts` stays in sync by
hand (AGENTS.md contract). No new remote origins: enrichment and feedback reuse the
already-allowlisted TIDAL/Last.fm/ListenBrainz hosts.

`config.toml` additions (serde `default`, no migration framework needed): `[content]`
`block_explicit`, `block_ai`; `[ui]` `close_to_tray` (default true); `[history]`
`enabled` (default true), `max_entries` (default 10000); `[hotkeys]` action →
accelerator table.

## 6. Features

### 6.1 Session restore (Plan 10.1)

- On login / silent `auth_restore`, the UI calls `get_restored_session`. When present,
  `player.restore()` is extended to `(queue, index, position, shuffle, repeat,
  playSource)` and runs `playTracks(..., start_paused) → seek → setShuffle → setRepeat →
  playSource.set` with no audio. The same-process session-expired snapshot wins when
  present (fresher than the debounce); disk is the restart fallback.
- Exact queue order is preserved. A test pins that restoring shuffle does not reorder
  the queue; if the engine's `SetShuffle` reshuffles, the plan adds a restore-safe path.
- Lifecycle: explicit logout clears `session.json` and UI state; session-expired keeps
  it. Clearing the queue saves an empty session. Removed/unavailable tracks are skipped
  by the existing engine advance logic; a restore with nothing playable is an empty
  state, not an error.
- The play-source marker persists through `set_play_source` (playlist views already
  call `playSource.set`); every other play origin persists `null`, matching today's
  clearing semantics.

### 6.2 Sleep timer (Plan 10.2)

- Player-bar moon control: Off / 15 / 30 / 45 / 60 / custom minutes / End of current
  track, with a live countdown while armed.
- Duration mode fires `FadeAndPause(5000)` when the countdown ends; end-of-track mode
  fires it when the 500 ms position ticks show ≤5 s remaining. Pausing before the track
  end stops the advance; the ≤500 ms granularity is accepted.
- The timer is in-memory, resets on restart, and disarms after firing or when the queue
  ends. The pause keeps queue and position, so playback resumes where it stopped.

### 6.3 Playback history and stats (Plan 10.3)

- A `history.rs` recorder rides the same engine-event feed as the scrobbler (no new
  engine work). A play finalizes on track transition / queue end / app exit with the
  §5.1 record. Skips under ~30 s are not recorded; tracks under 60 s need ≥50%.
  `completed` is true at ≥90% listened or natural end.
- `get_history_page` prepends the in-flight play from memory so the current track
  appears immediately (Stats counts it only once finalized). `clear_history` empties
  the log.
- UI: a History route from the library sidebar with two tabs:
  - **Recently played** — grouped by local day, relative timestamps, cover/title/
    artist/album; clicking a row plays that track.
  - **Stats** — total listening time (all-time / 7 / 30 days), play counts, top
    artists/tracks/albums, and a 30-day plays bar chart as inline SVG/CSS (no new
    dependency). Day grouping uses the local timezone; `started_at` stays UTC.
- Stats are derived on read from the log. PM-D1 gate: `get_stats` ≤50 ms at the 10k
  cap; otherwise the plan adds a periodic aggregate.

### 6.4 Scrobble love (Plan 10.4)

- UI: a love toggle in the player bar / NowPlaying (visible when a love-capable service
  is connected) plus a track-row context-menu action. It is visually and behaviorally
  distinct from the TIDAL heart; no mirroring (QoL-D7). State reads from `loves.json`;
  no read-back reconciliation with services.
- **Last.fm:** `track.love`/`track.unlove` signed with the existing session key.
  Failures and offline periods enqueue `Love`/`Unlove` intents onto the existing
  scrobble queue so backoff/retry is reused; `get_love_state` returns local state,
  including which loves are still pending.
- **ListenBrainz:** now-playing submissions add `return_msid: true` and cache the
  returned msid per track. Love posts `recording-feedback` with score 1 (unlove → 0).
  If no msid is known, one `/1/metadata/lookup` attempt by artist+title; on failure the
  love stays pending in `loves.json` and flushes on the track's next play or a service
  reconnect.
- Disconnecting a service leaves local love state intact; reconnecting does not
  retroactively replay old loves (only pending ones).

### 6.5 Content blocking (Plan 10.5)

- Settings gains a **Content** tab: `block_explicit` and `block_ai` toggles with copy
  explaining client-side enforcement and its limits.
- **Flags.** `explicit` comes from the already-parsed `Track` DTOs and is applied
  immediately. `ai` comes from public v2 `tracks` attributes via `track_flags.rs`:
  `get_track_flags(ids)` batches ≤20 ids per request, caches results in memory +
  `cache/track_flags.json` (recording both flags), and fails open (QoL-D11). The UI
  `trackFlags` store enriches views asynchronously; rows render immediately and gray out
  when flags land (a brief un-grayed flash for AI is accepted).
- **Enforcement.** `TrackTable` rows show muted styling plus `E`/`AI` badges; clicking a
  blocked row shows a toast pointing at Settings. Every enqueue path filters blocked
  tracks: `play_track_ids` does it Rust-side (it already fetches track DTOs, and AI
  enrichment rides the same batch); UI-built `play_tracks` paths filter through the
  flags store first, awaiting enrichment before play.
- **Toggles.** Enabling `block_explicit` clears the queue behind a confirm (official
  parity); enabling `block_ai` removes blocked items from the queue. If the current
  track is affected, playback advances. Turning either filter off is non-retroactive.
- **Restore.** `session.json` carries each item's cached flags at save time; restore
  drops blocked items under the current config and fails open on unknown flags —
  no boot-time API storm.
- **Considered and rejected:** v2 `searchResults` `explicitFilter` (server-side) because
  search is v1-shaped; search filters client-side like every other surface.
- **Documented limitations:** PageItem cards (Home/Explore) cannot gray out before play
  (no track flags on the DTO), though playback is still blocked at the queue; removing
  AI content from server-side personalization is official-app-only.

### 6.6 System tray and global hotkeys (Plan 10.6)

- Enable Tauri's `tray-icon` feature. Tray menu: Show/Hide, Play/Pause, Next, Previous,
  separator, Quit; the tooltip mirrors the current track from `NowPlaying`; menu items
  dispatch through the existing engine commands.
- `ui.close_to_tray` (default true): `CloseRequested` → `prevent_close()` + `hide()`;
  tray Quit runs the normal exit path (session save, scheme restore, cache-clear-on-
  exit). Setting off restores today's close-to-exit behavior.
- Fix the single-instance callback to `show + unminimize + set_focus` so relaunching
  recovers a hidden window (today it only focuses it).
- Global hotkeys via `tauri-plugin-global-shortcut`: config-mapped actions `play_pause`,
  `next`, `prev`, `volume_up`, `volume_down`, `show_hide`, `love`. Settings gets
  per-action capture fields (click-to-record, clear, parser validation); conflicts show
  a toast and keep the previous binding. Registration runs at startup and on save,
  unregistering the old set first. Defaults are empty. While the window is focused, the
  global handler skips accelerators that match the in-app shortcut table so the DOM path
  owns them (no double-fire). Hardware media keys remain on SMTC/MPRIS.
- Platform caveats: Linux tray depends on libappindicator (already in
  `docs/linux-build.md`) and GNOME/Wayland may need an extension; Wayland global
  hotkeys depend on an XDG portal. Failures surface a notice and never block the app;
  bare-metal verification follows the Track 8 exemption pattern.

## 7. Testing and verification

- **Rust (`cargo test --workspace`, wiremock only, `MemoryStore` for keyring):**
  session store round-trip/versioning/atomic write/malformed-`.bak`; save-debounce
  behavior; history recorder state machine, threshold, retention, stats math; love
  intents, Last.fm signing, LB feedback + msid capture + pending flush + lookup
  fallback; `track_flags` batching/cache/fail-open; engine tests for `FadeAndPause`,
  `start_paused`, `current_index`; config serde defaults.
- **UI (`npm --prefix ui test`, `npm --prefix ui run check`):** extended restore
  sequence (all six fields, start-paused ordering); flags store gray-out and
  pre-enqueue filtering; sleep-timer countdown/end-of-track firing; hotkey validation;
  tray-setting persistence; history/stats rendering with fixture data.
- **Manual GUI smoke checklists** in `plans/notes/` (human, real account): restart
  restore paused, sleep fade at expiry and end-of-track, history/stats views after a
  real session, love on both services incl. an offline retry, content blocking incl.
  queue clear/prune and restore filtering, close-to-tray + Quit, global hotkeys from an
  unfocused window.

## 8. Performance (PM-D1)

- Session writes are debounced, atomic, and off the position-tick path.
- Flag enrichment is batched (≤20/request), cached, and asynchronous; no list render
  waits on it.
- `get_stats` ≤50 ms at the 10k-entry cap (`get_history_page` paginated); the plan
  states its impact against the Track 3 budgets.
- Restore happens after login completes, so no new work lands before first paint; the
  existing S1/S2 gates stay the reference.
- No new polling: history/love/flags are event-driven or command-invoked.

## 9. Risks and open items

- `SetShuffle` may reorder on restore — pinned by a 10.1 test; fallback is a
  restore-safe no-op path.
- LB may not return `msid` despite `return_msid: true` on our token — the pending path
  keeps love correct regardless.
- `ai` may be absent for some catalog items/countries — fail-open by design, so
  blocking is best-effort there.
- Tray/hotkey builds on Linux depend on libappindicator/portal availability — caveats
  documented; verification follows the Track 8 exemption pattern.
- `history.jsonl` growth is bounded by pruning, but prune+append cost at the cap is
  measured in 10.3; a rotation scheme is the fallback.
- Global hotkey conflicts are user-environment-specific; failed registration is
  surfaced, not retried aggressively.

## 10. Plan sequence and closure

Plans execute in order, each with TDD, the full gate suite, and its own smoke note:

1. **10.1 Session restore** — `session.rs`, engine `current_index` + `start_paused`,
   `get_restored_session`/`set_play_source`/`clear_session`, UI restore extension.
2. **10.2 Sleep timer** — `FadeAndPause`, timer UI, countdown/end-of-track logic.
3. **10.3 History and stats** — `history.rs`, commands, History route and stats UI.
4. **10.4 Scrobble love** — `loves.rs`, Last.fm love/unlove intents, LB msid/feedback,
   UI toggles.
5. **10.5 Content blocking** — `track_flags.rs`, Content tab, badges/gray-out, enqueue
   filtering, toggle/restore behavior.
6. **10.6 Tray and global hotkeys** — tray feature + menu, close-to-tray, single-
   instance fix, hotkey settings/registration.

Track closure uses the Track 3/Track 8 completion-note pattern: a dated
`plans/notes/` note listing what was verified on Windows, what is WSL2-verified, and
what is exempted for bare-metal Linux, plus the AGENTS.md updates for the new modules,
commands, and config sections.
