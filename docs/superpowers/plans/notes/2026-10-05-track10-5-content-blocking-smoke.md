# Track 10.5 content blocking — GUI smoke checklist

Human, real account, Windows. Record pass/fail per step; steps 3–5 and 8 need a
second launch. Steps 7–8 need a real track flagged `explicit` or AI-labeled.

1. Settings opens on Appearance; the tab strip now reads
   Appearance · Playback · Content · Connections · Storage · System, and
   left/right keyboard navigation reaches Content.
2. Content shows both toggles off on a fresh config; copy mentions
   client-side enforcement and unknown flags playing.
3. Enable "Block explicit content": the confirm dialog appears; Confirm clears
   the queue (toast/queue rail empties) and playback stops. Cancel leaves the
   queue untouched.
4. With the filter on, an explicit track row is grayed/struck with an `E`
   badge; clicking its row or double-clicking shows the Settings toast and
   does not start playback; the context menu's Play / Track Radio / Add to
   queue are disabled; Add to playlist and Favorite still work.
5. Disable the filter: rows return to normal, and previously blocked tracks
   play again (turning a filter off is non-retroactive).
6. Block AI: first load may flash un-grayed rows, then AI rows gray out with
   an `AI` badge; "Play all" on that list starts at the first unblocked track
   with the corrected start index; "Add to queue" on a blocked row toasts.
7. Play a list whose current (first) track is AI-labeled, then enable Block
   AI: playback advances to the nearest unblocked track without restarting
   it, and the queue no longer lists the blocked item.
8. Pause mid-queue with a blocked item *after* the current one, enable Block
   AI: the queue drops it, the current track keeps playing at position, and
   toggling Block AI off does not re-add it.
9. Play from Home/Explore cards (`play_track_ids`): blocked tracks are
   skipped server-side; a fully blocked card errors with the toast instead of
   playing.
10. Restart the app with an explicit track as the saved current track and
    Block explicit on: restore comes back paused on the next kept track at
    position 0, and the queue no longer contains blocked items.
11. Offline check: disconnect the network, open a list view with Block AI on;
    rows render (unknown flags play), no error toast storm, playback works.
12. `%APPDATA%\OxiTide\config\cache\track_flags.json` exists after step 6/7 and its
    entries contain numeric keys with `explicit`/`ai`.
