# Player bar polish — manual smoke checklist

GUI smoke checks for the player bar three-zone layout, button animations, and
Now Playing entrance (2026-09-28). Requires a human, a real account, and a
browser login. Run with `npm --prefix ui run app:dev`.

## Layout

- [ ] Bar is slightly taller than before (80px): cover 52px, play button 44px,
      icon buttons 30px — everything still vertically centered and proportionate.
- [ ] Bar is a three-zone layout: transport controls sit **dead-center** in the
      window, not just between the sides.
- [ ] Left zone order: cover → title/artist → heart → elapsed/total time.
- [ ] Center zone order: shuffle, prev, play, next, repeat.
- [ ] Right zone order (left→right): queue button, volume slider, quality
      badge, bit-perfect badge (when active).
- [ ] Frosted glass background is unchanged.
- [ ] Seek (wave on): the wave is fully visible at rest — full amplitude and
      opacity, no longer squashed/faint.
- [ ] Seek hover/focus: the wave grows a touch further (~1.3x), the unplayed
      portion brightens, and the bar's top separator line fades out so only
      the progress shows.
- [ ] While scrubbing, the wave stays grown even if the pointer leaves the
      bar's top edge mid-drag.
- [ ] Seek (wave off, Instrument): the plain line rests at 4px and grows to
      6px + thumb on hover.
- [ ] Long track titles still ellipsize; shrink the window and confirm the
      left zone truncates before pushing the center off-middle.
- [ ] Narrow the window under ~900px: the bit-perfect badge hides first; the
      quality badge stays.
- [ ] "Nothing playing" state still renders; trigger a playback error and
      confirm the error + Retry float over the bar's right side without
      shifting the zones.

## Button animations

- [ ] Icon buttons: hover shows a round halo + slight lift; press dips
      noticeably (deeper than other buttons in the app).
- [ ] Play button: hover grows it with a soft accent glow; toggling play/pause
      crossfades/scales the icon swap (no hard flip).
- [ ] Favoriting the current track pops the heart (quick scale bounce) as it
      fills; unfavoriting does not animate.
- [ ] Shuffle/repeat/queue active states show accent color **plus** a small
      accent dot under the icon; the dot scales in/out on toggle.
- [ ] Cover thumbnail hover: slight zoom + chevron-up hint overlay.

## Now Playing open/close

- [ ] Clicking the cover opens Now Playing: view rises + fades in, artwork
      scales up from ~92%, then title/byline/badges/next-up stagger in.
- [ ] Closing (click artwork or Esc) dissolves Now Playing in place over the
      returning view — no layout jump or scrollbar flash in the main area.
- [ ] Repeat open/close rapidly: no stuck states, animation restarts cleanly.
- [ ] Check both cover shapes (sleeve and disc via appearance settings).
- [ ] Queue rail open/close alongside Now Playing still behaves.

## Reduced motion

- [ ] With OS "reduce motion" on: entrance becomes a plain fade (no
      rise/scale/stagger), hover/press animations are effectively instant.
