# Detail headers + search bars — manual smoke checklist

GUI smoke checks for the Playlist/Album/Mix header rework and the shared search
field recipe (2026-10-02). Requires a human, a real account, and a browser
login. Run with `npm --prefix ui run app:dev`.

Result: passed (owner smoke, 2026-10-03).

## Header layout (all three pages)

- [ ] Open a playlist, an album, and a mix: each shows a 208px square cover, an
      uppercase kicker (`PLAYLIST` / `ALBUM` / `MIX`) above a large title, and a
      stats line under it.
- [ ] Playlist stats read `N TRACKS (H:MM:SS)`, plus `• N FANS` when TIDAL
      reports followers and `• N UNAVAILABLE` when tracks are unavailable.
- [ ] Album shows the artist line between title and stats; mix sums the loaded
      tracks (count and runtime) since mixes carry no metadata totals.
- [ ] Play/Shuffle are 48px pills under the cover; Play is accent-filled with a
      solid triangle, Shuffle is a bordered surface pill; both brighten on
      hover and are disabled only while the list is empty.
- [ ] Action cluster sits at the bottom right, aligned with Play/Shuffle.

## Playlist actions

- [ ] Owned playlist: Edit, Public/Private (accent when public), Share, More.
- [ ] Edit opens the modal with name + description prefilled; Save persists
      (reopen the page to confirm) and toasts "Playlist renamed"; Escape,
      backdrop click, and Cancel dismiss without saving.
- [ ] Public/Private toggles with a success toast, flips the accent state
      immediately, and reverts with an error toast if the write fails.
- [ ] Share copies `https://tidal.com/playlist/<uuid>` and toasts "Link
      copied"; paste it into a browser to confirm it opens the playlist.
- [ ] More → Refresh reloads covers/tracks; More → Delete playlist opens the
      existing confirm dialog and deletes on confirm.
- [ ] Non-owned playlist: no Edit/Public/Delete; Share works and More only
      offers Refresh.
- [ ] If TIDAL omits the public flag, the Public/Private button is hidden (no
      wrong state shown).

## Album + mix actions

- [ ] Album: Favorite toggles accent fill and persists across navigation;
      Add opens the playlist picker with the album's tracks; Share copies
      `https://tidal.com/album/<id>`.
- [ ] Mix: Add opens the picker with the mix tracks; Share copies
      `https://tidal.com/mix/<id>`.

## Fans data

- [ ] Open a playlist: the stats line shows `N FAN(S)`. Source is the public
      OpenAPI host — `GET openapi.tidal.com/v2/playlists/<uuid>` — because the
      internal `api.tidal.com/v2` playlist GET omits `numberOfFollowers`.
      Verified live 2026-10-02: the CheeseMan playlist returns
      `numberOfFollowers = 2` through `TidalClient::playlist`.
- [ ] A playlist with zero followers still renders `0 FANS` (the count is
      present, not absent).
- [ ] If the OpenAPI call fails (offline, scope rejection), the page still
      renders normally and the fans segment is simply absent.

## Search fields

- [ ] Explore search: muted search glyph inset on the left, surface fill,
      line border, rounded control radius, accent-tinted border on focus.
- [ ] TrackTable "Filter tracks": same fill/border/radius/placeholder
      treatment; the clear X still appears when typing.
- [ ] Both match in Atmosphere and Instrument modes and in comfortable and
      compact density.

## Regression

- [ ] Play/Shuffle still start the queue at the right track (play from a row
      and via the header) for all three pages.
- [ ] Rename, delete, favorite, and add-to-playlist flows behave as before.
- [ ] Search suggestions dropdown still positions under the input and stays
      legible over bright artwork.
