# Tracks plays while the list loads — owner smoke

**Status:** Implemented on `release/1.x` for v1.1.7, not yet released; owner smoke open.

From the reporter's second comment on https://github.com/Zephulbr/OxiTide/issues/7: on the first
play after opening the app, a click in Liked Songs left every row unclickable for a while before
anything played; later plays were fine.

**Cause (from the code, not measured):** a click in Tracks awaited `favoritesPager.ensureAll()`,
the whole collection in 1000-row windows, before sending the play, and `TrackTable` disabled
every row (`loadingAll`) while a page's `onPlay` ran. The pager's cache lives for the app run,
so only the first play of a run paid for the download. It is not specific to a PC boot.

## What changed

- **A click plays at once while the list loads** in Tracks, a playlist and an artist's All
  tracks (`views/CollectionTracks.svelte`, `Playlist.svelte`, `ArtistAll.svelte`; new
  `lib/contextPlay.ts`, `createEarlyPlays`). In the list's own order (no filter or sort), a row
  or Play all plays the rows loaded so far, and the rest joins the queue once loaded
  (`queue_extend_context`, the search play's mechanism). With shuffle on, the late tracks are
  scattered among the upcoming ones, so the queue is still one even shuffle of the whole list.
  A playlist cannot be edited while it loads (its remove and drag actions wait for `done`), so
  the rest is always the list past the rows played.
- **What needs the whole list still waits for it:** the Shuffle buttons (started early, the
  first track would always come from the top of the list, #7's symptom) and a filtered or
  sorted view. The rows stay clickable meanwhile, and any desktop play started meanwhile wins
  (`lib/playClaims.ts`: every play `api.ts` sends claims one). Known gap, left for `main`: a
  play from the phone remote does not claim, so a desktop play still waiting can replace it
  when its list arrives. The clean fix is engine-side (a play skipped when the queue context
  changed since its click) and touches the state payload both clients mirror.
- **If the rest fails to load**, the first rows keep playing and the list's Retry finishes the
  queue. The engine alone decides whether a rest still applies (`queue_extend_context` returns
  whether it did), so a play clicked later that never started does not cut it short.
- **No table-wide lock** (`lib/components/TrackTable.svelte`): `loadingAll` is gone.
- **The rest survives queue edits** (`crates/oxitide-audio/src/queue.rs`). `extend_context` was
  gated on the queue revision, so adding a song while the rest loaded would have left the queue
  with the first page only. It is now gated on a context id that a new play, a restore or a clear
  bumps (replaying the same list too); edits and a queue jump keep it. The rest lands after the
  user's added tracks and before Autoplay's suggestions, in one linear pass when shuffled
  (`merge_at_random`; it was one `Vec::insert` per track, quadratic for a 10k collection on
  the main thread). `play_tracks` returns that id (`Engine::play`); the search play
  (`lib/artistTracks.ts`) uses the same helper.
- Tests: `queue.rs` (`extend_context_*`, `merge_at_random_*`,
  `replaying_the_same_list_starts_a_new_context`, `a_queue_jump_keeps_the_context`),
  `lib/contextPlay.test.ts`, `views/CollectionTracks.test.ts` (its first version failed 3 of 4
  on the old page), a `TrackTable.test.ts` case, `artistTracks.test.ts`.
- Not measured: no perf scenario covers a click during the load.

## Checks (installer, a real account with a large Liked Songs)

- [x] Quit the app fully (tray too), reopen, open Library → Tracks and click a song straight
      away, while the refresh icon still spins: it plays at once, and the other rows stay
      clickable.
- [x] Once the refresh icon stops, the queue (desktop rail) holds the whole collection: old
      favorites at the end, in list order with shuffle off.
- [x] Same with shuffle on in the player bar: the clicked song plays first, and old favorites
      appear among the upcoming tracks once the list has loaded.
- [x] Reopen the app, click a song at once, then "Add to queue" another song before the refresh
      icon stops: the added song plays next, and the queue still grows to the whole collection.
- [x] Reopen the app and press the Shuffle button at once: nothing plays until the list has
      loaded, then a shuffle of the whole collection plays (the first track is not always a recent
      favorite). Click a row while it waits: the row plays, and the shuffle does not replace it.
- [x] Reopen, type a filter in Tracks at once and click a match: it plays the matches, then the
      rest of the collection (issue #3's order).
- [x] Search → a song (the artist play): the song plays at once, and the artist's other tracks
      join the queue as before.
- [x] A large playlist (a few thousand tracks), a row clicked at once after opening it: it plays
      at once, and the queue grows to the whole playlist, in order with shuffle off.
- [x] The same playlist's Shuffle, pressed at once: it waits for the list, then shuffles all of
      it; a row clicked while it waits wins.
- [x] An artist's All tracks, a row clicked at once: it plays at once, and the artist's other
      tracks join the queue.
- [x] Reopen, click a song in Tracks at once, then turn the network off before the refresh icon
      stops: the song keeps playing and Tracks shows the error. Network back on, Retry: the
      queue grows to the whole collection.
