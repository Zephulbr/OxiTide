# Detail header long-title overflow — manual smoke checklist

GUI smoke check for the `DetailHeader` overflow fix (2026-10-03). The shared
header is used by Playlist, Album, and Mix; the title clamps to two lines with
an ellipsis and a native hover tooltip, and unbroken strings break instead of
running off screen. Requires a human and a real account (a playlist with a
long name). Run with `npm --prefix ui run app:dev`.

Result: pending owner smoke.

## Reproduction (pre-fix)

- [ ] Create or rename a playlist to an unbroken name of ~150+ characters
      (the 2026-10-03 repro: `Testbfhudbsouhbnoufdhsbnouhgfdsuhdassbhuigreshbuiydsrhuygfiru…`).
      Before the fix the title ran past the right edge of the window.

## Fixed behavior

- [ ] Open that playlist: the title wraps to at most two lines, ends in an
      ellipsis, and nothing extends past the header; Play/Shuffle and the
      action cluster keep their positions.
- [ ] Hover the title: the native tooltip reveals the full name.
- [ ] A long title containing spaces clamps the same way.
- [ ] The stats line (`N TRACKS …`) still renders directly under the clamped
      title.
- [ ] Album with a long unbroken title: same clamp; the artist subtitle wraps
      (no unbroken string escaping the hero) and Favorite/Add/Share stay put.
- [ ] Mix with a long title: same clamp.
- [ ] Short titles stay on one line with no ellipsis; hovering one still
      shows the full-name tooltip.
- [ ] Descender-heavy short titles (e.g. `Playing Games`, `Symphony No. 5`)
      render fully — no clipped g/y/p tails from the clamp's clip box.
- [ ] A title that exactly fills two lines clamps without an ellipsis; only
      a real overflow onto a third line shows one.

## Regression

- [ ] Check both Atmosphere and Instrument modes and both densities.
- [ ] Narrow the window: the hero meta column shrinks without horizontal
      overflow (cover + title + controls stay usable).
- [ ] Play/Shuffle still start the queue, and rename/edit flows still work.
