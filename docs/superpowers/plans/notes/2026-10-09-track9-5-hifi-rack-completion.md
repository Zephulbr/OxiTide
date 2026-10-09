# Track 9.5 — Hi-Fi Rack completion note

- **Date:** 2026-10-09
- **Branch:** `track9-5-hifi-rack` (from `main` `67e121c`), 18 commits
- **Spec:** `docs/superpowers/specs/2026-10-08-oxitide-track9-5-hifi-rack-design.md`
- **Plan:** `docs/superpowers/plans/2026-10-08-track9-5-hifi-rack.md`
- **Design:** `ui/design/DESIGN.md`, `ui/design/rack/DESIGN.md` (+ frozen reference mockup)
- **Smoke checklist:** `docs/superpowers/plans/notes/2026-10-08-track9-5-rack-smoke.md`

## What shipped

Hi-Fi Rack as the third UI mode (Settings → Appearance → Mode → Hi-Fi Rack):

- **Shell:** selector unit (Home, Explore, Feed, History, Library + Sleep/Mini/Setup on
  one baseline), rack column (deck, amp, optional EQ, Playlists unit), display unit with
  every page rendered as glass panes under printed labels; queue drawer; library
  sub-keys matching the sidebar's "My Collection". One sanctioned shell branch in
  `App.svelte`; pages shared through `RoutedPage.svelte`.
- **Deck:** Transport / CD player / Display faces (setting), VFD with 7-segment time,
  annunciators and a segmented seek slider; fixed-size transport and play-mode keys.
- **Now Playing:** expanded deck with the turntable (headroom for the tonearm), a
  centred VFD/key strip; right column with a symmetric amp front, an extra unit
  (Spectrum / Recorder / Tuner / EQ, setting + head keys), a Now playing unit
  (nine-line lyrics + track info in the accent + signal path)
  and Up next.
- **Amp:** Needles / Bars / LED / Spectrum meters (setting) calibrated to programme
  level (−10 dBFS RMS = 0), spring-needle VU ballistics, PPM-like bars/LEDs; LED-ring
  volume knob; status LEDs; EQ / ReplayGain / Crossfade keys.
- **EQ:** presets + ten faders bound to the shared `dspStore` (also used by Settings).
- **Lamp colour:** Amber / Blue / Green / Red / White / Cover with fixed per-role
  lightness and a brightness setting that keeps VFD text ≥ 4.5:1.
- **Setup:** Settings restyled as hardware inside the display; Rack group in Appearance.
- **Mini player:** Rack skin.
- **Audio:** read-only `LevelTap` after DSP at the output write (try_lock, disabled by
  default); `set_level_meter` 30 Hz `level_frame` emitter, on only while a meter is
  mounted and the window is visible. Bit-perfect unaffected.
- **Config:** `ui.mode = "rack"`, `[rack]` (deck_face, meters, analyzer, lamp, eq_unit,
  lamp_brightness) via `set_rack_settings`.

## Owner decisions during polish (2026-10-09)

Recorded in the spec: no light/silver variant (Rack is always dark); playlist tree as a
column unit instead of a drawer; no Search key (search lives on Explore); meters
recalibrated after they "hovered near the top"; the Now Playing extra unit went through
waterfall ("too full") and stereo scope ("too distracting") before the owner kept
Spectrum, Recorder, Tuner and EQ; a tape/cassette view was redrawn three times and then
dropped as not convincing.

## Verification

| Gate | Result |
|---|---|
| `cargo test --workspace` | green (audio tap, emitter, config incl. legacy analyzer values) |
| `cargo clippy --workspace -- -D warnings` | clean |
| `npm --prefix ui run check` | 0 errors, 0 warnings |
| `npm --prefix ui test` | 717 passed (Rack: lamp maths + contrast floors, ballistics, levels store, deck/slot helpers, dspStore, appearance, keyboard contracts) |
| Owner review | Iterative, screenshot-driven through nine design rounds and seven polish rounds; the owner asked to close the track on 2026-10-09. |
| Smoke checklist (formal pass) | **Pending** — owner to tick `2026-10-08-track9-5-rack-smoke.md`. |
| PM-D1 perf re-runs (S1/S5 default mode; S1/S5 in Rack) | **Pending** — owner run. Rack off costs nothing by construction (tap disabled, no emitter thread, no Rack CSS paths active). |

## Deferred (9.5c, recorded in the spec)

Narrow-window fold into a bottom strip below ~1000px, a Rack-specific mini layout, and
a queue changer unit.

## Hand-off

Merged `track9-5-hifi-rack` to `main` on 2026-10-09 at the owner's request; the formal
smoke tick and PM-D1 perf rows follow on `main`, as with earlier tracks.
