# Plan 11 smoke — Atmosphere mode

Requires: real account, browser login, `npm --prefix ui run app:dev`. If the
config still says `mode = "instrument"` from Plan 10, switch Mode to
Atmosphere in Settings once (also set Typeface to Default).

- [ ] Mode switch live-applies: Atmosphere (blur backdrop, glass, wave seek) ↔ Instrument (flat, solid, hairline) with no restart.
- [ ] Backdrop follows the playing cover with a 1200ms crossfade; no artwork → the static warm room gradient.
- [ ] **Skip-storm:** spam Next 6+ times — colors and backdrop drift continuously, never flash a stale cover, and settle on the final track.
- [ ] Accent follows the cover: play button, active nav, seek fill, playing-row tint, focus rings.
- [ ] **Monochrome cover with a broad cast** (e.g. sepia/duotone album): accent is a boosted version of the cover's overall tint — never brand teal.
- [ ] **B&W cover with a shadow-only cast** (e.g. halftone art like The Perfect Girl): plain-light accent, no invented hue.
- [ ] **Fully neutral cover** (pure white/gray/dark, no tint): plain-light accent with neutral badges/disc ring — never brand teal.
- [ ] **Bright cover:** accent keeps the art's own lightness and chroma (vivid, gamut-mapped, not clipped); extreme bright covers cannot reach 4.5:1 under the 55% scrim (known spec gap — see final review).
- [ ] **Vivid red cover** (e.g. red-on-black art): accent is a true red, not a pale pink.
- [ ] **Mostly-white cover with one color pop:** accent is the color, not white.
- [ ] **Badges/disc ring:** only teal when the cover itself has that hue; monochrome covers collapse to a Primary tint.
- [ ] Dynamic color Accent-only: accent follows the cover, backdrop/scrollbar stay brand; Off: fully static brand.
- [ ] Disc spins only while playing, eases to a stop on pause; Sleeve setting shows the static sleeve.
- [ ] Wave seek: played portion fills accent, wave changes per track.
- [ ] Instrument + Dynamic color Full: accent recolors work on the flat surfaces.
- [ ] Reduced motion (Windows setting on): instant recolors, static disc, no backdrop crossfade — in both modes.
- [ ] Offline replay of cached tracks: accent keeps the last palette (no reset to teal), no error toasts.
- [ ] `cache/palettes.json` exists and grows; restart with network off still colors previously-seen covers from the disk cache.
