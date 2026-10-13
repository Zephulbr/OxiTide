# Track 9.5c — Hi-Fi Rack follow-ups (completion note)

- **Date:** 2026-10-10
- **Branch:** `track9-5c-rack` (from `main` `8e7f4f4`)
- **Spec:** `docs/superpowers/specs/2026-10-08-oxitide-track9-5-hifi-rack-design.md` §9 (RK-D15…D18)
- **Design:** `ui/design/rack/DESIGN.md` (§2 fold/mini/queue, §4 segment font, §5.11 changer)

## What shipped

- **Narrow-window fold** (RK-D15): below 1000px wide the column folds into
  `RackStrip.svelte` under a full-width display (cover window, deck VFD, transport and
  play-mode keys, Now Playing key, 0.8× volume knob); it rearranges again below 860px
  and 760px. Now Playing while folded stacks its units in one scrolling column.
- **Rack mini player** (RK-D16): `RackMini.svelte`, a compact deck over a compact amp
  (Bars meter + knob) with the queue as a unit sheet. `App.svelte` picks it from the same
  `appearance.rackShell` flag; the old Rack skin over `Mini.svelte` was removed from
  `rack.css`.
- **Queue changer unit** (RK-D17): `ChangerUnit.svelte` takes the Playlists unit's slot
  while the queue is open (DISC / OF / REMAIN VFD, five paged disc keys with lamps,
  full `QueueRail` in a glass window). The drawer remains when folded or in Now Playing.
- **Segment font** (RK-D18): DSEG7 Classic Bold Italic (OFL 1.1, unmodified, 5KB) at
  `ui/src/assets/fonts/dseg7-classic-bold-italic.woff2`, listed in
  `THIRD_PARTY_NOTICES.md`. `SevenSeg` now draws a ghost layer and a lit layer
  (`segment.ts`); it is also used for the CD face's REMAIN time and the changer.
- Refactor: `DeckVfd.svelte` and `DeckKeys.svelte` are extracted from `DeckUnit`, so the
  deck, strip and mini player share one VFD and one set of key trays.

## Verification

| Gate | Result |
|---|---|
| `npm --prefix ui run check` | 0 errors, 0 warnings |
| `npm --prefix ui test` | green (new: changer paging/slots, segment layers, `ChangerUnit` component contract) |
| Segment font render | Checked in a static page: ghost and lit layers line up cell for cell at 14–64px |
| Rust gates | No Rust changes on this branch |
| Owner visual review + smoke below | **Closed 2026-10-13** - owner accepted with Track 9; rows below were not ticked individually |

## Smoke checklist (human, real account, Mode = Hi-Fi Rack)

- [ ] Narrow the window below 1000px: the column disappears, the strip shows under the display, and every control works (transport, seek, shuffle/repeat/♥, queue drawer, Now Playing, knob). Widen it again: the column comes back with the same state.
- [ ] Below 860px the keys move under the VFD. Below 760px the cover hides. At the 720×520 minimum it is still usable.
- [ ] Now Playing while folded scrolls as one column; Esc returns.
- [ ] Queue key with the column visible: the changer replaces Playlists. The DISC/OF/REMAIN readout is right, the current disc lamp is lit, ◂ ▸ page through the queue, a disc key plays that track, and the page follows the current track again after a track change. The close key and Esc restore Playlists.
- [ ] Mini from Rack opens the Rack mini player: cover, VFD (title, segment time, seek, annunciators), keys, Bars meter moving, knob, Restore. The queue sheet opens and closes with its key, the backdrop and Esc. Below 400px tall the cover hides.
- [ ] Segment digits (deck time, CD REMAIN, changer) are crisp, the unlit segments faintly visible, and they follow lamp colour and brightness.
- [ ] Atmosphere/Instrument mini player and shell are unchanged.
