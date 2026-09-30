# Discord Rich Presence — manual smoke checklist

Requires a real account, a running signed-in Discord desktop client, and the registered
application from Task 0. Record date and result per item.

- [ ] Settings → Connections → enable "Discord Rich Presence"; the choice persists after a
      restart and is honored on login.
- [ ] Play a track: the Discord profile card shows title, artist, album art (hover shows the
      album), the OxiTide small logo, a progress bar anchored to the real position, and a
      "Listen on TIDAL" button that opens the track in a browser.
- [ ] Status reads "Listening to OxiTide".
- [ ] Pause: the state line shows "Paused — {artist}" and the progress bar disappears; resume
      re-anchors the bar from the current position.
- [ ] Seek: within a few seconds the bar re-anchors and never shows a large offset.
- [ ] Rapid skips (5+ in a row): the card converges to the latest track.
- [ ] Stop playback / clear the queue: the presence clears.
- [ ] Toggle the setting off while playing: the presence clears; toggle on: it reappears with
      the current track, no restart needed.
- [ ] Discord closed at app start, then started: the presence appears without restarting
      OxiTide.
- [ ] Quit and restart Discord while a track plays: the presence returns within ~30 s.
- [ ] A track with no album art shows the OxiTide logo instead of a blank image.
- [ ] A track whose title/artist exceed 128 characters renders clipped, without errors.
- [ ] Logout and app exit: the presence clears; no hang.
- [ ] Playback, scrobbling, SMTC, and the UI are unaffected throughout.
