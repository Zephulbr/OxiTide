# Phone queue marking and opening, and punctuation-blind list filters — owner smoke

**Status:** Implemented on `release/1.x` for v1.1.7, not yet released; owner smoke open.

Found by the owner during the v1.1.6 smoke (`2026-11-20-v1.1.6-release.md`, "Owner's
follow-ups for 1.1.7").

- **Phone queue marks the playing and played tracks** (`remote/pages/Queue.svelte`). Before, the
  playing row only had an accent title, which a near-grey dynamic accent (a black-and-white cover)
  made invisible, and played rows looked like upcoming ones. Now the playing row has a
  `--surface-2` wash, a 3px accent bar, the accent title and the desktop's now-playing disc
  (`disc-3`) over its cover, turning while the PC plays; played rows fade to 50% like the desktop
  rail's.
- **Phone queue opens on the playing track.** It always opened at the top. It now scrolls to the
  playing row (one row of context above) when it opens, and follows it to the next track only
  while that row was on screen, so a listener who scrolled away is left there (the desktop
  rail's rule). The scroll maths are pure (`remote/queueDrag.ts`: `scrollTopFor`,
  `rowOnScreen`).
- **List filters ignore punctuation and accents** (`lib/searchText.ts`, desktop and phone). "AC DC"
  did not find AC/DC. The filter now folds case, accents and punctuation on both sides, matching
  each field in two forms (words split at punctuation, and punctuation dropped inside words), so
  "ac dc", "acdc" and "ac/dc" find AC/DC, "beyonce" finds Beyoncé, "guns n roses" finds Guns N'
  Roses, and "dont" finds Don't, while "ear" still does not find "The Arcade". Used by the
  desktop track filter (`trackView`: every track list), the desktop Artists filter
  (`artistSort`), and every phone filter (`remote/filter.ts`). Search (TIDAL's catalogue search)
  is unchanged: it is TIDAL's.
- Tests: `lib/searchText.test.ts`, `remote/queue.test.ts`, cases in `trackView.test.ts`,
  `artistSort.test.ts`, `remote/filter.test.ts`, `remote/helpers.test.ts`. The `warmTrackSearch`
  test now counts `normalize` calls (the fold) instead of `toLowerCase`.
- Not re-run: the perf harness's S6 filter scenario (10k rows). Folding costs more per row than
  lower-casing, but runs once per track (cached, and warmed in idle chunks); a keystroke still
  only compares strings.

## Checks (installer, a real account, phone on the same Wi-Fi)

- [x] Phone Queue tab while a song plays: the playing row stands out (wash, accent bar, disc over
      the cover), the disc turns; pause on the PC: the disc stops.
- [x] Rows above the playing one are faded; rows below are not.
- [x] A song with a black-and-white cover (low-colour accent): the playing row is still easy to
      find.
- [x] Open the Queue tab with the playing track far down the queue: it opens on that track, not at
      the top.
- [x] Leave the Queue tab open while a track ends: the view follows to the next track.
- [x] Scroll away from the playing row, let a track end: the view stays where you scrolled. The
      "Show the current track" button still brings it back.
- [x] Desktop Library → Tracks: "AC DC", "ac dc" and "acdc" each find AC/DC tracks; a playlist's
      filter does the same.
- [x] Desktop Library → Artists: "ac dc" finds AC/DC; an accented artist is found without the
      accent.
- [x] Phone Library → Tracks and a playlist: "ac dc" finds AC/DC.
- [x] A filter for a plain word still behaves as before (no surprising extra matches).
