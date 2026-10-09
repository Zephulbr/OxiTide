# Track 9.5 — Hi-Fi Rack Implementation Plan

> **For agentic workers:** execute task by task; steps use checkbox (`- [ ]`) syntax.

**Status (2026-10-09): closed** — Tasks 1–16 done on `track9-5-hifi-rack` plus seven
owner polish rounds; see `notes/2026-10-09-track9-5-hifi-rack-completion.md`. All four
automated gates green; the formal smoke tick and PM-D1 perf re-runs are owner items.
Deviation recorded in the spec §5.4: panes use a `pane` hook class styled only by
`rack.css` instead of new shared tokens.

**Goal:** Ship Hi-Fi Rack as the third UI mode per
`docs/superpowers/specs/2026-10-08-oxitide-track9-5-hifi-rack-design.md` (RK-D1…D14):
mode plumbing, rack shell and units, panes, Setup restyle, lamp colour, level tap and
real meters.

**Architecture:** Rust gains `UiMode::Rack`, a `[rack]` config section and a read-only
`LevelTap` in the playback worker with an app-side 30 Hz emitter. The UI gains
`ui/src/lib/rack/` (shell + units + stores), new shared semantic tokens (pane, label,
control, readout) that are neutral outside Rack, and a single `appearance.rackShell`
branch in `App.svelte`. Visual values come from `ui/design/rack/DESIGN.md`.

**Tech stack:** Rust (std atomics/mutex, no new crates), Tauri 2 events, Svelte 5 runes,
TypeScript, Vitest (node; jsdom per file for component tests).

## Global constraints

- Svelte 5 runes only; no stores/`export let`.
- Components outside `ui/src/lib/rack/` read semantic tokens only and never branch on
  mode; `--rack-*` tokens are read only inside `ui/src/lib/rack/`.
- New commands are registered in `main.rs` `generate_handler!`; `api.ts` mirrors DTOs.
- The audio thread never blocks on the tap (`try_lock`); identity DSP stays a no-op and
  `bit_perfect` semantics are unchanged.
- No new remote origins, no new crates; FFT is in-crate.
- Every task keeps `npm --prefix ui run check`, `npm --prefix ui test`,
  `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` green before it
  is committed.
- PM-D1: Rack off costs nothing (tap disabled, no emitter thread).

## Review focus

1. **Neutral pane tokens** must not move Atmosphere/Instrument pixels — pinned by a
   token-default test and the smoke's A/B row.
2. **Tap never blocks audio** — `try_lock` + enabled flag; a contention test.
3. **Emitter lifecycle** — enable on Rack mount + visible, disable on hide/unmount/mode
   switch; idempotent start/stop in Rust.
4. **DSP single source of truth** — Settings and the EQ unit share `dspStore`.
5. **Keyboard parity** — every PlayerBar duty reachable in Rack; global shortcuts keep
   working (`defaultPrevented` guard respected by the rack sliders).

---

### Task 1: Rust config + commands for Rack

**Files:** `crates/oxitide-app/src/config.rs`, `commands.rs`, `main.rs`.

- [ ] `UiMode::Rack` (serde `"rack"`).
- [ ] `RackConfig { deck_face: DeckFace, meters: MeterStyle, lamp: LampColor, eq_unit: bool, lamp_brightness: u8 }` with serde lowercase enums, `#[serde(default)]`, defaults display / needles / amber / true / 100; `Config.rack`.
- [ ] `clamp_lamp_brightness` (40–100); `set_rack_settings(settings: RackConfig) -> RackConfig` saves the clamped struct and returns it.
- [ ] Tests: defaults, round trip, `ui.mode = "rack"` parses, unknown enum values fall back via default on missing section, brightness clamp.

### Task 2: Level tap (pure) — `crates/oxitide-audio/src/meter.rs`

- [ ] `fft_in_place(re, im)` radix-2 (n power of two), `hann(n)`, `band_edges(rate, bands, lo, hi)` log-spaced, `bands_db(mags, edges, rate, n)`.
- [ ] `LevelTap::new()`, `set_enabled`, `is_enabled`, `push(&PcmBuffer, channels)` (no-op when disabled; `try_lock`), `read() -> LevelFrame`.
- [ ] Tests: sine 1 kHz peaks in the band containing 1 kHz; full-scale sine peak ≈1, RMS ≈ .707; I32 full scale ≈1; mono duplicates to L/R; read without pushes → zeros; disabled push ignored; a held lock makes `push` return immediately.

### Task 3: Engine wiring

**Files:** `engine.rs`, `lib.rs`.

- [ ] `Inner.level_tap: Arc<LevelTap>`; push after every successful `output.write` in the session loop (main, tail fade, queue-end tail).
- [ ] `Engine::level_tap() -> Arc<LevelTap>`.
- [ ] Test: with the tap enabled, a played track yields a non-zero peak on `read()`; disabled yields zeros.

### Task 4: App emitter + DTO

**Files:** `crates/oxitide-app/src/player.rs` (or new `meter.rs`), `commands.rs`, `main.rs`, `state.rs`.

- [ ] `LevelFrameDto { peak: [f32;2], rms: [f32;2], bands: Vec<f32> }`.
- [ ] `set_level_meter(enabled)`: toggles the tap; starts one emitter thread (33 ms tick, emits `level_frame`) if not running; stopping signals and joins. Idempotent.
- [ ] Test the start/stop state machine without Tauri (closure emitter).

### Task 5: UI API + appearance

**Files:** `ui/src/lib/api.ts`, `appearance.svelte.ts`, `appearance.test.ts`.

- [ ] Types: `UiMode` + `"rack"`, `RackSettings`, `LevelFrame`; `api.setRackSettings`, `api.setLevelMeter`, `LEVEL_FRAME_EVENT`.
- [ ] `appearance.rack` (settings), `setRack(patch)` (local-first, revert on failure), `rackShell` flag; Rack resolves flags like Instrument; `resolveDynamicColor(*, "rack") = "off"`; typeface auto → Manrope.
- [ ] Tests for the above.

### Task 6: Shared semantic tokens

**Files:** `ui/src/styles/tokens.css`, `app.css`, `DetailHeader.svelte`, `TrackTable.svelte`, `PageSections.svelte`, `views/Artist.svelte`, `views/Settings.svelte`, `views/History.svelte`, `views/Home.svelte`, `views/Explore.svelte`, `themeTokens.test.ts`.

- [ ] Add `--pane-*`, `--label-*`, `--control-*`, `--readout-*` with neutral defaults reproducing today.
- [ ] Section bodies consume pane tokens; section headings consume label tokens; Settings segmented groups/ranges/inputs consume control/readout tokens.
- [ ] Test: neutral defaults asserted in `tokens.css`.

### Task 7: Rack tokens + lamp colour

**Files:** `ui/src/styles/modes.css` (or new `rack.css` imported by `main.ts`), `ui/src/lib/rack/lamp.ts`, `lamp.svelte.ts`, tests.

- [ ] `[data-mode="rack"]` block: shared semantic rewrites + private `--rack-*` tokens + `@property --lamp-h/--lamp-c`.
- [ ] `lamp.ts` (pure): `LAMP_PRESETS`, `hexToOklch`, `coverLamp(palette)`, `oklchToSrgb`, `contrast`.
- [ ] `lamp.svelte.ts`: writes `--lamp-h/--lamp-c/--lamp-dim` from settings; fetches the palette only when lamp = cover (latest-wins).
- [ ] Tests: presets, cover choice/fallback, VFD text contrast ≥ 4.5:1 on `--rack-vfd-bg` for each preset at dim .4 and 1.

### Task 8: Shared DSP store

**Files:** `ui/src/lib/dspStore.svelte.ts`, tests, `views/Settings.svelte`.

- [ ] `dspStore.settings`, `load(from config)`, `set(next)` (local-first, 150 ms debounce, flush on demand), `flush()`.
- [ ] Settings uses it (replaces its pending/timer code) and mirrors external changes.
- [ ] Tests: debounce, flush, failure re-sync.

### Task 9: Rack primitives

**Files:** `ui/src/lib/rack/{Unit,LightKey,KeyTray,SourceKey,Vfd,SevenSeg,VfdSeek,Dimple}.svelte`, tests.

- [ ] Build per design file §5.1–5.4; VfdSeek keyboard map; component tests for LightKey `aria-pressed`, VfdSeek keys.

### Task 10: Levels store

**Files:** `ui/src/lib/rack/levels.svelte.ts`, `ballistics.ts`, tests.

- [ ] Pure ballistics (VU, peak hold/fall, spectrum fall) + `dbToVu`, `vuPosition`.
- [ ] Store: listens to `level_frame`, toggles `setLevelMeter` with mount/visibility, exposes a frame-synced snapshot to meters.

### Task 11: Amp unit

**Files:** `ui/src/lib/rack/{AmpUnit,VolumeKnob}.svelte`, `meters/*.svelte`.

- [ ] Four meter styles; status LEDs; DSP keys via `dspStore`; knob slider.
- [ ] Component test: knob keyboard map + `aria-valuetext`.

### Task 12: Deck unit

**Files:** `ui/src/lib/rack/{DeckUnit,Turntable}.svelte`, `faces/*.svelte`, `deck.ts` + tests.

- [ ] `deck.ts`: `upNext(state, n)`, `calendar(state)`, `remainingMs(state)`, `annunciators(state)`, `formatLine(stream, state)`.
- [ ] Three faces + expanded turntable; transport + light keys; VFD message line for errors/notices.

### Task 13: EQ unit

**Files:** `ui/src/lib/rack/EqUnit.svelte`.

- [ ] Ten faders + presets + Custom state bound to `dspStore`; fader moves enable EQ.

### Task 14: Rack shell + App branch

**Files:** `ui/src/lib/rack/{RackShell,Selector,Drawer,LibraryKeys}.svelte`, `sources.ts` + tests, `App.svelte`.

- [ ] Source mapping (`sourceOf(route)`), selector, library key tray, drawers (PlaylistTree, QueueRail), expanded deck layout, window-fit rules.
- [ ] `App.svelte`: `{#if appearance.rackShell}<RackShell …/>{:else}…{/if}` reusing the same page switch (extract the routed page block into `RoutedPage.svelte` so both shells share it).

### Task 15: Settings → Appearance

**Files:** `views/Settings.svelte`.

- [ ] Mode gains "Hi-Fi Rack"; Rack group (Deck face, Amp meters, Lamp colour, EQ unit, Lamp brightness) shown when mode = rack; Theme note.

### Task 16: Verification + docs

- [ ] All four gates green.
- [ ] Smoke checklist `docs/superpowers/plans/notes/2026-10-08-track9-5-rack-smoke.md`.
- [ ] Perf: default-mode S1/S5 re-run; Rack S1/S5 recorded.
- [ ] AGENTS.md (mode list, rack folder, level tap, design docs), roadmap status.
