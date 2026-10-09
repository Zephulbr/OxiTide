# Hi-Fi Rack — design file

Shared foundation: [../DESIGN.md](../DESIGN.md). Decision record:
`docs/superpowers/specs/2026-10-08-oxitide-track9-5-hifi-rack-design.md` (RK-D1…D14).
Approved interactive reference: [reference-mockup.html](reference-mockup.html) — serve
the repo root statically (it loads `/ui/src/assets/fonts/*`) and open it; its toolbar
switches deck faces, meters and lamp colours, and its source keys switch pages.

## 1. Personality

You are looking at **a real piece of premium hi-fi, late 80s / early 90s** — not a
vintage pastiche and not an app wearing a costume.

- **Weight and texture.** Black-glass faceplates with fine grain and a single top-left
  sheen. Units sit on a shelf: shadows fall down, lips catch light at the bottom edge.
- **Light means something.** Lamps, light bars and VFD segments glow only to say
  "on", "here" or "level". Nothing glows for decoration.
- **Real instruments.** Needles have ballistics, faders have caps, the knob has a skirt
  and an LED ring, displays have dot mesh and glass.
- **Plain printing.** Unit names in sentence case, small silk labels; no screws, no fake
  model numbers, no all-caps shouting outside the VFDs.

Rejected (keep rejected): screws; the segmented "tray" source selector (unlit labels
unreadable); lit dotted-lens round buttons (read as speaker grilles); screen-in-a-bezel
content; widely spaced lowercase labels; app-like flat buttons inside the rack.

## 2. Layout

```
┌ Selector unit ─ brand · ◂ ▸ · [Home][Explore][Feed][History][Library] ··· [Sleep][Mini][Setup] ● Power ┐
├ Rack column ─────────────┬ Display unit (open faceplate) ──────────────────────────────┤
│ Deck (face / turntable)  │  [sub-source keys when in Library]                           │
│ Amplifier                │  Label ───────────────────                                    │
│ Graphic EQ (optional)    │  ┌ glass pane ┐  Label ─────  ┌ glass pane ┐                │
│ Playlists (tree, slack)  │  …                          (queue drawer slides in ⟶)        │
└──────────────────────────┴───────────────────────────────────────────────────────────────┘
```

| Measure | Value |
|---|---|
| Rack padding / unit gap | 10px / 4px (column), 6px (grid) |
| Column width | `clamp(300px, 30vw, 392px)` |
| Unit padding | 14px 18px; unit radius 5px |
| Display inner padding | 16px; page padding 22px 26px 34px |
| Deck face height `--pl` | `clamp(104px, 100vh − 600px, 196px)` |
| Column and display | share one bottom edge: units keep their natural height and the Playlists unit takes the slack (never stretch a unit with fixed parts) |
| Keys | fixed sizes — transport 46×34, play-mode icon keys 38×34; keys never stretch to fill a row |
| Needle meters | sized from the 96px slot height (1.55:1), never from the unit width |
| Expanded deck | column spans both tracks; deck left (1.4fr), amp / spectrum (150px VFD) / lyrics (slack, five lines) right (≥ 380px); platter `max(260px, min(100vh − 340px, 56vw − 160px, 860px))`; VFD + keys a centred strip ≤ 680px |
| Below 1000px wide | EQ unit hidden |
| Below 760px tall | Playlists unit hidden |
| Below 680px tall | deck face windows hidden (VFD + keys stay) |

Selector: sources mirror the default sidebar — Home, Explore (search lives there),
Feed, History, Library (every collection and detail route). Every selector key, sources
and utility keys alike (Sleep, Mini, Setup), is a lamp above a 34px key on one baseline;
Power uses the same two rows. The Sleep lamp lights while a timer is armed.

Expanded deck right column: amp (wide: DSP keys stacked left, 150px meters centred,
1.35× knob right) · spectrum (150px) · Now playing (lyrics VFD 1.25fr — nine lines
fading 17% per line, 15px, current 23px with glow — beside a 1fr column: track-info
glass window with printed labels, then a "Signal path" tray of five lamps TIDAL ·
Decoder · DSP · Output · Bit-perfect with values printed under them) · Up next (glass
window, rows like a track list: VFD-amber number, 30px cover, title/artist, time).

Mini player: unchanged layout with the Rack skin — faceplate, cover in a glass window,
title/artist on a VFD, transport and secondary keys as light-bar keys in trays.

## 3. Materials and colour

### 3.1 Faceplate (private `--rack-*` tokens, `[data-mode="rack"]` only)

| Token | Value |
|---|---|
| `--rack-room` | `#050505` |
| `--rack-plate-base` / `--rack-plate-hi` | `#0d0c0b` / `#1a1816` (180° gradient hi→base) |
| `--rack-sheen` | `linear-gradient(160deg, rgba(255,255,255,.09) 0, rgba(255,255,255,.02) 30%, transparent 31%)` |
| Grain | fractal-noise tile, soft-light, alpha .05 |
| Unit shadow | `inset 0 1px 0 rgba(255,255,255,.10), inset 0 −2px 0 rgba(0,0,0,.45), inset 1px 0 0 rgba(255,255,255,.04), 0 2px 0 #000, 0 10px 24px rgba(0,0,0,.55)` |
| `--rack-silk` / `--rack-silk-dim` | `#b8ab98` / `#6f665a` (printed labels) |
| `--rack-key` / tray | key `#222120` with `border-top: 1px solid #353331`; tray `#000` with `inset 0 1px 3px rgba(0,0,0,.9), 0 1px 0 rgba(255,255,255,.05)` |
| `--rack-screen-text` / `--rack-screen-muted` | `#e8ddcb` / `#8a7f70` (content inside panes) |
| `--rack-meter-red` | `#ff5a3a` (over zone; hot white `oklch(.9 .06 60)` when the lamp is red) |

### 3.2 Lamp colour (one hue drives every lit thing)

Inputs: `--lamp-h` (hue°), `--lamp-c` (OKLCH chroma), `--lamp-dim` (0.4–1, Lamp
brightness). Both `--lamp-h` and `--lamp-c` are `@property`-registered numbers so changes
glide (1.2 s; instant under reduced motion). Lightness is **fixed per role**:

| Role | Formula |
|---|---|
| `--rack-lamp` (lamps, light bars, LEDs) | `oklch(.58 + (dim − .4)/3, c, h)` (L .78 at full) |
| `--rack-lamp-glow` | `oklch(.72 c×1.05 h / calc(.6 × dim))` |
| `--rack-vfd` (VFD text/segments) | `oklch(.60 + (dim − .4)/3, c, h)` (L .80 at full; ≥ 4.5:1 at the 40% floor) |
| `--rack-vfd-ghost` (unlit segments) | `oklch(.8 c h / .06)` |
| `--rack-vfd-bg` | `oklch(.115 c×.1 h)` |
| `--rack-meter-face` | radial `oklch(.31 c×.55 h)` → `oklch(.18 c×.32 h)` → `oklch(.10 c×.12 h)` from the bottom |
| `--rack-meter-ink` | `oklch(.83 c×.85 h)` |
| `--rack-needle` | `oklch(.90 c×.5 h)` |
| `--accent` (screen accent) | `oklch(.79 c h)`; `--on-accent` `#140c04` |

| Preset | h | c |
|---|---|---|
| Amber (default) | 64 | .16 |
| Blue | 235 | .14 |
| Green | 165 | .15 |
| Red | 27 | .19 |
| White | 80 | .025 |
| Cover | hue of the most chromatic palette role | .15 (amber if chroma < .05) |

Glows use `color-mix(in srgb, var(--rack-lamp) N%, transparent)` — never a hard-coded
amber.

## 4. Typography

- Unit names: Manrope 500, 14px, `#d6cbbb`, sentence case. Brand: Manrope 400 20px +.03em.
- Silk labels: Manrope 600–700, 9.5–11px, +.05em, `--rack-silk-dim`, `text-shadow 0 1px 0 rgba(255,255,255,.05)`.
- VFD: Manrope 700 13px +.08em uppercase with `text-shadow 0 0 6px vfd, 0 0 1px vfd`;
  sub line 10.5px; annunciators 8.5px 800 +.12em (unlit at .12 opacity).
- 7-segment time: SVG digits, 22px tall, skewX(−6°), ghost segments in `--rack-vfd-ghost`.
- Content titles: Manrope 700 32px −.015em (no serif in Rack); bylines Manrope 500 16px
  `#cfc3b1`.

## 5. Components

### 5.1 Light-bar key (the only key style)

46×34 transport / 58–92 × 30–36 text keys; radius 3px; `#222120` with a 1px `#353331`
top edge; label 9.5–11px 700 `#8d8478` (lit: `#f0e2cb`). Under the label a 70% wide,
3px rounded bar: unlit `#1a1918` with `border-top 1px #2f2d2b`; lit `--rack-lamp` +
`0 0 8px --rack-lamp-glow`. Lit when `aria-pressed="true"`, `aria-current`, or while
held. Held: `perspective(200px) rotateX(−16deg)` from 50% 40%, inner shadow
`inset 0 −14px 10px rgba(0,0,0,.5)`, top edge warms. Bar turns on in 80 ms and fades off
in 350 ms (incandescent). Keys sit in a black tray (4px padding, 5–6px gap, radius 6).

### 5.2 Source key

A key with a 18×4 lamp above it; the lamp lights (with glow) for the current source.

### 5.3 Dimple selector (EQ presets)

14px round dimple (`linear-gradient(145deg,#262524,#121110)`, raised shadow); selected
= pressed (inset shadows, scale 1.08) with a 5px `--rack-lamp` pip. Label 10px 600.
`role="radio"` in a `radiogroup`.

### 5.4 VFD

Radius 3px, padding 8px 12px, background `--rack-vfd-bg`, inset shadow; overlay: 170°
reflection (.07 → 0 at 40%) + dot mesh (`radial-gradient` 2.5px grid). Rows: title +
sub on the left, 7-seg time on the right; segmented seek bar (dim segments .12, elapsed
glowing, 3px/2px pattern, 12px hit height); annunciator row.

### 5.5 Glass window (panes, deck windows)

Radius 7px (panes) / 5px (deck windows); padding 20px 22px.
Background `radial-gradient(110% 70% at 50% 125%, lamp 5%, transparent 62%),
linear-gradient(180deg, #040302, #090706 40%, #0b0908)`.
Cut: `0 0 0 1px #000, 0 1px 0 1px rgba(255,255,255,.075), 0 −1px 0 1px rgba(0,0,0,.55),
inset 0 3px 8px rgba(0,0,0,.95), inset 0 0 0 1px rgba(255,255,255,.02)`.
Glass overlay (`::after`): `linear-gradient(166deg, rgba(255,255,255,.08) 0, .022 34%,
0 34.3%)` + top sheen `rgba(255,255,255,.05) → 0 in 12px` + `inset 0 0 18px
rgba(0,0,0,.55)` vignette. The first (header) pane uses 162°, .11 / .03 / 30%.
Tables inside panes run edge to edge (22px outer cell padding). Playing row:
accent text + `text-shadow 0 0 10px lamp 35%`.

### 5.6 Printed label + engraved rule

Label 600 11px +.05em `--rack-silk-dim`; a flex rule follows to the edge:
`linear-gradient(90deg, rgba(0,0,0,.75), transparent)` 1px with a `0 1px 0
rgba(255,255,255,.045)` highlight. Optional meta (count, key tray) after the rule.

### 5.7 Volume knob

58px knob (skirt `repeating-conic-gradient(#2b2a28 0 2deg, #121110 2deg 4deg)`, cap
radial highlight at 35% 30%, 11px pointer in `--rack-lamp`), inside a 76px box with a
23-dot LED ring (radius 35px, −135°…+135°): dots up to the value lit. `role="slider"`.

### 5.8 Faders

EQ: vertical 5px black slot, ticks only at +12 / 0 / −12 both sides, cap 24×13 ribbed
metal with a lamp-coloured centre line. Settings: horizontal 190×28 groove, ticks at
0/50/100%, cap 13×22 with a vertical lamp line, value readout in VFD colour.

### 5.9 Meters (amp, 96px tall slot)

| Style | Build |
|---|---|
| Needles | Two 1.55:1 meters; face `--rack-meter-face`; arc and ticks in meter ink, red over-zone 0…+3 (5px stroke); scale −20,−10,−7,−5,−3,−2,−1,0,+1,+2,+3 VU; "VU" Manrope 700 17px; needle `--rack-needle` 1.4px pivoting −46°…+46°; glass 160° reflection + bottom glow (screen blend). Position = `(10^(dB/20) − .1)/(10^(3/20) − .1)`. |
| Bars | VFD; L/R rows of 4px segments with 2px gaps, red past 93% (0 dB on −40…+3), dim track at .1, 3px hold tick; scale −40 −30 −20 −10 −6 −3 0 +3; header "PROGRAM HOLD ±x.x dB". |
| LED | Black recess; two rows of 20 LEDs (12px tall, radius 2) at −40 −30 −24 −20 −17 −14 −12 −10 −8 −7 −6 −5 −4 −3 −2 −1 0 +1 +2 +3; unlit tinted dark, last three red; lit with glow; peak LED stays lit. |
| Spectrum | VFD; 20 bars of 3px/2px stacked segments with falling 2px caps; scale 63 250 1k 4k 16k. |
| Stereo scope (Now Playing analyzer, "Stereo monitor") | After McIntosh-style audio monitors. VFD with a square goniometer (graticule circle, M vertical, L/R diagonals; mono = vertical line, width = how open the figure is) beside a dual-trace oscilloscope (L lane above, R below, centre lines + faint time grid). 1.3px traces with a soft glow and 0.72 per-frame phosphor afterglow; gain 1.6 (a −10 dBFS RMS programme fills about half the screen). Fed 160 L/R pairs (~10 ms) per tap frame. The analyzer setting is Auto / Spectrum / Scope; Auto shows the scope when the amp's meters are already a spectrum. (A waterfall was tried and rejected: too full.) |

Calibration: every meter reads programme level against **−10 dBFS RMS = 0 VU / 0 dB**
(never digital full scale — that pins meters on modern masters).
Ballistics (`ballistics.ts`): needle = damped spring, 2.0 Hz, ζ 0.8 (~300 ms, ~1.5 %
overshoot, pinned at both ends), fed RMS averaged over ~100 ms; bars/LED = 10 ms rise,
13 dB/s fall, 1.5 s hold then 10 dB/s; spectrum = 50 ms rise, 0.9/s fall, caps 0.25/s.

### 5.10 Deck faces (compact deck)

| Face | Contents |
|---|---|
| Transport | Cover window (square, `--pl`) + "Up next" window (three rows: n, title, time). Name "Network transport", silk "TIDAL · FLAC". |
| CD player | Black tray 34px: drawer with lip and "Digital audio" silk + 44px eject key; calendar VFD: 10×2 track numbers (present .8, current boxed, absent .1) + REMAIN big time + "n TR · total". |
| Display | Tall VFD: format annunciators (codec, bits, rate, EXCL/SHRD · TIDAL MAX), L/R bar meter + scale, NEXT line; title 16px. |

Expanded (any face): "Direct drive deck · 33⅓", plinth + record with the cover as the
label (spins 1.8 s/rev while playing), SVG tonearm sweeping 0→22° with progress.

## 6. Content (panes)

The display is open faceplate. Each section = printed label + glass pane (§5.5, §5.6).
Album/playlist/artist headers are one pane (cover 168px + title block + action keys in a
tray); track lists one pane; shelves/grids one pane each; pairs sit side by side with
equal heights. Library and search show a key tray (sub-sources / result types) and VFD
inputs (filter, search) above the panes.

## 7. Setup (Settings in Rack)

Same structure as every mode. Segmented choices → key trays; toggles → single key
labelled On/Off; ranges → horizontal faders with a VFD value; text inputs, selects and
read-only values (device, cache use, hotkeys) → VFD readouts. Rack group in Appearance:
Deck face, Amp meters, Lamp colour (keys whose bars glow in their own colour; Cover's bar
is a hue sweep), EQ unit, Lamp brightness.

## 8. Motion

Light bars on 80 ms / off 350 ms; key tilt 80 ms; lamp colour glide 1.2 s; platter
1.8 s/rev only while playing; tonearm follows progress; meters run on rAF only while
visible. Reduced motion: no platter spin, tonearm jumps, no cap animation, glides
instant; meters still show level (they are information).

## 9. Accessibility

Keys are buttons with names and `aria-pressed`/`aria-current`; knob, faders and the VFD
seek are sliders with value text and full key maps; presets are radios. Meters,
platter, tonearm, VFD ornament: `aria-hidden`. VFD and screen text meet 4.5:1 for every
lamp preset at every brightness ≥ 40%. Focus ring as foundation.

## 10. Do / don't

- Do keep one key style. Don't add round buttons, pills, or flat app buttons.
- Do print labels on the metal. Don't put section headings inside the glass.
- Do let only lit things carry colour. Don't tint faceplates or panes with the lamp
  beyond the faint pane backlight.
- Do drive every glow from `--rack-lamp`. Don't hard-code amber.
- Do keep the turntable for the expanded deck only.
