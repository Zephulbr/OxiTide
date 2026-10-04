# Track 10.2 sleep-timer smoke checklist (2026-10-04)

Human check with a real account and audio. Run `npm --prefix ui run app:dev`
(Vite + Tauri). Use the custom option with 1 minute for the expiry check.
Mark each item pass/fail with a note; failures become plan fixes, not silent
exceptions.

1. Duration fade
   - Play a track. Moon menu → Custom → 1 minute.
   - PASS = the moon shows a live countdown (1:00, 0:59, …); at 0:00 the volume
     ramps down over ~5 s, audio pauses at silence, and the volume slider is
     back at its prior value.
2. Resume
   - Press play after the fade. PASS = playback resumes at the paused position
     at full volume.
3. End of track
   - Pick a track with more than ~10 s left, arm End of current track, and seek
     to ~6 s remaining. PASS = the volume fades over the last ~5 s and playback
     pauses at/near the end; no audible sliver of the next track. Pressing play
     continues normally.
4. Off / cancel
   - Arm 15 minutes, then choose Off. PASS = the countdown clears and no fade
     happens later.
5. Queue clear
   - Arm 15 minutes, clear the queue. PASS = the moon returns to off.
6. Manual pause during the fade
   - Arm 1 minute and wait for the ramp to start, then press pause.
     PASS = volume returns to the prior level and playback stays paused.
7. Volume during the fade
   - Arm 1 minute and wait for the ramp to start, then move the volume slider.
     PASS = the fade stops, the chosen volume takes effect, and playback keeps
     playing.
8. Restart resets
   - Arm a timer, quit and relaunch. PASS = the timer is off.
9. Exclusive mode (only if configured)
   - Repeat 1 with exclusive output. PASS = same behavior, and the system
     volume is back to normal afterward.

Record results below.

- [ ] 1 duration fade
- [ ] 2 resume
- [ ] 3 end of track
- [ ] 4 off
- [ ] 5 queue clear
- [ ] 6 pause cancels
- [ ] 7 volume cancels
- [ ] 8 restart resets
- [ ] 9 exclusive
