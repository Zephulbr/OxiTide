# Plan 2 Smoke Checklist

> Note: The items below require a human with a logged-in TIDAL account, a browser
> login, and an audio device. They are PENDING a human run and are intentionally
> left unchecked. Automated checks (cargo test/clippy, UI test/check) were run
> separately and passed — see the Task 16 report for exact counts.
>
> Run with `npm --prefix ui run app:dev`.

- [ ] Clicking ▶ on an album track starts playback (sound is audible).
- [ ] Player bar shows the track title and artist.
- [ ] Play/pause toggles playback; the icon updates.
- [ ] Next/prev move through the album's tracks.
- [ ] The progress bar and elapsed time advance during playback (seeking itself is not implemented yet).
- [ ] The volume slider changes loudness.
- [ ] Clicking Queue shows the queue with the current track.
- [ ] Queue reorder (drag), remove, and clear work.
- [ ] Settings lists output devices and changing quality persists.
- [ ] A track at Max (Hi-Res) plays (24-bit/192 kHz source is decoded).
- [ ] Logging out stops playback and clears the queue.
