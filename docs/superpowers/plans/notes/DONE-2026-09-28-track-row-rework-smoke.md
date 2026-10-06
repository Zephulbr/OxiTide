# Track row rework smoke — 2026-09-28

Requires: real account, browser login, `npm --prefix ui run app:dev`.

- [ ] Row order is `#` (right-aligned, leftmost) → cover → Title → Artist → Album → Time (right-aligned) → Quality → actions; headers align with their cells.
- [ ] Comfortable rows are 46px, Compact 38px (Settings → Appearance toggle); covers stay full-bleed square and fill the row; toggling density mid-scroll on a long playlist keeps position with no blank gap or row overlap.
- [ ] Title/Artist/Album share leftover width proportionally (title widest): no big void before Time at any window width; long titles/artists ellipsis; actions stay pinned to the right edge.
- [ ] Row hover shows a subtle background tint; the thumb play overlay still appears.
- [ ] Queue (plus) and playlist (list-plus) buttons are hidden at rest and fade in on row hover; the heart is always visible; clicking where a hidden button sits (row not hovered) does NOT trigger it.
- [ ] Keyboard: tabbing into a row reveals the queue/playlist buttons (focus-within); they are operable.
- [ ] Queue button adds the track to the queue (queue rail updates / toast); playlist button opens the picker where provided; heart toggles favorite and fills accent.
- [ ] Playing row shows the stronger accent tint + accent title, with no colored ring around the cover; hover deepens the tint slightly.
- [ ] Playlist view drag-and-drop: the drop line aligns with row boundaries at both densities.
- [ ] Album view (no Album column, 7 columns) lays out correctly.
- [ ] Check both Atmosphere and Instrument modes (surfaces differ per mode).
- [ ] `npm --prefix ui run check`, `npm --prefix ui test` green.
