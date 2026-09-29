# Track 4b discovery — GUI smoke checklist

- **Date:** (fill in)
- **Build:** dev run from `.worktrees/track4b-discovery`, account: (fill in country/region if known)
- **Spec:** `docs/superpowers/specs/2026-09-30-oxitide-track4b-discovery-design.md`

## API probe (optional, before clicking)

In the dev WebView inspector console (`Ctrl+Shift+I`, dev builds only):

```js
await window.__TAURI_INTERNALS__.invoke("get_for_you")
await window.__TAURI_INTERNALS__.invoke("get_track_radio", { trackId: <id from any visible track> })
await window.__TAURI_INTERNALS__.invoke("get_artist_radio", { artistId: <id from an artist page> })
await window.__TAURI_INTERNALS__.invoke("get_similar_artists", { artistId: <id> })
```

Record each returned count. Empty results are acceptable only if the UI also degrades
(hidden section / info toast) — note the observed region behavior.

## Checklist

- [ ] Mixes & Radio shows For You section(s) above "Favorite mixes"; cards show art/title/subtitle.
- [ ] Clicking a For You mix replaces the queue and plays it, like a favorite mix.
- [ ] Favorite mixes grid unchanged; both empty → "No mixes yet."
- [ ] Track menu → "Track Radio" from Album, Playlist, Tracks, and Explore; queue replaced, playback starts.
- [ ] Artist page: radio button plays artist radio (queue replaced).
- [ ] Artist page: Similar Artists row renders with round art; clicking navigates to that artist; back works.
- [ ] A radio with no playable tracks (or a region gap) shows the info/error toast and leaves playback untouched.
- [ ] Feed still shows release activity (unchanged).
- [ ] Playback, queue, and the rest of the app behave as before (no regressions).

## Results

- (fill in per item; attach log snippets for any failure)
