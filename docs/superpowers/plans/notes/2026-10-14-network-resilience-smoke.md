# Network resilience smoke (2026-10-14)

Owner smoke for the network-resilience work, Windows, real account, `app:dev` or an
installed build:

- TIDAL client retries (`9e9bb29`)
- buffered play-out (`f86cffd`)
- download-ahead (`0a4c36e`)
- network wait (`ea0af3e`)
- offline cache playback (`b22e63f`)
- one retry policy + OS network events (`f32d318`)
- in-place download recovery (`da047ff`)
- offline start (`f9b1f39`)
- older cache entries offline (`4dc4455`)

Toggle the network with Wi-Fi off/on or by unplugging the cable. The log (`RUST_LOG`
info) shows `network down` / `network up` when Windows reports a change.

Status: **pending**

## Download ahead

- [ ] Play a Max (hi-res) track. Within ~30 s its file is complete in
      `%APPDATA%\OxiTide\cache` as `<id>-max-<hash>.fmp4` with a `.fmp4.meta` beside it,
      while the track is still playing.
- [ ] On a slow line (throttle with a metered hotspot if available), the playing track does
      not stutter while the next track's download waits its turn.
- [ ] Seek far into an uncached track: a `spill-*.part` file appears and disappears when
      the track changes.

## Mid-track drop (in-place recovery)

- [ ] Once a track is fully downloaded (above), turn the network off: the track plays to
      its end, with no error.
- [ ] Turn the network off shortly after a new track starts (before it is on disk): audio
      continues while the buffer lasts, then the player bar reads "waiting for network…"
      (Rack: BUF lamp). Turn the network back on after a minute: playback continues
      **within a second or two**, from exactly where it stopped, with no error toast.
- [ ] Same in **exclusive** output mode: on resume there is no device re-lock click (the
      device stays open; the log shows no output reopen).
- [ ] With crossfade/AutoMix on, a drop near the end of a track still crosses into the next
      track once the network is back.
- [ ] While "waiting for network…", press pause, restore the network, press play: it
      resumes where it stopped (not at 0:00).
- [ ] Leave Wi-Fi off for more than 5 minutes: it keeps waiting (Windows reports the
      network down, which does not count against the limit) and continues when Wi-Fi
      returns.
- [ ] Network "up" but no internet for more than 5 minutes (unplug the router's WAN, keep
      Wi-Fi): the error appears, and Retry after reconnecting resumes at the position.

## Track boundary and offline cache

- [ ] Network off while track N plays, after N+1 has fully downloaded: N+1 plays.
- [ ] Network off, then play a track that was played earlier at the same quality: it plays
      from the cache, without an error.
- [ ] Same for a track cached **before** this work (no `.meta` file beside it): it plays;
      ReplayGain is not applied to it.
- [ ] Network off, then play a track never played before: "waiting for network…", then
      it plays once the network is back.

## Offline start

- [ ] With the network off, start the app: it signs in from the saved session (no login
      screen); browsing pages fail as expected, and the queue/cached tracks play.
- [ ] Turn the network on: browsing works again without signing in (the token refreshes
      on the first request).
- [ ] Sign out: `config\tidal_session.json` is removed.

## Housekeeping

- [ ] Settings → clear cache removes `.fmp4`, `.fmp4.meta`, and `.part` files, and does
      not touch `palettes.json` / `lyrics.json`.
