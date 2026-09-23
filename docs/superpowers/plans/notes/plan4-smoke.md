# Plan 4 Smoke Checklist

> Requires a human, a logged-in TIDAL account, and an audio device. Run with
> `npm --prefix ui run app:dev`.

## Automated checks (tree at 4ce1cbb)
- cargo test --workspace: 190 passed, 0 failed, 1 ignored (oxitide-app 17, oxitide-audio 127 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 45, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 13 passed, 0 failed across 3 files (format 2, quality 5, player 6)
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)

## Manual checks
- [ ] Play an album once, then replay it: the second play fetches no segments (cache hits).
- [ ] The cache directory grows while playing and never exceeds the configured cap.
- [ ] Settings → Cache shows used/cap, and Clear cache empties the directory.
- [ ] Clear on exit empties the cache when the app closes.
- [ ] With fallback off, a track that cannot be served at the requested quality errors with a clear message; the queue is kept.
- [ ] With fallback on, the same track plays and the player bar shows its true quality.
- [ ] A Low or High track plays as AAC.
- [ ] The display toggle switches the badge between compact and detailed.
- [ ] Detailed mode shows bit depth / sample rate / bitrate, with a dash where unknown.
- [ ] Shuffle shuffles the rest of the queue once; the current track keeps playing.
- [ ] Repeat cycles off → all → one from the player bar.
- [ ] Streaming, display, and cache settings survive an app restart.
- [ ] Making the cache directory unwritable (or pointing cache.dir at a file) disables the cache with a Settings notice and playback still works.
