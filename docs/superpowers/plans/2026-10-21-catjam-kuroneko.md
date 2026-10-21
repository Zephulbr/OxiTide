# CatJam & Kuroneko Implementation Plan

> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** add two easter eggs.
- **CatJam:** an original pixel cat on the player bar cover that bobs to the beat. Tempo
  and beats come from local analysis of the decoded audio; no track id leaves the
  process.
- **Kuroneko:** a black companion cat for oneko (right-click), and petting (rubbing the
  cursor over either cat floats hearts).

**Architecture:**
- **Beats:** a read-only `BeatTap` in `oxitide-audio` feeds `automix::analyze_beats` on
  a helper thread. The result goes out as `EngineEvent::Beat`.
- **Output lead:** the backends report the queued, unheard audio (`AudioOutput::queued_ms`).
  The engine smooths it onto the `Position` event as `lead_ms`.
- **UI:** a pure beat clock turns position minus lead into a beat phase. `CatJam.svelte`
  draws frames from a sprite rendered at runtime from text grids.
- **Kuroneko:** a second `createNekoEngine` in `Neko.svelte`, plus a pure rub detector.

**Spec:** `docs/superpowers/specs/2026-10-21-oxitide-catjam-kuroneko-design.md`

## Global constraints

- **Do not commit.** The owner commits. The working tree already holds the owner's
  uncommitted UI work: `TrackTable`, `NowPlayingMark`, `app.css`, `rack.css`, and
  `ui/design/DESIGN.md`. Don't touch those files, except for additive edits to
  `DESIGN.md` in Task 11.
- **No new dependencies.**
- **No network access of any kind** in the feature. Tests use synthetic audio only.
- **Styling:** semantic tokens only, and no `data-mode` branching (AGENTS.md).
- **IPC:** camelCase args from the UI, snake_case Rust params. Keep `api.ts` in sync by
  hand.
- **Command registration:** each new `#[tauri::command]` goes in
  `generate_handler![]` (`main.rs`). Each is classified in `SYNC_COMMANDS`
  (`commands.rs` tests, ~L2710) when sync, and in remote `NOT_EXPOSED`
  (`remote/rpc.rs` ~L496).
- **Gates, each task:**
  - Rust tasks: `cargo test --workspace` and
    `cargo clippy --workspace -- -D warnings`.
  - UI tasks: `npm --prefix ui run check` and `npm --prefix ui test`.

## Review focus

Each item is pinned to a test in the task that owns it.

1. **The written position runs ~200 ms ahead of what is heard.** The lead must come
   through as a number, not as 0 (Task 2: fake output reporting `queued_ms`; WASAPI
   occupancy arithmetic).
2. **A seek inside a track must not mix pre- and post-seek audio in one analysis window**
   (Task 3: discontinuity resets the window).
3. **A beat result for a track that is no longer playing must be dropped** (Task 4:
   stale result test; Task 7: component ignores another `trackId`).
4. **The tap must never block the playback worker** (Task 3: `push` with a held lock).
5. **Paused means idle:** no RAF loop and no tracking when CatJam is unmounted (Task 7:
   mount/unmount calls; paused loop test).
6. **Petting must not fire when the cursor just passes over a cat** (Task 9:
   slow-pass and outside-box cases).

---

### Task 1: Sprite module + preview (owner checkpoint, CJ-D11)

**Files:** create `ui/src/lib/catjamSprite.ts` and `ui/src/lib/catjamSprite.test.ts`.

**Interfaces:**
- `CATJAM_SIZE = 32`, `BOB_FRAMES` (8 grids), `SWAY_FRAMES` (2 grids).
- `PALETTE: Record<string, string | null>`: `.` transparent, `k` outline, `w` fur,
  `p` inner ear / nose, `g` shade.
- `bobFrame(beatIndex, phase): number`, `swayFrame(tMs): number`.
- `renderSheet(doc?: Document): string`, which renders a `data:` URL sheet with one row
  per frame set.

**Steps:**
- [x] Write the tests:
  - every grid is 32×32;
  - only palette characters appear;
  - `bobFrame` covers frames 0–7 across two beats and lands on frames 0 and 4 at
    `phase = 0`;
  - `swayFrame` alternates.
- [x] Author the grids: oneko style, a front-facing sitting cat. The head tilts left
  across frames 0–3 and right across 4–7, the ears flick on 0 and 4, and the body stays
  still.
- [x] Build a throwaway preview: an HTML page that draws the grids at 1× and 3× on the
  dark and light surface colours and animates at 120 BPM with a click track. Publish it
  with `html_render`.
- [x] **STOP for owner approval.** The shape was approved on 2026-10-21, and the owner
  asked for more colours.
- [x] Coats (CJ-D12):
  - add the marking characters `s c e l m` to the grids;
  - add `COATS`/`COAT_IDS`/`COAT_LABELS`/`isCoat` and `renderSheet(coat)`;
  - add tests that every character is defined in every coat, that White's markings are
    invisible, and that eye/lid contrast holds (≥3 and ≥2.5).
- [x] The owner confirms the six coats in the preview (2026-10-21).

### Task 2: Output lead (`queued_ms` → `Position.lead_ms`)

**Files:**
- `crates/oxitide-audio/src/output.rs`: the trait method, with default `None`.
  `NullOutput` returns `Some(0)` plus a settable test value.
- `wasapi_output.rs`: ring occupancy plus the negotiated buffer duration, stored at
  setup.
- `asio_output.rs`: ring occupancy, plus `2 × buffer_frames / rate`, plus `get_latencies`
  output. Read it at open, and refresh it on `K_LATENCIES_CHANGED`.
- `routed_output.rs`: delegates to the open backend.
- `alsa_output.rs`: ring occupancy if it uses `ring.rs`, otherwise `None`.
- `engine.rs`:
  - `Position { position_ms, duration_ms, lead_ms: u32 }`;
  - an EMA (α = 0.3) sampled in `emit_position_if_due`, reset on a new session or a
    flush.
- `crates/oxitide-app/src/player.rs`: the `EngineEventDto::Position` mapping.
- The other `Position` match sites get `..` patterns: `media_controls.rs`,
  `history.rs`, `session.rs`, `scrobble/tracker.rs`, `remote/events.rs`, and the tests.
- `ui/src/lib/api.ts`: the `Position` event type gains `lead_ms`.

**Steps:**
- [x] Add `ring::queued_ms(capacity, free_slots, samples_per_sec) -> u32` with unit
  tests: a full 200 ms ring reads 200, an empty one reads 0, and a zero rate reads 0.
- [x] Add `output::LeadEstimate`: α = 0.1 per write, a 0.5 s settle after each reset,
  the value kept across resets, and cleared when the output reports `None`. It has
  unit tests.
- [x] Add an engine test (`position_events_carry_the_output_lead`): a `NullOutput`
  reporting 230 ms gives `output_lead_ms() == 230`, and the next session's first
  `Position` carries 230.
- [x] Backends:
  - WASAPI: `get_buffer_size()` at setup (wasapi 0.24 has no `GetStreamLatency`);
  - ASIO: `Driver::latencies()`, re-read on `K_LATENCIES_CHANGED`, falling back to
    two buffers;
  - routed: delegates, with a test;
  - ALSA: left at `None` (Linux follow-up, see spec §5.2).
- [x] Run the gates (2026-10-21): workspace tests, clippy, svelte-check and vitest
  all green.

### Task 3: `BeatTap` (`crates/oxitide-audio/src/beat.rs`)

**Interfaces** (spec §5.1):
- `BeatClock { track_id, bpm: Option<f32>, period_ms, anchor_ms, confidence }`;
- `BeatTap::{new, set_enabled, is_enabled, push(track_id, start_frame, &PcmBuffer, channels, rate), take_window() -> Option<Window>}`;
- `analyze(window) -> Option<BeatClock>`.
The window uses mono `f64` samples at the decimated rate, with `start_ms`.

**Constants:** `FIRST_WINDOW_S = 8`, `WINDOW_S = 12`, `REFRESH_S = 30`, decimation
`floor(rate / 11025)`.

**As built:** `push` returns the completed `Option<Window>` directly, so there is no
`take_window`. Decimation is the largest factor ≤ `rate / 11025` that divides the rate,
which keeps the analysis rate an integer (192 kHz → 16, 12 kHz).

**Steps:**
- [x] Make the automix test module and its `drums`/`band`/`RATE` `pub(crate)` (test
  only).
- [x] Write the 12 tests in `beat::tests`, all green:
  - the first 8 s window finds 90/128/174 BPM, octave-matched within 1%, with the
    anchor within 30 ms;
  - a dense band mix works;
  - anchors stay in track time when the tap starts mid-track;
  - a seek or a new track restarts the window;
  - later windows are 12 s and start 30 s after the previous start;
  - a disabled tap ignores pushes, and disabling drops a partial window;
  - `push` doesn't wait on a held lock;
  - `I32` input is handled;
  - a held tone claims no beat;
  - decimation stays an integer rate.
- [x] Implement it. The low-confidence threshold is `automix::CONFIDENCE_FLOOR`.
- [x] Run the gates.

### Task 4: Engine wiring

**Files:** `crates/oxitide-audio/src/engine.rs` and `lib.rs`.

**As built:**
- The API is `Engine::set_beat_tracking(bool)` rather than a public tap getter.
  Enabling it re-sends the cached clock for the current track.
- Two test seams exist: `BeatTap::set_window_ms` (`cfg(test)`), and
  `Inner.beat_analyzer` (`beat::analyze` in production). The 2 s fixture can't hold a
  real 8 s window; real analysis is covered by `beat::tests`.
- The helper holds a `Weak<Inner>` and exits with the engine. `shutdown` wakes it.

**Steps:**
- [x] Add `beat_tap`, `beat_jobs`, `beat_cache` and `beat_analyzer` on `Inner`, and
  `Engine::set_beat_tracking`.
- [x] Push in the main write loop before `dsp.process(buf)` with
  `(track_id, frames, &buf, channels, rate)`.
- [x] Add the `oxitide-beat` helper (lazy, newest window wins):
  - a stale result is remembered but not sent;
  - it keeps a 64-entry LRU;
  - the cached clock is sent at track start.
- [x] Write the 5 engine tests:
  - a clock is sent for the playing track;
  - with tracking off, nothing is analyzed;
  - a stale clock is dropped but remembered;
  - re-enabling re-sends the known clock;
  - the LRU evicts the oldest entry.
- [x] Add a `Beat` arm to `history`, `media_controls`, `scrobble`, `remote/events`
  (ignored there, desktop-only) and `player::to_dto`.
- [x] Run the gates.

### Task 5: App wiring

**As built:**
- The DTO fields are snake_case like the sibling `position` event:
  `{ type: "beat", track_id, bpm, period_ms, anchor_ms, confidence }`.
- The coat is a typed `CatjamCoat` serde enum, like `CoverShape`/`DynamicColor`. An
  unknown id is rejected by deserialization at the IPC boundary. A hand-edited config
  with an unknown coat is treated like any other bad enum value (the config is backed
  up and reset).
- `AppState` remembers the wanted tracking state and applies it in `set_player`, so an
  engine rebuilt after re-login keeps tracking. The level meter re-enables itself every
  tick; beat tracking has no tick to do that.

**Steps:**
- [x] `player.rs`: `EngineEventDto::Beat`, plus a DTO shape test (with `lead_ms` on
  `position`).
- [x] `commands.rs`: `set_catjam`, `set_catjam_coat(coat: CatjamCoat)` and
  `set_beat_tracking`. All are sync and listed in `SYNC_COMMANDS` (beat tracking with
  its reason); all are registered in `main.rs` and listed in `NOT_EXPOSED`.
- [x] `config.rs`: `catjam` and `catjam_coat` (default off, white), the literal
  updates, a round-trip test, and an id test matching the UI's `COAT_IDS`.
- [x] Run the gates: 452 app, 387 audio and 222 tidal tests pass, and clippy is clean.

### Task 6: UI plumbing + beat clock

**Files:**
- `ui/src/lib/api.ts`: the `BeatClock` type, `setBeatTracking`, `setCatjam`,
  `setCatjamCoat`, `Config.ui.catjam`, `Config.ui.catjam_coat`, and the Beat event
  union member.
- `ui/src/lib/appearance.svelte.ts`:
  - `catjam` + `setCatjam`, as with `oneko`;
  - `catjamCoat: CoatId` + `setCatjamCoat`, with `isCoat` guarding the loaded value.
- `ui/src/lib/beatClock.ts` and `beatClock.test.ts`.

**As built:**
- The player store is untouched. `CatJam.svelte` subscribes to `playback_event`
  itself and feeds a non-reactive clock from `position` and `beat` events, so no other
  reader pays for them.
- `CatjamCoat` lives in `api.ts`; `catjamSprite.ts` types `COAT_IDS` against it
  (`satisfies`).
- `beatClock` adds `JUMP_MS` (1 s): a larger drift is an unflagged seek and snaps.

**Steps:**
- [x] Add `api.ts`: the `CatjamCoat` and `BeatClock` types, the `beat` event member,
  `lead_ms` on `position`, `setCatjam`/`setCatjamCoat`/`setBeatTracking`, and the
  config fields.
- [x] Add `appearance`: `catjam` and `catjamCoat` with rollback-on-failure setters; an
  unknown stored coat reads as white. It has 4 tests.
- [x] Add `beatClock` and its 10 tests (lead subtraction, sub-15 ms drift ignored,
  easing, seek/jump snap, pause freeze, beat math before and after the anchor, a
  degenerate period).
- [x] Run the gates.

### Task 7: `CatJam.svelte` + mount

**Files:** create `ui/src/lib/components/CatJam.svelte` and `CatJam.test.ts` (jsdom);
modify `PlayerBar.svelte` (beside the `<Cover>` at ~L72).

**As built:**
- The props are `{ player, coat }`; the player bar passes `appearance.catjamCoat`.
- The cat is **32 px, not 40**. The thumb is 52 px (the preview showed an 80 px cover),
  and native size keeps the pixels square.
- It sits in a new `.thumb-wrap` beside the button, because the button clips
  (`overflow: hidden`).
- A `beat` event is kept even if the player has not named its track yet;
  `clockForCurrent` filters.

**Steps:**
- [x] Write the 6 jsdom tests:
  - tracking on while mounted and off with unlisten when removed;
  - `aria-hidden`;
  - another track's clock is ignored;
  - no loop while paused;
  - idle until a clock, then a bob frame;
  - the coat is drawn and redrawn on change.
- [x] Implement it, and mount it in `PlayerBar.svelte`.
- [x] Run the gates.

### Task 8: Settings

**As built:** Settings has no radio groups. Its choice control is `Segmented` (a
labelled group of `aria-pressed` buttons with arrow-key focus), so
`settings/CoatPicker.svelte` follows that pattern, with `<img>` swatches from
`renderIdle(coat)` (a `data:` URL, already allowed by `img-src`). The coat row isn't
indexed because it is only on the page while the toggle is on; "coat" is a keyword of
the Jamming cat entry instead.

**Steps:**
- [x] Add the "Jamming cat" `ToggleRow` (with the reduced-motion note) and the "Coat"
  row while it is on.
- [x] Add `CoatPicker` and its 3 jsdom tests (labels and pressed state, the pick, arrow
  keys that don't reach the transport shortcuts).
- [x] ~~Add the credit line and its URL to `opener:allow-open-url`.~~ Removed after
  review: the feature is too far from the plugin for the credit to make sense.
- [x] Add the `settingsIndex` entry `catjam`; the index id test passes.
- [x] Run the gates.

### Task 9: Petting + `pet()`

**Files:** create `ui/src/lib/petting.ts` and `petting.test.ts`; modify `oneko.ts` and
`oneko.test.ts`.

**Steps:**
- [x] Write the 7 `petting` tests: one pet per rub, a new rub needed after it, spread-out
  turns, a straight pass, outside the box, leaving the box resets, jitter under 2 px.
- [x] Write the 5 `oneko` tests:
  - `pet()` holds still with `tired` for `PET_TICKS`, ignoring the cursor;
  - a drag ends it;
  - no pet while dragged;
  - a sleeping cat stays asleep;
  - `asleep()` also reports dozing off on her own (new, used by `pet()`).
- [x] Implement both.
- [x] Run the gates.

### Task 10: Kuroneko + hearts in `Neko.svelte`

**As built:**
- **Context menu:** the app's menus are per-row actions (`menuFor`), and the window
  listener (`nativeContextMenu.ts`) only suppresses the webview menu. So
  `preventDefault` on the cat is enough and `stopPropagation` isn't needed.
- **Shared sleep:** each double-click flips `sleepCommanded` for both cats. Kuroneko is
  synced to it when summoned, so a cat summoned while oneko sleeps lies down beside her.
- **No rubs while dragging:** while a cat is dragged, rubs are ignored, because she
  stays under the cursor.
- **Hearts:** the heart is a 7×6 pixel SVG in `--accent` (like the favourite heart).
  Three float up per pet, staggered by 140 ms.

**Steps:**
- [x] Refactor the shell around two `Cat` records sharing one RAF loop, the drag code,
  and the double-click.
- [x] Kuroneko: right-click to summon or dismiss, she follows oneko, uses
  `filter: invert(1)`, and gets the perch 28 px left.
- [x] Hearts: float-and-fade over 900 ms, removed on `animationend`, at most 6.
- [x] Persistence: `oxitide:kuroneko` plus `kx/ky` in `oxitide:oneko`; old values still
  load.
- [x] Add `Neko.test.ts` (new, 10 jsdom tests with the loop driven by hand):
  - summon and dismiss, and the right-click is suppressed;
  - restore, with and without kuroneko;
  - she follows oneko;
  - a double-click on kuroneko puts both to sleep;
  - rub hearts that clear themselves;
  - passing over a cat floats nothing, and nor does shaking her mid-drag;
  - at most 6 hearts.
- [x] Run the gates.

### Task 11: Docs + smoke

**Steps:**
- [x] Add the `AGENTS.md` Architecture bullet "Easter eggs" (local-only beats and never
  sending track ids, mount-driven tracking, `lead_ms`, the sprite as code, kuroneko and
  petting).
- [x] Add an "Easter eggs" row to the `ui/design/DESIGN.md` §3 "Where things live" table
  (additive).
- [x] Create `docs/superpowers/plans/notes/2026-10-21-catjam-kuroneko-smoke.md`.
- [x] Run the full gates (2026-10-21), all green:
  - `cargo test --workspace`: 452 app, 387 audio and 222 tidal tests pass;
  - `cargo clippy --workspace -- -D warnings`: clean;
  - `npm --prefix ui run check`: 0 errors;
  - `npm --prefix ui test`: 136 files, 1077 tests pass.
- [ ] Owner smoke (`notes/2026-10-21-catjam-kuroneko-smoke.md`). Pending.
