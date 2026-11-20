# Phone remote filter, Play next, and added-track order (GitHub issue #4) — owner smoke

**Status:** Shipped in v1.1.4 and v1.1.5; the unavailable-track paging fix (last section) is on
`release/1.x`, not yet released; owner smoke open. Part 2b is done the
cheap way (added tracks play before the rest of the context, in order; see below). A real second
queue (its own section in the queue view, added tracks dropped once played, kept across a new
play) is left for V2 and builds on the same marker.

- Issue: https://github.com/Zephulbr/OxiTide/issues/4 ("Remote search function for Tracks
  playlist and other Playlists + Play Next button in the Remote and Exclusive Queue").
- What changed (phone remote only; the desktop already had both):
  - Playlist pages and Library → Tracks show a filter box above the list
    (`remote/components/TrackList.svelte`, `filterable`; matching in `remote/filter.ts`). It
    matches title or artist, case-insensitive, like the desktop filter (not the album).
  - Typing loads the rest of the list first, so the filter covers every track, not only the pages
    scrolled so far ("Loading all tracks…", Retry on failure). Page loads are serialised, so the
    scroll loader and the filter never fetch the same page twice.
  - While filtered: a tapped row plays with the matches queued first, then the rest of the list
    (the desktop's `matchesThenRest`); Play does the same from the first match; Shuffle shuffles
    the matches only; Add all to queue / to a playlist take the matches only.
  - Every track menu (lists, search, artist, feeds, Now Playing's More) leads with **Play next**,
    which calls the already-allowlisted `queue_play_next` (the track goes right after the current
    one). Toast: "1 track plays next".
  - New `remote.json` strings reuse the desktop's wording; their translations were copied from each
    language's `app.json` (`menu.playNext`, `table.manyPlayNext`, `filter.*`, `table.noMatch`)
    and marked `machine`.
  - Added-track order (part 2b, desktop and phone alike; `Queue::push` / `added_end`, see
    `docs/agents/audio.md`): "Add to queue" puts tracks after the tracks added earlier that are
    still to play, else right after the current track, instead of at the end of the playlist or
    album. "Play next" goes right after the current track and a later "Add to queue" still lands
    after it. Turning shuffle on keeps the added tracks next, in order. Autoplay suggestions stay
    after added tracks. A new play, Clear, an app restart and a Repeat All wrap forget which
    tracks were added. Dropping tracks below the queue rail's last row still appends at the end.
- Local gates, 2026-11-18: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace`, `npm --prefix ui run check`, `npm --prefix ui test`,
  `node scripts/i18n/check.mjs` (0 errors; warnings unchanged from before).

## Checks (dev build, a real account, phone on the same Wi-Fi)

- [ ] A long playlist (several hundred tracks): open it and type in the filter before scrolling.
      "Loading all tracks…" shows briefly, then matches from the end of the playlist appear.
- [ ] Library → Tracks with a big collection: type one letter, then more. Typing stays responsive
      and kept rows do not re-animate on each keystroke.
- [ ] Filter by an artist, tap the second match: it plays; the queue holds the other matches, then
      the rest of the list. No duplicate tracks from the paging.
- [ ] Filtered Play starts at the first match (same order); filtered Shuffle plays only matches.
- [ ] A filter with no matches shows "No tracks match “…”"; the x clears it and the full list is
      back.
- [ ] Track menu → Play next while something plays: the toast shows, and the phone's Queue tab (and
      the desktop rail) list the track right after the current one.
- [ ] One non-English phone language (e.g. German): the filter and Play next text are translated.
- [ ] Desktop: play an album from track 1, "Add to queue" song A, then song B. The queue rail shows
      A, B right after the playing track, then the rest of the album; they play in that order.
- [ ] Then "Play next" song C: C sits right after the playing track, before A; another "Add to
      queue" D lands after B.
- [ ] Let A play, then add E: E lands after B (and D), not right after A.
- [ ] Turn shuffle on with added tracks waiting: they stay next, in order; the album is shuffled
      after them; the playing track keeps playing without a gap.
- [ ] Click a later row in the queue rail, then add a song: it lands after the added tracks still
      ahead (or right after the new current track when none are left).
- [ ] Let the queue run out so Autoplay adds suggestions, then "Add to queue": the song goes before
      the suggestions.
- [ ] Drag a track from a list onto the queue rail below its last row: it lands at the very end.
- [ ] Phone remote: "Add to queue" and "Play next" place tracks the same way.

## Follow-up: filters on more phone lists (ships in v1.1.5, `2026-11-18-v1.1.5-release.md`)

The filter box (`remote/components/FilterBox.svelte`) now shows on any long list, from 20 items
(`FILTER_MIN`; shorter lists, including short playlists, show none): Mix and Album track lists
and Recently played (same behaviour as the playlist filter; album rows keep their track numbers),
and Library → Albums (title, artists, year), Artists (name), My Mixes (title, description) and
Playlists (names in the open folder, not nested ones). Five new `remote.json` strings, machine
drafts in all shipped languages.

- [ ] A playlist with fewer than 20 tracks shows no filter; one with more does.
- [ ] Library → Albums: type an artist's name; their albums remain. Clear it with the x.
- [ ] Library → Artists: typing "artist" matches only artists with it in their name.
- [ ] Playlists: filter the root, open a folder: the folder starts unfiltered.
- [ ] My Mixes, a long Mix, a long album (a compilation) and Recently played filter as expected;
      the album's matches keep their own track numbers.
- [ ] A filter with no matches on a Library grid shows "Nothing matches “…”".

## Follow-up: unavailable tracks cut phone lists short (issue #4 reply, 2026-11-20)

Reported on issue #4: filtering Library → Tracks for "veil" found 6 tracks on the desktop and 1
on the phone, in a collection of 3,119 tracks with 248 unavailable. TIDAL leaves unavailable
tracks out of a page's `items` but counts them in `total`. The phone's pager
(`remote/paged.svelte.ts`) asked for the next page at the number of rows received and took an
empty page as the end, so 100 or more unavailable tracks in a row ended the list silently (and
smaller gaps re-read rows, which could repeat). It now advances by the page size and ends at
`total`, like the desktop pager and `get_all_pages`; Library → Tracks drops repeated ids,
playlists keep a track that is in them twice. `TrackList` follows the pager's `done`/`left`
instead of comparing `total` with the rows, and "Add all to queue / to a playlist" load the rest
of the list first, as Play does. Recently played uses the same pager.

Found in the same pass over the phone remote, fixed with tests (`remote/views.test.ts`):

- A state event for the same song (pause, volume) refetched the lyrics (blanking them to
  "Loading…") and Now Playing's artist/album link ids.
- Clearing the search box while a search was in flight brought its results back.
- Now Playing's playback-error Retry button was hard-coded English (now `common.retry`).
- "Add to a playlist" with more than 5,000 tracks was refused by the PC; it now sends the first
  5,000, as Add to queue does; their toasts count what was sent.
- A malformed `%` escape in the page address threw while the page started; it now opens Now
  Playing.

- [ ] Library → Tracks on an account with unavailable favorites: the filter finds matches older
      than the unavailable ones (the reporter's case: all six "veil" tracks).
- [ ] Scroll Library → Tracks to the end without a filter: no track shows twice, the spinner
      stops, and the list does not end early.
- [ ] A playlist with unavailable tracks: scroll and filter reach its last track.
- [ ] Library → Tracks, unfiltered, before scrolling: Add all to queue; the toast's count equals
      the desktop Tracks header's count (playable tracks; at most 5,000).
- [ ] Recently played: scroll to the end and filter; no play shows twice, and the spinner stops.
- [ ] Desktop: a playlist with unavailable tracks shows its last tracks and the "N unavailable"
      count; Play all queues through to the end. (The desktop pager, the phone pager and
      `get_all_pages` now keep walking past an empty window that still reports the total; an
      empty window with no total still ends the walk.)
- [ ] Lyrics open on the phone: pause, resume, and change the volume; the lyrics stay put.
