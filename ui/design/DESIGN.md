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
| Resting depth | `--shadow-panel`, `--shadow-cover` | Neutral dark drop shadows (warm-ink and softer in light); never accent-coloured, so never a glow |
| Semantic | `--danger`, `--success`, `--warning` | Fixed, never cover-derived |

**Panel edges.** `.lit` (app.css) is the key-lit panel: `--surface-2` fill with the
`--edge-lit` rim, bright top-left to dark bottom-right. `.lit-tint` is the accent-tinted
variant (a 7% accent wash and an even 30% → 14% accent hairline, no highlight); Home's
containers use it.

**Section hook class.** Shared components mark their section body with `pane`
(plus `pane-flush` for edge-to-edge tables). No mode styles it except Rack, whose
`rack.css` turns it into a glass window inside the display unit; keep adding `pane` to
new section containers so they join the Rack's look for free.

**Page header and toolbars** (`app.css`, 2026-10-10 polish). List and collection views
(Playlists, Profiles, History, Feed) open with `header.page-head`: the `h2` title on the
left; a muted `.count` and the tools on the right, all 38px tall. Tools come from the field
family — `.search-input` for search/filter, `SortSelect` for sort, `.tool-btn` (icon +
label) for actions — never mixed with round icon buttons in the same row. Rack skins
`.page-head > h2` as a printed label and `.tool-btn` as a light-bar key.

**Settings page** (`views/Settings.svelte`, 2026-10-15 overhaul). A left category nav
(`role="tablist"`, vertical, Lucide icon + label; a scrolling strip below 760px of page width)
beside a content column capped at 1320px. Each tab is a `.cols` grid of hand-balanced column
stacks (`.col`; two where 520px each fit, about 1060px of content, else one; `.wide` stacks span
both). Stacks keep a constant height: controls that do not apply are disabled, not removed
(EQ bands, ReplayGain sliders, AutoMix). The shorter stack's cards share the leftover height
evenly, each spreading it across its rows, so both columns always end on one line — when
adding a card, put it in the shorter stack. The page header carries a settings search
(`SettingsSearch`, a combobox over `lib/settingsIndex.ts`): results name the tab and card, and
choosing one opens the tab, scrolls the row to the middle, rings it briefly and focuses its
control. Every searchable row carries `id="…"` (rendered as `data-setting`); **a new setting
needs an `SETTINGS_INDEX` entry with that id** — a test fails if an entry's id is missing from
the page sources, and entries for controls not currently shown (Rack options, the
exclusive-mode fallback) are hidden from results. Switching tabs animates with transform and
opacity only: the nav highlight is one pill that glides between tabs (`--dur-view`,
`--ease-in-out`; first placement instant; hidden in Rack, whose keys light their own lamp), and
the new tab's cards rise in (10px, `--dur-view`) staggered 45ms down the page; reduced motion
drops the lift and the glide, leaving a quick fade. Each category is a stack of `SettingsSection` cards
(`lib/components/settings/`: icon chip, 15/650 title, optional description and header action)
holding hairline-separated `SettingRow`s — label and help text left, control right, stacked
for wide controls; a row wraps by its own width (flex-wrap), so the same row works full-width,
in a half-width column card, or in a narrow window. The page-level nav switch uses a
container query on `.settings`, not a viewport one. Controls are one family: `Toggle`/`ToggleRow` (native checkbox over a drawn switch; the
whole row toggles), `Segmented` (2–4 short choices; keeps the `.segmented`/`.active` hooks),
`Select` (longer lists or options with a hint line; the `SortSelect` panel recipe, flips
upward near the window bottom), `Slider` (filled track, `origin="center"` for gains, vertical
for the EQ bank), `NumberField` (clamps and writes back), `Button` (the `.tool-btn` recipe plus
primary/danger), `Notice` (info/warning/success callout). Order within a tab is by how often a
setting is touched: Playback runs Streaming → Output → Volume leveling → Equalizer → Crossfade
→ AutoMix. Sections hide controls that do not apply (fallback only with exclusive/ASIO, Dynamic
color not in Rack, AutoMix needs a crossfade). Rack skins the switch as a square lamp key in
`rack.css`.

**Collection grids** share one recipe: `repeat(auto-fill, minmax(160–168px, 1fr))`, 12px
card padding (Albums, Mixes, Playlists, Feed releases).

**Media cards** (`MediaCard.svelte`: Home, Explore/For You sections, Mixes, Playlists,
Albums, Artist, Purchases) share one card. Variants: `tile`, `carousel` (210px), `mini`
(148px), `row`. Every cover carries a 1px `--line` ring and `--shadow-cover`. Playlist
covers keep sharp corners, and the old offset "stacked sleeve" box behind them is gone.
Hover: the card's own cover colour (`--card-tint`, a registered colour, from
`lib/coverTint.ts` → `analyze_cover`, prefetched as cards scroll into view) spreads from
the cover and blends into whatever the card sits on; the cover lifts 3px with a deeper
shadow; tracks and cards that start playback (`playButton`) show an accent play button at
the cover's bottom-right. Feed releases keep their own card (`FeedTimeline`), whose
hover is still `--surface-2`. Profiles
draw each artist as a **record** (portrait as the label on a vinyl disc; `--vinyl*`
material tokens, black in every mode) and, in name order, letter groups as crate
dividers.

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

Easings: `--ease-enter` (entrances, hover/state changes), `--ease-exit` (exits),
`--ease-in-out` (two-way glides); linear only for spins.

Only `transform` and `opacity` animate (plus registered colour/number properties), with
two recorded exceptions (addendum `2026-10-15-oxitide-atmosphere-rules-addendum.md`):
hover/press washes (`background-color`, `color`, `border-color`) ease over `--dur-micro` —
the global `button` rule in `app.css` carries them, and track rows do the same — and the
docked queue's open/close glide (AR-5 as amended by AR-9). The rail animates its width
(380 ms, `--ease-in-out`). The page is held at its final width, and its single layout change
is FLIP-animated in step with the rail by `lib/queueTransition.ts`:
- positions travel, and cards with a visible box animate their width. So do clipping
  boxes whose content fits their width, such as a track table's scroller or a hero
  painted by absolute layers; their contents follow by layout. A carousel row that
  already overflows sideways is excluded, and its cards use the clipped-card fade.
- hidden or clipped elements leave a fading inert copy, and revealed ones fade in.

Any other layout animation needs a ruling.
`backdrop-filter` surfaces are a fixed set and never nest. Menus mark the selected
option with the accent, not a second background.
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
- Clickable names (track → album, album, artist) use `NameLink` / `ArtistLinks`: they keep
  the surrounding text's colour and weight, underline on hover (muted list cells also lift
  to `--text`), and carry the name as a tooltip. They are `role="link"` spans so ellipsis
  still truncates them. In rows they take no Tab stop; the track menu's "Go to" items are
  the keyboard path. Names for the current page stay plain text. Inside rows that are
  themselves play buttons (queue, History) names are pointer-only nested links and the
  row menu carries the "Go to" items (spec `2026-10-15-oxitide-name-links-design.md`).

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
| App mark | `ui/src/lib/components/Logo.svelte` (inline SVG; ring = `--accent`, wave = accent hue-rotated −130°, brand colours as fallback; detail steps down by `size`; shown in Sidebar, Rack header, Login; icon-file sources in `crates/oxitide-app/icons/svg/`) |
| Window chrome (Windows) | Undecorated window: `WindowControls.svelte` caption buttons pinned top-right over every shell, `lib/titlebar.ts` drag band (top `--titlebar-h` = 32px; presses on non-controls drag, double-click maximizes). Shells without a top gutter (Mini, Rack, Rack mini) pad their top to `--titlebar-h`; the queue steps down under it. Close hover uses `--caption-close` (platform red). Snap Layouts: native overlay in `crates/oxitide-app/src/titlebar.rs`. Linux keeps the system frame. |
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