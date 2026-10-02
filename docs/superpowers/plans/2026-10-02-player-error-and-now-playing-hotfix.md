# Player Error Toast and Now Playing Tab Hotfix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move engine playback errors (notably the device-stall failure) into the top-right toast host with friendlier copy and a Retry action, and keep the Now Playing **Lyrics / Next up** tab selection across track changes.

**Architecture:** The engine `error` event already lands in `player.svelte.ts` as `state.error`; `createPlayer` now takes the toast store and mirrors each error into one persistent toast (replacing the previous one, dismissed on `now_playing`/retry). The Rust stall message is reworded and `Error::Output`'s Display drops the `output error:` prefix. The lyrics store keeps `tab` across `open(track)` calls and only resets when playback stops; the view only shows the loading row when the Lyrics tab is active.

**Tech Stack:** Rust (thiserror), Svelte 5 runes + TypeScript (vitest). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-29-oxitide-track4a-lyrics-design.md` (L-D1 amended by this hotfix). No spec covers the player error surface; the diagnosis below is the authority.

## Diagnosis (root causes)

- **Device-stall error blocks the player bar.** `wasapi_output.rs` returns `Error::Output("audio device stalled")` after a 2 s push timeout; `Error::Output` displays as `output error: {0}` (`error.rs`). The UI mirrors it as `player.state.error` and `PlayerBar.svelte` draws an absolutely-positioned `.player-error` box at `right: 14px; top: 50%` over the bar's volume/quality controls (`Mini.svelte` duplicated it inline). The notification surface the rest of the app uses is the fixed top-right `ToastHost`, which already persists errors and supports an action button.
- **Now Playing tab is not sticky.** `lyrics.svelte.ts` reset `tab = "lyrics"` on every track change (Track 4a L-D1: "tab choice resets per track"), and the view keys the fallback column off `lyrics === null`, so a skip while a fetch is pending (or for a track without lyrics) never preserves the chosen page.

## Global Constraints

- Echo the earlier project rules: no new crates/packages; exact backend error strings used by tests stay stable apart from the intentional `Error::Output` change; HTTP tests never hit live TIDAL; tokens are never logged.
- TDD: every behavior change lands with a test watched failing first.
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **One error toast, not a stack:** a second `error` event replaces the first; `now_playing` and `retry()` dismiss it. `playerErrors.test.ts` pins all three.
- **Optional toasts:** `createPlayer()` without a `Toasts` keeps working headless (`player.restore.test.ts` unchanged).
- **No duplicate bottom-right copy:** the bar's `bit_perfect_notice` is hidden while `state.error` is set, since the engine mirrors the error into the notice (`engine.rs`).
- **Tab persistence:** `setTab("next")` then a track change keeps `next`; `open(null)` resets to `lyrics`. `lyricsStore.test.ts` pins both.
- **Loading row:** only shown when the Lyrics tab is active, so a persisted Next up selection is not replaced by "Loading lyrics…".

---

### Task 1: Player errors as toasts

**Files:**
- Modify: `crates/oxitide-audio/src/error.rs`, `crates/oxitide-audio/src/wasapi_output.rs`
- Modify: `ui/src/lib/player.svelte.ts`, `ui/src/App.svelte`
- Modify: `ui/src/lib/components/PlayerBar.svelte`, `ui/src/views/Mini.svelte`
- Test: `ui/src/lib/playerErrors.test.ts`

- [x] **Step 1: Failing test** — `playerErrors.test.ts` mocks `@tauri-apps/api/event` + `./api` and asserts: error event shows one persistent `severity: "error"` toast with a `Retry` action; a second error replaces it; `now_playing` dismisses it; invoking the action dismisses it and calls `api.retry`; a toast-less `createPlayer()` still records `state.error`.
- [x] **Step 2: Rust copy** — `wasapi_output.rs` message → `"Audio device has been disconnected or stalled"`; `error.rs` `#[error("{0}")]` for `Output`; stable-message test added.
- [x] **Step 3: Store wiring** — `createPlayer(toasts?)` shows/replaces/dismisses the toast; `App.svelte` creates `toasts` before `player` and passes it.
- [x] **Step 4: Remove inline surfaces** — delete `.player-error` from `PlayerBar.svelte` and the inline error from `Mini.svelte`; hide `bit_perfect_notice` while `state.error` is set.
- [x] **Step 5: Verify** — `npm --prefix ui test -- src/lib/playerErrors.test.ts` green (5/5).

### Task 2: Sticky Now Playing tab

**Files:**
- Modify: `ui/src/lib/lyrics.svelte.ts`, `ui/src/views/NowPlaying.svelte`
- Test: `ui/src/lib/lyricsStore.test.ts`

- [x] **Step 1: Failing test** — rename the reset test to `keeps the chosen tab on a track change` (`next` survives `open(track 2)`); extend the null-track test to assert the reset to `lyrics`. Watched fail first.
- [x] **Step 2: Store change** — drop `tab = "lyrics"` from the track-change path, keep it in the null branch.
- [x] **Step 3: View change** — `{#if lyrics.tab === "lyrics" && lyrics.loading}` gates the loading row.
- [x] **Step 4: Verify** — `npm --prefix ui test -- src/lib/lyricsStore.test.ts` green (8/8).

### Task 3: Docs and gates

- [x] **Step 1:** amend Track 4a L-D1 and §6 (persistence, loading-row wording).
- [x] **Step 2:** `cargo test --workspace` (510 passed, 1 ignored); `cargo clippy --workspace -- -D warnings` clean; `npm --prefix ui test` (388 passed); `npm --prefix ui run check` (0 errors, 0 warnings).
- [x] **Step 3 (manual, human):** disconnect the output device mid-play → toast appears top-right reading "Audio device has been disconnected or stalled" with Retry; Retry resumes from the failure point. Skip tracks while the Lyrics tab is selected and while Next up is selected → the page sticks; skip to a track without lyrics → Next Up column shows (no tabs), and the next track with lyrics returns to the previously chosen tab. *(Owner-verified 2026-10-02.)*

---

## Follow-up pass (second owner report)

Two regressions surfaced after the pass above:

1. **The stall text stayed in the player bar after a successful Retry.** On failure the engine writes the message into `inner.output_status.notice` (`engine.rs`) and the UI hides that notice while `state.error` is set. Recovery reopened the output and refreshed `inner.output_status` (`engine.rs` open path) but never published a `State`, so the last delivered state kept the stale notice; the UI also cleared `state.error` as soon as the retry command resolved, exposing it.
2. **The Next Up column flashed on every skip while the Lyrics tab was open.** The view derived `showLyrics` from `lyrics !== null`; a track change clears the store (and the store lags the track by one effect), so the fallback column rendered until the fetch settled.

### Task 4: Publish the recovered output status

**Files:**
- Modify: `crates/oxitide-audio/src/engine.rs`
- Test: `crates/oxitide-audio/src/engine.rs` (`output_failure_pauses_keeps_queue_and_recovers`)

- [x] **Step 1: Failing test** — subscribe before playing; record `(failure_seen, cleared_after_failure)` from `State` notices; after the device recovers and samples flow, assert the cleared notice was published (failed before the fix).
- [x] **Step 2: Engine change** — after a successful open refreshes `inner.output_status`, emit `EngineEvent::State(build_state(inner))`.
- [x] **Step 3: Verify** — `cargo test -p oxitide-audio output_failure_pauses_keeps_queue_and_recovers` green.

### Task 5: Latch output errors until recovery

**Files:**
- Modify: `ui/src/lib/player.svelte.ts`
- Test: `ui/src/lib/playerErrors.test.ts`, `ui/src/lib/player.test.ts`

- [x] **Step 1: Failing tests** — an `output` error survives `retry()` and a still-failing `State`; a `State` whose `bit_perfect_notice` no longer matches the error clears both the error and its toast; a non-output error still clears on retry.
- [x] **Step 2: Store change** — add `PlayerState.error_code` (set/cleared with `error`); in the event listener, clear output errors only when a `State` reports a different notice; `retry()` keeps output errors latched but clears the rest.
- [x] **Step 3: Verify** — `npm --prefix ui test -- src/lib/playerErrors.test.ts src/lib/player.test.ts` green (23/23).

### Task 6: Keep the lyrics layout while loading

**Files:**
- Modify: `ui/src/lib/lyrics.svelte.ts`, `ui/src/views/NowPlaying.svelte`
- Test: `ui/src/lib/lyricsStore.test.ts`

- [x] **Step 1: Failing tests** — `trackId` is set synchronously when a fetch starts, kept after a settled miss, and nulled when playback stops.
- [x] **Step 2: Store change** — expose `trackId` (set in `open`, kept on miss, cleared for a null track).
- [x] **Step 3: View change** — derive `currentLyrics`/`awaitingLyrics` from `trackId`; while Lyrics is selected and the fetch is pending, keep the lyrics layout and spine mounted with the "Loading lyrics…" placeholder inside the slot; the Next Up column only returns on a settled miss. Removed the loading row from the fallback.
- [x] **Step 4: Verify** — `npm --prefix ui test -- src/lib/lyricsStore.test.ts` green (10/10).

### Task 7: Follow-up docs and gates

- [x] **Step 1:** amend Track 4a §6 (loading stays in the lyrics column; Next Up only on a confirmed miss).
- [x] **Step 2:** `cargo test --workspace` (516 passed, 1 ignored); `cargo clippy --workspace -- -D warnings` clean (exit 0); `npm --prefix ui test` (392 passed); `npm --prefix ui run check` (0 errors, 0 warnings).
- [x] **Step 3 (manual, human):** disconnect mid-play → toast + Retry; reconnect → Retry → the bar text never reappears after recovery (and no bottom flash between click and resume). Skip tracks with the Lyrics tab open → no Next Up frame, even after a track without lyrics; skip with Next up open → the list stays. *(Owner-verified 2026-10-02.)*
