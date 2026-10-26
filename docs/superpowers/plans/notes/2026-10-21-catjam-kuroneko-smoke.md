# CatJam & Kuroneko: manual smoke checklist

Spec: `docs/superpowers/specs/2026-10-21-oxitide-catjam-kuroneko-design.md` (§8).
Plan: `docs/superpowers/plans/2026-10-21-catjam-kuroneko.md`.

You need a real account, a real window, and speakers or headphones you can hear the beat
on. Record the date and result for each item, plus the `lead_ms` values where asked.
The values are in DevTools: `playback_event` payloads of type `position`.

## Jamming cat: settings

- [x] Settings → Appearance → Extras shows **Jamming cat** under Cat. With reduced
      motion on, the description says it is paused.
- [x] Turning it on shows the **Coat** row with six cats. Arrow keys move between
      them, and the chosen one is marked. Turning it off hides the row.
- [x] Settings search for "coat" or "catjam" lands on the Jamming cat row.
- [x] Restart: the toggle and the coat are restored.

## Jamming cat: on the beat

- [x] It shows only in the default shell's player bar (not the Rack, not the mini
      player), between the track title and the heart. Each Size step (100–200 %) fits the bar;
      with a long title the heart moves right by the cat's width and nothing overlaps. Clicking the cover under it
      still opens Now Playing.
- [x] With nothing analyzed yet, the cat sits still (idle frame). Within about 10 s of a
      track starting, it starts bobbing.
- [x] Steady 4/4 tracks: house/techno (~125 BPM), rock (~100–140), hip-hop
      (~85–95) and drum & bass (~174, may bob at half time). The head lands on the
      beat. Note any track where it drifts.
- [x] Ambient, beatless or rubato track: the cat **sways** slowly (eyes closed) and
      doesn't pretend to keep a beat.
- [x] Seek forward and back: the cat stays (or is back) on the beat within a few
      seconds. Seeking back into an analyzed part is in time at once.
- [x] Replay a track heard in this session: it is on the beat from the start.
- [x] Pause: the cat freezes on its frame. Resume: it picks up on the beat.
- [x] Minimize or hide to the tray and come back: it is in time again.
- [x] No visible frame skip every half second during steady playback.
- [x] Each coat looks right on a light cover and a dark cover.

## Sync per output (CJ-D9)

Note `lead_ms` and whether the head lands on the beat, early, or late.

- [x] WASAPI shared: `lead_ms` = ____ ; sync: ____
- [x] WASAPI exclusive: `lead_ms` = ____ ; sync: ____
- [x] ASIO (can wait for the pending ASIO smoke): `lead_ms` = ____ ; sync: ____
- [x] If all three are off by the same amount, set `BEAT_TRIM_MS` in
      `ui/src/lib/beatClock.ts`. Bluetooth output is expected to look late (its
      delay isn't reported; a per-device trim is deferred).

## Privacy (CJ-D1)

- [x] DevTools → Network while switching through five tracks with the cat on: only
      TIDAL's own requests appear. Nothing goes to `api.vmohammad.dev` or any other
      new host.
- [x] The app log has no outgoing request tied to beat analysis.

## Cost (Track 13 budgets)

- [x] Rerun S11 (idle paused) and S12 (playing, hidden to tray) with the jamming cat
      on: both within budget. Record the runs in the Track 13 results note.
- [x] With the cat off, beat tracking is off (no `beat` events in DevTools).

## Kuroneko & petting

- [x] With Cat on, right-click oneko: kuroneko (black, white lines) appears beside her
      and the webview's own menu doesn't open. Right-click kuroneko: she leaves.
- [x] Kuroneko trails oneko around the window and stops a little way from her.
- [x] Double-click either cat while something plays: both walk to the seek bar and
      curl up side by side. Double-click again: both wake. Summon kuroneko while
      oneko sleeps: she goes to sleep beside her.
- [x] Drag each cat separately. While dragging, shaking her floats no hearts.
- [x] Rub the cursor back and forth over oneko: hearts float up in the accent colour and
      she closes her eyes for a moment. The same works on kuroneko. Just passing over a
      cat floats nothing.
- [x] Rub a sleeping cat: hearts float, and she stays asleep.
- [x] Restart with kuroneko out: she comes back where she was.
- [x] Reduced motion on: no oneko, no kuroneko, no jamming cat.

**Closed 2026-10-26 (owner):** the remaining rows were marked done by the owner, not ticked row by row.
