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
- [ ] Frosted glass background is unchanged; the bar has **no separate top
      border** — the seek itself is the top edge.
- [ ] Seek (wave on) at rest is **completely straight**: a 2px line (played
      portion in accent) straddling the bar edge, no wiggle at all.
- [ ] On hover/focus the flat line fades as the wave inflates **upward** from
      the edge (~1.2x, full opacity) and the unplayed portion brightens to
      `--text-muted`; un-hover collapses it back to the straight line.
- [ ] Above the wave you always see the content behind (track list, sidebar,
      queue) — never a bar-background band.
- [ ] No dark gap under the wave while it is grown: the cap fills up to the
      curve in the backdrop-wash color (growing in lockstep with the stroke),
      including over the sidebar and queue rail bottoms.
- [ ] The cap strip is solid backdrop wash, not raw content: hover the wave
      over the sidebar's bottom nav and over a scrolled track row — the strip
      hides what's behind it and reads as a continuation of the bar.
- [ ] The cap tracks the bar per cover: a bright cover gives a light strip,
      a black-and-white cover a gray one (not the warm fallback), and the
      color crossfades on track change. With dynamic color off it stays the
      static room tone.
- [ ] No faint doubled-tint line where the cap meets the bar's glass: the
      fill's bottom edge lands exactly on the bar edge while grown/dragging
      (check the hairline along the top of the glass).
- [ ] Scroll a track list so rows slide beneath the hanging wave.
- [ ] The seek hit strip covers ~9px of the content above the bar: hovering
      or clicking there seeks rather than hitting the row (should feel
      intentional; main has 16px bottom padding).
- [ ] While scrubbing, the wave stays grown even if the pointer leaves the
      wave mid-drag.
- [ ] Seek (wave off, Instrument): the plain line rests at 3px and grows to
      5px + thumb on hover.
- [ ] Mini player (inline seek) is unaffected: full wave at rest when enabled,
      otherwise the 4px line with a visible thumb.
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
