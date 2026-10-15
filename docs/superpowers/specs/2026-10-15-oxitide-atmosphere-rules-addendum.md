# OxiTide — Atmosphere rules addendum (2026-10-15)

- **Status:** Approved (owner, 2026-10-15)
- **Owner:** V
- **Amends:** `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (the 2b record).
  Where the two disagree, this addendum wins. The living rules are in
  `ui/design/DESIGN.md` and `ui/design/atmosphere/DESIGN.md`, updated in the same change.
- **Context:** an owner walkthrough of the Atmosphere UI (polish pass, queue overhaul,
  now-playing mark) re-reviewed the 2b rules that the shipped UI had outgrown or that
  had drifted from the code. All eight items below were accepted.

## AR-1 Glow is allowed — as light from the music, not decoration

Supersedes the 2b record's "depth from blur and light, not drop shadows" reading as a
no-glow rule, and narrows the rejected "play-state bloom" (§14) to what it actually was:
a breathing bloom on the play button.

- A **glow** is a soft, zero-offset `box-shadow`/halo in `--accent` (or `--danger` on
  destructive controls), mixed with `color-mix`. It reads as emitted light.
- **Where it may appear:** the primary transport (play button), the **current track**
  (its cover in the queue / Now Playing, the playing row's mark), **focus/active field
  states**, slider thumbs while used, destructive confirm buttons on hover, and a soft
  halo behind the Now Playing cover.
- **Where it may not:** resting chrome — cards, panels, sidebar, headers, nav items,
  badges, toasts. Glows never stack on one element, and never animate on their own
  (no breathing, pulsing or bloom loops; they may fade with a state change).
- **Budget:** a small control's glow stays within ~10px blur; an ambient halo (behind a
  cover) stays at or below ~35% alpha so the artwork remains the brightest thing on
  screen (principle 1).
- Existing glows that this legitimizes: search/filter field focus, volume thumb,
  danger confirm hover.

## AR-2 Sanctioned micro-interactions

Supersedes §10.3 "toast slide-in with slight overshoot — the only one" and the heart-pop
veto in §14. The sanctioned set is now:

1. Toast slide-in with slight overshoot.
2. **Heart pop** when a track is favourited (scale 1 → 1.35 → 1 over `--dur-state`).
3. **Icon-button lift and press** in the player bar: hover lifts 1px, press squashes to
   0.92 (deeper than the global 0.98, for tactility).

Still vetoed: the play-button "breath" and any idle animation of chrome.

## AR-3 The current cover may be lit

Supersedes §7.3 "no ring around the cover". The current track's cover may carry a thin
accent ring and/or an AR-1 glow. **Accent left bars on rows stay vetoed**, as do toast
severity stripes.

## AR-4 The now-playing mark is the disc motif

Clarifies the identity budget (§3) and the tide-motif cap (§10.2). The spinning
now-playing mark (`NowPlayingMark.svelte`: queue current cover, playing table row,
sidebar playing playlist) is ruled a use of the existing **disc** element, not a fifth
signature. The tide motif is now: waveform seek; disc (loading spinner, Now Playing
disc art, now-playing mark). The reserved third slot (pull-to-refresh ripple) is
unchanged. "No animated equalizers" stands — bars are not the disc.

## AR-5 Layout animation: the queue dock glide is an approved exception

Amends the PM-D1 rule "only `transform` and `opacity` animate" (§9). The docked queue
opens and closes by animating its **width** (260 ms, `cubic-in-out`), with the rail kept
at full width inside and clipped so its rows never reflow. This re-lays-out the main
column for the length of the glide only; nothing animates layout at rest or per track.
Any further layout animation needs its own ruling. **Perf check pending:** the scripted
scenarios do not toggle the queue; measure an open/close over a 10k-row table (S6
conditions) before the next perf sign-off.

The hover/press colour washes (`background-color`, `color`, `border-color` over
`--dur-micro`) recorded in the 2026-10-14 polish pass remain the other exception.

## AR-6 Nested glass

Reaffirms §9: `backdrop-filter` surfaces stay a fixed set and never nest (Chromium drops
the inner blur anyway). The queue's drag ghost lost its blur and relies on the opaque
`--surface-float` veil. (Card badges blur over plain content, not glass, and are fine.)

## AR-7 Easing tokens renamed

The 2b tokens were named backwards (`--ease-in` was the entrance/decelerating curve).
Renamed everywhere in the UI:

| Token | Curve | Use |
|---|---|---|
| `--ease-enter` (was `--ease-in`) | `cubic-bezier(.2, .7, .3, 1)` | entrances, hover/state changes |
| `--ease-exit` (was `--ease-out`) | `cubic-bezier(.5, 0, .75, 0)` | exits |
| `--ease-in-out` | `cubic-bezier(.4, 0, .2, 1)` | two-way glides (unchanged) |

## AR-8 Superseded text in the 2b record

Read the 2b record with these corrections (the design files already describe the
current state):

- **Dark-only / light-ready (§4.4, §11):** light theme shipped (Track 9, §6.1 of
  `2026-10-05-track9-ui-completion-design.md`).
- **Player bar (§7.1):** 80px floating glass bar with the waveform seek as its top edge,
  a 52px cover thumb, and the transport **centred** (not 72px / 48px / centre-left).
- **Expanded Now Playing (§7.2):** the sidebar steps aside while it is open and the
  queue may stay docked beside it; synced lyrics shipped as a tab (Lyrics / Next up /
  Info), so the "reserved slot" is filled.
- **Queue rail slide (§9 `--dur-view`):** the dock now glides by width (AR-5).
- **Micro-interactions, glow, cover ring:** see AR-1 to AR-3.
