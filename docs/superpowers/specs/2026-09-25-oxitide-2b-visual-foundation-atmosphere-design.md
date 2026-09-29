# OxiTide — Track 2b Visual Identity: Foundation + Atmosphere Mode

- **Status:** Draft for review
- **Date:** 2026-09-25
- **Owner:** V
- **Parent:** `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 2b: visual identity + layout/IA)
- **Siblings (delta docs, written after this one):** `…-2b-visual-mode-instrument-design.md`, `…-2b-visual-mode-hifi-rack-design.md`
- **Process note:** direction was brainstormed live with browser mockups (session artifacts in gitignored `.superpowers/brainstorm/`). This document is the standalone source of truth; every decision below was owner-approved during the session.

---

## 1. Purpose & Outcome

Give OxiTide a deliberate visual identity and layout/IA to replace the current
placeholder styling (Spotify-gray, unicode glyphs, ad-hoc CSS), and structure it as
**one design system with three shippable modes**:

| Mode | Role | This doc? |
|---|---|---|
| **Atmosphere** | Default. Cover-reactive, frosted glass, alive while playing | Defined here, on top of the shared foundation |
| **Instrument** | Static, minimal-motion performance mode | Delta doc (mostly "foundation with effects off") |
| **Hi-Fi Rack** | Retro vinyl/analog skeuomorphism theme | Delta doc (personality layer) |

The shared foundation — tokens, typography, layout/IA, component anatomy, motion rules,
accessibility floors — is defined once, here. Modes only ever rewrite tokens and a small
list of feature flags; components never branch on mode.

Success = the app is recognizably OxiTide in a screenshot; the identity survives a
monochrome album cover; Instrument mode is the same app with the atmosphere dial at zero;
and nothing in this doc can eat the Track 3 performance budget.

## 2. Scope

**In scope:** design tokens; color system (incl. cover-reactive dynamic color);
typography (incl. user-selectable UI font); layout/IA (shell, queue, expanded
now-playing, density, window fit, mini player); component treatments; motion spec;
accessibility floors; the Settings → Appearance surface.

**Out of scope:** implementation; mini-player *sequencing* (approved in principle,
scheduled at plan time); true waveform peak data (stylized wave ships first);
synced lyrics (Track 4 — a slot is reserved in now-playing); light mode (token-ready
only, §4.4); code signing/packaging.

## 3. Design principles (tone)

1. **Music is the light source.** The artwork illuminates the UI; no chrome competes
   with it for saturation or brightness.
2. **Glass, not walls.** Surfaces are translucent layers over the artwork; depth comes
   from blur and light, not drop shadows.
3. **Motion answers the music.** Ambient motion exists only while playing. Pause and
   the room visibly stills. Chrome motion is fast and functional.
4. **Chrome is quiet until needed.** Controls sit at low contrast until hover/focus;
   covers and titles carry the visual weight.

Personality words: *immersive, warm-dark, unhurried, premium.* Reference feeling: a
listening room at night — the record sleeve is the lamp.

**Identity budget (guardrail):** the personality lives in exactly four places — the
Fraunces display face, lit edges + film grain, the tide motif (waveform seek + disc
spinner), and artwork reactivity. Resist adding a fifth; the main long-term risk to this
design is well-meaning re-addition of ambient motion and accent stripes, not blandness.

## 4. Token architecture

### 4.1 Three layers

| Layer | Contents | Changes at runtime? |
|---|---|---|
| **Base** (static) | Room background, text/muted neutrals, fixed semantic hues (danger/success/warning), surface alpha recipes | Never |
| **Dynamic seed** | Two+ colors extracted per cover artwork (§5.2) | On track change, 1200ms crossfade |
| **Semantic** | Everything components consume: `--text`, `--text-muted`, `--accent`, `--on-accent`, `--surface-1/2/3`, `--line`, `--edge-lit`, `--danger`, … | Only via mode switch or dynamic recolor |

Components **never** read base or dynamic-seed values directly — only semantic tokens.
Mode switching, dynamic color, and a future light mode are all semantic-layer rewrites.

### 4.2 Naming & mechanics

- Tokens are CSS custom properties on `:root`, authored in OKLCH (hex fallbacks fine
  for statics). Dynamic color tokens are registered via `@property` so track-change
  crossfades interpolate natively — no per-frame JS color tweening.
- Mode = a `data-mode="atmosphere|instrument|rack"` attribute on the root element;
  each mode's stylesheet only overrides semantic tokens and the feature flags in §4.3.
- User appearance settings apply *after* the mode layer and always win over mode defaults.

### 4.3 Mode feature flags

`--flag-backdrop-art` · `--flag-glass-blur` · `--flag-grain` · `--flag-disc-spin` ·
`--flag-wave-seek` · `--flag-dynamic-color` (tri-state: full/accent-only/off) ·
`--flag-ambient-motion`. Components consult flags, not mode names.

### 4.4 Dark-only, light-ready

Only dark ships. Because color is fully semantic and OKLCH-authored, a light variant is
a token-swap exercise later, not a redesign. Known inversion work (noted, not built):
scrim math inverts (light scrims), glass tints become dark-on-light, grain opacity drops.

## 5. Color

### 5.1 Base palette — "Warm Graphite"

| Token | Value | Use |
|---|---|---|
| `--bg` | `#17120e` | Room: window background |
| `--text` | `#f2ece6` | Primary text |
| `--text-muted` | `#a99f94` | Metadata, secondary text |
| `--surface-*` | `rgba(255,240,225, .05/.07/.09)` | Glass tints, lightest→strongest |
| `--surface-float` | `rgba(23,18,14,.82)` (Instrument: `#2b241c`) | Floating overlays over art (menus, dropdowns): opaque veil so a bright cover cannot wash them out; components add a slight `--accent` tint via `color-mix` |
| `--line` | `rgba(255,240,225,.10)` | Hairlines |
| `--edge-lit` | gradient, §10.1 | Lit panel borders |
| `--accent` (brand) | `#2ee2c4` | Fallback + static contexts |
| `--on-accent` | `#0b2420` | Text/glyphs on accent fills |
| `--danger` | `#ff8a7a` | Fixed; never cover-derived |
| `--success` | `#5fd68a` | Fixed; warm green, distinct from teal accent |

### 5.2 Dynamic color — three roles, guarded

Per cover artwork, extraction (downscaled bitmap → clustered candidates) fills three
roles, each clamped to an OKLCH envelope so the palette is good **by construction**:

| Role | Job | Envelope |
|---|---|---|
| **Primary** | Interactive accent: play button, active nav, seek/volume fill, focus rings, ♥-on, links, **playing/selected row tint + title text** | L ∈ [0.70, 0.85], C ∈ [0.08, 0.17]; ≥4.5:1 against the §5.3 composite ceiling (≥3:1 for icons); `--on-accent` computed per cover |
| **Secondary** | Atmosphere: backdrop wash, glass tint hue, scrollbar | L ∈ [0.25, 0.45], C ≤ 0.05; never carries text |
| **Tertiary** | **Informational emphasis, never interactive:** quality/metadata badges, now-playing disc label ring, decorative details | L ∈ [0.65, 0.80], C ∈ [0.06, 0.14]; badge text ≥4.5:1 against the §5.3 composite ceiling, decorative uses ≥1.5:1; ≥30° hue separation from Primary (else collapses to a Primary tint) **so it never reads as interactive** |

Rules:
- **Cover-first, harmony-fallback.** Use the art's real colors when usable and mutually
  harmonious; otherwise keep the best and derive the rest by hue rotation.
- **Semantic split:** Primary = interactive, Tertiary = informational. Interactive
  elements never use Tertiary; informational surfaces never use Primary.
- **Degradation paths:** a cover with no compliant candidate (monochrome art) yields
  brand-teal Primary, warm-neutral Secondary, teal-tint Tertiary — the app keeps an
  identity floor. **No artwork at all** takes the identical color fallback, and with
  nothing to blur the backdrop renders as the room's static warm gradient (a no-art
  track briefly looks like Instrument's backdrop — deterministic, no special casing).
- **Semantic colors (danger/success/warning) never go dynamic.**
- **60-30-10:** Primary/Tertiary only ever fill their semantic slots — a cover cannot
  flood the UI.
- All dynamic tokens crossfade over 1200ms on track change. Rapid skips
  **interrupt-and-restart**: a new change cancels the in-flight crossfade and starts a
  new one *from the current interpolated values* (natural via `@property`) — never
  queues, never restarts from the previous track's start. The backdrop image crossfade
  follows the same rule; skip-storms drift continuously, with no flashes.

### 5.3 Contrast chain — one dependent system

The clamp envelopes above are **not** evaluated against an idealized constant, because
the real backdrop varies: it is a blurred version of the same cover the colors were
extracted from, and blur does not flatten luminance. The guarantee is built as a chain:

1. **Structural floor (static):** scrim minimum opacity (≥55%) over the backdrop and
   glass-tint minimum alpha bound how bright the panel composite can get for most covers.
2. **Composite ceiling (per cover):** the extraction step (Rust side, already working on
   the decoded artwork bitmap) additionally computes a **luminance map of the composite**
   — blur → scrim → tint math applied over the regions panels actually occupy — and takes
   a high percentile (**P90**) as the *composite ceiling* for this track.
3. **Clamp evaluation:** Primary and Tertiary contrast clamps are evaluated against that
   per-track composite ceiling. Uniformly dark covers get a relaxed clamp (more vibrant
   accents); covers with bright regions get extra clamping only when that cover needs it.

Cost: one luminance reduction over an already-decoded tiny bitmap — trivial, but noted
as extraction-step work for the plan. Exact percentile and region sampling are finalized
at plan time (§15).

**Bound of the guarantee (Plan 11 implementation finding):** the chain's 4.5:1 text floor
is achievable while the per-cover composite ceiling stays at or below ~0.18. Above that — a
very bright cover under the ≥55% scrim — no colour inside Primary's L ∈ [0.70, 0.85]
envelope can reach 4.5:1; even pure white text tops out near ~4.4:1. Such extreme covers
degrade to the plain light Primary fallback (Primary is still clamped as far as the envelope
allows). The text floor is therefore documented as holding **within the ceiling bound**, not
unconditionally.

### 5.4 Setting

**Dynamic color: Full / Accent-only / Off** (notched slider). Accent-only = Primary
follows the cover, Secondary/Tertiary stay static brand. Off = fully static brand.

## 6. Typography

| Slot | Spec |
|---|---|
| Display (view titles, now-playing title) | **Fraunces** 28/640, high `opsz`, roman; bylines (artist–album) Fraunces italic 520. Theme-fixed, display sizes only |
| UI (everything else) | **Manrope** variable, bundled (default) |
| Scale (UI font) | Display 28/700 −0.02em · Section 18/600 · Body 14/450–500 · Meta 11.5/500 · Micro 10/700 +0.09em uppercase |
| Weights (UI font) | 400/500/600/700 only — Fraunces display weights (640 roman / 520 italic) are separate |
| Numerals | Tabular (`tnum`) for all durations/times/readouts |

**User-selectable UI font (Settings → Appearance):** Manrope (default) / Inter /
System / Custom… (any installed font name; fallback chain always ends `system-ui`).
Live-applies; user pick beats mode default. The Custom field is not blind free text:
**discovery** (autocomplete from installed families via `queryLocalFonts()` where
available, otherwise a curated list of common Windows fonts), a **live preview** line
rendered in the candidate before applying, and **post-apply validation** via the Font
Loading API (`document.fonts.check()`) — a name that doesn't resolve shows an inline
"not found — using fallback" note rather than failing silently. Only Manrope + Inter are bundled; no
user-uploaded font files (YAGNI; FontFace API later if wanted). Instrument mode
defaults to Inter; the display face (Fraunces) is part of the shared identity and is
*not* swapped by the font setting.

## 7. Layout & IA

### 7.1 Shell ("Refined Classic")

- **Sidebar** 248px ↔ 64px icon rail; manual toggle + auto-rail below 1080px window width.
- **Queue:** persistent right rail, **hideable and user-resizable 260–420px (default
  320px), width persisted**. Below 760px window width it can no longer dock — it opens
  as a glass overlay over the main pane instead of pushing content.
- **Player bar:** 72px glass, full-width bottom; **hairline seek across its top edge**
  (thin at rest, grows + thumb appears on hover/drag); 48px cover thumb opens expanded
  now-playing; transport cluster center-left; badges + volume right.
- **Min window 720×520** (was 900×600 in early drafts).

### 7.2 Expanded Now Playing — "Editorial Split"

Replaces the main pane (sidebar + player bar persist); Esc / clicking artwork returns.

- Left: the artwork. **Cover shape setting: Disc (default) / Sleeve.** Disc spins only
  while playing and *eases* to a stop on pause (~600ms); CSS-only groove + sheen
  overlays, no canvas; spin speed calm (spec ~12s/rev — a design value, not literal
  33⅓). `prefers-reduced-motion` forces no-spin regardless of setting.
- Right metadata column: eyebrow ("Now playing"), Fraunces display title, italic serif
  byline (artist — album), badge row (quality, bit-perfect, sample rate), divider,
  **Next Up** (3 rows), and a reserved slot where synced lyrics land (Track 4).

### 7.3 Track rows

- **Cover thumbnails** in mixed-content lists (playlists, search, library, queue, home):
  full-bleed square fills the row top to bottom; hover reveals a play-glyph overlay on
  the thumb. **Not shown in single-album view** (redundant). Rows: 46px Comfortable /
  38px Compact.
- Playing/selected row = accent-tinted background (18% accent mix, 24% on hover) + accent
  title text; no ring around the cover. **No left bars, no animated equalizers** (owner
  veto).

### 7.4 Density & window fit

- **Density setting: Comfortable (default, 14px body / 46px rows) / Compact (13px /
  38px).** One toggle drives font-size, row-height, and padding tokens together.
- **Mini player** (approved in principle; may ship as a follow-up slice right after 2b
  core): ~300×430 — cover, metadata, hairline seek, transport, queue shortcut; toggled
  from player bar / View menu; window size limits relax while active. Exists because
  users keep music apps small: offer a smaller instrument rather than a shrunken app.

## 8. Components

Radii: controls 8px · cards 12px · sheets/modals 16px · play button circular.
States: hover = surface brighten (~+2% alpha) · press = scale .98 · disabled = 40%
opacity · **focus-visible = 2px accent ring, 2px offset, keyboard only** ·
selected = accent tint + accent text (rows).

| Component | Treatment |
|---|---|
| Buttons | Primary = accent fill + `--on-accent`; Ghost = glass; Danger = fixed red ghost |
| Icons | Line icons, 1.75px rounded stroke, Lucide-family **inlined SVGs** via `currentColor` (follows dynamic accent for free); no icon font; unicode glyphs retired |
| Track table | §7.3; virtualized as today; sortable headers (Plan 9) styled as quiet Micro labels |
| Cards (album/artist) | 12px radius, lit edge, hover = slight brighten + cover zoom 1.03 |
| Seek / volume | §10.2 tide waveform (seek); volume hairline; thumb on hover/drag only |
| Badges | Bordered chips: MAX **Tertiary-outlined** (informational emphasis, §5.2 — early kit mockups showing it in Primary are superseded); BIT-PERFECT / sample-rate muted. (LED-readout style is parked for Hi-Fi Rack) |
| Toasts | Glass, **severity glyph** (check/alert in severity color) + action link; **no left stripes** (owner veto); slide-in with slight overshoot is the one sanctioned micro-interaction |
| Context menu / dialog | Floating glass, lit edge; dialog titles in Fraunces; danger items in `--danger` |
| Tooltip | Small dark glass, no arrow |
| Scrollbar | Thin (8px), warm translucent thumb, accent on hover; track transparent |
| Loading | **Disc spinner** (small record with accent tick) — owner-called-out favorite |
| Empty states | Fraunces headline + muted body + one primary action |

## 9. Motion spec

| Token | Value | Used for |
|---|---|---|
| `--dur-micro` | 120ms | hover, press, toggles |
| `--dur-state` | 180ms | selection, expand/collapse, tab switches |
| `--dur-view` | 240ms | pane transitions, queue rail slide, overlays |
| `--dur-mood` | 1200ms | backdrop + dynamic-color crossfade on track change |
| disc spin | linear | playing only; pause = ~600ms ease to stop |

Easings: entrances `cubic-bezier(.2,.7,.3,1)` · exits `cubic-bezier(.5,0,.75,0)` ·
linear only for the spin.

Rules (standing, per PM-D1):
- Only `transform` and `opacity` animate — compositor-only, never layout properties.
- Ambient motion exists only while playing (principle 3).
- `backdrop-filter` surfaces are a fixed, countable set (sidebar, panels, player,
  overlays) — no nested or stacked glass.
- `prefers-reduced-motion`: ambient off, crossfades instant, slides become 120ms fades.
  Instrument mode ≈ this as a permanent user choice.

## 10. Signature treatments

### 10.1 Lit edges + film grain

- Glass panels use a **gradient border** (consistent top-left key light): bright warm
  edge top-left falling to near-black bottom-right, via padding-box/border-box layered
  backgrounds. Panels read as *objects*, not frosted rectangles.
- The backdrop carries a **fractal-noise grain tile at ~4–5% opacity** — one fixed,
  tuned value, **never a user setting** (settings proliferation rots systems).
- Both are static CSS (layered gradients + one tiled SVG/PNG asset) — zero per-frame cost.

### 10.2 Tide motif (name-carrying, capped at these places)

1. **Waveform seek bar:** a soft stylized wave line; played portion fills in accent.
   2b ships the stylized wave; true per-track peaks are an optional later upgrade.
2. **Disc spinner** for loading.
3. (Reserved third slot: pull-to-refresh ripple if ever needed — do not invent more.)

### 10.3 Micro-interactions

Toast slide-in with slight overshoot — **the only one**. Heart pop and play-button
breath were prototyped and vetoed by the owner as too much.

## 11. Accessibility

- Contrast floors: text ≥4.5:1 for covers whose composite ceiling is ≤ ~0.18 (§5.3 bound),
  enforced by one dependent chain (§5.3): structural scrim/tint minimums *plus* per-cover
  composite-ceiling clamping. Accent clamps evaluate against the actual composite, never an
  idealized constant. Extreme bright covers (> ~0.18 ceiling) degrade to the plain light
  Primary fallback and cannot meet the floor by construction.
- Focus-visible ring (2px accent, 2px offset) keyboard-only; never on click.
- Reduced-motion handling per §9; cover spin additionally gated by §7.2.
- Hit targets ≥24px in Comfortable density (Compact exempts list rows, which keep full-row hit areas).
- Semantic colors fixed (§5.2) — learnable, never cover-derived.
- Dark-only ship; light-ready structure per §4.4.

## 12. Settings → Appearance (new surface)

| Setting | Values | Default |
|---|---|---|
| Mode | Atmosphere / Instrument / Hi-Fi Rack | Atmosphere |
| Dynamic color | Full / Accent-only / Off (notched slider) | Full |
| Cover shape | Disc / Sleeve | Disc |
| Typeface | Manrope / Inter / System / Custom… | Manrope |
| Density | Comfortable / Compact | Comfortable |

All live-apply, all persisted, user picks beat mode defaults. When the OS reports
`prefers-reduced-motion`, ambient motion and the disc spin are forced off and crossfades
become instant **regardless of mode or settings** — this is a hard floor, not a suggestion.

## 13. Performance notes (PM-D1)

- Backdrop = plain `filter: blur()` on an `<img>` (cheaper than stacked
  `backdrop-filter`); glass blur reserved for the fixed panel set.
- Cover color extraction happens off the UI critical path (Rust side, from already
  decoded artwork bytes; exact home decided at plan time).
- Grain is a static tile; lit edges are static gradients; crossfades run on the
  compositor via `@property`.
- Every later spec touching these surfaces must state its perf impact (roadmap rule).

## 14. Rejected options (recorded so they stay rejected)

- Gradient two-tone accent + play-state bloom — declined at polish review.
- Heart pop / play-breath micro-interactions — vetoed ("too much").
- Accent left-bars on rows; severity stripes on toasts — vetoed.
- Full-window now-playing takeover (shell option B) — declined; immersion already comes
  from the artwork bleeding through the whole window; queue-editing ergonomics win.
- Cool Slate / True Neutral base palettes — Warm Graphite won.

## 15. Open items for the implementation plan

- Mini-player sequencing (in-2b vs immediate follow-up slice).
- Queue rail drag-resize mechanics (pointer handling, constraints, persistence key).
- Dynamic-color extraction host (Rust vs frontend worker), cache keying, and the §5.3
  composite-ceiling details (percentile, region sampling).
- Fraunces/Manrope/Inter subsetting and bundle budget (~250KB total ceiling).
- Track 3 harness acceptance: atmosphere mode must stay within agreed frame/GPU budgets
  (values set when the harness lands).
