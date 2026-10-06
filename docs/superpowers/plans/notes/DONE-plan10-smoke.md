# Plan 10 smoke — Visual foundation + Instrument mode

Requires: real account, browser login, `npm --prefix ui run app:dev`.
(Atmosphere mode arrives in Plan 11; this pass runs in Instrument, the only
mode that exists here.)

- [ ] Boots into Warm Graphite (warm dark `#17120e`, off-white text); no Spotify-gray remains.
- [ ] Body text renders in Manrope; view titles (`Home`, `Library`, `Settings`, album/playlist/artist names) render in Fraunces serif; DevTools Network shows the woff2 files (155KB total).
- [ ] Settings → Appearance: typeface swaps live (Manrope/Inter/System); Custom "No Such Font" shows the "not found — using fallback" note; a real installed font (e.g. "Georgia") previews and applies; "Browse installed fonts…" appears where `queryLocalFonts` is supported.
- [ ] Density Compact re-heights track rows (28px) and queue rows (28px); Comfortable restores (38px); toggling mid-scroll on a 2k playlist keeps position with no blank gap.
- [ ] Sidebar collapses to the 64px icon rail below 1080px and via the toggle; the wordmark becomes "O"; window will not shrink below 720×520.
- [ ] Queue drags between 260–420px and persists across restart; below 760px it floats over a backdrop and closes via backdrop click or Esc.
- [ ] Covers render: track rows (not in album view), queue rows, home cards, search/library rows, album/playlist/artist headers; missing art shows the warm fallback tile (verify a playlist/artist without an image).
- [ ] Home cards show live artwork (confirms `HomeItem.artwork` field precedence against the real API) and zoom on hover.
- [ ] Player bar: hairline seek (grows + thumb on hover), 48px cover opens now-playing; Esc and artwork click close it; Next Up lists 3 rows; play/pause is an accent circle.
- [ ] Playing row is accent-tinted with accent title and a ringed thumb; hover shows the thumb play glyph.
- [ ] Track row action buttons are Lucide icons (grip, play, plus, list-plus, heart, chevrons, x) — no unicode glyphs anywhere.
- [ ] Toasts have severity glyphs (check/alert) and slide in with a slight overshoot; dialogs have lit edges + Fraunces titles; loading states show the disc spinner; empty search shows the Fraunces empty state.
- [ ] `%APPDATA%\OxiTide\config.toml` round-trips all six new `[ui]` keys (`mode`, `typeface`, `custom_typeface`, `density`, `cover_shape`, `queue_width`) with correct TOML names.
- [ ] Reduced motion (Windows "Show animations" off): state transitions become 120ms fades, no shake.
- [ ] `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check` all green.
