# Detail header long-title overflow — manual smoke checklist

GUI smoke check for the `DetailHeader` overflow fix (2026-10-03). The shared
header is used by Playlist, Album, and Mix; the title is a single ellipsized
line inside a 512px column, and hovering an overflowing title slowly pans it
left to reveal the end. Requires a human and a real account (a playlist with a
long name). Run with `npm --prefix ui run app:dev`.

Result: pending owner smoke.

## Reproduction (pre-fix)

- [ ] Create or rename a playlist to an unbroken name of ~150+ characters
      (the 2026-10-03 repro: `Testbfhudbsouhbnoufdhsbnouhgfdsuhdassbhuigreshbuiydsrhuygfiru…`).
      Before the fix the title ran past the right edge of the window.

## Fixed behavior

- [ ] Open that playlist: the title stays on a single line, ends in an
      ellipsis, and nothing extends past the header; Play/Shuffle and the
      action cluster keep their positions.
- [ ] On a wide window the kicker/title/subtitle/stats column ends around
      512px instead of stretching to the right edge, and a long subtitle wraps
      inside that same column.
- [ ] Hover the title: the line slowly pans left (linear, ~8s) until the end
      of the name is visible; the ellipsis disappears while panning and the
      line eases back in ~2.5s when the pointer leaves.
- [ ] A short title that fits the column shows no ellipsis and does not move
      on hover.
- [ ] A long title containing spaces gets the same single-line treatment (no
      wrapping) and pans the same way.
- [ ] Very long unbroken names pan their full length without escaping the
      column or clipping mid-glyph at rest.
- [ ] The stats line (`N TRACKS …`) still renders directly under the title.
- [ ] Descender-heavy short titles (e.g. `Playing Games`, `Symphony No. 5`)
      render fully — no clipped g/y/p tails from the clip box.
- [ ] Album with a long unbroken title: same behavior; the artist subtitle
      wraps (no unbroken string escaping the hero) and Favorite/Add/Share stay
      put.
- [ ] Mix with a long title: same behavior.

## Regression

- [ ] Reduced motion: with the OS "reduce motion" setting on, hovering an
      overflowing title reveals the end instantly (no pan) and nothing
      animates.
- [ ] Check both Atmosphere and Instrument modes and both densities.
- [ ] Narrow the window: the hero meta column shrinks without horizontal
      overflow (cover + title + controls stay usable).
- [ ] Play/Shuffle still start the queue, and rename/edit flows still work.
