# Track 11 phone remote — human smoke checklist

Real phone, real account, OxiTide built from `track11-phone-remote`
(`npm --prefix ui run app:build` for the release path; `app:dev` exercises the Vite proxy).
Record pass/fail and notes inline.

**Result (owner, 2026-10-11): PASS** — every section passed on a real phone and account;
the Linux item is deferred to the batched Linux test run. Owner feedback (heart sync lag,
device rename, polish, infinite scroll, volume without the bar) was addressed in smoke
round 1 (`31da50a`) — see the plan record.

## Setup and pairing
- [x] Settings → Connections → Phone remote off by default; no listener (`netstat -ano | findstr 47820` empty).
- [x] Turn on: status shows `http://<LAN IP>:47820/remote`; Windows firewall prompt appears — choose Private.
- [x] Pair a phone: QR + URL + countdown; phone camera opens the page; "Pair <phone>?" prompt on the PC; Allow → phone shows Now Playing; address bar has no `#p=` token.
- [x] Decline on a second attempt → phone shows "Pairing failed … declined"; the same QR no longer works.
- [x] Approval off → a new phone pairs without a prompt.
- [x] Second phone paired; both listed with connected dots; revoke one → it returns to "Pair this phone" within seconds; the other keeps working. *Owner asked for renaming — added in round 1.*
- [x] Wrong interface case (WSL/Hyper-V installed): Auto picks the Wi-Fi/Ethernet address; manual override works.

## Now Playing
- [x] Cover, title, artist/album links, quality badges, bit-perfect/DSP badges match the PC.
- [x] Play/pause, previous/next, seek while playing, shuffle, repeat cycle, volume (does not change `config.toml`), Pause with fade.
- [x] Heart toggles the favorite. *Owner: PC/phone hearts did not update live — fixed in round 1 (`favorite_changed` + SSE `favorite`).*
- [x] Lyrics: synced highlight follows playback; plain lyrics scroll; "No lyrics" state.
- [x] Desktop actions (skip on the PC) appear on the phone within ~1 s.

## Queue
- [x] Long queue (≥1,000, ideally an S7-size 10k context): scrolls smoothly, rows load on scroll.
- [x] Tap to jump; remove; drag to reorder; Clear (with confirmation); changes from the PC show up.

## Discovery and library
- [x] Search → play from here, Add to queue, Add to playlist…, Favorite, Go to album/artist, Track radio.
- [x] Home / For You / Explore / My Mixes shelves open albums, playlists, mixes, artists; feed tracks play.
- [x] Library: favorite tracks (Load more), albums, artists, playlists + folders, history.
- [x] Album / artist (Play, Radio, top tracks, albums, similar) / playlist (paged) / mix pages.

## Resilience
- [x] PC logout → phone shows "Sign in on the PC"; log back in → phone resumes without re-pairing.
- [x] PC sleep/wake or app closed → phone shows reconnecting, then recovers (no blank page).
- [x] Phone screen off / tab backgrounded → back to foreground reconnects with a fresh state.
- [x] Remote turned off mid-session → phone stream ends; turning on again reconnects.
- [x] Phone on cellular (Wi-Fi off) cannot reach the server.
- [x] Firewall: deny the prompt → phone cannot connect; Settings guidance fixes it.
- [ ] Linux (WSL2 or bare metal) build: server reachable; `ufw` hint shown. **Deferred** (owner, 2026-10-11) to the batched Linux test run; the Linux cargo gates pass.

## Performance (PM-D1)
- [x] S1–S3, S5–S7 with the remote **enabled and one phone connected** (simulated SSE client), plus a disabled control — 2026-10-10, see `2026-10-10-track11-completion.md`. No remote-attributable regression; seven pre-existing breaches noted there.
