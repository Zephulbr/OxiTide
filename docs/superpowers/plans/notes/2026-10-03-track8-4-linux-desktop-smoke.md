# Track 8.4 Linux desktop integration smoke checklist (2026-10-03)

WSLg-verifiable checks for Plan 8.4 (MPRIS, Settings copy, app launch, deep
links, Wayland geometry) plus the deferred real-DE items. Companions:
`2026-10-03-track8-wslg-smoke.md` (umbrella WSLg run),
`2026-10-03-track8-bare-metal-checklist.md` (Track 8 closure gate), and the
Track 8 spec §4.3–§4.6.

Environment: WSL2 Ubuntu 22.04 (Plan 8.1, `docs/linux-build.md`), WSLg, a
session D-Bus. Install once:

```bash
sudo apt install -y playerctl xdg-utils desktop-file-utils
```

Record each item as PASS/FAIL/BLOCKED with date and evidence. FAIL becomes an
issue; do not silently skip.

## App launch and Settings copy (Task 3)

- [ ] Launch the app (`npm --prefix ui run app:dev` with Vite warm, or the
      installed deb from Plan 8.3) under WSLg: the window renders and no
      GTK/WebKit init errors appear in the terminal.
- [ ] Settings → Connections shows **Media controls** (no "Windows" wording)
      and the platform-neutral description.
- [ ] Uncheck Media controls → `playerctl -l` no longer lists the player (or
      lists it Stopped with empty metadata); re-check → it returns without a
      restart. Playback is unaffected either way.

## MPRIS via playerctl (spec §4.3)

- [ ] `playerctl -l` lists `org.mpris.MediaPlayer2.OxiTide`.
- [ ] `playerctl status` reflects Playing/Paused; `playerctl metadata` shows
      `xesam:title`, `xesam:artist`, `xesam:album`, `mpris:length`, and an
      `mpris:artUrl` on `resources.tidal.com`.
- [ ] Track change (next or auto-advance): metadata flips to the new track;
      no stale title/art (Review Focus 1).
- [ ] `playerctl position` advances during playback; `playerctl position 30`
      seeks and the next reading is ~30 s (no backward jump). Then
      `playerctl position 60` while playing and let the track end: `Seeked`
      is not emitted during normal progress (Review Focus 2).
- [ ] `playerctl play-pause`, `playerctl play`, `playerctl pause`,
      `playerctl next`, `playerctl previous`, `playerctl stop` all drive the
      app. Rapid `play-pause` then `stop` does not desync the widget
      (Review Focus 3).
- [ ] Raise focuses from another WSLg window:
      `gdbus call --session --dest org.mpris.MediaPlayer2.OxiTide --object-path /org/mpris/MediaPlayer2 --method org.mpris.MediaPlayer2.Raise`
- [ ] Clearing the queue / stopping playback clears the metadata instead of
      leaving the last track.
- [ ] Quit while playing: no hang and the bus name disappears
      (`playerctl -l` no longer shows it).

## Deep links in WSLg (spec §4.4; requires Plan 8.3)

- [ ] After the first run, the runtime registration (`register_all()`,
      `crates/oxitide-app/src/main.rs:251`) leaves a desktop entry carrying
      `MimeType=x-scheme-handler/tidal;`:
      `grep -rl "x-scheme-handler/tidal" ~/.local/share/applications/*.desktop`
- [ ] Installed deb: `grep -qx 'MimeType=x-scheme-handler/tidal;' /usr/share/applications/OxiTide.desktop`
      after `sudo apt install ./target/release/bundle/deb/*.deb`.
- [ ] `xdg-mime query default x-scheme-handler/tidal` returns the OxiTide
      entry (Plan 8.3 requires `xdg-utils` + `desktop-file-utils`).
- [ ] Best effort: `xdg-open 'tidal://login/auth?code=smoke'` reaches the app
      under WSLg. Record the result either way; full browser-redirect +
      single-instance verification stays deferred.
- [ ] Windows snapshot untouched:
      `git diff -- crates/oxitide-app/src/scheme.rs` is empty (the HKCU
      registry guard is Windows-only; Linux uses the `.desktop` handler).

## Wayland geometry (spec §4.5)

- [ ] Record the session: `echo "$XDG_SESSION_TYPE $WAYLAND_DISPLAY"` (WSLg
      is Wayland).
- [ ] Size restore and entering/exiting mini still work. **If** mini exit
      shows an error toast (i.e. `set_position` rejects on Wayland and
      `apply_geometry` propagates it), apply the Task 4 guard in
      `window.rs` and re-run the Windows gates; record which happened.
- [ ] Saved-position restore degrades to a compositor-placed window; no
      crash, no off-screen window. Under X11 (bare metal) both restore fully.

## WebKit CSS findings (spec §4.6; Track 9 freeze; Plan Task 5)

- WebKitGTK version (`dpkg-query -W -f='${Version}\n' libwebkit2gtk-4.1-0`):
- `CSS.supports("color", "color-mix(in srgb, red 50%, blue)")`:
- `CSS.supports("backdrop-filter", "blur(2px)")` / `-webkit-` prefix:
- `CSSPropertyRule` present in `document.styleSheets` (i.e. `@property` parsed):
- Visual: dynamic-color crossfade (Now Playing), glass blur (sidebar/player
  bar/menu), accent washes (search focus, playing row): observed / broken.
- Fixes applied: none / list with `file:line`.
- The oldest-base WebKitGTK 4.1 floor check remains on the bare-metal
  checklist.

## Deferred to real hardware / real DE

- [ ] GNOME and KDE media widgets show metadata/status; their hardware media
      keys route to OxiTide (`2026-10-03-track8-bare-metal-checklist.md`).
- [ ] `xdg-open tidal://...` from a real browser flow launches/focuses the app
      and hits the single-instance path (bare-metal checklist).
- [ ] X11 restores window position fully; Wayland matches the documented
      degradation (bare-metal checklist).
- [ ] WebKitGTK 4.1 on the oldest base (Ubuntu 22.04 / Debian 12) renders
      both modes after the CSS pass (bare-metal checklist).
