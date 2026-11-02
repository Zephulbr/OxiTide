# OxiTide showcase site: plan

Status (2026-11-02): the page is the "Tide" redesign, ported here from its prototype (local only; deleted once ported, 2026-11-02). It replaces the v1 page built on 2026-10-29. v1 is archived locally in `.superpowers/site-v1` (gitignored; `site/` had never been committed). Published 2026-11-02 at <https://zephulbr.github.io/OxiTide/> (GitHub Pages, `.github/workflows/pages.yml`).

## Goal

A single static page that showcases OxiTide: what it looks like, what it adds on top of TIDAL, and where to download it. It should feel like the app, a music player, not a generic landing page.

Non-goals:
- No accounts, analytics, comments, or backend.
- No blog, docs site, or multi-page structure. The README and `docs/` stay the documentation.
- No live copy of the app running in the page. Playback, meters, and DiscArt are too heavy and need a TIDAL login.
- No new brand or logo work.
- No showcase of standard TIDAL features (Home, Explore, search, playlists, Save for Later, collaborators, basic queue). The page covers what OxiTide adds.

## Where it lives

- Folder: `site/` in this repo. It is isolated from `ui/` (the Tauri frontend, with its own CSP and build) and from `docs/` (specs and plans).
- Deployment: GitHub Pages from a workflow that publishes `site/`. Not set up until the page is ready to publish.
- Downloads link to the GitHub Releases page that `release.yml` produces. The site never hosts binaries.

## Tech

- Plain static HTML, CSS, and two scripts (`scripts/main.js`, classic; `scripts/looks.js`, a module). No framework and no build step. `README.md` lists the files.
- No token snapshot any more: the page's palette is its own (the rooms in `styles/tide.css`), and the look replicas are hand-coloured. Changing the app's tokens cannot break the site.
- Fonts: copies of the app's vendored Manrope, Inter and DSEG7. Their licences stay covered by `THIRD_PARTY_NOTICES.md`.

## Design ("Tide", owner picks 2026-10-30 to 2026-11-02)

- Dark only.
- Rooms: each section has the colours of a cover "playing" for it. As you scroll, the backdrop and the player bar glide between them, slowly and eased, with a short settle before a switch. The sections' own accents stay in their room. Hero and install: the OxiTide logo's room. Looks: Slack Water (teal), turning to Sodium Lights (amber) at the first look change. Sound: Sodium Lights. Phone remote: Field Notes (green). And More: Night Ferry (purple).
- Background: the room's cover, blurred and toned down, with a glow; texture is a barely visible grain.
- The bar is the header: there are no in-page headings. One sentence is set into each section's visual, and the bottom player bar announces each section like a track change. The bar starts as a now-playing pill in the hero and grows into a full player bar (record, section title, transport, section queue, Download). Its segmented progress bar seeks through the page.
- Cover art is original placeholder art. Never use TIDAL album covers or artist images.
- Visually hidden `h2`s give screen readers one heading per section (its bar title), and each section is a named region.

## Page structure

0. **Hero**: "Now playing · Unofficial TIDAL desktop client", the name, app-style badges (Bit-perfect, Fast, Three looks, Phone remote, No telemetry), download rows for Windows and Linux (experimental), "Needs a paid TIDAL plan". The logo is a record rising over water, with its reflection.
1. **Three looks**: one large frame that morphs Atmosphere → Studio → Hi-Fi Rack as you scroll, with the look's name above it. The cover and title travel between their places under a veil while the window swaps. The Rack's meters are v1's drawn VU faces.
2. **Sound**: the crossfade styles (Classic, Quick, Blend, Auto) as an album page's tracks. Beside them, the app's "Last transition" readout is printed on a record sleeve, with the style's record half out. The record swaps when the style changes, and comes further out on hover. Exclusive output, ASIO and AutoMix (experimental) follow as small tags. EQ and ReplayGain are no longer shown (owner pick, 2026-11-02).
3. **Phone remote**: a desktop and a phone side by side, acting out play/pause, volume, skip and like. Chips run each step. Scrolling on, the computer's screen grows into the next section.
4. **And More**: a lyrics-view mockup, one screen tall, for the song "The Small Things". Each line is a small feature: network resilience, synced lyrics (LRCLIB), scrobbling, global hotkeys, mini player, close to tray, taskbar controls, history and stats, idle when paused, no telemetry. Each line acts itself out on the mockup, and the mockup works as a small player.
5. **Install**: one "Download OxiTide" row that goes back up to the hero's download rows.
6. **Footer** ("liner notes"): the app mark and one line, then Project and Notes link columns, then the small print (not affiliated with TIDAL, paid plan needed, version).

## Accessibility and performance

- Semantic HTML, one `h1`, logical heading order, keyboard-reachable controls (real buttons), visible focus styles.
- Mockups are decorative (`aria-hidden`) where the page text already says what they show.
- Reduced motion: the demos don't advance on their own, the records don't turn, and the looks change with a cut.
- Smoothness: the owner asked for "buttery smooth". Frame times are measured with rAF deltas in Chrome at 144 Hz (dev machine). The demos run at about 6.9 ms median and 7.1 ms p95, with no frame over 12 ms. A scripted top-to-bottom scroll (14 s) runs at about 142 fps, median 6.9 ms, p95 7.1 ms, with one or two frames over 20 ms (worst 21–28 ms).
  - Room changes (fixed 2026-11-02): each used to drop two to four frames (up to 49 ms). A trace showed whole-page style recalcs: about 12 ms whenever any variable on `:root` changed, about 20 ms more to switch every element's transitions on, and 16 ms to switch them off again. Now each section keeps its own room and only the `[data-follow]` parts change (`styles/tide.css`). A room change costs at most one frame of about 14 ms, from the GPU starting the new blurred backdrop.
  - Still at about 14 ms a frame for a moment (one missed 144 Hz refresh, well within 60 Hz): the morph from Studio to Rack, and the Remote screen growing into And More.
  The rules that keep it smooth are in `README.md`.
- Lighthouse (local server, 2026-11-02), desktop and mobile: Accessibility 100, Best Practices 100, SEO 100. Re-run on the published URL.

## Legal and wording

- "Unofficial TIDAL desktop client" appears in the hero, and "Not affiliated with TIDAL" in the footer.
- Do not use TIDAL's logo or name in a way that suggests endorsement. Use the name only to describe compatibility.
- Licence: AGPL-3.0-or-later, linked from the footer.
- Last.fm and ListenBrainz: scrobbling is described as working (OxiTide ships its own Last.fm key since `9f02491`; connecting happens in the browser).

## Milestones

1. **Spike** (v1): done, see "v1 history".
2. **Skeleton** (v1): done.
3. **Feature sections**: done in v1, then redesigned as Tide (prototyped 2026-10-30 to 2026-11-02, ported 2026-11-02).
4. **Polish**: done: phone layouts, reduced motion, the a11y pass, meta tags and favicon, link check. Open: a copy review, a pass on a real mid-range laptop plus Safari and Firefox, and Lighthouse on the published URL.
5. **Publish**: done 2026-11-02 on GitHub Pages (`og:url` and a canonical link set). Open: an `og:image`, Lighthouse on the live URL, a custom domain if wanted, and a link from the README.

## Open decisions

- **Codeberg link:** the footer's "Mirror on Codeberg" goes to `codeberg.org/Zephulbr/OxiTide`, which returned 404 to visitors on 2026-11-02 (the user page exists, so the repo is private or unpublished). Make it public before publishing, or drop the link.
- **Unsigned installer:** v1's install section said the Windows installer is unsigned and pointed to `SHA256SUMS.txt` on the release page. The Tide page doesn't (the install cards with the checksum were turned down on 2026-10-31). Decide whether a line about it belongs near the download rows.
- **Looks replicas:** the looks chapter uses simplified replicas, and its morph runs once per frame on scroll. v1 had full-detail replicas on a scroll timeline (compositor-driven); its `scripts/main.js` (`initMorph`), `styles/mockups.css` and token snapshot are in the local archive. Both measure smooth on the dev machine. Swapping v1's in is possible if the simplified ones look too plain.
- **Offline wording:** the copy describes the cache as a buffer for short connection issues, not offline playback (owner decision 2026-10-29). `AGENTS.md` describes `DiskCache::find_offline` and offline startup restore, and `docs/privacy.md` says "not an offline mode". Reconcile the code docs with the owner's position before publish.
- **Performance numbers:** the page states no figures. Add some only once the final release perf run completes.
- **Custom domain:** wanted, or is the default `github.io` address fine?
- **README link:** link the site from the README once published (proposed: yes).
- **Plan location:** this file lives in `site/` as requested. `AGENTS.md` says feature plans go in `docs/superpowers/plans/`; a dated copy can be added there.

## v1 history (2026-10-29)

v1 opened in Atmosphere and used the app's design tokens (a snapshot of `ui/src/styles/`). Its signature was one fixed "device" frame that changed look as the reader scrolled (Atmosphere → Studio → Rack → Atmosphere).

The spike tried the literal morph first: matching regions (cover, title, transport, progress, volume) moved between slots with FLIP transforms on a `ViewTimeline`, and the rest crossfaded. Decision: literal morph. Measurements (Chrome 156, 144 Hz, a scripted 6 s scroll):

| Run | Morph chapter | Plain section |
|---|---|---|
| No throttling | 143.0 fps, p95 7.2 ms, 2 frames > 20 ms (worst 34.8 ms) | 140.7 fps, p95 7.1 ms, 1 frame > 20 ms (worst 138.7 ms) |
| 4× CPU throttling (mid-range stand-in) | 144 fps, p95 7.1 ms, worst 7.4 ms | 144 fps, p95 7.1 ms, worst 7.5 ms |
| 4× throttling, JS fallback (`ViewTimeline` removed) | 143.8 fps, p95 7.1 ms, worst 13.9 ms | n/a |

The owner then found v1's look too spec-sheet (the same split layout repeated), which led to Tide. Carried over from v1: the hero's spinning record, the three-looks morph (veil plus travelling cover and title), the crossfade-styles visual, and the Rack's VU faces.
