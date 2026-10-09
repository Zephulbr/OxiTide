# Track 11 phone remote — human smoke checklist

Real phone, real account, OxiTide built from `track11-phone-remote`
(`npm --prefix ui run app:build` for the release path; `app:dev` exercises the Vite proxy).
Record pass/fail and notes inline.

## Setup and pairing
- [ ] Settings → Connections → Phone remote off by default; no listener (`netstat -ano | findstr 47820` empty).
- [ ] Turn on: status shows `http://<LAN IP>:47820/remote`; Windows firewall prompt appears — choose Private.
- [ ] Pair a phone: QR + URL + countdown; phone camera opens the page; "Pair <phone>?" prompt on the PC; Allow → phone shows Now Playing; address bar has no `#p=` token.
- [ ] Decline on a second attempt → phone shows "Pairing failed … declined"; the same QR no longer works.
- [ ] Approval off → a new phone pairs without a prompt.
- [ ] Second phone paired; both listed with connected dots; revoke one → it returns to "Pair this phone" within seconds; the other keeps working.
- [ ] Wrong interface case (WSL/Hyper-V installed): Auto picks the Wi-Fi/Ethernet address; manual override works.

## Now Playing
- [ ] Cover, title, artist/album links, quality badges, bit-perfect/DSP badges match the PC.
- [ ] Play/pause, previous/next, seek while playing, shuffle, repeat cycle, volume (does not change `config.toml`), Pause with fade.
- [ ] Heart toggles the favorite (visible on the PC after refresh).
- [ ] Lyrics: synced highlight follows playback; plain lyrics scroll; "No lyrics" state.
- [ ] Desktop actions (skip on the PC) appear on the phone within ~1 s.

## Queue
- [ ] Long queue (≥1,000, ideally an S7-size 10k context): scrolls smoothly, rows load on scroll.
- [ ] Tap to jump; remove; drag to reorder; Clear (with confirmation); changes from the PC show up.

## Discovery and library
- [ ] Search → play from here, Add to queue, Add to playlist…, Favorite, Go to album/artist, Track radio.
- [ ] Home / For You / Explore / My Mixes shelves open albums, playlists, mixes, artists; feed tracks play.
- [ ] Library: favorite tracks (Load more), albums, artists, playlists + folders, history.
- [ ] Album / artist (Play, Radio, top tracks, albums, similar) / playlist (paged) / mix pages.

## Resilience
- [ ] PC logout → phone shows "Sign in on the PC"; log back in → phone resumes without re-pairing.
- [ ] PC sleep/wake or app closed → phone shows reconnecting, then recovers (no blank page).
- [ ] Phone screen off / tab backgrounded → back to foreground reconnects with a fresh state.
- [ ] Remote turned off mid-session → phone stream ends; turning on again reconnects.
- [ ] Phone on cellular (Wi-Fi off) cannot reach the server.
- [ ] Firewall: deny the prompt → phone cannot connect; Settings guidance fixes it.
- [ ] Linux (WSL2 or bare metal) build: server reachable; `ufw` hint shown.

## Performance (PM-D1)
- [ ] S1–S3, S5–S7 with the remote **enabled and one phone connected**; record vs. `scripts/perf/budgets.json`.
