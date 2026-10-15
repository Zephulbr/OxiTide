# OxiTide — Home redesign (2026-10-15)

- **Status:** Shipped (owner-reviewed in the running app, iteratively, 2026-10-15)
- **Owner:** V
- **Scope:** Atmosphere first. The page renders in every mode through the shared components.
  Instrument, Rack and the light theme are not yet visually checked.
- **Commits:** `5193850` (Home + card polish), `0f489c7` (queue glide), `28f32f6` (toast
  offset).
- **Rules:** Atmosphere addendum AR-9 (queue glide v2) and AR-10 (card hover and resting
  depth), recorded in `2026-10-15-oxitide-atmosphere-rules-addendum.md`.
- **Living description:** `ui/design/atmosphere/DESIGN.md` §3 (Home, queue rail) and §4
  (resting depth, card hover); shared parts in `ui/design/DESIGN.md` §2.1 and §2.3.

## 1. Problem

Home was a "Quick access" strip of owned playlists over TIDAL's `pages/home` rows. Those
rows are editorial and generic ("The Hits", "New Tracks"). The page looked bland and
empty and said nothing about the listener, though the app already held session restore,
local history and stats, the follow feed, For You mixes and radio.

## 2. Decisions

| # | Decision |
|---|---|
| HR-D1 | Lead with the listener: a greeting and a "Continue listening" hero, then personal shelves. TIDAL's editorial rows come last, capped to 4, under a "From TIDAL" eyebrow. |
| HR-D2 | UI-only. Every shelf uses existing commands; no new Tauri command or Rust change. |
| HR-D3 | Every shelf loads on its own (`Promise.allSettled`) and hides when its source is empty or fails, so a new account or a flaky endpoint never blanks Home. |
| HR-D4 | History records carry names only. For each distinct recent album, one representative track is resolved through `getTrack` to real album/artist ids, cached for the session (`lib/homeSuggest.ts`). Those ids drive the album, radio and similar-artist shelves. A failed lookup drops only that album. |
| HR-D5 | Shelves take deliberately different shapes so the page has rhythm (compact rows, small cards, standard carousel, wide radio cards, round artists, a stats panel). A "feature" layout for Made for you was tried and reverted as unbalanced. |
| HR-D6 | The hero keeps the cover tint, toned to 22% → 7% accent. It stands off the backdrop through a drop shadow and an accent-tinted edge, not brightness. |
| HR-D7 | Containers use `.lit-tint` (accent wash plus even accent hairline) instead of `.lit`'s bright-to-dark rim, plus `--shadow-panel`. |
| HR-D8 | Jump back in is exactly one row. A CSS container query hides cards that would wrap. A ResizeObserver count was tried and replaced because it lagged the queue glide by a frame (AR-9). |

## 3. Page structure (top to bottom)

1. **Greeting:** Fraunces, by local hour (morning / afternoon / evening / night).
2. **Hero** (`HomeHero.svelte`), shown when there is a current or restored track:
   - cover lit per AR-3, eyebrow ("Now playing" / "Continue listening"), Fraunces title,
     italic byline, progress line, Resume/Pause pill (AR-1 transport glow), Next;
   - right side: the next four queued tracks with "+N more", set off by a full-height
     fading hairline, with hairlines between rows. Clicking a row jumps the queue there.
     Hidden below 1100px.
3. **Jump back in:** owned playlists ordered by last played (`playlistRecency`), then
   saved mixes. Cap 8, one row.
4. **Recently played:** distinct albums from the last 60 plays, as `mini` cards; opens
   the album.
5. **Made for you:** the mix cards of `pages/for_you`, or its first row when it has no
   mixes. Carousel; "See all" goes to Mixes.
6. **New from artists you follow:** Feed releases, with the unseen count in the title;
   "Open feed".
7. **Keep exploring:** wide radio cards (92px art, `playButton`) for the top 3 recent
   artists by plays, plus the last played track. They start radio through `lib/radio.ts`.
8. **Because you listen to X:** similar artists of the top recent artist, without the
   seeds themselves.
9. **Your listening** (`HomeListening.svelte`): the 7-day total with a bar strip, top 3
   artists, top 3 tracks (click to play), and "All stats". Shown only with plays.
10. **From TIDAL:** the first 4 non-empty `pages/home` rows through `PageSections`.

Data shaping: `lib/homeData.ts` (greeting, mixes, feed cards, unseen count, editorial
cap, last days) and `lib/homeSuggest.ts` (album grouping, id resolution, artist
ranking, radio seeds, similar artists). Both are pure and unit-tested.

## 4. Shared changes that came with it

- **Media cards** (`MediaCard.svelte`, used across the app):
  - cover ring plus `--shadow-cover`; the offset "stacked sleeve" box behind playlist
    covers is retired;
  - the cover-colour hover tint: `lib/coverTint.ts` calls `analyze_cover`, prefetched as
    cards scroll into view, 3 at a time, with hover jumping the queue. Collage playlists
    use their first collage cover;
  - a 3px lift with a deeper shadow, an accent play button, and a new `mini` variant.
- **Tokens:** `--shadow-panel` and `--shadow-cover` (softer warm-ink in light), the
  registered `--card-tint`, and the `.lit-tint` class.
- **Queue glide** (`lib/queueTransition.ts`, AR-9): 380 ms `--ease-in-out`, with the page
  held at its final width and FLIP-animated in step with the rail.
- **Toasts** sit below the custom title bar (`top: calc(var(--titlebar-h) + 8px)`).

## 5. Known limits and follow-ups

- Instrument, Rack and light-theme visual checks are pending.
- Cards with full-URL artwork (some mix images) cannot be palette-analysed and keep the
  accent tint.
- Elements hidden or added by JS a frame after a width change still pop during the queue
  glide (AR-9: width-driven layout must be CSS).
- AR-5's pending perf check (queue toggle over a 10k-row table, S6 conditions) now
  includes the FLIP pass.
- Scrobble/stats-driven suggestions beyond similar artists (e.g. "More like <album>")
  were not attempted.
