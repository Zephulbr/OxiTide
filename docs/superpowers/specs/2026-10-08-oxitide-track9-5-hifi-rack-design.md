# OxiTide — Track 9.5 Hi-Fi Rack (third UI mode)

- **Status:** Implemented and closed (2026-10-09) after owner polish rounds; plan
  `docs/superpowers/plans/2026-10-08-track9-5-hifi-rack.md`, completion
  `docs/superpowers/plans/notes/2026-10-09-track9-5-hifi-rack-completion.md`
- **Date:** 2026-10-08
- **Owner:** V
- **Parents:** `2026-09-25-oxitide-2b-visual-convergence.md` §2 (Rack deferred, "not a
  delta"); `2026-10-05-track9-ui-completion-design.md` §2 (Rack excluded from 9.0–9.4,
  `data-mode="rack"` contract); `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md`
  §4 (token layers), §9 (motion), §11 (a11y floors);
  `2026-10-07-oxitide-track7-advanced-audio-design.md` (DSP stages, bit-perfect rules).
- **Design system:** `ui/design/DESIGN.md` (shared foundation) and
  `ui/design/rack/DESIGN.md` (the Rack's visual language, component catalogue and
  measurements). This spec is the decision record; the design file is the build
  reference. The approved interactive mockup is frozen at
  `ui/design/rack/reference-mockup.html`.
- **Process note:** nine mockup rounds on 2026-10-08 (`.superpowers/brainstorm/rack/`,
  gitignored): Era blend → Black Glass (v2) → modern components (v3) → v4 convergence →
  panes → polish → deck faces → pages + Setup → meters + lamp colour. Every decision
  below was confirmed by the owner in that session.

---

## 1. Purpose & outcome

Hi-Fi Rack is the third UI mode: the app as **a piece of real, premium late-80s/90s
hi-fi equipment** — black-glass faceplates with weight and texture, amber-lit VFDs,
light-bar keys, real needle meters and knobs. It is not a token reskin: it has its own
shell (a stacked rack column beside a display) and custom elements.

**Outcome:**

- `Rack` is selectable next to Atmosphere and Instrument (Settings → Appearance), live,
  persisted, and composes with every existing route: all pages render inside the
  rack's display unit as recessed glass **panes**.
- The rack column carries a **deck** (three user-selectable faces; expands into a
  turntable for Now Playing), a **stereo amplifier** (four meter styles, LED-ring volume
  knob, DSP keys, status LEDs) and an optional **graphic EQ** bound to the real EQ.
- Meters are **real**: a read-only level/spectrum tap after DSP in the playback worker
  feeds them; bit-perfect output is untouched.
- **Lamp colour** is user-selectable (five presets + Cover) and always reads as a lit
  screen.

**Success =** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
`npm --prefix ui run check`, `npm --prefix ui test` green; the Rack smoke checklist
passes with a real account; Track 3 scenarios re-pass under unchanged budgets in the
default mode (Rack off costs nothing) and S1/S5 are recorded once in Rack (PM-D1).

## 2. Scope

**In scope (this track, plan 9.5):**

- Mode plumbing: `UiMode::Rack`, `[rack]` config, commands, `appearance` flags.
- `RackShell`: selector unit, rack column, display unit, playlist/queue drawers,
  expanded deck (Now Playing in Rack).
- Deck faces: Transport, CD player, Display; turntable when expanded.
- Amp: meters (Needles, Bars, LED, Spectrum), volume knob with LED ring, DSP keys,
  status LEDs; EQ unit with presets and ten faders.
- Panes: semantic pane/label tokens consumed by the shared section components, so every
  page reads as glass windows in Rack and is unchanged elsewhere.
- Rack-styled Settings controls via tokens (segmented controls read as light-bar key
  trays, sliders as faders) and the Rack section in Settings → Appearance.
- Lamp colour (presets + Cover) and lamp brightness.
- Level tap (Rust) + 30 Hz `level_frame` events while Rack is visible.
- Tests, smoke checklist, perf re-runs, AGENTS.md + design docs.

**Deferred to 9.5c — built 2026-10-10 (`track9-5c-rack`, §9):**

- **Narrow-window fold** into a bottom strip below ~1000px (this track ships a simpler
  narrowing rule, §6.9).
- **Rack mini player** (compact deck + amp); the mini player keeps its current design.
- **Queue changer unit** (the queue is a drawer in this track).
- Segment-display font asset (7-segment digits are drawn in SVG).

**Non-goals:** new remote origins; changing Atmosphere/Instrument visuals (the pane
tokens are neutral there); a meter that is sample-accurate to the DAC (§6.7).

## 3. Decisions (owner-confirmed 2026-10-08)

| # | Decision | Rationale |
|---|---|---|
| RK-D1 | **Rack is a third `UiMode`** (`rack`), selected in Settings → Appearance; `App.svelte` mounts `RackShell` instead of the sidebar/player-bar shell from **one** flag (`appearance.rackShell`). Pages are reused unmodified inside the display. | Convergence §2 accepted that Rack is "not a delta"; one documented branch point keeps "components never branch on mode" true everywhere else. |
| RK-D2 | **Visual direction = Black Glass v4**: near-black faceplates with fine grain and a top sheen, no screws, plain sentence-case unit names, light-bar keys as the single key style, VFDs with dot mesh, lamp-above source keys. | Mockup rounds 2–4. Tray selector, lens buttons and bezel content were rejected. |
| RK-D3 | **Content = panes**: the display is open faceplate; each page section is a recessed glass window; section names are printed on the metal with an engraved rule. Implemented as **semantic pane/label tokens** that are neutral (no box) in the other modes. | Owner idea (round 5); tokens keep the mode contract. |
| RK-D4 | **Layout**: selector unit across the top (sources + history + utility keys), rack column left (deck, amp, EQ), display right. **Now Playing = the deck expands** (turntable + spectrum + lyrics VFD; display hidden). | Owner answers 2026-10-08. |
| RK-D5 | **Deck face is a setting**: Transport (cover + up next), CD player (drawer + music calendar + remaining), Display (tall VFD with format line, bar meter, next track). **Default: Display.** The expanded deck is always the turntable. | Round 7: owner asked to keep all three. Display is the one that never repeats the content area. |
| RK-D6 | **Amp meters are a setting**: Needles (analog VU), Bars (VFD peak + hold), LED (discrete ladder, peak hold), Spectrum (20 bands). **Default: Needles.** | Round 9: owner liked all four. |
| RK-D7 | **Level tap**: read-only, after DSP and crossfade mix, at the output write site; never blocks the audio thread; enabled only while Rack is visible; a 30 Hz app-side emitter publishes peak/RMS per channel + 28 log bands. Identity DSP and bit-perfect stay sample-exact. | Owner: real meters; the tap only reads. |
| RK-D8 | **Lamp colour is a setting**: Amber (default), Blue, Green, Red, White, Cover. Each preset is an OKLCH hue + chroma; **lightness is fixed per role** so every choice reads as phosphor/lamp. **Cover** takes the hue of the most vivid colour of the existing cover palette at chroma 0.15; covers below chroma 0.05 fall back to amber; changes glide 1.2 s. Red lamps turn the meters' over-zone hot white. | Round 9: owner condition — "only if it looks like a screen glow". |
| RK-D9 | **Dynamic colour in Rack** is replaced by Lamp colour: the global Dynamic colour setting is ignored in Rack; the palette is fetched only when Lamp = Cover. `--accent` follows the lamp. | Two competing accent systems would fight; the lamp is Rack's accent. |
| RK-D10 | **Units**: EQ unit optional (setting, default on); a **Playlists unit** (the playlist tree in a glass window) always sits under the EQ and takes the rest of the column (owner, 2026-10-09, replacing the pull-out drawer); the spectrum and a lyrics unit appear in the expanded deck; the queue is a drawer over the display. | Owner answers; changer deferred. |
| RK-D11 | **Setup**: Settings opens from a "Setup" key beside the power lamp. In Rack, Settings controls render as hardware through tokens (key trays, faders, VFD readouts); the Appearance tab gains a Rack group (Deck face, Amp meters, Lamp colour, EQ unit, Lamp brightness). | Round 8: owner approved Setup. |
| RK-D12 | **Rack is always dark**: it ignores `data-theme` and there is no light variant (no silver faceplate is planned). | Owner, 2026-10-08: "we don't need a light mode for the rack UI". |
| RK-D13 | **Phasing inside 9.5**: (a) mode plumbing + shell + units + panes + Setup + lamp colour; (b) level tap + meters + expanded deck spectrum. Both ship in this track; 9.5c is recorded in §2. | Keeps each step testable; meters need the tap. |
| RK-D14 | **PM-D1**: Rack off ⇒ zero cost (tap disabled, no emitter thread, no rack CSS paths active). Rack on: meter drawing runs on `requestAnimationFrame` only while the window is visible; the platter spins only while playing; `prefers-reduced-motion` stops platter, tonearm sweep and peak-cap animation. | Standing perf gate. |

## 4. Current state (recon 2026-10-08, branch `track9-5-hifi-rack` from `main` `67e121c`)

1. `UiMode` is `instrument | atmosphere` in `config.rs` and `appearance.svelte.ts`;
   `apply()` writes `data-mode`, `data-theme`, `data-density`; mode flags are getters.
2. `App.svelte` composes Sidebar + `<main class="main">` + QueueRail + PlayerBar; Now
   Playing replaces the routed page when `nav.nowPlayingOpen`.
3. Sections are drawn by shared components: `DetailHeader`, `TrackTable`,
   `PageSections`/`Carousel`, `MediaCard`, Artist `.shelf`, Settings `<h3>` groups,
   History day groups, collection grids.
4. DSP settings live in `config.dsp`; Settings owns a local copy and debounces
   `set_dsp_settings` (150 ms). No other UI writes DSP.
5. The playback worker writes `PcmBuffer::{F32,I32}` to `AudioOutput::write` after
   `dsp.process` and crossfade `mix_into` (`engine.rs` session loop); tail fades and the
   queue-end tail have separate writes.
6. Cover palettes come from `analyze_cover(uuid, theme)` → `CoverPalette` hex roles.
7. No FFT dependency on `main` (`rustfft` exists only on the unmerged automix branch).

## 5. Architecture

### 5.1 Mode plumbing

- Rust: `UiMode::Rack` (`"rack"`); new `RackConfig` under `[rack]`:
  `deck_face: transport|cd|display` (default `display`), `meters:
  needles|bars|led|spectrum` (default `needles`), `lamp: amber|blue|green|red|white|cover`
  (default `amber`), `eq_unit: bool` (default `true`), `lamp_brightness: u8` 40–100
  (default 100). Command `set_rack_settings(settings)` (whole struct, clamped) and
  `set_level_meter(enabled)`; both registered in `main.rs`; `api.ts` mirrors.
- UI: `UiMode` gains `"rack"`; `appearance.rack` exposes the settings with setters;
  new flag getters `rackShell` (mode === rack); existing flags resolve for Rack as
  Instrument does (no backdrop art, no glass blur, no grain overlay, no wave seek,
  no disc spin, no ambient motion) because Rack draws its own materials.
- `resolveDynamicColor` returns `off` in Rack (RK-D9); `resolveTypeface("auto", rack)`
  → Manrope.
- `data-mode="rack"` block in `modes.css` rewrites semantic tokens (§5.4).

### 5.2 Shell

`ui/src/lib/rack/RackShell.svelte` owns the layout grid (RK-D4):

```
┌ selector: brand · ◂ ▸ · Home Explore Feed History Library ··· Sleep Mini Setup · power ┐
├ column (392px) ───────────┬ display ───────────────────────────────────────────────┤
│ Deck   (face | turntable) │  <main class="main rack-main"> routed page </main>     │
│ Amp    (meters, knob, DSP)│  (the queue drawer slides over the display ⟶)          │
│ EQ     (optional)         │                                                         │
│ Playlists (tree, slack)   │                                                         │
└───────────────────────────┴─────────────────────────────────────────────────────────┘
```

- Source keys mirror the default sidebar (owner, 2026-10-09: no separate Search key —
  search lives on Explore, Ctrl+F still focuses it): Home `home`; Explore `explore |
  section`; Feed `feed`; History `history`; Library `playlists | folder | playlist |
  albums | tracks | profiles | mixes | purchases | album | artist | artist-all | mix`;
  Setup `settings`. Sleep, Mini and Setup are utility keys of the same shape (lamp
  above a 34px key, one baseline); the Sleep lamp lights while a timer is armed.
- In collection routes the display shows a sub-source key tray matching the sidebar's
  "My Collection": Playlists, Albums, Tracks, Mixes & Radio, Profiles, Purchases.
- Expanded deck (`nav.nowPlayingOpen`): the column spans the shell; the deck becomes the
  turntable (platter + tonearm tracking progress) with a centred VFD/key strip; the right
  side stacks the amp (symmetric front, 150px meters), the spectrum (150px), a **Now
  playing** unit that takes the slack — lyrics VFD on the left (nine lines fading with
  distance, current line 23px; title/artist/album when unsynced) and on the right a track
  info window (title, artist, album, track n of N, length, format, ReplayGain) above a
  **signal path** strip (TIDAL → Decoder → DSP → Output → Bit-perfect lamps with values)
  — and an **Up next** changer (next four tracks in a glass window like a track list;
  a slot plays from there). Esc / the deck's collapse key returns.
- Mini player in Rack keeps its layout and gets the Rack skin: faceplate, cover in a
  glass window, title on a VFD, light-bar keys, the queue sheet as a unit.
- All PlayerBar duties are covered: transport, seek (VFD seek bar), shuffle, repeat,
  favourite, queue, volume, sleep timer (existing control), mini player, quality/bit-
  perfect/DSP indicators, buffering and error/notice text (VFD message line).
- Global shortcuts, skip link, `inert` dialog layering and route titles keep working:
  `RackShell` renders the same `<main id="main">` landmark and the same hosts.

### 5.3 Units (components under `ui/src/lib/rack/`)

| Component | Role |
|---|---|
| `Unit.svelte` | Faceplate chassis: material, sheen, grain, unit head (name + silk). |
| `LightKey.svelte` / `KeyTray.svelte` | The one key style: lit bar when pressed/active; tilts while held. `aria-pressed` for toggles, `aria-current` for sources. |
| `SourceKey.svelte` | Selector key with lamp above. |
| `Vfd.svelte`, `SevenSeg.svelte` | VFD panel with dot mesh + glass; SVG 7-segment time. |
| `VfdSeek.svelte` | Segmented seek bar, `role="slider"` (drag, arrows ±5 s, PageUp/Down ±30 s, Home/End). |
| `DeckUnit.svelte` + `faces/{Transport,Cd,DisplayFace}.svelte` + `Turntable.svelte` | Deck. |
| `AmpUnit.svelte` + `meters/{NeedleMeter,BarMeter,LedMeter,SpectrumMeter}.svelte` + `VolumeKnob.svelte` | Amp. |
| `EqUnit.svelte` | Presets (dimple radios) + ten vertical faders bound to `dspStore`. |
| Playlists unit / queue drawer (in `RackShell`) | PlaylistTree in a glass window under the EQ; QueueRail in a drawer over the display. |
| `levels.svelte.ts` | Subscribes to `level_frame`, enables the tap while mounted and visible, runs ballistics (VU ~300 ms; peak instant attack, 1.5 s hold, 24 dB/s fall) in one rAF loop shared by all meters. |
| `lamp.svelte.ts` | Lamp hue/chroma from preset or cover palette; writes `--lamp-h`/`--lamp-c`/`--lamp-dim`. |

### 5.4 Tokens

- Rack-only material tokens (`--rack-*`: room, plate, sheen, grain, key, tray, vfd-bg,
  lamp, lamp-glow, vfd, vfd-ghost, meter-face, meter-ink, meter-red, needle, silk,
  silk-dim, screen-text, screen-muted) are **defined only inside `[data-mode="rack"]`**
  and read only by `ui/src/lib/rack/*` components (they are Rack's private semantic
  layer, like a component library's own tokens).
- Lamp colours derive from three numbers: `--lamp-h`, `--lamp-c`, `--lamp-dim`, with
  lightness fixed per role (values in the design file).
- Shared semantic tokens rewritten for Rack: `--bg`, `--text`, `--text-muted`,
  `--accent` (= screen accent), `--on-accent`, `--surface-1..3`, `--surface-float`,
  `--line`, `--edge-lit` (flat), `--radius-*`, `--player-height: 0px`,
  `--font-ui` default Manrope.
- **Panes and the hardware skin (implementation decision, 2026-10-08):** instead of
  new shared tokens with per-component neutral defaults, shared components carry one
  explicit **hook class**, `pane`, on their section body (`DetailHeader`, `ArtistHero`,
  `TrackTable`'s scroller with `pane-flush`, `Carousel` rows, `PageSections` grids,
  `CollectionAlbums` grid, Home shortcuts), and `ui/src/styles/rack.css` styles those
  hooks plus standard elements (page `h2`, section `h3`, `[role=tablist]`, Settings
  `.segmented`, ranges, text inputs/selects, checkboxes) **only under
  `:root[data-mode="rack"] .rack-main`**. Atmosphere and Instrument have no rules for
  the hooks, so they cannot shift by construction; the `:root` prefix outranks the
  components' scoped styles regardless of stylesheet order. This is the one mode
  stylesheet that targets elements rather than tokens; it is confined to the Rack's
  display unit.
- **Lamp brightness** pulls lit lightness down by at most 0.2 (L 0.80 → 0.60 for VFD
  text at 40%) and scales glow alpha, so dimmed text keeps 4.5:1 (`lamp.ts`
  `vfdLightness`, pinned by tests).

### 5.5 Level tap (Rust)

- `crates/oxitide-audio/src/meter.rs` (pure, tested):
  - `LevelTap { enabled: AtomicBool, state: Mutex<TapState> }`; `push(&PcmBuffer,
    channels)` does nothing unless enabled and uses `try_lock` (never blocks the audio
    thread); it accumulates per-channel peak and sum of squares since the last read and
    keeps the last 2048 mono samples in a ring.
  - `read() -> LevelFrame { peak: [f32; 2], rms: [f32; 2], bands: [f32; 28] }`
    (linear 0..1 peaks/RMS; bands in dBFS) and resets the accumulators. No samples since
    the last read ⇒ silence (zeros, bands at floor).
  - Spectrum: Hann window, in-crate radix-2 FFT (2048), magnitudes summed into 28
    log-spaced bands 40 Hz–16 kHz (edges from the stream rate). No new dependency.
  - `I32` samples scale by 2⁻³¹; mono/multichannel map to L/R (mono duplicates;
    >2 channels use the first two).
- Engine: `Inner.level_tap: Arc<LevelTap>`; every successful `output.write` in the
  session loop (main path, tail fade, queue-end tail) pushes the written buffer;
  `Engine::level_tap()` exposes the `Arc`.
- App: `set_level_meter(enabled)` flips the tap and starts/stops one emitter thread that
  sleeps 33 ms, reads, and emits `level_frame` (`LevelFrameDto`, camelCase-free snake
  case like other DTOs). Starting twice is idempotent; stopping joins.
- Known approximation: buffers are measured when written, so meters lead the speaker by
  the output buffer (WASAPI period). Accepted (RK-D7); VU integration hides it.

## 6. Behaviour details

### 6.1 Deck faces (compact)

- **Transport:** cover window + "Up next" window (next three queue items, from
  `player.state.queue` / `current_index`).
- **CD player:** disc drawer + eject key (decorative; eject = stop is *not* wired —
  the key opens the queue drawer), music-calendar VFD (track numbers of the current
  play source up to 20: present tracks lit, current boxed, beyond ghosted; for
  queues > 20 shows the window of 20 around the current), "REMAIN" time of the queue
  from the current position.
- **Display:** tall VFD — format line (codec, bit depth, rate, EXCL/SHRD, TIDAL MAX/
  LOSSLESS), L/R bar meter (same level feed), "NEXT" line.
- All faces share the VFD (title, artist · album, 7-seg elapsed, annunciators RPT/SHUF/
  GAPLESS/BUF, TRK nn/nn), the VFD seek bar, transport keys, and the Shuffle / Repeat /
  ♥ / Queue light keys. Repeat cycles off → all → one (annunciator RPT / RPT1).

### 6.2 Amp

- Meters per RK-D6; the Bars face also shows "PROGRAM HOLD ±x.x dB".
- In the expanded deck the amp is a symmetric front: DSP keys stacked vertically on the
  left, meters (150px) centred, a 1.35× knob on the right.
- Status LEDs: quality (MAX/LOSSLESS/AAC), Bit-perfect (green) or DSP (amber, with the
  stage list as accessible text), format (`24/96`) from `stream`.
- DSP keys: EQ (toggles `eq.enabled`), ReplayGain (off ↔ last non-off mode, default
  album), Crossfade (0 ↔ last non-zero, default 6 s). Long settings stay in Setup.
- Volume knob: `role="slider"`, 0–100, drag vertical, wheel, arrows ±2, PageUp/Down ±10,
  Home/End; LED ring of 23 dots; persists on release like the VolumeSlider.

### 6.3 EQ unit

Ten faders ±12 dB in 0.5 dB steps (drag, wheel, keys), presets as dimple radios (the
existing `EQ_PRESETS`; shows "Custom" when no preset matches). Moving a fader with EQ
off turns EQ on. Writes go through the shared `dspStore` (debounced 150 ms) which
Settings also uses, so both stay in sync.

### 6.4 Panes and labels

Shared components wrap their section body in the pane tokens; section headings use the
label tokens. Track tables run edge to edge inside their pane; playing row gets the
accent glow. Cards/tiles stay as they are inside panes.

### 6.5 Settings in Rack

Settings is unchanged structurally; tokens restyle segmented groups (key trays), range
inputs (horizontal faders with an amber-centre cap) and text inputs/selects (VFD
readouts). The Appearance tab adds the Rack group (shown only when Mode = Rack) and the
Mode control gains "Hi-Fi Rack". While in Rack, Settings notes that Rack is always dark.

### 6.6 Lamp colour

Presets `amber 64/.16, blue 235/.14, green 165/.15, red 27/.19, white 80/.025`
(hue °/OKLCH chroma). Cover: `analyze_cover` palette (`primary, secondary, tertiary,
backdrop`), choose the highest-chroma colour; chroma < 0.05 ⇒ amber; otherwise hue at
chroma 0.15. Glide via `@property` registration of `--lamp-h`/`--lamp-c` (1.2 s,
instant under reduced motion). Lamp brightness scales the lit lightness and glow alpha
(40–100%).

### 6.7 Meters

**Calibration (revised 2026-10-09 after owner smoke: meters "move way too fast and
always hover near the top").** Real hi-fi meters read programme level against a
reference, not digital full scale; with 0 VU at −18 dBFS RMS and bars on sample peaks,
modern masters pinned every meter. All meters now read **programme level relative to
−10 dBFS RMS** (0 VU = 0 dB = −10 dBFS RMS; a typical −14 dBFS master sits around −4,
loud passages reach the red).

- Needles: a damped mass-spring movement (2.0 Hz, ζ 0.8: settles in ~300 ms with ~1.5 %
  overshoot, stops at both pins), driven by RMS averaged over ~100 ms. Scale −20…+3 VU.
- Bars/LED: PPM-like programme meters on the same reference — 10 ms rise, 13 dB/s fall
  (20 dB in ~1.5 s), 1.5 s hold then 10 dB/s; scale −40…+3 dB, red above 0. Header
  "PROGRAM HOLD ±x.x dB".
- Spectrum: 20 bands (amp) / 28 (expanded) from the tap's 28, −60…0 dB, 50 ms smoothed
  rise, 0.9/s fall, caps 0.25/s.
- Paused/stopped ⇒ needles return to rest, bars fall.
- **Analyzer unit (Now Playing)**: `[rack] analyzer = auto | spectrum | recorder`
  (default auto). The **level recorder** is a calm chart-recorder strip of L/R programme
  level over the last 60 s (one point per 250 ms, smooth glide, still while paused),
  built UI-side from the existing RMS readings. Auto picks it whenever the amp's meters
  are a spectrum so the two units never show the same thing; two keys in the unit's head
  switch it directly. History (owner, 2026-10-09): a waterfall read as "too full" and a
  stereo scope as "too distracting"; both briefly shipped values (`waterfall`, `scope`)
  load as `recorder`, and the scope's extra tap data was removed again.
- **Slot comparison (owner, 2026-10-09: "add all options as buttons and we will decide
  what to keep")**: the unit is Now Playing's *extra unit*. **Decision (owner,
  2026-10-09): keep Spectrum, Recorder, EQ (the graphic EQ embedded) and Tuner (FM dial
  whose pointer travels the timeline).** Off was dropped, and Tape after three
  redraws ("the cassette does not look convincing"). `[rack] analyzer = auto | spectrum
  | recorder | tuner | eq`; saved `none` / `cassette` load as `auto`.

### 6.8 Accessibility

Every control is a native button/slider with name and state; meters, platter, tonearm
and decorative VFD glyphs are `aria-hidden`; the VFD exposes its text (title, artist,
elapsed via the seek slider's value text). Light keys keep the global focus ring.
Contrast: VFD text vs `--rack-vfd-bg` and screen text vs pane ≥ 4.5:1 for every lamp
preset (tests compute OKLCH → sRGB contrast). Hit targets ≥ 24px.

### 6.9 Window fit

Column width `clamp(300px, 30vw, 392px)`. Below 1000px wide the EQ unit hides; below
680px tall the deck face windows hide (VFD and transport stay). The min window
(720×520) remains usable.

## 7. Testing

- Rust: `meter.rs` (peak/RMS math, I32 scaling, mono/multi mapping, band edges, FFT on
  a sine lands in the right band, silence when nothing pushed, `try_lock` contention
  never blocks), engine test that writes push into the tap only when enabled, config
  round-trip/defaults/clamps for `[rack]` and `ui.mode = "rack"`.
- UI (vitest): appearance (rack flags, dynamic-colour off, persistence), lamp maths
  (presets, cover fallback, contrast floors), ballistics (VU, peak hold/fall), deck
  helpers (up-next, calendar window, remaining), source-key mapping, dspStore sync,
  component tests for LightKey/VolumeKnob/VfdSeek/EqUnit keyboard contracts.
- Smoke: `docs/superpowers/plans/notes/DONE-2026-10-08-track9-5-rack-smoke.md` (human).
- Perf (PM-D1): default-mode S1/S2/S5/S6 unchanged; record S1 + S5 with Rack on.

## 8. Risks

- **Pane tokens touching shared components** could shift Atmosphere/Instrument by a
  pixel — mitigated by neutral defaults and a token test that asserts the defaults.
- **Settings coupling to `dspStore`** — the store replaces Settings' local debounce; its
  tests pin flush-on-unmount and external updates.
- **Level emitter while hidden** — the UI disables the tap on `visibilitychange` and
  on unmount; the Rust side also stops when no listener toggles it on.
- **Meter lead vs audio** — accepted, documented (§5.5).

## 9. 9.5c (2026-10-10)

Built on `track9-5c-rack` from `main`; visual choices below are implementation
decisions awaiting the owner's look (same screenshot-driven loop as 9.5).

| # | Decision |
|---|---|
| RK-D15 | **Narrow fold**: below 1000px wide (`matchMedia`, `RackShell.fold`) the rack column is not rendered; the display spans the width and `RackStrip.svelte` sits under it — cover window, deck VFD, transport + play-mode keys (+ a Now Playing key), volume knob at 0.8×. Under 860px the keys drop under the VFD; under 760px the cover goes. DSP keys, meters and EQ live in Setup while folded. Now Playing while folded stacks its units in one scrolling column. Replaces the §6.9 "EQ hides below 1000px" rule. |
| RK-D16 | **Rack mini player** (`RackMini.svelte`): a compact deck (head with quality + Restore key, cover window taking the slack, the deck VFD, transport and play-mode trays) over a compact amp (Bars meter on a VFD + knob at 0.7×; Bars regardless of the amp's meter setting, since needles need 96px). The queue sheet is a unit with a focus trap. Mounted from `App.svelte`'s existing Rack branch (`appearance.rackShell`); the old Rack skin over `Mini.svelte` in `rack.css` is removed. |
| RK-D17 | **Queue changer** (`ChangerUnit.svelte`): with the column visible, Queue opens a column unit in the Playlists unit's place (it takes the slack, min 300px) — a VFD (DISC n · OF N · REMAIN), five disc keys with lamps (one page of the queue; arrow keys page; the page follows the current track until paged), and the full `QueueRail` in a glass window. Folded or in Now Playing, the drawer remains. |
| RK-D18 | **Segment font**: DSEG7 Classic Bold Italic (OFL, unmodified, 5KB) replaces the SVG digits. `SevenSeg` draws two layers — a ghost of "8"s and the lit text (blanks as DSEG's `!`) — via `segment.ts`. The CD face's REMAIN time and the changer readout use it too. |

The deck's VFD and key trays were extracted (`DeckVfd.svelte`, `DeckKeys.svelte`) so the
deck, strip and mini player share one implementation.
