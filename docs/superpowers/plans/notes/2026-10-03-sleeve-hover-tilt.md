# Now Playing sleeve hover tilt + glare — tuning reference

- **Date:** 2026-10-03
- **Commit:** `29ee020` — all tuning below is in that commit; only this note is uncommitted at time of writing. Files: `ui/src/views/NowPlaying.svelte`, `ui/src/lib/tilt.ts` + `tilt.test.ts`, `ui/src/styles/tokens.css`, `ui/src/styles/modes.css`.
- **Scope:** Now Playing, sleeve (square) cover only. The disc shape, mini player, and queue thumbs are unaffected.

## Behavior

Hovering the square sleeve tilts it in perspective toward the cursor with a soft specular glare mirrored through the center; on leave it settles flat. The edge under the cursor tips away and the opposite edge comes toward the viewer; the glare sits on that near side (consistent with a key light at the viewer). Click still closes Now Playing; press dips to 0.99×. Mouse only; fully disabled under reduced motion (JS early-return plus a `prefers-reduced-motion` stylesheet backstop).

## Tuning knobs

Line numbers are as of `29ee020`.

| Knob | Current | Lives at | Effect / how to tune |
| --- | --- | --- | --- |
| `MAX_TILT` | `8` deg | `NowPlaying.svelte:84` | Peak rotation. Radial-capped, so corners never exceed it. |
| `ENTER_MS` | `480` ms | `NowPlaying.svelte:87` | First-contact window: the entry eases on the slow return curve, then tracking hands over to the tight follow. |
| `--tilt-return` | `480ms` | `NowPlaying.svelte:433` | Shared entry/leave duration (also the glare fade + glide). |
| Entry/leave curve | `var(--ease-in)` | `.art-sleeve`, `.art-sleeve.entering`, `.art-glare`, `.art-sleeve.entering .art-glare` | Fast-start, decelerating settle (the token closest to the reference's leave curve). |
| Tracking | `var(--dur-micro)` = 120ms, linear | `.art-sleeve.hovering`, `.art-sleeve.hovering .art-glare` | Tight follow while the pointer moves. |
| `perspective` | `1400px` | `.art-stack` (`NowPlaying.svelte:381`) | Lower (e.g. 800px) = stronger depth; that read as the far edge bulging toward the viewer. |
| Glare size | `40%` falloff stop | `.art-glare` background (`NowPlaying.svelte:462`) | Smaller % = smaller spot. |
| `--glare-alpha` | `0.16` atmosphere / `0.10` instrument | `tokens.css:44`, `modes.css:28` | Peak spot alpha. |
| Blend | `screen` | `.art-glare` (`NowPlaying.svelte:464`) | Needed to read on dark covers; `soft-light` was invisible and `overlay` nearly so. |
| Press scale | `0.99` | `.art-sleeve:not(:disabled):active` | `--tilt-scale` composes with the tilt rather than replacing it. |
| Hover grow | none | — | Tried 1.03; removed because the uniform grow read as the far edge lifting. |

## Pointer math (`ui/src/lib/tilt.ts`)

- `tiltFromPointer(rect, clientX, clientY, maxTilt)` returns `{ rx, ry, gx, gy, glare }` (degrees, percents, 0..1).
- Offsets normalized to -1..1, clamped to the card bounds; `d = hypot(nx, ny)`, `r = min(1, d)`, magnitude `r * maxTilt`; axes `rx = -uy * mag`, `ry = ux * mag` with `(ux, uy) = (nx, ny) / d`.
- Direction-normalizing caps the combined corner tilt at `maxTilt`. Independent axes summed to `maxTilt * sqrt(2)` at corners, which read as a jump.
- Glare mirrors the cursor through the center: `gx = (1 - px) * 100`, `gy = (1 - py) * 100`; opacity is `r` (0 at center, 1 at edges).
- Zero-size rect guard returns flat defaults.
- Tests: `ui/src/lib/tilt.test.ts` (9 cases: center, edges, corners capped, proportionality, clamp, zero-size).

## Implementation notes

- Handlers live on the rectangular `.art` wrapper (`role="presentation"`), not the rounded button: with `border-radius`, hit-testing clips at the corners, so a `pointerleave` there reset the tilt and re-entry snapped flat. The button stays the click target.
- Custom properties are written straight to the wrapper's `style` on `pointermove` (no rune churn); the button and glare child inherit them. `@property --tilt-gx/--tilt-gy` (registered in `tokens.css:15-19`, `inherits: true`) lets the spot glide instead of snapping.
- Entry/leave handoff: `pointerenter` starts the `entering` window; the first moves ease on `--tilt-return`. After `ENTER_MS` the class drops and later moves use the 120ms linear track. On leave, props and classes are cleared and the base transition settles tilt, glare position, and fade together.
- Reduced motion: handlers early-return, and the media query sets `.art-sleeve { transform: none }` + `.art-glare { display: none }`.
- `will-change: transform` is on `.art-sleeve`; only the hovered card animates.

## Verification

- `npm --prefix ui test` — 481/481 at `29ee020`; `tilt.test.ts` covers the geometry.
- `npm --prefix ui run check` — 0 errors / 0 warnings at `29ee020`.
- Manual smoke: Settings → Cover shape → Sleeve, hover the Now Playing cover. Check entry/leave, corners (no snap), glare size/strength on dark and bright art, instrument mode, and reduced motion (OS setting).

## Tuning log (2026-10-03)

| Iteration | Change | Feedback / why |
| --- | --- | --- |
| 1 | 12 deg, independent axes, `soft-light` 0.35, 80% falloff, 800px, grow 1.03 | Corners overshot to ~17 deg combined; shine barely visible. |
| 2 | Radial smoothstep cap, 8 deg, 180ms `ease-out` tracking, `screen` 0.5, 72% falloff | Tracking felt stocky and jumpy. |
| 3 | Linear radial (smoothstep dropped), 120ms linear tracking, alpha 0.25, 80% falloff | Back to the tight feel; leave still snapped. |
| 4 | Leave/entry 360 → 420 → 480ms | Softer settle; owner kept 480. |
| 5 | Entry window added (`ENTER_MS`, then 480) | Slow first contact, then tight tracking. |
| 6 | Angle/size sweeps: 6 → 5 → 3 deg; glare 55% → 45%; then 6 → 8 deg with 35% | Owner tuning by feel. |
| 7 | Alpha 0.25 → 0.2 → 0.16; falloff 35% → 40%; instrument 0.14 → 0.10 | Less punch, same spot size. |
| 8 | Perspective 800 → 1400px; hover grow removed | Far edge read as bulging/lifting. |
| 9 | Glare opacity fade aligned to `--tilt-return` (was 180ms) | Shine appears/disappears on the same curve as the cover's leave. |
