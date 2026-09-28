# Sidebar / My Collection smoke checklist (2026-09-28)

Requires a real account, `npm --prefix ui run app:dev`, and a browser login.

## Endpoint probes

- [ ] Feed renders content, or shows "Feed isn't available for this account."
      Record which candidate endpoint answered (watch `%APPDATA%\OxiTide\logs\oxitide*.log`
      for requests to `pages/feed` vs `pages/my_collection_feed`) and paste the observed
      JSON shape here:
- [ ] Purchases renders content or the empty state. Record the observed path and JSON
      shape (probe `users/{id}/purchases`; if the shape differs, note the fields):
- [ ] Mixes & Radio lists mixes and clicking one starts playback of its tracks.

## Navigation

- [ ] Sidebar order: Home, Explore, Feed, divider, My Collection (expanded), Playlists,
      Albums, Tracks, Show more, then the playlist tree, then Settings/Log out.
- [ ] Show more reveals Mixes & Radio, Videos, Profiles, Purchases.
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
- [ ] Videos lists saved videos with the unsupported-playback note and no play action.
