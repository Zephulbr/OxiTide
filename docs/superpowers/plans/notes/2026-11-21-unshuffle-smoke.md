# Shuffle off restores the order — owner smoke

**Status:** Implemented on `release/1.x` (`42e514b9`), shipping in v1.1.8; owner smoke passed
(all 7 checks, 2026-11-21).

From the reporter's third comment on https://github.com/Zephulbr/OxiTide/issues/7: on the phone,
the shuffled tracks stayed in the queue after shuffle was turned off. The desktop did the same:
`Queue::set_shuffle(false)` left the shuffled order in place on purpose.

## What changed

- **Turning shuffle off puts the context back in its original order**, and playback goes on
  from the current track's place in it, as in Spotify (`crates/oxitide-audio/src/queue.rs`,
  `Queue::unshuffle`). Tracks heard while shuffled may come up again; the current track keeps
  playing.
- Each context track carries its place, `QueueItem::ordinal`: set when a play starts (before any
  shuffle), continued by the late-loaded rest (`extend_context`), copied back on a queue jump
  (the UI's re-sent list has none: the field is `#[serde(skip)]`, never sent over IPC or to the
  phone), and saved in the session (`SessionItem::ordinal`).
- **What stays put:** songs you added (the added run) stay right after the current track in
  their order; a song dropped into the queue stays after the track it was dropped after;
  Autoplay's suggestions stay at the end.
- **An added song playing when shuffle goes off** has no place of its own. If shuffle was
  turned on while it played, it sits at the top with the songs added after it, and the whole
  list follows in order. Otherwise it stays after the list song it followed.
- **A session saved before 1.1.8** has no places: turning shuffle off there keeps the order,
  as before. The next play gets them.
- Not in this change: keeping added songs when a new playlist starts (an engine change planned
  for a later 1.x release), and a separate "Next in queue" section with the added songs saved
  across restarts (planned for `main`).
- Tests: `queue.rs` (`shuffle_off_*`, `places_are_not_sent_over_ipc`), `engine.rs`
  (`shuffle_off_retargets_a_stale_prefetch`), `session.rs` (`the_original_place_survives_a_save`).

## Checks (installer, a real account)

- [x] Play a playlist in order from its third song, turn shuffle on, skip a few tracks, turn
      shuffle off: the queue is the playlist in order, the song playing keeps playing, and the
      next one is the one after it in the playlist.
- [x] Same with Liked Songs' Shuffle button (a fully shuffled start), then shuffle off.
- [x] With shuffle on, "Add to queue" two songs, then turn shuffle off: the two songs still
      play next, in the order added.
- [x] With shuffle on, click a later song in the queue rail (a queue jump), then shuffle off:
      the order comes back.
- [x] Shuffle on, quit the app, reopen, shuffle off: the order comes back.
- [x] From the phone remote: shuffle on, then off: the phone's queue shows the playlist order
      and marks the playing song.
- [x] Autoplay on, let a list run out so suggestions join, shuffle on then off: the suggestions
      are still at the end.
