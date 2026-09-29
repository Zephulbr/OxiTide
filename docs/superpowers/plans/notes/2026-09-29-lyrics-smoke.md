# Lyrics — manual smoke checklist

GUI smoke checks for the Track 4a lyrics feature (2026-09-29). Requires a human,
a real account, and a browser login. Run with `npm --prefix ui run app:dev`.
Reference: `docs/superpowers/specs/2026-09-29-oxitide-track4a-lyrics-design.md`.

## Real-response check (fixture-first)

- [ ] Play a well-known track with lyrics, open Now Playing: the lyrics panel
      appears and lines highlight in time. If they don't, capture the raw
      TIDAL response (temporary `tracing::debug!` in `track_lyrics`), compare it
      with `TidalLyrics`/`parse_lrc`, and update the DTO/tests to match.
- [ ] TIDAL's response for the track shows in the lyrics panel (not just lrclib):
      pick a track you expect TIDAL to cover and confirm lines render.
- [ ] A track TIDAL lacks falls back to lrclib and still renders.

## Sync behavior

- [ ] The active line tracks playback and stays in sync through pause/resume
      and a seek by clicking the waveform.
- [ ] Clicking a lyric line seeks to it and following resumes.
- [ ] Manual wheel/drag scroll pauses following; the jump-to-current glyph
      appears; clicking it resumes and recenters the active line.
- [ ] Reduced motion (Windows "Show animations" off) jumps instead of scrolling.
- [ ] RTL lyrics (if any track can be found) render right-aligned.

## States

- [ ] Track with no lyrics keeps the plain Next Up column (no tabs, no empty panel).
- [ ] Instrumental track keeps Next Up.
- [ ] Unsynced lyrics render as plain text: no highlight, no auto-scroll, no cursor.
- [ ] Track change while Now Playing is open swaps lyrics without showing the
      previous track's lines.
- [ ] Rapidly skipping tracks never shows mismatched lyrics.
- [ ] Closing and reopening Now Playing on the same track shows lyrics instantly.

## Cache and restart

- [ ] On a second visit to a track, lyrics appear without a loading row.
- [ ] Quit and relaunch, then play a cached track: lyrics appear immediately
      (disk cache) with no lrclib traffic.
- [ ] A track with no lyrics is not re-queried after a restart (no loading row,
      Next Up immediately).

## Design match

- [ ] Looks correct in both Atmosphere and Instrument modes and both densities.
- [ ] Resize the window to the 720×520 minimum: lyrics scroll, artwork stays
      centered, nothing overlaps the player bar.
- [ ] Tabs match the quiet Micro labels elsewhere; the jump glyph is an inlined
      Lucide icon (no unicode).
