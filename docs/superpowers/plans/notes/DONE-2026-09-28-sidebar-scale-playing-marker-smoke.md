# Sidebar scale + playing-playlist marker smoke — 2026-09-28

Requires: real account, browser login, `npm --prefix ui run app:dev`.

- [ ] Sidebar reads one step larger throughout: wordmark, nav labels/icons, section headers, "+" button, row paddings; tree covers are bigger (32px comfortable / 24px compact via `--thumb-size + 2`); Compact density still shrinks labels and thumbnails proportionally.
- [ ] Sidebar is 248px wide when expanded; rail collapse still works and stays 64px.
- [ ] Tree indentation/tree-notes align with the new cover size; long playlist names still ellipsis.
- [ ] Play a playlist (Play, Shuffle, or double-click a row): its sidebar row shows a play glyph over the cover + accent label, with no row background.
- [ ] Navigate to a different playlist while one is playing: the playing one keeps glyph + accent label; the open one shows the usual accent background tint.
- [x] Open the playlist that is the playback source: glyph + accent label **and** the usual accent background tint — the open highlight combines with the playing marker instead of being suppressed. (owner-verified 2026-10-02)
- [ ] Hover the source-only row: surface tint appears, label stays accent. Hover the source-active (open) row: the accent highlight persists, label stays accent.
- [ ] Start playback from Album / Search / Tracks collection / a mix: the sidebar glyph and accent disappear.
- [ ] Queue rail: jump/next/prev keep the glyph; "Clear" removes it.
- [ ] Rail (collapsed) mode still shows the glyph over the cover.

## Last-route restore (added 2026-10-02)

- [x] Open a playlist (or album/folder), quit the app, relaunch: it lands on that page; back/forward still work. (owner-verified 2026-10-02)
- [x] Open Settings, quit, relaunch: Settings is restored (any route is remembered). (owner-verified 2026-10-02)
- [x] Log out, then relaunch: the app starts at Home (logout clears the remembered route). (owner-verified 2026-10-02)
- [x] Simulate corrupt storage (`localStorage.setItem("oxitide.nav", "nope")` in the webview console), relaunch: Home. (owner-verified 2026-10-02)

## Footer/scroll fix (many playlists)

- [ ] With enough playlists to overflow: only the upper area (nav + collection + tree) scrolls; Settings and Log out stay pinned at the bottom and never move.
- [ ] No playlist row is ever drawn over or around the Settings/Log out buttons; the list clips cleanly just above them.
- [ ] Scrolling reaches the last playlist; the scrollbar sits at the sidebar's right edge and scrolling works with the wheel over the list.
- [ ] With only a few playlists: layout looks as before (Settings/Log out at the bottom, no scrollbar).
- [ ] Rail mode with many playlists: same behavior (nav scrolls, footer pinned).
- [ ] Minimum window size (720×520): Settings/Log out remain visible and the upper area scrolls — nothing is clipped or unreachable.
- [ ] Both Atmosphere and Instrument modes.
- [ ] `npm --prefix ui run check`, `npm --prefix ui test` green.
