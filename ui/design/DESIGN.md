# OxiTide design system

The build reference for OxiTide's UI. Each mode has its own design file because the modes
are deliberately different products on one engine; this file holds what they share.

| Mode | `data-mode` | Personality | Design file |
|---|---|---|---|
| Atmosphere (default) | `atmosphere` | A listening room at night; the record sleeve is the lamp. Glass over the artwork, motion answers the music. | [atmosphere/DESIGN.md](atmosphere/DESIGN.md) |
| Instrument | `instrument` | The mastering desk: Atmosphere with the dial at zero. Static, dense, nothing moves unless you touched it. | [instrument/DESIGN.md](instrument/DESIGN.md) |
| Hi-Fi Rack | `rack` | A premium late-80s/90s component stack: black glass, amber VFDs, light-bar keys, real needles. | [rack/DESIGN.md](rack/DESIGN.md) |

Decision records live in `docs/superpowers/specs/` (dated); these files describe the
current system and must be updated in the same change that alters it.

---

## 1. The contract every mode keeps

1. **Three token layers.** Base (static) → dynamic seed (`--dyn-*`, driven by cover
   analysis) → **semantic** (`--bg`, `--text`, `--accent`, `--surface-*`, `--pane-*`, …).
   Components read semantic tokens only. Files: `ui/src/styles/tokens.css` (base +
   semantic + motion + flags), `modes.css` (per-mode overrides), `themes.css` (light
   inversion), `fonts.css`.
2. **Components never branch on mode or theme.** A component that needs `if mode` means
   the token or flag surface is incomplete — extend it. The single sanctioned exception
   is `App.svelte` choosing `RackShell` from `appearance.rackShell` (Rack spec RK-D1);
   Rack-only components under `ui/src/lib/rack/` may read Rack's private `--rack-*`
   tokens, which exist only under `[data-mode="rack"]`.
3. **Mode and theme are orthogonal attributes** on `<html>`: `data-mode`
   (`atmosphere | instrument | rack`) and `data-theme` (`dark | light`). Rack is always
   dark and ignores `data-theme` (no light variant is planned).
4. **Feature flags** live as CSS numbers (`--flag-*`) for presentation gates and as JS
   getters on the `appearance` singleton for behaviour (`discSpin`, `waveSeek`,
   `rackShell`, …).
5. **Settings are per user, not per mode**, except Typeface and Dynamic colour which have
   per-mode defaults; Rack replaces Dynamic colour with Lamp colour.

## 2. Shared foundations

### 2.1 Colour roles

| Role | Token | Notes |
|---|---|---|
| Page background | `--bg` | Mode/theme specific |
| Text / secondary text | `--text`, `--text-muted` | ≥ 4.5:1 on their surface |
| Accent + text on accent | `--accent`, `--on-accent` | Atmosphere: dynamic; Instrument: brand; Rack: lamp |
| Surfaces | `--surface-1..3`, `--surface-float` | Raised layers; float = menus/popovers |
| Hairlines | `--line`, `--edge-lit` | |
| Semantic | `--danger`, `--success`, `--warning` | Fixed, never cover-derived |

**Section hook class.** Shared components mark their section body with `pane`
(plus `pane-flush` for edge-to-edge tables). No mode styles it except Rack, whose
`rack.css` turns it into a glass window inside the display unit; keep adding `pane` to
new section containers so they join the Rack's look for free.

### 2.2 Type

- Display: **Fraunces** (Atmosphere/Instrument titles). Rack uses **Manrope 700** for
  titles (hardware type, no serif).
- UI: Manrope (default) / Inter (Instrument default) / System / Custom, user-selectable;
  fallback always ends in `system-ui`.
- Weights 400/500/600/700; tabular numerals for all times and readouts.

### 2.3 Motion

| Token | Value | Use |
|---|---|---|
| `--dur-micro` | 120 ms | hover, press, toggles |
| `--dur-reveal` | 80 ms | seek reveal |
| `--dur-state` | 180 ms | selection, tabs, expand |
| `--dur-view` | 240 ms | panes, drawers, overlays |
| `--dur-mood` | 1200 ms | colour glides on track change |

Only `transform` and `opacity` animate (plus registered colour/number properties).
Ambient motion only while playing. `prefers-reduced-motion` is a hard floor in every
mode: ambient off, glides instant, slides become 120 ms fades.

### 2.4 Accessibility floors (all modes)

- Text ≥ 4.5:1; large text and UI glyphs ≥ 3:1.
- Focus-visible ring: 2px accent, 2px offset, keyboard only.
- Hit targets ≥ 24px (list rows keep full-row targets).
- Every custom control is a native button/input or carries the full ARIA pattern
  (slider: value text + keys; tabs: roving + panels; menus: active descendant).
- Decorative motion and meters are `aria-hidden`; their information is available as text.
- `document.title` follows the route; a skip link reaches `#main`.

### 2.5 Performance (PM-D1)

Every visual change states its cost and re-runs the affected Track 3 scenarios under
unchanged `scripts/perf/budgets.json`. Instrument is the performance baseline. A mode
that is not active must cost nothing.

## 3. Where things live

| Concern | Location |
|---|---|
| Tokens | `ui/src/styles/{tokens,modes,themes,fonts}.css` |
| Appearance state | `ui/src/lib/appearance.svelte.ts` |
| Dynamic colour | `ui/src/lib/dynamicColor.svelte.ts`, `crates/oxitide-app/src/{artwork,palette}.rs` |
| Rack shell + units | `ui/src/lib/rack/` |
| Icons | `ui/src/lib/components/Icon.svelte` (inlined Lucide paths, no glyphs) |
| Covers | `ui/src/lib/components/Cover.svelte`, `ui/src/lib/artwork.ts` |
| Rack reference mockup | `ui/design/rack/reference-mockup.html` (open via any static server at the repo root) |
| Phone remote | `ui/remote.html`, `ui/src/remote/` (styles: `remote/remote.css` + component styles) |

## 4. Phone remote (Track 11)

The phone page served by the LAN remote is a separate entry, not a fourth mode.

- **Look:** Atmosphere tokens (`data-mode="atmosphere"`) with **Dynamic color (full)**: the PC
  analyses the current cover (`analyze_cover` over RPC, cached on the PC) and the page
  applies the same `--dyn-*` tokens as the desktop through `lib/coverPalette.ts`.
  Components still read semantic tokens only; never `--rack-*`.
- **Room:** `remote/components/Backdrop.svelte` puts the current cover behind every page,
  blurred (56px), slowly drifting (38s alternate; off under reduced motion), with an accent
  wash at the top and the shared scrim (`--scrim-top/--scrim-bottom`) as the legibility
  floor. Covers crossfade over `--dur-mood` after preloading.
- **Theme:** follows the phone's `prefers-color-scheme` (`remote/theme.svelte.ts`; the page
  CSP forbids inline scripts), so `themes.css` applies unchanged and the palette is
  re-analysed for the active room.
- **Layout:** mobile-first single column, 16px gutter (`--remote-gutter`), fixed bottom dock
  (mini bar `--remote-minibar` + five-tab bar `--remote-tabbar` with an accent pill behind
  the active tab), safe-area insets respected; feeds render as horizontal shelves, track
  lists as 60px rows. Long lists load the next page as the end scrolls into view.
- **Volume:** phone hardware keys cannot reach a web page, so Now Playing has no volume bar.
  Swiping up/down on the artwork changes the PC's volume (full cover height = 100%,
  6px dead zone so taps never change it) with a level overlay; the artwork is a keyboard /
  screen-reader slider (arrows step 5%). A small speaker button shows the level and mutes.
- **Touch:** every control is at least 44×44px (`--remote-touch`); presses scale to 0.92;
  row menus open a bottom sheet; queue reorder uses a drag handle (`touch-action: none`).
- **Icons:** the shared `Icon.svelte` (Lucide only, no glyphs). Covers load straight from
  `resources.tidal.com`; a missing cover shows the `music` icon on `--surface-2`.
- **Motion:** page enter (fly 14px, 260ms), title fade per track, cover settles to 0.9 while
  paused, rows/tiles rise in with a short stagger (first 16 rows / 10 tiles), sheet rise,
  toast/mini-bar slide. Durations come from the `--dur-*` tokens or `remote/motion.ts`,
  so `prefers-reduced-motion` removes them.