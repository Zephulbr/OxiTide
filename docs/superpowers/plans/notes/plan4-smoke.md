# Plan 4 Smoke Checklist

> Requires a human, a logged-in TIDAL account, and an audio device. Run with
> `npm --prefix ui run app:dev`.

## Automated checks (tree at f593ad6)
- cargo test --workspace: 196 passed, 0 failed, 1 ignored (oxitide-app 19, oxitide-audio 131 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 45, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 13 passed, 0 failed across 3 files (format 2, quality 5, player 6)
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

## Manual checks
- [x] Play an album once, then replay it: the second play fetches no segments (cache hits).
- [x] The cache directory grows while playing and never exceeds the configured cap.
- [ ] Settings → Cache shows used/cap, and Clear cache empties the directory. #does not show correct value, stuck at 0.00Mb #fixed: shows used (+downloading) of cap with a Refresh button; pending human re-check
- [x] Clear on exit empties the cache when the app closes.
- [x] With fallback off, a track that cannot be served at the requested quality errors with a clear message; the queue is kept.
- [x] With fallback on, the same track plays and the player bar shows its true quality.
- [ ] A High (AAC 320 kbps) track plays; Low is retired (HE-AAC is unsupported by the built-in decoder). # High AAC works but when switching to Low AAC does not produce sound (song is stuck) #fixed: Low removed from Settings; HE-AAC now errors with a clear message; pending human re-check
- [x] The display toggle switches the badge between compact and detailed.
- [x] Detailed mode shows bit depth / sample rate / bitrate, with a dash where unknown.
- [x] Shuffle shuffles the rest of the queue once; the current track keeps playing.
- [x] Repeat cycles off → all → one from the player bar. 
- [x] Streaming, display, and cache settings survive an app restart.
- [x] Making the cache directory unwritable (or pointing cache.dir at a file) disables the cache with a Settings notice and playback still works.

## Smoke findings (2026-09-23)

- **Cache size read 0.00 MB (not a bug).** `used_bytes` counts committed `.fmp4` entries only; the clear-on-exit check had emptied the cache, and in-flight `.part` downloads were invisible. Fixed: Settings now shows `used (+downloading) of cap` with a Refresh button (one fetch on open; no polling).
- **Cache entry lifecycle.** A download creates `{trackId}-{quality}-{manifestHash}.fmp4.part`; it is renamed to `.fmp4` when the whole track is fetched, and deleted if the track is skipped/superseded. Committed entries disappear only when the LRU eviction brings the cache under the configured cap. At Max quality (~200 MB/track) the default 2 GiB cap holds only a few albums, so eviction churn during long sessions is expected.
- **Albums and playlists cache identically.** Caching is per track: key = track id + requested quality + normalized manifest hash, regardless of whether the track was played from an album, playlist, search, or the queue. The same track from two playlists shares one entry; the same track at different qualities gets separate entries.
- **Low (96 kbps HE-AAC) retired.** `symphonia-codec-aac` implements AAC-LC only and rejects SBR streams at init (`aac: aac too complex`), which is why Low looked stuck while High (320 kbps AAC-LC) worked. The Settings quality list now offers High (AAC 320 kbps), Lossless, and Max; a persisted `quality = "low"` is coerced to `high` on load, and any HE-AAC stream that still arrives fails with a clear message instead of hanging.
