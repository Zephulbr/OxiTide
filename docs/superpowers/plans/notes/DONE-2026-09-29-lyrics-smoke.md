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

## Layout modes (follow-up)

- [ ] LYRICS tab: the cover sits left, the now-playing details (eyebrow, title,
      byline, badges) render centered under the cover, and the right column is
      pure lyrics starting at the top.
- [ ] The vertical LYRICS / NEXT UP spine sits just right of the cover and reads
      top-to-bottom; both labels switch modes from both layouts.
- [ ] NEXT UP tab: the layout returns to cover-left / details-and-list-right,
      with the spine still present.
- [ ] A track without lyrics shows no spine and keeps the plain Next Up layout.
- [ ] At the 720×520 minimum window, cover, details, and spine fit in lyrics
      mode without clipping.
- [ ] Spine active state (accent edge) and focus ring look right in both modes
      and both densities.

## Full window and motion (second follow-up)

- [ ] Opening Now Playing hides the sidebar and the docked queue rail; the view
      spans the whole window above the player bar. Esc restores both.
- [ ] The floating queue overlay still opens over Now Playing on narrow windows.
- [ ] Switching LYRICS / NEXT UP glides the columns (~0.5 s): the cover drifts
      left and shrinks, the details cross-fade to under the cover, and the
      lyrics fade in — no hard jump.
- [ ] Switching back reverses the motion; the Next Up list fades in.
- [ ] With reduced motion on (OS setting), the mode switch is instant and no
      slide/fade transitions play.

## Bug-fix follow-ups (third pass)

- [ ] Next-up rows lay out as thumb | title over artist | duration right; the
      duration is never pinned next to the thumb and titles are not pushed to
      the far edge.
- [ ] The cover never jitters while switching modes: the details slide in/out
      underneath it and the cover recenters smoothly.
- [ ] In the Lyrics tab the cover is larger (up to ~56vh) but its center stays
      where it was; the details sit a little lower.
- [ ] Lyrics text is large (~24px comfortable / ~23px compact), with a wide gap
      (~96px) between the cover group and the lyrics (62% stage share, 1800px
      composition cap **in lyrics mode only** — the Next Up tab keeps its
      original 1520px composition), and still wraps cleanly at the minimum
      window; the details below the cover are never clipped at 720×520.

## Morph pass (fourth follow-up)

- [ ] Cover size is identical in both tabs at every window size (shared cap
      `min(60vh, 100vh − 360px)`); switching tabs only steps it left/right.
- [ ] Opening LYRICS: the right-column details, divider, and Next Up list rise
      away; the lyrics rise in from below; the details appear under the cover
      rising into place.
- [ ] The lyrics panel appears already anchored on the current line (no
      scroll-from-the-top), rising once while the columns settle; later
      follows and click-to-seek still glide smoothly.
- [ ] Returning to NEXT UP mirrors: lyrics descend away, details/list descend
      in from above, under-cover details descend away, cover steps right.
- [ ] At ≤1200px window width the lyrics column stays usable (stage 54%,
      gap 48px); at 720×520 nothing overlaps or clips.
- [ ] Reduced motion: every morph is instant, no slides or fades.
