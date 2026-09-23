# Plan 5 Smoke Checklist

> Requires a human, a logged-in TIDAL account, an audio device, and the built
> installer. Run with `npm --prefix ui run app:dev` (or the installed app).

## Automated checks (tree at dd6622e)
- cargo test --workspace: 216 passed, 0 failed, 1 ignored (oxitide-app 31, oxitide-audio 139 + 1 ignored [requires an audio device], oxitide-audio pipeline 1, oxitide-tidal 45, doc-tests 0)
- cargo clippy --workspace -- -D warnings: clean, 0 warnings (exit 0)
- npm --prefix ui test: 26 passed, 0 failed across 5 files (format 2, quality 5, player 6, player.restore 3, pager 10)
- npm --prefix ui run check: 0 errors, 0 warnings (exit 0)
- npm --prefix ui run app:build: exit 0; `target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe` (4,349,769 bytes), `target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi` (6,377,472 bytes); `Test-Path` True for both

## Manual checks
- [ ] Install → launch → upgrade-in-place → uninstall (see plan5-release.md).
- [ ] Session expiry: while a track plays, revoke the session (TIDAL account → "Log out of all devices"; if unavailable, delete the keyring entry and wait for the access token to expire), then click Library. Expect "Session expired — sign in to continue"; after login the queue and position resume (paused if it was paused).
- [ ] Network off while browsing: the view shows the error with Retry; restore the network and press Retry — the view loads.
- [ ] Network off mid-stream: the player bar shows the error with Retry; restore the network and press Retry — playback restarts near its last position.
- [ ] Window geometry survives a restart; a maximized window reopens maximized.
- [ ] A saved position on a disconnected monitor is ignored: the window opens at the default position with its saved size (edit config.toml to an off-screen position to simulate).
- [ ] %APPDATA%\OxiTide\logs\oxitide.<date>.log exists and grows; Settings → Logs shows the directory and Open logs folder opens Explorer there.
- [ ] Library with a large favorites list paints after the first page, appends while scrolling, and "Play all" shows "Loading all…" then queues the full list.
- [ ] Playlist paging behaves the same; revisiting Library/Playlist within a session does not refetch (session cache).

## Notes

The manual items above require the human owner: a real TIDAL account (browser
login), a real audio device, and the built installer. The automated portion ran
on 2026-09-24 against tree dd6622e; no manual box is checked.
