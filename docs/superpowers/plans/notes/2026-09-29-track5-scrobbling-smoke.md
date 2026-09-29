# Track 5 scrobbling — manual smoke

Prereqs: a real Last.fm app key/secret (export `OXITIDE_LASTFM_API_KEY` and
`OXITIDE_LASTFM_API_SECRET`), a Last.fm account, a ListenBrainz account + token,
and a real TIDAL login. Run `npm --prefix ui run app:dev`.

- [ ] Last.fm: Settings → Connections → Connect opens the browser; authorizing
      flips the row to "Connected as <user>" and the toggle is enabled.
- [ ] ListenBrainz: pasting the token and clicking Connect shows the username;
      a bogus token shows an inline error and stays disconnected.
- [ ] Play a track longer than 30 s and let it pass half; the scrobble appears on
      both services' profiles within a minute.
- [ ] Now-playing shows on Last.fm while the track plays, and refreshes after a
      pause longer than 30 s.
- [ ] Turn Last.fm off; only ListenBrainz receives the next scrobble. Turn it back on.
- [ ] Disconnect while offline mid-track; go back online; the queued count drains
      (session-only queue — confirm it does NOT survive an app restart).
- [ ] Revoke/invalidate the ListenBrainz token, then play; one error toast appears,
      the row says "Reconnect needed", and Last.fm keeps scrobbling.
- [ ] A track under 30 s never scrobbles; skipping before half never scrobbles;
      `repeat=one` scrobbles each loop.
- [ ] Disconnect clears the row back to Connect and the keyring entry is gone.
- [ ] Both modes (Atmosphere/Instrument) and both densities render the rows;
      no unicode glyphs appear.
- [ ] With the OS reduced-motion setting on, the Connections rows render and the
      Last.fm "Waiting for authorization" spinner/status stays usable (motion suppressed).
- [ ] Window resized to minimum width keeps the rows usable.
