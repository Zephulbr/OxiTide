# Plan 8 Smoke Checklist

> Requires a human, a logged-in TIDAL account, a browser login, and a mouse with
> side buttons. Run with `npm --prefix ui run app:dev` (or the installed app).

## Automated checks (tree at ad3783f)
- cargo test --workspace: 247 passed, 0 failed, 1 ignored (oxitide-app 33, oxitide-audio 139 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 74, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 117 passed, 0 failed across 13 files (format 2, quality 5, errors 4, toasts 7, menu 5, menus 9, shortcuts 11, nav 11, player 6, player.restore 3, playlists 14, favorites 12, pager 28)
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

Counts recorded after the final whole-branch review fix (`ad3783f`): no-op shortcuts no longer swallow native scrolling and partial playlist failures report both facts. The pre-fix tree (`b610a62`) ran 113 UI tests.

## Manual checks
- [x] Toast stack: add tracks to a playlist from Search → success toast appears top-right → another action stacks a second toast → both auto-dismiss after ~6 s.
- [x] Error toast persistence: turn the network off and toggle a favorite → error toast appears and stays until the ✕ is clicked; a fourth outcome evicts the oldest auto-dismissing toast, never the error.
- [x] Mapped error copy: attempt to rename a playlist you do not own → the toast reads "You can only edit your own playlists" (not "forbidden").
- [x] Logout failure: with the network off, click Log out → the error toast is visible on the login screen.
- [x] Track row menu: right-click a row in Search, an Album, a Playlist, and Library favorites → Play / Add to queue / Add to playlist / Favorite match the inline buttons; playlist rows also show Move up / Move down / Remove with edges disabled; Playlist editing items are absent while the pager is loading.
- [x] Queue item menu: right-click a queue row → Play now and Remove work.
- [x] Library playlist menu: right-click a playlist → Open, Rename, and red Delete all do what the inline buttons do.
- [x] Menu edges and keyboard: open a menu near each of the four window edges → it stays fully on-screen; ↑/↓ wrap and skip disabled items; Enter runs; Esc closes; clicking away or scrolling closes.
- [x] Shortcuts while playing: Space toggles, ←/→ seek ±5 s, Shift+←/→ change tracks, ↑/↓ change volume by 5%, none act while a text field is focused.
- [x] Ctrl+F and `/` focus the Search input; Ctrl+F does not open the WebView find bar.
- [x] Esc precedence: with a menu open it closes the menu; with the queue rail open and no menu it closes the rail; with a dialog open the dialog closes and the rail stays.
- [x] Alt+←/→ and the mouse side buttons move the history; the Sidebar ‹/› buttons enable/disable correctly; a new navigation clears forward history; toggling the queue does not.
- [x] Typing guards: with focus in the Search input, Space/arrows edit or move the caret and never control playback.
- [x] Home cards: clicking a playlist/editorial card opens the playlist view, an album card opens the album; track/mix cards are inert panels.
- [x] Pager spinner: open a large playlist and Library favorites → a small spinner sits beside the "N tracks" label while paging.
- [x] Queue clear: Clear asks "Remove all tracks from the queue?"; Cancel keeps the queue; Confirm clears it.
- [x] Modal focus: open the playlist picker → a control inside the dialog is focused; close it → focus returns to the button that opened it; with the delete confirmation open, focus restores after navigation is skipped safely.
- [x] Search no-results: search for gibberish → "No results for “<query>”"; Album/Artist/Home loading and empty states all use the shared components (no bare "Loading…" text in views).

## Known caveats
- Mouse side buttons depend on WebView2/driver mapping; Alt+←/→ is the fallback.
- ~~History is in-session only; restarting starts at Home (P8-D5).~~ Superseded 2026-10-02: the last route and history persist across launches; logout clears them.
- Toast actions exist in the store but no Plan 8 caller renders one yet (deferred undo).
- Menus are additive; inline row buttons are unchanged.
