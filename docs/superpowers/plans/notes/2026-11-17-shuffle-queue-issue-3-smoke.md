# Shuffle and queue fixes (GitHub issue #3) — owner smoke

**Status:** Ships in v1.1.2 (gate note `2026-11-17-v1.1.2-release.md`). The last three checks (the first-page play's follow-up and the search now-playing mark) were not reported before the release.

- Issue: https://github.com/Zephulbr/OxiTide/issues/3 ("Shuffle, search and queue not working properly").
- What changed:
  - Turning shuffle on shuffles the whole queue, the tracks before the current one included; the
    current track moves to the top and keeps playing (`Queue::set_shuffle`). Turning it on again
    while it is on changes nothing.
  - A new play while shuffle is on (a row, Play all) plays the picked track first and shuffles the
    rest (`EngineCmd::PlayContext`). Queue jumps (rail, Home's Up next, the rack's changer and
    slots, the phone's queue) keep the queue's order (`api.queueJump`, `play_tracks` with
    `keepOrder`, `EngineCmd::PlayTracks`).
  - My Collection → Tracks with the filter set: a played row queues the matches, then the rest of
    the collection (`matchesThenRest`), so the collection carries on instead of autoplay. Play all
    does the same; Shuffle still shuffles the matches only.
  - Search (Explore): a clicked song, in the Tracks list or as the Top result, plays first and is
    followed by every other track of its (first credited) artist — the artist page's Tracks tab,
    loaded in full through the shared `artist:{id}:tracks` pager (`lib/artistTracks.ts`). An
    artist as the Top result plays all their tracks (was the top 50). Play all / Shuffle on the
    search Tracks list still queue the results; a song whose artist list fails to load falls back
    to them.
  - No wait for big artists: unless the list is cached, the play starts from the first page (one
    request, 100 tracks) and the rest joins once loaded (`queue_extend_context` at the revision
    `play_tracks` returned; dropped if the queue changed meanwhile; scattered among the upcoming
    tracks when shuffle is on).
  - Compact track lists (search's preview) show the now-playing mark on the playing row's cover;
    they have no number column, where other tables show it.
- Local gates, 2026-11-17: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, svelte-check and vitest; results in the commit.

## Checks (dev build, a real account)

- [x] Collection Tracks: play row ~20, turn shuffle on. The track keeps playing without a gap, it
      sits at the top of the queue, and rows 1–19 appear among the upcoming tracks.
- [x] With shuffle on, play a row from an album or playlist: that track plays, the rest of the
      album/playlist is queued shuffled, the shuffle button stays lit.
- [x] With shuffle on, click a later row in the queue rail (and Home's Up next): it plays and the
      queue order does not change.
- [x] Shuffle off, then on again: the playing track stays, no gap; toggling off leaves the order.
- [x] Collection Tracks: filter by an artist name, play the second match. The other matches follow,
      then the rest of the collection; no autoplay rows appear until the collection ends.
- [x] Same with shuffle on: the picked match plays first, everything else (all matches and the rest
      of the collection) is shuffled after it.
- [x] Phone remote: jump to a queue row with shuffle on; the order stays.
- [x] Search an artist, click one of their songs in Tracks: it plays, and the queue holds the rest
      of that artist's tracks (as on their page's Tracks tab), not autoplay; the player bar reads
      "From artist".
- [x] Same from the Top result when it is a song, and when it is the artist (all their tracks).
- [x] With shuffle on: the clicked song plays first, the artist's other tracks follow shuffled.
- [ ] A big artist (hundreds of tracks): the song starts about as fast as any other play; a few
      seconds later the queue grows to the artist's full list (shuffled in when shuffle is on).
- [ ] Right after such a play, add a song to the queue (or start another play) before the rest
      arrives: the late tracks do not land in the changed queue.
- [ ] Search page: the playing song's row shows the spinning mark on its cover (still when
      paused), in both the overview preview and See all.
