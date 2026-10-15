# Atmosphere — design file

Shared foundation: [../DESIGN.md](../DESIGN.md). Decision record:
`docs/superpowers/specs/2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md`
(+ convergence `2026-09-25-oxitide-2b-visual-convergence.md`, light theme
`2026-10-05-track9-ui-completion-design.md` §6.1, rules addendum
`2026-10-15-oxitide-atmosphere-rules-addendum.md` — AR-1…AR-10, wins over the record;
Home redesign `2026-10-15-oxitide-home-redesign-design.md`).

## 1. Personality

*Immersive, warm-dark, unhurried, premium.* A listening room at night — the record
sleeve is the lamp.

1. **Music is the light source.** Artwork illuminates the UI; chrome never competes with
   it for saturation or brightness.
2. **Glass, not walls.** Translucent layers over the artwork; depth from blur and light.
3. **Motion answers the music.** Ambient motion only while playing; pause stills the room.
4. **Chrome is quiet until needed.**

**Identity budget:** exactly four signature places — the Fraunces display face, lit edges
+ film grain, the tide motif (waveform seek + the disc: loading spinner, Now Playing disc
art, now-playing mark — AR-4), artwork reactivity. Do not add a fifth.

## 2. Tokens (deltas over the foundation)

| Token | Dark | Light |
|---|---|---|
| `--bg` | `#17120e` (Warm Graphite) | `#f6f1ea` (warm paper) |
| `--surface-1/2/3` | warm white alphas .05/.07/.09 | warm ink alphas .045/.065/.085 |
| `--surface-float` | `rgba(23,18,14,.82)` | `rgba(250,246,240,.94)` |
| `--glass-blur` / `--overlay-blur` | 18px / 8px | same |
| `--accent` | `--dyn-primary` (cover) | `--dyn-primary` (light-room extraction) |
| flags | backdrop art, glass blur, grain, disc spin, wave seek, dynamic colour, ambient motion: **on** | same |

Dynamic colour: three roles (Primary, Secondary, Tertiary) + backdrop wash, guarded by a
contrast chain (structural scrim minimums + per-cover composite-ceiling clamping).
`Auto` = Full here.

## 3. Layout

- Shell "Refined Classic": sidebar 248px ↔ 64px rail (auto below 1080px), main, queue
  rail 260–420px (overlay below 760px), floating 80px glass player bar with the seek as
  its top edge.
- Now Playing "Editorial Split": artwork left (Disc default / Sleeve), metadata column
  right (eyebrow, Fraunces title, italic byline, badges, tabs Lyrics / Next / Info). The
  sidebar steps aside; the queue rail can stay docked beside it (Now Playing narrows).
- Queue rail (`QueueRail.svelte`, shared with Mini and the Rack drawers): header with a
  Fraunces "Queue" title, "N up next · 1 h 32 min left", locate-current and clear icon
  buttons (circle-dot = "go to current", shared with lyrics). Full-bleed rows (hover and
  the current tint run edge to edge) 58px / 48px (compact): cover, title over muted
  artist, duration on the right (swapped for × on hover/focus); a grip fades in at the
  left edge on hover. The
  current row takes the 18% accent tint and accent title, and its cover + text glide 34px
  right (transform, `--dur-view`) to uncover the now-playing mark beside the cover, which
  rolls in a beat later (scale + quarter-turn, 320 ms); 28px of air sits above and below
  it (gaps close during a drag); played rows sit at 50%. The docked rail glides open/closed by width (380 ms, `--ease-in-out`
  curve, rows clipped, not reflowed; the routed page is held at its final width for the
  glide — `lib/queueTransition.ts` — so it lays out once instead of every frame, and that
  one change is FLIP-animated in step with the rail: on-screen elements travel from their
  old to their new position, and cards/panels with a visible box also animate their width
  so their edges glide; every on-screen element is considered (not a fixed list): elements that
  appear fade in in place, and elements that drop out (e.g. a Jump back in card that no
  longer fits, anything a CSS width rule hides, or carousel cards that end up clipped by
  their narrower row — that copy is trimmed to what was on screen) leave an inert copy at their old spot
  that fades out over the first 60% of the glide; the opening glide is parked on its first frame and started by the
  rail's `introstart`, so both begin on the same frame); the floating one slides in from the right. It opens scrolled to the current track and follows track
  changes only while the old current row was in view. Drag is pointer-based: the row
  lifts into a lit-edge float-veil ghost (no blur: no nested glass, AR-6) that follows the pointer, rows between glide apart, and the
  landing slot shows as a dashed accent outline; the list auto-scrolls near its edges and
  Escape cancels. Keyboard: ↑/↓ move between rows, Alt+↑/↓ reorder, Delete removes.
- Track rows 46px (Comfortable) / 38px (Compact); cover thumbs in mixed lists; playing
  row = 18% accent tint + accent title, and its number gives way to the now-playing mark;
  the current cover may carry a thin accent ring / glow (AR-3); no left bars, no animated
  equalizers.
- Now-playing mark (`NowPlayingMark.svelte`): a glyph-size accent record (grooves, centre
  label, one white sheen arc) — the disc motif, not a new element. It spins at one turn
  per 2 s while playback runs and eases to a stop on pause; still without ambient motion.
  Used on the playing track-table row (# column), beside the queue's current cover, and
  over the sidebar's playing playlist cover (on a 55% `--bg` scrim).
- Home (`views/Home.svelte`): a Fraunces time-of-day greeting over a "Continue listening"
  hero (`HomeHero.svelte` — current or restored track; cover-tinted card with a drop
  shadow, the cover lit per AR-3, progress line, Resume/Pause pill with the
  primary-transport glow, Next, and an "Up next" list of the next four queued tracks on the right, set off by a
  full-height hairline with hairline rules between rows; hidden below 1100px),
  then personal shelves, each hidden when its source is empty or fails, deliberately in
  different shapes so the page has rhythm: Jump back in (compact rows, one line — only as many
  cards as fit across, decided by a CSS container query so it never lags a layout change: playlists by last played + saved mixes), Recently played (albums
  from history, `mini` cards), Made for you (For You mixes, standard carousel), New from artists you follow (Feed, with the unseen
  count), Keep exploring (wide lit radio cards with 92px art, for your most-played recent
  artists plus the last track), Because you listen to X (similar artists of the top recent artist), Your listening (`HomeListening.svelte`: 7-day total + bars, top artists and
  tracks, from local history), and last a capped "From TIDAL" block of the editorial
  `pages/home` rows. Data shaping lives in `lib/homeData.ts`; the history-driven shelves in
  `lib/homeSuggest.ts` (history stores names only, so one track per recent album is
  resolved through `getTrack`, cached for the session).
- List views follow the shared page header ([../DESIGN.md](../DESIGN.md) §2.1). History is
  a play log (clock time gutter · cover · title/artist · album · length; day headers with
  plays and listening time; partial plays at 60%). Stats: three lit summary cards
  (Fraunces value, "23 h 43 min" — never clock notation), a lit "Plays per day" panel
  (accent gradient bars, faint gridlines, date axis) and ranked top lists with a 7% accent
  share bar per row. Feed: date groups on the timeline rail, each a grid of release
  covers; unseen releases in "New updates" get an accent "New" pill.

## 4. Components

Radii: controls 8 · cards 12 · sheets 16 · play button round. Hover = surface brighten
(media cards: cover-colour tint, below);
press = scale .98; disabled 40%. Buttons: primary accent fill, ghost glass, danger red
ghost. Badges: bordered chips (MAX Tertiary-outlined). Toasts: glass + severity glyph,
slide-in with slight overshoot. Loading: disc spinner. Scrollbar: 8px warm translucent
thumb.

**Resting depth:** plain dark drop shadows — `--shadow-panel` (Home's containers: hero,
Jump back in, radio cards, listening panel) and `--shadow-cover` (every `MediaCard`
cover, with a 1px `--line` ring; this replaced the offset "stacked sleeve" ghost behind
playlist covers). Shadows are neutral, never accent-coloured, so they are not glows
(AR-1); light theme uses softer warm-ink shadows (`themes.css`). Home's containers use
`.lit-tint` (app.css) instead of `.lit`: a faint accent wash and an even accent hairline
rather than the bright-to-dark key-light rim.

**Card hover (`MediaCard`):** the card's own cover colour (`analyze_cover`, prefetched as
the card scrolls into view — three at a time — with hover/focus jumping the queue; cached;
accent until it arrives or for URL artwork; the registered `--card-tint` glides between
the two instead of snapping) spreads from the cover —
from the left on row cards, from the top on stacked ones — and fades out before the far
side, blending with the accent wash the card already carries; the cover lifts 3px with a
deeper shadow layer fading in, and the cover image still zooms 3%; an accent play button
(40px, 30px on rows, with the AR-1 transport glow) rises from the cover's bottom-right
corner on tracks and on cards that start playback (`playButton`, e.g. Home's radios).
Collage playlists (no artwork of their own) take the tint from the collage's first cover.
Only transform and opacity animate, plus the registered `--card-tint` colour (AR-10).

**Micro-interactions (AR-2):** toast overshoot; heart pop on favourite; player-bar icon
buttons lift 1px on hover and squash to 0.92 on press. Nothing breathes or idles.

**Glow (AR-1):** emitted accent light only — play button, the current track (cover,
mark), focus/active fields, slider thumbs in use, destructive confirm on hover, a soft
halo behind the Now Playing cover. Never on resting chrome (cards, panels, sidebar,
headers, nav, badges, toasts); never stacked; never animated on its own. Small controls
≤ ~10px blur; ambient halos ≤ ~35% alpha so the art stays the brightest thing.

## 5. Motion

Backdrop + dynamic-colour crossfade 1200 ms; disc spin ~12 s/rev (now-playing mark
2 s/rev), eases to stop in ~600 ms on pause; queue dock width glide 380 ms on `--ease-in-out`
(AR-5 as amended by AR-9; the page's card widths follow it for the length of the glide —
see §3); everything else per foundation §2.3.
