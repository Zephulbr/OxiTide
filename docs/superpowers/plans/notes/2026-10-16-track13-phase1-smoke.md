# Track 13 Phase 1 — owner smoke (pending)

Branch `track13-efficiency` at `7f73920`. Release or dev build; a real account.

## Queue only on change (EF-D1)

- [ ] Play an album: the queue rail lists it. Add a track (Add to queue), Play next, drag to
      reorder, remove one, Clear: the rail follows every edit at once.
- [ ] With a long queue (a big playlist, Play all), drag the volume slider back and forth:
      smooth, no hitch; the rail still shows the full queue afterwards.
- [ ] Shuffle on/off and repeat cycle: rail order and flags stay right.
- [ ] Reload the webview (Ctrl+R in dev) mid-queue: the rail repopulates.
- [ ] Phone remote (if paired): queue page and jump-to-track still work.

## Volume (EF-D2)

- [ ] Slider drag: audio follows smoothly; after release, restart the app — the volume is kept.
- [ ] Rack knob: drag, wheel fast in both directions (no lost steps), arrows/PageUp; restart
      — the last value is kept.
- [ ] Mute toggle and the volume hotkeys still work.

## Session (EF-D3)

- [ ] Play, pause, quit from the tray, relaunch: queue, track and position restored.
- [ ] Play for over a minute, end the process from Task Manager, relaunch: queue and track
      restored; position within the last 30 s.

## Paused is idle (EF-D4)

- [ ] Pause for a minute, resume: playback continues at the same spot, no glitch.
- [ ] While paused, change volume, then resume: the new volume applies.
- [ ] While paused: seek, next/previous, change EQ — each behaves as before.
- [ ] Atmosphere Now Playing with the disc: pause — it spins down and stops; play — it spins
      up from where it stopped.
- [ ] Rack: pause — needles and bars fall to rest and the recorder freezes; play — meters move
      again within a fraction of a second.
- [ ] Sleep timer fade-to-pause still ends paused at the user's volume.
