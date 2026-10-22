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

**Track table edges** (`TrackTable.svelte`). A table that scrolls itself (`fill`: Tracks,
Album, Playlist, Mix, ArtistAll) has no header frost. Its header is a separate table above the
scroller (both `table-layout: fixed` over one `<colgroup>`, both reserving the scrollbar
gutter), and a static mask on the scroller fades the rows out over 48px just below the header
and just above the player bar (the scroller runs under it). The mask never moves with the
scroll, so it can't trail a fast scroll and costs nothing per frame; a mask whose stops follow
`scrollTop` lags the compositor and restyles on every frame. Each fade grows with the scroll
distance left that way, so a list at rest, or at its end, shows every row whole. The header
stays unmasked and untinted, and the scrollbar strip (`--scrollbar-w`, the standard thin
bar's 10px) is left out of the mask. Tables that
don't scroll themselves keep the frost.

**Track table tools.** Sort (SortBar's `.quiet` buttons) and Now playing (`.locate`) share
one look: 30px, 12px/600 muted text on a soft `--surface-2` fill (55%); hover raises the fill
to `--surface-2`, adds the `--line` border, lifts the text to `--text` and turns the icon
`--accent`; press scales to 0.97. Double-clicking Now playing follows the playing track
(label "Following", `aria-pressed`, styled like the primary sort chip: accent border 55%,
accent 16% fill) and re-centres it on every track change until double-clicked again.

**Track table columns.** Every column but the number, cover and actions takes a share of the
width (title 21%, artist 19%, album 18%, optional columns 14%, time and quality 8%), so the
date, time and quality spread across the row instead of packing at the right edge (time is
left-aligned like the other shared columns; the compact list keeps it right-aligned); a table
without some of them hands the spare width to the rest by share. Long names end in an
ellipsis and never widen their column (the full name is the tooltip). The number and cover
cells pad 18px (text cells 12px) so the row's start breathes. While the playing track is
among the listed rows, a quiet "Now playing" button (`circle-dot`, the queue's "Show the current track" icon) after the sort
controls scrolls it to the middle of the list (smooth unless reduced motion).
Favorites arrive newest-favorited first, so the Tracks page's original order is "recently
added". Row actions show on hover/focus; the heart stays visible except on the favorites
list (Tracks), where every row is a favorite. The Tracks count line ends in a refresh icon
(spins while loading) and, when TIDAL counts favorites it no longer serves, an
"N unavailable" info button whose tooltip and toast explain them (they can't be listed).

**Track table sorting** (`TrackTable.svelte` + `SortBar.svelte`, 2026-10-19 multi-sort spec).
Sortable headers are text buttons. A plain click sorts by that column alone; Shift+click, or the
round `+` that appears on another header's hover/focus once a sort is on, adds it as the next
level (three at most). Active headers take `--accent` and show a direction arrow (Lucide
`arrow-up`/`arrow-down`, never glyphs). When more than one level is active they also show a
small filled rank dot (`--accent` / `--on-accent`). `aria-sort` marks only the primary. Above the
table, beside the filter, the sort bar shows one chip per level:
- chips use an accent-tinted `--surface-2` fill, with the primary chip tinted more strongly;
- clicking a chip flips its direction, × removes it, and Alt+←/→ reorders;
- a quiet "Add sort" opens the shared `ContextMenu`;
- with no sort the bar is a single quiet "Sort" button.

The optional columns (Date added, Release date, Popularity, Plays) are toggled from a "Columns"
menu on the header row. They hide under a 760px container query on the table scroller.

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

Streaming quality restores **Low** with the hint **HE-AAC 96 kbps** in the existing
Select, alongside High, Lossless (still the default), and Max. Saved `low` preferences
are preserved on load. Compact badges stay **AAC** for served Low streams; detailed
readouts show the served bitrate. No new styling, animation, or background work. The
player bar's quality plate (Atmosphere §3) also switches the tier from a menu, with the
same four tiers.

**Page frame:** the main scroller pads every page `90px 28px` (`--page-top` on `.main`:
32px of frame plus 58px of air above the title), with the floating player's clearance at
the bottom. Now Playing is a full-window composition and keeps a bare 32px top. A page
that draws its own top reads `var(--page-top)`: Profiles' sticky head carries it as
padding and sticks at `32px − --page-top`, so the stuck band stays 32px deep.

**Page titles** (a page's own `h2`, bare or in its `.page-head`; Home's greeting and the
Tracks and artist-catalogue heads read the variable in their own styles) are 46px (`--page-title-size` on `.main`, line-height 1.1) in the display face. Section
headings stay at 20/600; detail-page headers (album, playlist, artist) have their own
scale. Rack's display is also `.main` but resets both variables (`.main.rack-main`), so it
keeps its own page padding and small title labels (`rack/DESIGN.md`).

**Detail headers** (`DetailHeader.svelte`: Album, Playlist, Mix) put the cover beside one
column as tall as the cover: the Play/Shuffle row (with the playlist's search pill) sits on
the cover's bottom edge, and the title block sits 28px above it, 10px apart top to bottom:
stats (counts like "702 TRACKS" and "2 FANS" in `--accent`), the byline, then the title
(`clamp(30px, 3.6vw, 48px)`) directly over the buttons. The kicker (PLAYLIST / ALBUM / MIX) runs vertically down the cover's right
edge (reading top to bottom), centred on it. The page's actions (Edit, Favorite, Share, More, …) sit at the
row's right end, level with Play. Nothing sits under the cover, so the track list starts
right below it.

**Collection grids** share one recipe: `repeat(auto-fill, minmax(160–168px, 1fr))`, 12px
card padding (Mixes, Feed releases). The two library collections, Playlists and Albums,
use an `18px 14px` gap, 10px card padding and a 15/600 title over a 13px subtitle.
Playlists has large tiles, `minmax(210px, 1fr)` (about six across a wide window); on
hover its cards get no fill, only the cover's lift and zoom and the Shuffle/Play buttons. Albums
sets eight across inside an extra 24px side inset, with container queries stepping down
one column per ~160px of width so that tiles stay at about 140px or wider. Their headers carry
the count under the title (`.title-block`: 14px at 62% text, 2px below) and a sort select (Albums: Recently added, Release date, Title A–Z, Artist A–Z), and
a sort change glides the cards to their new places (`animate:flip`, 320ms, none under
reduced motion).

**Media cards** (`MediaCard.svelte`: Home, Explore/For You sections, Mixes, Playlists,
Albums, Artist, Purchases) share one card. Variants: `tile`, `carousel` (210px), `mini`
(148px), `row`. Inside a `Carousel` the `carousel` and `mini` cards size to the row
instead: a container query picks a whole-card count, and each card takes the width that
leaves 0.4 of the next peeking past the edge (190–220px and 136–152px at usual widths),
under the row's 72px edge fade. Every cover carries a 1px `--line` ring and `--shadow-cover`. Playlist
covers keep sharp corners, and the old offset "stacked sleeve" box behind them is gone.
Full-length albums (release type `ALBUM`, `media.isAlbumRelease`) have fully square covers
(radius 0) everywhere they show as albums: cards, the album page header, search rows. EPs,
singles and albums of unknown type keep the card radius.
Hover: the card's own cover colour (`--card-tint`, a registered colour, from
`lib/coverTint.ts` → `analyze_cover`, prefetched as cards scroll into view) spreads from
the cover and blends into whatever the card sits on; the cover lifts 3px with a deeper
shadow; tracks and cards that start playback (`playButton`) show an accent play button at
the cover's bottom-right. **Collection cards** (albums, playlists, mixes, inside a routed
page that provides `lib/cardActions.ts`) split into separate targets: the cover (the tab
stop) and the title open the page, the artist names link to their pages (`ArtistLinks`,
from `artist_refs`; otherwise the plain subtitle), the accent button at the bottom-right is
a real button that plays the collection, and right-click / Shift+F10 opens a menu (Play,
Shuffle, Play next, Add to queue, Add to playlist, Favorite for albums, Go to artist, Copy
link). Tracks, artists and action cards (`playButton`, e.g. Home's radios) stay one
button. Feed releases keep their own card (`FeedTimeline`), whose
hover is still `--surface-2`. Profiles
use Albums' shelf (24px inset, eight columns stepping down by container query, album
tile padding and caption type) and draw each artist as a round portrait; in Rack only
(`appearance.vinylRecords`) it is a **record** (portrait as the label on a vinyl disc;
`--vinyl*` material tokens, black in every theme). Hover only zooms the image inside its circle
and underlines the name like a `NameLink` (no card wash or lift). In name order, letter groups are crate dividers.

**Page-link tiles** (`PageLinkGrid.svelte`: Explore's genres, moods and decades, and the
same rows on linked pages). TIDAL sends these without artwork, so they are not cards: a
wrapping grid (`minmax(176px, 1fr)`, 12px gap) of 2.2:1 tiles. Each tile is a two-stop
OKLCH wash over `--surface-2` inside the lit edge, with the hues hashed from the name
(`lib/pageLinks.ts`, so a genre keeps its colour) and lightness/chroma/alpha from the
`--link-wash-l/-c/-a` tokens (dark default; lighter and paler in light theme; thinner in
Instrument; dim and near-neutral in Rack). A record's edge rises from the bottom-right
corner in the same two hues; on hover the tile lifts 2px and the record shifts in. Names
sit bottom-left at 15/700; a row made only of decades sets them as 30px display numerals,
centred. Every page-link row uses tiles, even when it carries image ids (they are not
square covers).

**Explore header**: the title keeps the page-title step; under it the search field is
the page's centred hero (48px, `min(680px, 100%)`, panel shadow), then one centred row of
pill chips the same width (recent searches with the history icon, top artists from
listening history with the user icon); chips that would wrap are clipped by the row's
fixed height. Untitled quick-link rows (New, Top, HiRes, …) follow "New to you".

**Search results** (`SearchResultsView.svelte`) replace the old all-in-one lists. While a
search shows results, the suggestion chips give way to type chips (All · Tracks · Albums ·
Artists · Playlists, only types that have results; the chosen one is accent-filled). The
**All** overview has three parts. The whole Explore page (title, browse rows and results)
is capped at 1680px and centred, so the title shares the results' left edge; the header's
bottom margin drops to 4px while searching, so the results' own heading margins set the
gap under the chips. A typo that finds only its top hit (no tracks) searches again under
the top hit's name (`searchCorrecting`); a centred 13px muted line under the chips says
"Showing results for **name**", and paging uses the corrected query.
- A **Top result** card (`SearchTopHit.svelte`): TIDAL's `topHit`, or else an artist whose
  name equals the query once case, accents and punctuation are ignored. It sits in a fixed
  380px column, 48px from the tracks, stretched to the height of the track preview, under
  a left-aligned heading like "Tracks". The card is a soft panel (20px padding) washed
  with the cover's own colour (`coverTint`, the accent until it arrives; 30% fading to
  8%, then `--text` at 4%), with a `--text` 4% layer on hover. Its content is centred in
  both axes: the cover (150px round for artists, 136px otherwise) lifts 3px on hover,
  then the display-face title (28px for artists, 24px otherwise, clamped to two lines),
  then a kind pill with the artists and a year, album or track count. The 48px accent
  Play button sits on the cover's bottom-right edge at rest (its glow comes up on hover).
  Play plays an artist's top tracks.
- **Tracks** beside the card: a compact `TrackTable` (`compact`). It has no toolbar,
  header, number or quality column, and no rules between rows. Rows are 10px taller than
  `--row-height`, with an inset cover rounded to 4px. The artists sit under the title;
  title and album share the width 55/45, and the album column drops out (a container
  query) once the list is under 640px. It shows five rows (eight when there is no top
  result). "See all" sits at the row's end before Shuffle and Play all, as in the
  carousel heads. A container query on the overview stacks the card above the tracks
  below 820px. A play from the preview queues every loaded result track.
- One `Carousel` row each for Artists, Albums and Playlists (`MediaCard` carousel cards,
  "See all"), with the top result's own type leading and 24px between rows. The Artists
  row moves artists with no image at all (bare credit-only profiles) to its end.
  Carousels (here, on Home and on Artist) fade the last 72px of a side that has more
  to scroll, and their cards size so a part of the next card always peeks under it.
A single type shows its full table, or a `minmax(168px, 1fr)` tile grid, and pages in
50 more per "Load more" (`search_type`).

### 2.2 Type

- Display: the UI face itself (`--font-display: var(--font-ui)`), set apart only by
  `--display-weight` (750) and `--display-tracking` (−0.015em), so titles follow the
  Typeface pick. No serif and no synthesized italics (Manrope ships upright only).
  The sidebar wordmark is fixed Manrope 800 (+0.01em). Rack uses **Manrope 700** for titles.
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
`--ease-in-out` (two-way glides); linear only for spins. `--rise` (10px, 0 under reduced
motion) is the entrance travel for content arriving in place.

**Arrival rules (page changes).** Content enters once, when it is ready, in page order:
- Route enter: the page rises (8px, `--dur-view`, `--ease-enter`). While a view shows
  only its loading state (`.page > .loading:only-child`) the rise is held and plays when
  the content replaces it, so a spinner never rises in ahead of a page that then snaps
  in. A page with anything beside its spinner (a header) rises at mount instead.
- Explore holds its chips, "New to you" and TIDAL's rows until both its history-based
  parts and the explore rows settle (2.5 s cap), then they rise in together; the title
  and search field stay put.
- `Loading` appears only after 320 ms (then fades in): quick loads show no spinner.
- A page whose data is cached (`lib/viewCache.ts`; revisits, or a card or sidebar link
  hovered before the click) renders complete on its first frame and plays the same rise;
  a background refresh that changes something updates in place.
- Library lists under an already-risen header (Feed, Albums, Playlists, Mixes, Profiles)
  bring their content and header count in together with `.arrive` (app.css: `--rise`,
  `--dur-view`) when they had to load. A count's space is held while loading (no "0"
  flash). Re-sorting Playlists keeps the grid up and FLIP-glides the cards to their new
  places (320 ms) instead of swapping the grid for a spinner.
- Covers do not fade on load. WebView2 defers load events for lazy images, so a cover can
  paint before its `load` fires; a load-driven fade then hides a visible cover and fades
  it back in.
- Independently loaded shelves never insert above content already on screen. Artist
  reveals a section once it and every section above it have settled; Home holds its
  shelves and editorial rows until the personal sources settle (2.5 s cap), then they
  rise in with a 40 ms stagger. Each shelf rises by `--rise`. Each Home shelf sits in
  its own error boundary: revealed together, one throwing shelf would otherwise abort
  the whole reveal.
- Scroll: a new history entry starts at the top; back/forward restores the entry's
  offset once the page is tall enough, abandoned on user input or after 4 s
  (`lib/scrollMemory.ts`). Now Playing covers the routed page (kept mounted, hidden)
  rather than replacing it, so closing it reveals the page as it was.

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
Hover-expand panels (Settings → "Expand panels on hover", `ui.hover_panels`; not in Rack)
never reflow the page, and they stay open while a context menu opened from them is up
(`lib/hoverIntent.svelte.ts`). The collapsed rail opens after a 140 ms dwell and closes
260 ms after the pointer leaves; the rail itself widens over the page to
`--sidebar-expanded-width` (`--dur-state`, `--ease-enter`; 140 ms back on close), its
contents laid out at full width and clipped by the moving edge, and it stops at the player
bar. The closed queue arrives as the floating overlay (`queueFloat`, 320 ms here) when the
pointer rests 100 ms on the window's right 8 px, between the title bar and the player bar,
and leaves 280 ms after the pointer does. Shortcuts: `Ctrl+B` sidebar, `Q` queue.
Panel styles (Settings → "Player bar" / "Queue", `ui.player_bar_style` / `ui.queue_style`,
Docked default / Floating; Atmosphere and Instrument share them, Rack has its own shell).
Floating applies only above 760px; below it the docked bar and the modal queue overlay
return. Both floating panels are inset `--float-gap` (12px), rounded `--radius-sheet`, with
a 1px `--line` edge and `--shadow-panel`:
- the floating bar sits over the main grid track only (it follows the sidebar rail, a
  docked queue and Now Playing). It is 76px with a 60px cover, and `--player-height` (the
  bottom clearance in both styles) becomes 76px + the gap. The seek line runs round the
  card's top corners and tapers to nothing down each side (SeekBar's rim: top border
  only, a track at 35% `--text` (fainter and its thinned corner vanishes) and the accent fill, the fill clipped to the played
  share of the width). The card itself has no top border, so its `--line` sides thin out
  up into the corners where the rim's taper ends, at both ends alike. The
  hover wave covers only the straight edge and is tapered (`SeekBar` `taper`, `wave.ts`
  `WaveTaper`): it lies flat on the bar edge for 28px past each corner, then rises over
  32px. The rim's corners stay on hover (in the wave's `--text-muted`) and cross-fade into
  that flat stretch with staggered 6px fades, so the line runs unbroken from each corner
  into the wave at the same height. At rest it is fully
  transparent: squashed to `scaleY(0)`, its non-scaling stroke would otherwise leave a
  straight line on the edge that the docked bar's flat line hides but the rim does not.
  The sidebar and a docked queue run to the window bottom.
- the floating queue is a non-modal card: no backdrop, no page reflow (no glide), and name
  links leave it open. It stands clear of the caption band and of the player bar in either
  style, fills with `--surface-float`, keeps the width handle, and enters with
  `queueCard` (24px slide + fade, 240 ms; 320 ms for the hover peek, which also takes the
  card form).
`backdrop-filter` surfaces are a fixed set and never nest. Menus mark the selected
option with the accent, not a second background.

Drag and drop (spec `2026-10-22-oxitide-drag-to-playlist-design.md`) has one look in every
mode, with no mode branches:
- **ghost:** a pill fixed under the pointer (offset 14/12 px), the context menu's glass
  (`--surface-float` with 8 % accent, `--glass-blur`). It shows one 34 px cover, or two
  stacked covers with an accent count badge, plus the label and a hint line: accent when a
  drop is accepted, `--text-muted` with the reason when refused. It scales in over
  `--dur-micro`, and appears at once under reduced motion.
- **targets:** a playlist row gets an inset 2 px accent ring on a 14 % accent wash; a grid
  card gets a 3 px accent ring around its cover. A refusing target dims to 55 % and the
  cursor becomes `no-drop`. While an add is running, a spinning disc (accent tick on
  `--line`) sits over the cover.
- **queue:** a 2 px accent insertion line between rows, with the current-track gaps closed
  while a drag is over the rail.
- **in-table reorder:** the existing table drop line.
- **confirm:** a drop of more than one track onto a playlist opens the playlist dialog's
  confirm step: a 16 px lead ("Add 20 tracks?"), a `--surface-2` card with small-caps
  From / To labels over the names, and a footer with an outlined Cancel and the accent
  "Add N tracks" button ("Adding…" while it runs).
- **duplicates:** a lead naming the playlist (accent), an "All" switch, and a scrolling list
  of rows (40 px cover, title, artist) each with a Keep one / Add both pill switch whose
  chosen half is accent-filled. The footer is "Skip duplicates" (outlined) and
  "Add N copies" (accent, disabled with none chosen).
- Long names in both wrap anywhere and clamp to two lines, with the full name on hover;
  track rows ellipsize.
- **selected track rows:** `--selection-bg` (13 % accent). A playing row keeps its stronger
  playing tint.

Save for Later and Collaborators (spec `2026-10-22-oxitide-save-for-later-collaborators-design.md`)
have one look in every mode, with no mode branches:
- **menu entry:** "Save for later" (`bookmark`) / "Remove from Save for later"
  (`bookmark-minus`), right after Favorite. The album, artist and mix headers carry a
  `DetailAction` ("Save" / "Saved", active when saved).
- **page:** Collection Albums' grid and insets. The type chips are 34 px pills
  (`--surface-1`, 1 px `--line`), and the chosen one is filled with `--accent` /
  `--on-accent`. Track cards carry a small uppercase "Track" tag (`--surface-3`) over the
  cover. Artists read from their round covers, playlists from their collages, and mixes
  from their usual mix tile (the Mixes & Radio chip comes after Playlists). A footnote
  counts saved videos, which are not shown.
- **Collaborators dialog:** the shared `Modal`. A full-width "Copy invitation link" row
  sits on a 40 px `--surface-3` circular icon well (a spinning accent tick while it
  works). Member rows have a 40 px round avatar (the picture, else initials on
  `--surface-3`), with "Owner" muted under the owner. A hairline footer holds the 7-day
  note and a text-button Reset link, whose confirm happens in place (Cancel / Reset link
  in `--danger`), with no second modal.
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
| Drag and drop | `ui/src/lib/dragDrop.svelte.ts` (controller, `dragSource` / `dropZone`), `ui/src/lib/components/DragGhost.svelte`, `ui/src/lib/playlistDrop.ts` |
| Save for Later / Collaborators | `ui/src/views/SaveForLater.svelte`, `ui/src/lib/{later.svelte,saveForLater}.ts`, `ui/src/lib/components/CollaboratorsDialog.svelte` (+ `lib/collaborators.svelte.ts`) |
| Rack reference mockup | `ui/design/rack/reference-mockup.html` (open via any static server at the repo root) |
| Phone remote | `ui/remote.html`, `ui/src/remote/` (styles: `remote/remote.css` + component styles) |
| Easter eggs | Oneko + kuroneko: `ui/src/lib/components/Neko.svelte`. Classic gif colours; kuroneko is the same sprite under `filter: invert(1)`; petting hearts are an 8×6 pixel SVG in `--accent` that float up and fade over 900 ms, at most 6 at once, `z-index: 8` like the cats. Jamming cat: `CatJam.svelte`, a 32 px pixel sprite in the player bar's left zone between the track title and the heart, at 100–200 % (Settings → Extras → Size, default 125 %; `pointer-events: none`). It has six coats (`lib/catjamSprite.ts`, chosen in Settings → Extras with `settings/CoatPicker.svelte`). None of them branch on mode; all are hidden under reduced motion. |

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