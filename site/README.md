# OxiTide showcase site

A single static page: plain HTML, CSS and two scripts, with no build step. The plan, the design decisions and what is still open are in [`PLAN.md`](PLAN.md).

The design is "Tide" (2026-11-02). It is dark only. As you scroll, the page's colours glide from one "room" to the next, as if a different cover were playing for each section. There is no header: the player bar at the bottom of the screen announces each section, like a track change.

## Preview

Serve this folder with any static server and open it, for example:

```bash
python -m http.server 8137 --directory site
```

Opening `index.html` straight from disk mostly works, but browsers block module scripts on `file://`, so the looks chapter stays still.

## Layout

| Path | What it is |
|---|---|
| `index.html` | The page. Icons are an inline sprite of Lucide paths, copied from `ui/src/lib/components/Icon.svelte`. |
| `styles/base.css` | Fonts, resets, the cover art classes (`.art-*`), and the spinning record. |
| `styles/tide.css` | The rooms: each section's colours (`--room`, `--glow`, `--acc`, `--tert`), and the glide timing. |
| `styles/site.css` | The page itself: backdrop, hero, sections, player bar, footer. |
| `styles/looks.css` | The simplified app replicas in the looks chapter (Atmosphere, Studio, Hi-Fi Rack), sized to their frame. |
| `scripts/main.js` | Rooms and the colour glide, the player bar (announcements, section queue, seekable progress), and each section's demo: the phone remote, the crossfade record, the lyrics player. |
| `scripts/looks.js` | Builds the three replicas and morphs between them as the looks chapter scrolls by. |
| `assets/fonts/` | Copies of the app's vendored Manrope, Inter and DSEG7 (OFL 1.1, see `THIRD_PARTY_NOTICES.md`). |
| `assets/covers/` | Original placeholder cover art. Never use TIDAL artwork here. |
| `assets/og-image.png` | The share image (1200 × 630) that link previews show: the logo, the name, and the badges, rendered from the page's own fonts and logo. |
| `favicon.svg` | A copy of `crates/oxitide-app/icons/svg/icon-64.svg`. |

## Rules that keep it smooth

The page is tuned to hold the display's full frame rate (144 Hz on the dev machine) while scrolling and in every demo (numbers in PLAN.md). When changing it:

- Never change a custom property on `:root` while the page runs: even an unused one restyles all ~1,400 elements (about 12 ms). Each section keeps its own room (`data-room`). A room change sets `data-room` only on the parts marked `data-follow` (the player bar, the looks' dots and glow), and while `[data-gliding]` is on them their colours ease over. The backdrop crossfades one stack per room (opacity only). Anything new that should follow the room gets `data-follow`, not a rule on `:root`.
- Never write transition rules for every element (`*`): switching them on and off restyles the whole page.
- Anything coloured by a gradient or a box-shadow snaps on a room change. Give it fixed colours (a cover, the crossfade sleeve), scope it to one room with `[data-bd]`, or fade an old-colour layer out over it.
- Animate `transform` and `opacity` only. Move a line by translating a full-width layer, not by animating `left`.
- Scroll-linked motion uses a scroll timeline where it can (the Remote to And More hand-off). Otherwise it runs once per frame, only while near the screen. Cache layout reads.
- Respect `prefers-reduced-motion`: demos don't advance on their own and records don't turn.

## Deploying

Published at <https://zephulbr.github.io/OxiTide/> by `.github/workflows/pages.yml`. It runs on every push to `main` that touches `site/`, and can also be started by hand. It publishes this folder as is, without its `.md` notes. Every download link points at GitHub Releases. The site never hosts binaries. The share image (`assets/og-image.png`) is referenced by absolute URL in `og:image`, so a moved page needs its tags updated too.
