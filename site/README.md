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
| `assets/covers/` | Original placeholder cover art. Never use TIDAL artwork here. `soft/` holds the backdrop covers pre-blurred for phones. |
| `assets/og-image.png` | The share image (1200 × 630) that link previews show: the logo, the name, and the badges, rendered from the page's own fonts and logo. |
| `favicon.svg` | A copy of `crates/oxitide-app/icons/svg/icon-64.svg`. |

## Rules that keep it smooth

The page is tuned to hold the display's full frame rate (144 Hz on the dev machine) while scrolling and in every demo (numbers in PLAN.md). When changing it:

- Never change a custom property on `:root` while the page runs: even an unused one restyles all ~1,400 elements (about 12 ms). Each section keeps its own room (`data-room`). A room change sets `data-room` only on the parts marked `data-follow` (the player bar, the looks' dots and glow), and while `[data-gliding]` is on them their colours ease over. The backdrop crossfades one stack per room (opacity only). Anything new that should follow the room gets `data-follow`, not a rule on `:root`.
- Never write transition rules for every element (`*`): switching them on and off restyles the whole page.
- Anything coloured by a gradient or a box-shadow snaps on a room change. Give it fixed colours (a cover, the crossfade sleeve), scope it to one room with `[data-bd]`, or fade an old-colour layer out over it.
- Animate `transform` and `opacity` only, never `height` or `width` (the equaliser bars once laid out and repainted the page every frame). Move a line by translating a full-width layer, not by animating `left`. Keep a translate's distance a length (`px`, `cqw`), not a percentage of the element's own box: Chrome won't run that on the compositor.
- Nothing runs forever unseen. A section off screen gets `[data-off]`, which rests its records, bars and playhead (add any new endless animation to that rule in `site.css`). Demos and timers start and stop on an `IntersectionObserver`, and a frame loop stops while its demo is paused: an idle `requestAnimationFrame` loop still makes the page render every frame. The water's SVG ripple pauses when the hero is out of view.
- Don't read `innerHeight` or `scrollY` after writing styles in a frame (each forces a style and layout pass). `main.js` keeps the viewport height in `geo.vh`.
- Re-measure on a `ResizeObserver` on the element that matters, not on window `resize`. A phone's toolbar fires window resizes mid-scroll.
- Phones and tablets (`(hover: none) and (pointer: coarse)`, at the end of `site.css`) get the light set: the backdrop is a pre-blurred image from `assets/covers/soft/`, standing still (no live blur, no drift); the glow blends normally and the overlay grain is off; frosted glass is a near-solid tint; the reflection is blurred, not rippled; the mockups' blurs are halved. A new cover for the backdrop needs its soft copy too (a 256 px WebP with `blur(14.6px) saturate(1.5) brightness(.62) contrast(.9)` baked in, edges mirrored before blurring). Desktop keeps the full set.
- Phones (`max-width: 640px`) get tighter section spacing. There, the looks' frame pins at its own height (`--lk-h`, which `main.js` sets on `.lk-stick`) instead of in a full-screen box, and each section's content starts 64 px pulled up toward the one before and settles as it rises (`pull-up`, a `view()` timeline, transform only). A new section's `.stage` gets the pull from that rule.
- Scroll-linked motion uses a scroll timeline where it can (the Remote to And More hand-off). Otherwise it runs once per frame, only while near the screen. Cache layout reads.
- Respect `prefers-reduced-motion`: demos don't advance on their own and records don't turn.

## Deploying

Published at <https://zephulbr.github.io/OxiTide/> by `.github/workflows/pages.yml`. It runs on every push to `main` that touches `site/`, and can also be started by hand. It publishes this folder as is, without its `.md` notes, and stamps the commit onto the style and script links in `index.html` (`?v=…`). Pages lets browsers keep files for 10 minutes, and without the stamp a reload showed the new page with old CSS. Every download link points at GitHub Releases. The site never hosts binaries. The share image (`assets/og-image.png`) is referenced by absolute URL in `og:image`, so a moved page needs its tags updated too.
