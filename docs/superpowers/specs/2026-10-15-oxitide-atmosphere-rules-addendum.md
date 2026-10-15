# OxiTide — Atmosphere rules addendum (2026-10-15)

- **Status:** Approved (owner, 2026-10-15)
- **Owner:** V
- **Amends:** `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (the 2b record).
  Where the two disagree, this addendum wins. The living rules are in
  `ui/design/DESIGN.md` and `ui/design/atmosphere/DESIGN.md`, updated in the same change.
- **Context:** an owner walkthrough of the Atmosphere UI (polish pass, queue overhaul,
  now-playing mark) re-reviewed the 2b rules that the shipped UI had outgrown or that
  had drifted from the code. All eight items below were accepted. AR-9 and AR-10 were
  added the same day during the Home redesign
  (`2026-10-15-oxitide-home-redesign-design.md`), each approved by the owner after
  review in the running app.

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

## AR-9 Queue glide v2: slower rail, page content moves with it

Amends AR-5. The owner found the 260 ms cubic-in-out dock "too fast/snappy". The main
column also re-flowed every frame during the glide (auto-fill grids jumping, covers
resizing), which read as jitter.

- **Rail:** 380 ms on `--ease-in-out` (`cubic-bezier(.4, 0, .2, 1)`). The floating queue
  and its backdrop fade use the same duration and curve.
- **Page:** held at its final width for the glide, so it lays out once. That one change is
  FLIP-animated with the rail's duration and curve (`lib/queueTransition.ts`):
  - every on-screen element travels from its old box to its new one (transform);
  - elements with a visible box (cards, panels) also animate their **width**, so their
    edges glide. This is the new layout animation the rule required a ruling for. It
    lasts only for the glide.
  - elements the new width hides (`display: none`) or clips (carousel cards past the
    row's new edge) leave an inert, `aria-hidden` copy at the old spot that fades out over
    60% of the glide. Elements it reveals fade in.
- **Sync:** opening snapshots the page before the rail enters the DOM and parks the
  glide on its first frame until the rail's `introstart`, so both start on the same
  frame. Svelte starts a transition's real animation one frame after insertion.
- **Reduced motion:** no glide. The page changes width once, and the rail keeps its
  120 ms fade.
- **Width-driven layout must be CSS, not measured.** A JS count from a ResizeObserver
  lands a frame after the glide measures, and the page jumps mid-glide. Use container
  queries (Home's Jump back in does).
- **Perf:** AR-5's pending check (open/close over a 10k-row table, S6 conditions) now also
  covers the FLIP pass. Up to 2000 on-screen elements are measured twice per toggle.

## AR-10 Card hover and resting depth

Narrows principle 2 ("depth from blur and light, not drop shadows") for resting depth.
Extends AR-1's play-button allowance to the card play button.

- **Resting depth:** neutral dark drop shadows are allowed: `--shadow-panel` on panels,
  `--shadow-cover` with a 1px `--line` ring on every media-card cover. They are never
  accent-coloured, so they are not glows. Light uses softer warm-ink shadows. The offset
  "stacked sleeve" box behind playlist covers is retired.
- **Tinted panels:** `.lit-tint` (accent wash plus even accent hairline) may replace
  `.lit`'s bright-to-dark rim where a panel should take the music's colour (Home).
- **Media-card hover:**
  - the card's own cover colour spreads from the cover and blends into the card's
    surface. It wipes in with transform plus opacity, and the registered `--card-tint`
    glides from the accent when the palette arrives.
  - the cover lifts 3px, and a deeper shadow layer fades in.
  - tracks and cards that start playback show a 40px accent play button (30px on rows).
    It carries the AR-1 transport glow (≤ 10px blur, steady) and rises 8px into place.
- **Still vetoed:** glows on the card itself, idle motion, and a hover ring on non-current
  covers (AR-3 stays limited to the current track).
