# Track 9.1 light mode — owner smoke checklist

- **Status:** Pending owner run (Windows, real account)
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §6.1

## Theme switching
- [ ] Settings → Appearance shows Dark / Light / System; each selection applies immediately.
- [ ] Light + Atmosphere: warm paper room, translucent surfaces, glass header/player, cover-art backdrop, no washed-out text.
- [ ] Light + Instrument: solid light surfaces, flat edges, no blur, no grain.
- [ ] Dark + both modes look unchanged from before this plan.
- [ ] System follows a Windows theme flip while the app is running (toggle Windows app mode and watch OxiTide follow).
- [ ] Restart in each setting; no dark/light flash before first paint.
- [ ] Hand-corrupt `localStorage["oxitide.theme"]` via devtools and restart: clean dark boot, no errors.

## Dynamic color
- [ ] Light + Atmosphere, dynamic color Default: nav/link/focus/playing-row colors follow the cover and stay readable.
- [ ] Switch a bright/warm cover and a dark/blue cover; no pale unreadable accents.
- [ ] Fully monochrome / near-black cover in Light mode: Info and playing text plus interactive accents remain readable (pins the accepted composite-ceiling caveat — the light plain-neutral primary is not contrast-fitted).
- [ ] Toggle theme mid-playback with dynamic color on: colors re-analyze once, no stale dark-room palette.
- [ ] Dynamic color Off in light theme: static light-safe teal accent (not dark-mode teal).
- [ ] Menus/overlays over bright art remain legible in light mode.

## Regression
- [ ] Player bar, seek, lyrics, queue, modals, toasts legible in all four mode×theme combinations.
