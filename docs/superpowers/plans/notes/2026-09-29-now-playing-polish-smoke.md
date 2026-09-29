# Smoke — Now Playing polish + karaoke lyrics (2026-09-29)

- **Status:** Closed — owner-verified 2026-09-30.

Plan: `docs/superpowers/plans/2026-09-29-now-playing-lyrics-polish.md`.
Human GUI checklist — needs a real account, a browser login, and
`npm --prefix ui run app:dev`.

## Karaoke lyrics (synced track — e.g. a well-known song with TIDAL subtitles)

- [x] Lines render big and bold (~30px, weight 700) — clearly larger than before
- [x] Active line is fully bright; upcoming lines sit dimmer (~50%)
- [x] Already-sung lines are dimmer than upcoming ones (~30%) — progress reads
- [x] Lines ≥4 away from the active line carry a slight blur (depth falloff)
- [x] Line-to-line transitions glide (opacity/blur animate, no snapping)
- [x] Active line rests ~35% from the top of the lyrics column
- [x] Watch item: no jank/stutter when the blur moves line to line (GPU cost);
      if it stutters, the knob is `filter: blur(2px)` in `LyricsPanel.svelte`

## Interaction regressions

- [x] Clicking a timed line still seeks and resumes following
- [x] Manual scroll pauses following; jump-to-current glyph appears and works
- [x] Hovering a tappable line brightens it (seek affordance)
- [x] Unsynced lyrics track: plain uniform list — no dim tiers, no blur
- [x] No-lyrics track: Next Up column unchanged
- [x] RTL track (if findable): alignment still correct

## View polish

- [x] Track change with the view open: artwork crossfades (no hard snap);
      disc restarting its spin angle from 0 is expected
- [x] Closing the view (click art): gentle sink + shrink on the fade, not a cut
- [x] Sleeve shape: soft drop shadow under the cover; disc already had one
- [x] Hovering the art (both shapes) lifts it slightly (~1.5%); press still dips
- [x] Lyrics ↔ Next Up tab morph feels calm (520ms glide); spine tab color
      animates instead of snapping
- [x] Lyrics-mode title under the art is slightly larger; spacing under art 24px

## Modes / a11y / layout

- [x] Reduced motion (Windows setting on): fades near-instant, no glide;
      blur tiers still apply (static, not motion)
- [x] Compact density: lyrics scale down with `--font-body-size`, layout holds
- [x] Instrument mode: same treatment, Inter at 700, no visual breakage
- [x] Narrow window (~min size): lyrics column stays usable
- [x] Both cover shapes (disc + sleeve) across a track change each

## Round 2 additions

- [x] Lyrics block sits ~48px closer to the artwork; cover, title, Next Up and
      player bar all unmoved (transform-only shift)
- [x] Active line sits vertically centered in the lyrics column
- [x] Words flip to the accent color in time with the vocals (cumulative trail
      stays accent until the line ends)
- [x] Pause freezes the trail mid-line; seeking re-anchors it; resuming continues
- [x] Untimed/unsynced tracks still render plain (no word spans, no accents)
- [x] If a track with enhanced-LRC word stamps can be found (lrclib), per-word
      timing is accurate; standard tracks use the length-weighted estimate
- [x] Estimation looks reasonable on fast and slow lines — words don't drag
      across long instrumental gaps
- [x] Lyrics text ~10% larger; nothing else in the view resized

## Round 3 additions (tab-swap choreography)

- [x] Next Up → Lyrics: the Next Up column leaves gently (no blink) while the
      layout glides; lyrics arrive as a crossfade, no empty beat between
- [x] Opening lyrics mid-song: lines settle around the current line (near
      lines first, top/bottom fill in over ~140ms) — no whole-list pop
- [x] Lyrics → Next up (reverse): same calm swap
- [x] Track change while lyrics are open: lines re-enter with the stagger,
      no hard swap
- [x] Reduced motion: slot transitions near-instant, lines appear instantly
      (no stagger, no delay)

## Round 4 additions (settled lyrics entrance)

- [x] Next Up → Lyrics mid-song: lines appear only after the glide settles —
      no word re-wrapping, no vertical snap; a brief empty column is expected
- [x] Opening Now Playing with cached lyrics: lines appear without an
      artificial wait
- [x] Skipping tracks while in the Lyrics tab: no 520ms empty-column wait
      (cached swaps mount at once)
- [x] Rapid Lyrics ↔ Next up toggling: the panel always comes back
- [x] Reduced motion: panel mounts instantly

## Round 5 additions (Next Up exit)

- [x] Next Up → Lyrics: the Next Up block stays vertically centered while it
      exits — no upward jump, no pop
- [x] The exit reads calm and ends as the lyrics cascade starts — no empty
      column beat
- [x] Lyrics → Next up: the incoming details/list appear centered from the
      start (not displaced by the fading lyrics)
- [x] Reduced motion: exits near-instant as before

## Round 6 additions (Next Up swap stability)

- [x] Lyrics → Next up on a long-title track: the rows fade in without any
      mid-swap shift; the block stays put as the column settles
- [x] The lyrics fade out (280ms) instead of vanishing in one frame
- [x] Next up → Lyrics still looks unchanged; forward swap unaffected
- [x] Narrow window: details/list still fit the column (the cap follows 100%)
- [x] Rapid Lyrics ↔ Next up toggling: the panel returns correctly

## Result

- Outcome: Closed — owner-verified 2026-09-30. Reviewed interactively across
  the polish rounds: karaoke tiers and word fill, both tab-swap directions,
  line entrance/stagger, the settled mount, the Next Up exit, and the swap
  stability fixes. All checks passed; no open watch items.
