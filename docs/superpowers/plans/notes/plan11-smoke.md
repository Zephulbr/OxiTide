# Plan 11 smoke — Atmosphere mode

Requires: real account, browser login, `npm --prefix ui run app:dev`. If the
config still says `mode = "instrument"` from Plan 10, switch Mode to
Atmosphere in Settings once (also set Typeface to Default).

- [ ] Mode switch live-applies: Atmosphere (blur backdrop, glass, wave seek) ↔ Instrument (flat, solid, hairline) with no restart.
- [ ] Backdrop follows the playing cover with a 1200ms crossfade; no artwork → the static warm room gradient.
- [ ] **Skip-storm:** spam Next 6+ times — colors and backdrop drift continuously, never flash a stale cover, and settle on the final track.
- [ ] Accent follows the cover: play button, active nav, seek fill, playing-row tint, focus rings.
- [ ] **Monochrome cover** (e.g. a grayscale album): brand teal accent, app keeps its identity.
- [ ] **Bright cover:** title/playing-row accent stays readable (≥4.5:1) over the bright blur.
- [ ] Dynamic color Accent-only: accent follows the cover, backdrop/scrollbar stay brand; Off: fully static brand.
- [ ] Disc spins only while playing, eases to a stop on pause; Sleeve setting shows the static sleeve.
- [ ] Wave seek: played portion fills accent, wave changes per track.
- [ ] Instrument + Dynamic color Full: accent recolors work on the flat surfaces.
- [ ] Reduced motion (Windows setting on): instant recolors, static disc, no backdrop crossfade — in both modes.
- [ ] Offline replay of cached tracks: fallback teal palette, no error toasts.
- [ ] `cache/palettes.json` exists and grows; restart with network off still colors previously-seen covers from the disk cache.
