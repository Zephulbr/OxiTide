# Sidebar / My Collection smoke checklist (2026-09-28)

Requires a real account, `npm --prefix ui run app:dev`, and a browser login.

## Endpoint probes

- [ ] Feed renders followed-artist releases from `v2/feed/activities` (rail groups:
      New updates / Last week / This month / Earlier) or the "No updates yet." empty
      state. Check covers on album rows, the E/H badges, and that clicking a release
      opens its album page. Paste the observed activity JSON shape here (album type,
      cover, audioQuality, any `historyMix` entries — history mixes are skipped for now):
- [ ] Purchases renders content or the empty state. Record the observed path and JSON
      shape (probe `users/{id}/purchases`; if the shape differs, note the fields):
- [ ] Mixes & Radio lists mixes and clicking one starts playback of its tracks.

## Navigation

- [ ] Sidebar order: Home, Explore, Feed, divider, My Collection (expanded), Playlists,
      Albums, Tracks, Show more, then the playlist tree, then Settings/Log out.
- [ ] Show more reveals Mixes & Radio, Profiles, Purchases.
- [ ] Every sidebar entry navigates to its page; active states highlight correctly.
- [ ] Ctrl+F jumps to Explore and focuses the search box; typing shows results; clearing
      restores the explore cards.
- [ ] Narrow the window below 1080px: rail mode shows page icons, folder icons, and
      playlist covers; no labels or headers. Toggle collapse manually.

## Collection pages

- [ ] Playlists page lists folders first (with "N playlists") and playlists (mosaic cover,
      "N items"); sort toggle switches name/newest-first.
- [ ] New playlist from the page header and sidebar `+` both create and refresh the tree.
- [ ] New folder creates a folder at the current level and refreshes both page and tree.
- [ ] Folder row opens the folder page with a breadcrumb back to All playlists.
- [ ] Rename playlist and folder from context menus; failures (e.g. offline) toast and
      leave the list unchanged.
- [ ] Delete an owned playlist asks for confirmation and removes it; a liked playlist
      offers "Remove from collection" instead.
- [ ] Move to folder moves a playlist and refreshes tree and page; nested folders appear
      indented in the picker.
- [ ] Tracks page matches the old Library favorites table (play, shuffle, refresh,
      unfavorite drops the row).
- [ ] Albums and Profiles open album/artist pages.

## Tree refresh

- [ ] Create a playlist from a track's "Add to playlist" picker → it appears in the sidebar tree without a reload.
- [ ] Create a playlist from the sidebar `+` → it appears in the tree and on the Playlists page.
- [ ] Delete a playlist from its detail page → it disappears from the tree and the breadcrumb/list.
- [ ] Toggle the Playlists sort between name and the default: record the observed default order (date-added vs name) so we know whether the server honors the omitted `order`.
