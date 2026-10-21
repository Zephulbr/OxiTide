# OxiTide — CatJam & Kuroneko (two easter eggs)

- **Status:** Revised after owner review (2026-10-21): custom video cut, output lead
  measured instead of hand-tuned, sprite preview checkpoint added, Rack/mini absence
  accepted
- **Date:** 2026-10-21
- **Owner:** V
- **References:** [nathanmontalvoGM/Custom-TidaLuna-CatJam `plugins/CatJam`](https://github.com/nathanmontalvoGM/Custom-TidaLuna-CatJam/tree/dev/plugins/CatJam)
  (behavior reference only; no license declared, so no code or assets are copied) ·
  `2026-09-29-oxitide-oneko-design.md` (the cat this builds on; §11 deferred the black cat)
- **Process note:** the egg choice, cat asset, and placement were picked by the owner
  (2026-10-21). This document is the standalone source of truth.

---

## 1. Purpose & Outcome

Two more easter eggs next to oneko:

1. **CatJam**: a small cat on the player bar's cover that bobs its head in time with
   the playing track.
2. **Kuroneko + petting**: right-click the oneko cat to bring out a black cat that
   follows her around. Rubbing the cursor back and forth over either cat pets her, and
   pixel hearts float up.

**Privacy outcome (the reason for this design):** the reference plugin gets tempo and
beat times by sending every track's TIDAL id to `api.vmohammad.dev/lyrics?filter=audioFeatures,audioAnalysis&tidal_id=…`.
OxiTide does **not** do that. Tempo and beats come from the audio the engine is already
decoding, through the existing `automix::analyze_beats` (Track 7b). No request is made,
nothing is written to disk, and no beat data leaves the process.

**Success:** with CatJam on, a steady 4/4 track has the cat visibly on the beat within
about 10 s of the track starting or a seek. Low-confidence music (ambient, rubato) gets
a calm sway rather than a fake beat. The cat freezes when paused and costs nothing when
off. Kuroneko and petting add no setting and no IPC.

## 2. Scope

**In scope:**
- a read-only `BeatTap` in `oxitide-audio`, its helper-thread analysis, and the beat
  event;
- one `set_beat_tracking` command, wired through the handler list, `SYNC_COMMANDS`, and
  the remote `NOT_EXPOSED` list;
- the output lead: how far the written position runs ahead of what is heard, measured
  by the output backends and carried on the `Position` event (§5.2);
- a pure UI beat clock;
- an original pixel CatJam sprite, approved by the owner from a frame preview before
  the engine work starts, in six coat colours chosen in Settings;
- the `CatJam.svelte` overlay on the player bar cover;
- the `ui.catjam` config flag;
- the kuroneko companion and the petting detector in the oneko shell;
- tests and a smoke checklist.

**Out of scope:**
- any network fetch, including TIDAL's own metadata (TIDAL's `bpm` field is not used, so
  the feature works offline and for cached tracks);
- custom video or GIF clips, local or remote (cut at review: keeping a video on the beat
  means seeking it, which hitches on keyframes, and it needs the clip's BPM typed in);
- the mini player, Now Playing, and the Rack. The player bar isn't rendered in the Rack
  shell or the mini player (`App.svelte`), so the cat doesn't appear there at all, and
  beat tracking stays off because the component isn't mounted. Accepted by the owner
  for now;
- the phone remote;
- the oneko cats bobbing to the beat (deferred, §11).

## 3. Decisions

| # | Decision | Rationale |
|---|---|---|
| CJ-D1 | **Beats are analyzed locally, never fetched.** The playback worker feeds a `BeatTap`, `automix::analyze_beats` runs on a helper thread, and the result is published as an engine event. | Owner requirement: no track ids sent to third parties. The analyzer already exists and is tested. |
| CJ-D2 | **Asset: an original pixel sprite only.** No custom video. *(Revised at review: the first draft also had a local video override.)* | The upstream `catjam.webm` (the Twitch catJAM meme) has no license. Video phase sync hitches on keyframe seeks and needs the clip's BPM typed in. |
| CJ-D3 | **Placement: player bar cover only.** The cat sits in the player bar's left zone between the track title and the heart (owner's choice after two looks: first the cover's top-right, then its bottom-left). Its size is a setting, 100–200 % of the sprite (`ui.catjam_scale`, default 125; capped at 2× so it fits the bar). It doesn't appear in the Rack shell or the mini player, which don't render the player bar. | Owner choice; it matches upstream's main spot. The Rack/mini absence was accepted at review. |
| CJ-D4 | **Toggle: `ui.catjam`** (default off), set by a Settings toggle in Extras, mirroring `ui.oneko`. | Same plumbing as oneko. |
| CJ-D5 | **Beat tracking runs only while CatJam is mounted.** `set_beat_tracking(true/false)` is called from the component's mount/unmount, like `set_level_meter`. While disabled the tap is a no-op. | The paused-is-idle rule and the S11 budget. |
| CJ-D6 | **Below `automix::CONFIDENCE_FLOOR` (0.35), the cat sways and does not bob.** The sway is a slow cycle that doesn't claim a beat. | A fake beat looks worse than none. |
| CJ-D7 | **Paused or hidden means frozen.** The RAF loop stops while paused, while `document.hidden` is true, or with no clock; it holds the current frame. | The Track 13 rule that paused is idle. |
| CJ-D8 | **Reduced motion: neither egg appears.** The same rule as oneko (N-D7). | Consistency. |
| CJ-D9 | **The output lead is measured, not hand-tuned.** Each output backend reports how much audio it holds that hasn't been heard yet (`AudioOutput::queued_ms`). The engine smooths it and publishes it as `lead_ms` on each `Position` event, and the UI beat clock subtracts it. A small `BEAT_TRIM_MS` (default 0) is left only for latency the driver doesn't report. | The written position leads the heard audio by about 200–260 ms (§5.2): roughly half a beat at 128 BPM, and the gap differs by backend. |
| CJ-D10 | **The cat snaps only on big drifts.** The UI extrapolates position between the 500 ms `Position` events and corrects only when it drifts more than 15 ms, easing the correction over about 200 ms. | Snapping on every event would make the cat skip a frame twice a second. |
| CJ-D11 | **Sprite approval first.** The first plan step is a frame preview (an HTML page that animates the frames at 120 BPM) for the owner to approve before any engine work. The shape was approved on 2026-10-21. | The sprite's look is the whole payoff of the egg. |
| CJ-D12 | **Coat colours: six palette swaps of the same frames.** The coats are White (the default, the approved draft), Black, Ginger tabby, Grey tabby, Tuxedo and Cream. The choice persists as `ui.catjam_coat`. | Owner request after approving the sprite (2026-10-21). Palette swaps add no frames. Markings that a coat doesn't use resolve to the fur or outline colour, so the White coat is pixel-identical to the approved draft. |
| KN-D1 | **Kuroneko is summoned by right-clicking the oneko cat**, and dismissed by right-clicking her. There is no setting. Her presence is kept in `localStorage` (`oxitide:kuroneko`), like oneko's position. | It's an easter egg. `preventDefault` stops the app context menu from opening. |
| KN-D2 | **Kuroneko is a second `createNekoEngine`** whose target is the white cat's position rather than the cursor, so the existing 48 px standoff keeps her trailing. She is drawn from the same gif with a CSS `filter: invert(1)`. | No new sprite and no engine change; the gif is MIT. |
| KN-D3 | **Shared sleep:** double-clicking either cat toggles sleep for both. Kuroneko's perch is the white cat's perch shifted 28 px left, so they curl up side by side on the seek bar. | It reads as a pair. |
| KN-D4 | **Petting is a rub:** at least 3 horizontal direction reversals over a cat's 32 px box within 1.2 s. A pet shows a contented frame for 1.5 s (no chasing during it) and floats 2–3 pixel hearts. A sleeping cat gets hearts but stays asleep. | Deliberate enough not to trigger when the cursor just passes by. |

## 4. Reference behavior (CatJam plugin, read 2026-10-21)

On every track change, the plugin calls `GET https://api.vmohammad.dev/lyrics` with
`filter=audioFeatures,audioAnalysis` and `tidal_id=<id>`. It reads `audioFeatures.tempo`
and `audioAnalysis.beats[].start` from the response. It then:
- sets the video's `playbackRate = tempo / 135.48` (the clip's own BPM);
- schedules `play()` on the next beat;
- resyncs on jumps of 2 s or more.

The video is muted and looped. It is appended to the small now-playing artwork, with an
opacity setting (default 80) and a library of video URLs.

We keep the idea of following the track's tempo and beat phase. We drop the service, the
video and its URL library, and the Now Playing copy.

## 5. Architecture

### 5.1 `crates/oxitide-audio/src/beat.rs` (new): `BeatTap`

The tap is modeled on `meter::LevelTap`:
- an `AtomicBool` enable flag, default off;
- `push` is a no-op while disabled and uses `try_lock`, so it never blocks the worker;
- it only reads samples.

```rust
pub struct BeatClock {
    pub track_id: u64,
    pub bpm: Option<f32>,   // None below CONFIDENCE_FLOOR
    pub period_ms: f32,
    pub anchor_ms: u64,     // track position of one beat
    pub confidence: f32,
}

impl BeatTap {
    pub fn set_enabled(&self, on: bool);
    /// `start_frame`: the track's own frame index of `buf[0]` (source rate, pre-DSP).
    pub fn push(&self, track_id: u64, start_frame: u64, buf: &PcmBuffer, channels: u16, rate: u32);
}
```

**Feeding the tap.** The worker pushes the decoded buffer just before `dsp.process(buf)`.
That buffer is in the track's own frames, at the source rate, before EQ and resampling,
so the beat positions map exactly onto `position_ms`. The tap box-decimates it to mono
at about 11 kHz (factor `floor(rate / 11025)`, the same as `onset_envelope`).

**Windows.** The tap fills a window from the stored samples:
- **8 s** for a track's first window, so the first estimate comes quickly;
- **12 s** for each later window, taken every 30 s of playback so tempo drift and
  section changes are followed.

**Discontinuities.** A new `track_id`, or a `start_frame` that doesn't follow on from the
last push (a seek), resets the window.

**Analysis.** A full window is handed to one `oxitide-beat` helper thread as a `Clip`
(mono, the decimated rate). Only one job runs at a time; a newer window replaces one
still queued, and a result for a track that is no longer current is dropped. The helper
runs `analyze_beats`, then:
- converts the fitted grid to `anchor_ms = window_start_ms + beats_s[last]·1000` and
  `period_ms`;
- sets `bpm = None` below `CONFIDENCE_FLOOR`.

The cost is one 512-point FFT pass over about 1,000–2,000 hops, a few ms every 30 s.

**Memory cache.** A per-`track_id` LRU of the last 64 clocks makes a replay or a seek
back into an analyzed track instant. A new window still refines the clock.

**`Engine` API.**
- `Engine::beat_tap() -> Arc<BeatTap>`, mirroring `level_tap()`.
- Results go out as `EngineEvent::Beat(BeatClock)`. There are about two per track
  minute, so the IPC cost is negligible.

**Bit-perfect.** The tap is read-only, so `bit_perfect` and `dsp_stages` are untouched.

**Other paths.** Crossfade, overlap, and `write_responsive` tail writes don't push. The
next track's own loop starts its own window.

### 5.2 Output lead (`oxitide-audio`)

**What was found (read 2026-10-21).** `position_ms` is stored right after
`output.write` returns, so it counts audio that has been queued, not audio that has been
heard. Both output backends sit behind a ring buffer:
- **WASAPI** (`wasapi_output.rs:123`): the ring holds `rate × channels / 5` samples,
  which is 200 ms. `ring::push_samples` blocks while the ring is full, so during steady
  playback the ring stays nearly full and the lead is about 200 ms. On top of that:
  - shared mode adds its device buffer (`buffer_duration_hns: def_time`, about 10 ms);
  - polling exclusive adds `def_time × 4` (about 40 ms).
- **ASIO** (`asio_output.rs:592`): the ring is the larger of 200 ms and 4 driver
  buffers. Then come the double buffer and the driver's output latency, which the
  `get_latencies` vtable entry in `asio_sys.rs` can report.

So the heard audio trails `position_ms` by roughly 210–260 ms, depending on the backend.
The first draft's `BEAT_LEAD_MS = 0` would have put the cat about half a beat late.

**The trait method.** `AudioOutput` gains
`fn queued_ms(&self) -> Option<u32> { None }`, with the default returning `None`.
- **WASAPI:** ring occupancy (`capacity − producer.slots()`) divided by
  `samples_per_sec`, plus the buffer duration negotiated at setup. Shared mode adds
  `GetStreamLatency` if it is available from the `wasapi` crate, otherwise `def_time`.
- **ASIO:** ring occupancy, plus `2 × buffer_frames / rate`, plus `get_latencies`
  output (read once at open and again on `K_LATENCIES_CHANGED`).
- **`RoutedOutput`:** delegates to the open backend.
- **`NullOutput` and the stubs:** return `Some(0)` / `None`.

All of these are reads of counters the writer side already owns. Nothing new runs on
the device thread or in `write`.

**Publishing it.** The worker samples `queued_ms` after every main-loop `write` and keeps
an exponential moving average (`output::LeadEstimate`, α = 0.1 per write).
- **Sampling per write, not per emit (as built):** the full ring swings between ¾ and
  full (the writer tops it up a quarter at a time). Per-write samples average that
  swing, and engine tests, which write faster than real time, can observe the value.
- **Resets:** after an open, a flush, a fade-to-pause, a resume from pause, and at
  each new session, the next 0.5 s of written audio is not sampled, because the ring
  reads low while it refills. The estimate lives on the engine and survives the
  reset, so a new session starts from the previous value instead of 0.
- **Backends:** WASAPI's device part is `IAudioClient::GetBufferSize` read at setup,
  since wasapi 0.24 has no `GetStreamLatency`. ALSA (Linux) keeps the default `None`
  for now (lead 0, so the cat runs about one ALSA buffer late on Linux). It is a
  follow-up for the deferred Linux batch.

The average rides on the event itself:
`EngineEvent::Position { position_ms, duration_ms, lead_ms }`, mirrored in
`EngineEventDto` and `api.ts`.
- **Why not `PlaybackState`:** state events are only sent on changes. A value captured
  at resume, while the ring is still refilling, would stay stale.
- **Other listeners:** existing listeners (SMTC, history, session, scrobble, remote
  SSE) ignore the new field through `..` patterns.

If no backend reports a value, the field is 0. A test with a backend that reports a
fixed value checks the averaging and the reset on flush.

### 5.3 App wiring (`crates/oxitide-app`)

- `commands::set_beat_tracking(enabled: bool)` goes through
  `AppState::set_beat_tracking`. That stores the wanted state, calls
  `Engine::set_beat_tracking` on the current player, and `set_player` applies it to
  any engine built later (re-login).
  - It is a plain sync command listed in `SYNC_COMMANDS` ("an atomic store plus one
    engine call; ordered with mount/unmount"), registered in `generate_handler!`, and
    listed in remote `NOT_EXPOSED`.
  - Turning tracking on re-sends the clock the engine already knows for the current
    track.
- `player.rs` maps `EngineEvent::Beat` to a desktop `playback_event` of type `beat`. The
  fields are snake_case like the sibling `position` event:
  `{ track_id, bpm, period_ms, anchor_ms, confidence }`, mirrored in `api.ts`. The SSE
  bridge ignores it.
- `config.rs`: two fields on `UiConfig`, plus round-trip tests:
  - `pub catjam: bool` (default false). `set_catjam` follows `set_oneko`.
  - `pub catjam_coat: CatjamCoat`, a lowercase serde enum (default `White`) whose ids
    mirror `COAT_IDS`. `set_catjam_coat(coat: CatjamCoat)` takes the enum, so an
    unknown id fails at the IPC boundary. An unknown value in a hand-edited config is
    handled like any other bad enum value (backed up and reset).
- Both commands are sync config writers listed in `SYNC_COMMANDS`, and both are in
  `NOT_EXPOSED`.

### 5.4 UI beat clock (`ui/src/lib/beatClock.ts`, new and pure)

```ts
export const BEAT_TRIM_MS = 0;          // residual trim for latency the driver doesn't report
export const SNAP_MS = 15;              // drift below this is left alone (CJ-D10)
export const EASE_MS = 200;             // larger drifts are eased out over this long
export function createPositionClock(): {
  sample(positionMs: number, leadMs: number, playing: boolean, nowMs: number): void;
  heardAt(nowMs: number): number;       // extrapolated heard position, ms
};
export function beatPhase(clock: BeatClock, positionMs: number): number;   // 0..1 within the beat
export function beatIndex(clock: BeatClock, positionMs: number): number;   // whole beats from the anchor
```

**Position.** `position_ms` arrives every 500 ms. The heard position is
`position_ms − lead_ms − BEAT_TRIM_MS`, extrapolated with `performance.now()`.

**Corrections.**
- **Small drift** (under `SNAP_MS`): new samples are ignored.
- **Larger drift:** the clock eases toward the new value over `EASE_MS`.
- **Seeks** (`player.pendingSeek`) and track changes snap at once.

**Pause.** The clock freezes while paused.

### 5.5 Sprite (`ui/src/lib/catjamSprite.ts`, new)

The sprite is original pixel art in the oneko style (white cat, black outline), drawn on
a 32×32 grid.
- **Frames:** 8 frames make one two-beat head bob: lean left and land on beat 1, lean
  right and land on beat 2, ears flick on the landings. 2 more frames form the
  low-confidence sway.
- **Source:** the frames are composed from readable string grids in the module: head
  (with ear-flick and closed-eye variants), body, and two tails. They are rendered once
  per coat to an offscreen canvas, then to a `data:` URL sheet (already allowed by
  `img-src`). There is no binary asset and no build step.
- **Coats (CJ-D12):**
  - The grid characters are `k` outline, `w` fur, `s` stripes, `c` chest/muzzle/paws,
    `e` eyes, `l` closed lids, `m` mouth, `p` inner ear/nose, and `g` blush.
  - Each coat is a palette over those characters (`COATS`, `COAT_IDS`, `COAT_LABELS`,
    `isCoat`).
  - The dark coats (Black, Tuxedo) use coloured eyes and light lids so the face stays
    readable.
  - `renderSheet(coat)` takes the coat id; an unknown id renders White.
- **Tests:**
  - grid dimensions;
  - every character is defined in every coat;
  - White's markings are invisible;
  - eyes and lids keep a minimum contrast against the fur in every coat;
  - `isCoat`.
- **Approval checkpoint (CJ-D11):** before any engine work, the frames go into a
  throwaway HTML preview that animates them at 120 BPM, at 1× and 3×, on the dark and
  light surfaces. The owner approves or redirects the look there. Revisions only touch
  this module.

### 5.6 `ui/src/lib/components/CatJam.svelte` (new)

**Mount.** In `PlayerBar.svelte`, beside the `<Cover>`:
`{#if appearance.catjam && !appearance.reducedMotion}<CatJam />{/if}`.

**Element.** One `aria-hidden` element in the left zone's flex row, between the title
and the heart (owner change, 2026-10-21; earlier tries were the cover's top-right and
bottom-left corners). It is sized `32 × scale / 100` px with a matching
`background-size`, so it takes real layout space; Settings offers 100/125/150/200 %
(`set_catjam_scale`, clamped 100–200 in Rust and UI). It has `pointer-events: none`, so it never blocks the cover's Now
Playing click, and `image-rendering: pixelated`.
- **Size (as built):** native sprite size. The bar's cover thumb is 52 px (the preview
  showed an 80 px cover), and 1× keeps the pixels square.
- **Placement (as built):** it sits in a `.thumb-wrap` beside the cover button, which
  clips its children.
- **Props:** `{ player, coat }`.

**Effects.**
- Mount calls `api.setBeatTracking(true)`; unmount calls `false`.
- It listens to the `beat` event and keeps the latest clock whose `trackId` matches the
  current track.
- One RAF loop runs only while the player is playing, the document is visible, and a
  clock exists. It computes the frame from `beatIndex`/`beatPhase` and writes
  `background-position` only when the frame changes. There is no per-frame Svelte state.

**States.**
- **No clock yet** (the first ~8 s of a track): the idle frame.
- **`bpm === null`:** the sway at a fixed 2.4 s cycle.
- **Paused:** the frame is held.

### 5.7 Settings (`ui/src/views/Settings.svelte`, Extras section)

- A **"Jamming cat"** toggle under "Cat", with the description "A cat on the player bar
  that bobs along to the beat." When reduced motion is on, the same muted note as Cat
  is shown.
- When it is on, a **"Coat"** row appears:
  - `settings/CoatPicker.svelte`, a labelled group of six `aria-pressed` swatch
    buttons, each showing that coat's idle frame (`renderIdle`) with its label as the
    accessible name. These are the semantics of Settings' `Segmented`; Settings has no
    radio groups;
  - arrow keys move focus as in `Segmented`, without reaching the transport shortcuts;
  - choosing a coat calls `appearance.setCatjamCoat`, optimistically, through
    `apply()`.
- No credit line (owner decision, 2026-10-21): the built feature shares only the idea
  of a beat-synced cat with the plugin. Its sprite, beat source and placement are all
  OxiTide's own, so a credit would misdescribe it. No opener URL was added.
- Kuroneko gets no row.

### 5.8 Kuroneko & petting (`Neko.svelte`, plus `ui/src/lib/petting.ts`, new and pure)

**`petting.ts`.** `createRubDetector({ reversals: 3, windowMs: 1200 })` exposes
`feed(x, t, insideBox) -> boolean` and reports a pet once per rub.

**Engine change.** `oneko.ts` gains one call, `pet(): void`. It sets `petTicks = 15`;
while `petTicks > 0` the engine:
- returns the `tired` sprite (eyes closed) in place;
- ignores the chase;
- keeps the sleep loop going if the cat is asleep.

A drag cancels the pet.

**Shell.** `Neko.svelte` runs engine A (oneko) and, when summoned, engine B (kuroneko):
- engine B's `setMouse` is fed engine A's last frame position each tick;
- a second div uses the same gif with `filter: invert(1)`;
- `contextmenu` on either cat calls `preventDefault` and toggles B;
- `dblclick` on either cat toggles sleep on both (B gets the perch shifted 28 px left);
- each cat drags independently;
- the window `mousemove` feeds each cat's rub detector; a pet calls `engine.pet()` and
  spawns hearts.

**Hearts.** Each heart is a short-lived 8×8 inline-SVG pixel heart (rects, not a glyph)
with a CSS float-and-fade animation of 900 ms. Hearts are removed on `animationend`, at
most 6 alive.

**Saved state.** B's spawn point is next to A. Her position is persisted with A's in
`oxitide:oneko` as `{x, y, kx?, ky?}`.

## 6. Error handling & degradation

- **Analysis returns `None`** (silence, too short): no event is sent and the cat stays
  idle. The next window retries.
- **Offline or cached playback:** this works the same way, since it only reads decoded
  audio.
- **Tracking disabled mid-window:** the window is discarded.
- **The backend reports no queue depth:** `lead_ms` is 0, so the cat runs about
  200 ms late on that backend. That can only happen on the non-Windows stubs today.
- **ASIO / exclusive `i32` output:** the tap reads the `I32` variant with the same
  scaling as `LevelTap`.

## 7. Performance

- **Off** (the default): one atomic load per decoded buffer.
- **On:** a mono decimating copy (about 1 add per sample) and about 5 ms of analysis
  every 30 s. The UI does at most one style write per frame change and runs no loop
  while paused or hidden.
- **Budget checks:** S11 (idle paused) and S12 (hidden to tray) must stay within budget
  with CatJam on. Rerun them as part of the smoke check.

## 8. Testing

**Rust (`beat.rs`):**
- Using the automix synthetic `drums()`/`band()` generators, the clock BPM is within 1%
  and the `anchor_ms` phase within 30 ms at 90, 128 and 174 BPM.
- `start_frame` mapping holds across a simulated seek, which resets the window.
- A disabled tap ignores pushes, and `push` never waits on a held lock.
- `I32` input is handled.
- A low-confidence input (a pure tone) gives `bpm: None`.
- **Output lead:** a WASAPI ring-occupancy unit test (pure arithmetic, factored out of
  the backend), and an engine test where a fake output reports a fixed `queued_ms`.
  That test checks the average reaches the fixed value and resets on flush.
- An engine test with a synthetic track and an enabled tap emits `EngineEvent::Beat`.
- `catjam` config round-trip.
- The remote allowlist/`NOT_EXPOSED` test and the `SYNC_COMMANDS` test pass with the new
  command classified.

**UI (vitest):**
- `beatClock`:
  - extrapolation, with the lead subtracted;
  - a sub-15 ms drift is ignored and a larger one eases;
  - seeks and track changes snap;
  - the clock freezes while paused;
  - the phase wraps.
- `catjamSprite`: grid sizes, every coat covering every character, White's markings
  invisible, eye/lid contrast, and `isCoat`.
- `CatJam.svelte` re-renders its sheet when the coat changes.
- `petting`: 3 reversals inside 1.2 s pet once; slow or outside-box movement doesn't.
- `oneko`: `pet()` holds position and sprite for 15 ticks and a drag cancels it.
- `CatJam.svelte` (jsdom): it calls `setBeatTracking` on mount and unmount, and it
  ignores a clock for another track.

**Checks:** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
`npm --prefix ui run check`, and `npm --prefix ui test`.

**Smoke** (`docs/superpowers/plans/notes/2026-10-21-catjam-kuroneko-smoke.md`):
- the cat is on the beat across house, rock, hip-hop and drum & bass, within about 10 s
  of the start and after a seek;
- ambient music gets the sway;
- pausing freezes the cat;
- check sync by ear on **WASAPI shared, WASAPI exclusive, and ASIO** (the ASIO part can
  wait for the pending ASIO smoke if no driver is at hand). Record each
  `lead_ms` value, and set `BEAT_TRIM_MS` only if all three are off the same way;
- no visible frame skip every 500 ms during steady playback;
- **no network request other than TIDAL's own while switching tracks** (DevTools
  Network tab, plus a check of the app's logs);
- S11/S12 are rerun;
- kuroneko summon/dismiss, follow, shared sleep on the seek bar, drag, and petting
  hearts on both cats;
- reduced motion hides everything.

## 9. Risks

- **Unreported latency.** `queued_ms` covers our ring and the buffer we negotiated. The
  OS mixer, a Bluetooth link (often 150–250 ms) and the DAC aren't covered. Bluetooth
  output will look late, and a per-device trim is deferred.
- **Tempo octave errors** (half or double time) look fine for a head bob.
- **First-window delay** is about 8 s on an unheard track. A later improvement can
  reuse AutoMix's pre-read head analysis when it already exists for the incoming track.

## 10. Licensing

- **CatJam sprite:** original, authored in this repo.
- **CatJam plugin:** a behavior reference only; nothing is copied and it is not
  credited in the app (see §5.7).
- **Kuroneko:** a CSS recolor of the vendored MIT `oneko.gif`; the existing
  `ui/src/assets/oneko/LICENSE` covers it.

## 11. Deferred

- the oneko cats bobbing on the beat while CatJam is on;
- CatJam in the mini player, Now Playing, or the Rack;
- an opacity setting;
- a custom video or GIF clip (cut at review, CJ-D2);
- a per-device latency trim (Bluetooth);
- reusing AutoMix head grids.
