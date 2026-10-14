# Network resilience smoke (2026-10-14)

Owner smoke for the network-resilience commits: TIDAL client retries (`9e9bb29`), buffered
play-out and segment retries (`f86cffd`), download-ahead (`0a4c36e`), network wait
(`ea0af3e`), and offline cache playback (`b22e63f`). Windows, real account, `app:dev` or an
installed build. Toggle the network with Wi-Fi off/on or by unplugging the cable.

Status: **pending**

## Download ahead

- [ ] Play a Max (hi-res) track. Within ~30 s its file is complete in
      `%APPDATA%\OxiTide\cache` as `<id>-max-<hash>.fmp4` with a `.fmp4.meta` beside it,
      while the track is still playing.
- [ ] On a slow line (throttle with a metered hotspot if available), the playing track does
      not stutter while the next track's download waits its turn.
- [ ] Seek far into an uncached track: a `spill-*.part` file appears and disappears when
      the track changes.

## Mid-track drop

- [ ] Once a track is fully downloaded (above), turn the network off: the track plays to
      its end, with no error.
- [ ] Turn the network off shortly after a new track starts (before it is on disk): audio
      continues while the buffer lasts, then the player bar reads "waiting for network…"
      (Rack: BUF lamp). Turn the network back on within a minute: playback resumes at the
      same position within ~15 s, with no error toast.
- [ ] While "waiting for network…", press pause, restore the network, press play: it
      resumes where it stopped (not at 0:00).
- [ ] Leave the network off for more than 5 minutes: the error appears, and Retry after
      reconnecting resumes at the recorded position.

## Track boundary and offline cache

- [ ] Network off while track N plays, after N+1 has fully downloaded: N+1 plays.
- [ ] Network off, then play a track that was played earlier at the same quality: it plays
      from the cache, without an error.
- [ ] Network off, then play a track never played before: "waiting for network…", then
      it plays once the network is back.

## Housekeeping

- [ ] Settings → clear cache removes `.fmp4`, `.fmp4.meta`, and `.part` files, and does
      not touch `palettes.json` / `lyrics.json`.
