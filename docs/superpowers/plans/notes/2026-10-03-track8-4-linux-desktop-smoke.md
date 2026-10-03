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

**Task 4 run (2026-10-04; WSL log timestamps `2026-10-03T23:2xZ`).** Built and
ran commit `ab8bdcf` in the WSL2 clone (`/root/oxitide-8.3`,
`npm ci --prefix ui && ./ui/node_modules/.bin/tauri build --no-bundle`,
incremental 58.5 s). Headless runs used `dbus-run-session` with
`DISPLAY=:0`, `WAYLAND_DISPLAY=wayland-0`, `XDG_RUNTIME_DIR=/run/user/0`,
`WEBKIT_DISABLE_COMPOSITING_MODE=1`,
`WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1` (root WSLg session), and
`HOME=/root`. `playerctl` 2.4.1 installed via apt. Items marked **PENDING
(human)** need a real TIDAL login, playback, or a visible window.

## App launch and Settings copy (Task 3)

- [ ] Launch the app (`npm --prefix ui run app:dev` with Vite warm, or the
      installed deb from Plan 8.3) under WSLg: the window renders and no
      GTK/WebKit init errors appear in the terminal.
      **PASS (headless) / PENDING (visual render)** 2026-10-04 — the `ab8bdcf`
      release binary stayed alive across four `dbus-run-session` runs (12–15 s
      each); the only log output was the expected keyring-unavailable warning
      (no `org.freedesktop.secrets` in WSL) and three GTK
      `Dropped Escape call with ulEscapeCode : 0x03007703` lines; no GTK/WebKit
      init error. Window rendering is not machine-checkable here.
- [ ] Settings → Connections shows **Media controls** (no "Windows" wording)
      and the platform-neutral description.
      **PENDING (human)** 2026-10-04 — the copy is verified in source by Task 3
      (`ui/src/views/Settings.svelte`, `task-3-report.md`); the live view needs
      the GUI.
- [ ] Uncheck Media controls → `playerctl -l` no longer lists the player (or
      lists it Stopped with empty metadata); re-check → it returns without a
      restart. Playback is unaffected either way.
      **PENDING (human)** 2026-10-04 — needs the Settings UI plus an
      authenticated session (the MPRIS session is spawned only after auth,
      `commands.rs:401-416`).

## MPRIS via playerctl (spec §4.3)

Blocked on a real TIDAL login in this environment: `build_player` is what spawns
`MediaSession` (`crates/oxitide-app/src/commands.rs:401-416`), and WSL has no
`org.freedesktop.secrets` service, so the app starts with a session-only token
store, sits on the login screen, and `playerctl -l` returns `No players found`.
The whole section is therefore PENDING (human, login + playback); nothing here is
a product failure.

- [ ] `playerctl -l` lists `org.mpris.MediaPlayer2.OxiTide`.
      **PENDING (human)** 2026-10-04 — pre-login observed `No players found`
      (exit 0); pre-login `gdbus ... Raise` returned
      `org.freedesktop.DBus.Error.ServiceUnknown`.
- [ ] `playerctl status` reflects Playing/Paused; `playerctl metadata` shows
      the five metadata fields including an art URL.
      **PENDING (human)** 2026-10-04 — login + playback.
- [ ] Track change (next or auto-advance): metadata flips to the new track;
      no stale title/art (Review Focus 1).
      **PENDING (human)** 2026-10-04 — login + playback.
- [ ] `playerctl position` advances during playback; seeks land at ~30 s/60 s
      with no spurious `Seeked`; the track end is clean (Review Focus 2).
      **PENDING (human)** 2026-10-04 — login + playback.
- [ ] `playerctl play-pause`, `play`, `pause`, `next`, `previous`, `stop` all
      drive the app; rapid `play-pause` then `stop` does not desync (Review
      Focus 3).
      **PENDING (human)** 2026-10-04 — login + playback.
- [ ] Raise focuses from another WSLg window.
      **PENDING (human)** 2026-10-04 — the bus name does not exist pre-login
      (see the `ServiceUnknown` evidence above); focus is visual.
- [ ] Clearing the queue / stopping playback clears the metadata instead of
      leaving the last track.
      **PENDING (human)** 2026-10-04 — login + playback.
- [ ] Quit while playing: no hang and the bus name disappears
      (`playerctl -l` no longer shows it).
      **PENDING (human)** 2026-10-04 — `SIGTERM` teardown left no process
      behind, but the quit-while-playing path needs playback (and no bus name
      was registered pre-login).

## Deep links in WSLg (spec §4.4; requires Plan 8.3)

- [ ] After the first run, the runtime registration (`register_all()`,
      `crates/oxitide-app/src/main.rs:251`) leaves a desktop entry carrying
      `MimeType=x-scheme-handler/tidal;`:
      `grep -rl "x-scheme-handler/tidal" ~/.local/share/applications/*.desktop`
      **PASS** 2026-10-04 — grep exit 0, matched
      `oxitide-app-handler.desktop`. After the first run its `Exec` was
      rewritten from the stale AppImage path to
      `Exec="/root/oxitide-8.3/target/release/oxitide-app" %u`, i.e.
      `register_all()` at `ab8bdcf` ran.
- [ ] Installed deb: `grep -qx 'MimeType=x-scheme-handler/tidal;' /usr/share/applications/OxiTide.desktop`
      after `sudo apt install ./target/release/bundle/deb/*.deb`.
      **PASS** 2026-10-04 — `apt-get install -y --reinstall
      /root/oxitide-8.3/target/release/bundle/deb/OxiTide_0.2.0_amd64.deb` exit
      0 (`oxi-tide 0.2.0`); the exact `grep -qx` exit 0.
- [ ] `xdg-mime query default x-scheme-handler/tidal` returns the OxiTide
      entry (Plan 8.3 requires `xdg-utils` + `desktop-file-utils`).
      **PASS** 2026-10-04 — returned `oxitide-app-handler.desktop` (exit 0);
      association present in `~/.local/share/applications/mimeinfo.cache`.
- [ ] Best effort: `xdg-open 'tidal://login/auth?code=smoke'` reaches the app
      under WSLg. Record the result either way; full browser-redirect +
      single-instance verification stays deferred.
      **BLOCKED (xdg-open in bare WSLg) + PASS via GIO** 2026-10-04 —
      `xdg-open` exits 4 with `/usr/bin/xdg-open: 811: : Permission denied`;
      `XDG_UTILS_DEBUG_LEVEL=3` shows `Selected DE generic` and
      `search_desktop_file` taking `first_word` of the quoted `Exec`, so `which`
      finds nothing and xdg-open:811 runs an empty command. That is xdg-open's
      shell parser hitting the spec-legal quoted path written by
      `tauri-plugin-deep-link` (GNOME/KDE route xdg-open through
      `open_gnome`/`open_kde`). Routing proof instead: `gio open
      'tidal://login/auth?code=smoke'` (exit 0) and `gio launch
      /root/.local/share/applications/oxitide-app-handler.desktop
      'tidal://login/auth?code=smoke'` (exit 0) both made the *already-running*
      instance log `deep-link login failed: no login in progress` — the URL
      arrived through the single-instance forward (the synthetic `code=smoke`
      cannot complete a PKCE login). Focus/raise is visual → deferred.
- [ ] Windows snapshot untouched:
      `git diff -- crates/oxitide-app/src/scheme.rs` is empty (the HKCU
      registry guard is Windows-only; Linux uses the `.desktop` handler).
      **PASS** 2026-10-04 — empty output in the Windows worktree.

## Wayland geometry (spec §4.5)

- [ ] Record the session: `echo "$XDG_SESSION_TYPE $WAYLAND_DISPLAY"` (WSLg
      is Wayland).
      **PASS (recorded)** 2026-10-04 — `XDG_SESSION_TYPE` is unset in the WSL
      login environment; `WAYLAND_DISPLAY=wayland-0`, `DISPLAY=:0`,
      `XDG_RUNTIME_DIR=/run/user/0` (WSLg symlinks `wayland-0` into the runtime
      dir). WSLg is Wayland plus XWayland.
- [ ] Size restore and entering/exiting mini still work. **If** mini exit
      shows an error toast (i.e. `set_position` rejects on Wayland and
      `apply_geometry` propagates it), apply the Task 4 guard in
      `window.rs` and re-run the Windows gates; record which happened.
      **PENDING (human)** 2026-10-04 — entering/exiting mini is a GUI action,
      so the failure could not be produced or ruled out. **Guard not needed /
      unobserved**: Step 5 was not applied and
      `crates/oxitide-app/src/window.rs` is untouched.
- [ ] Saved-position restore degrades to a compositor-placed window; no
      crash, no off-screen window. Under X11 (bare metal) both restore fully.
      **PENDING (human, visual)** 2026-10-04 — fresh config has no saved
      position; startup logged no `failed to apply window geometry` warning and
      the process never crashed, but off-screen/placement behaviour needs eyes.

## WebKit CSS findings (spec §4.6; Track 9 freeze; Plan Task 5)

- WebKitGTK version (`dpkg-query -W -f='${Version}\n' libwebkit2gtk-4.1-0`):
  **2.50.4-0ubuntu0.22.04.1** (observed 2026-10-04).
- `CSS.supports("color", "color-mix(in srgb, red 50%, blue)")`: **PENDING
  (Task 5)** — needs an in-webview JS probe; the release bundle exposes no
  usable inspector (with `WEBKIT_INSPECTOR_SERVER=127.0.0.1:9222` the port
  binds, but `/`, `/json`, `/json/list`, `/json/version` all return empty
  replies).
- `CSS.supports("backdrop-filter", "blur(2px)")` / `-webkit-` prefix:
  **PENDING (Task 5)** — same probe.
- `CSSPropertyRule` present in `document.styleSheets` (i.e. `@property`
  parsed): **PENDING (Task 5)** — same probe.
- Visual: dynamic-color crossfade (Now Playing), glass blur (sidebar/player
  bar/menu), accent washes (search focus, playing row): **PENDING (human)** —
  visible-window check.
- Fixes applied: none (no failure observed in this task; Task 5 owns fixes).
- The oldest-base WebKitGTK 4.1 floor check remains on the bare-metal
  checklist.

## Deferred to real hardware / real DE

Not exercised by Task 4; still owned by
`2026-10-03-track8-bare-metal-checklist.md`.

- [ ] GNOME and KDE media widgets show metadata/status; their hardware media
      keys route to OxiTide (`2026-10-03-track8-bare-metal-checklist.md`).
- [ ] `xdg-open tidal://...` from a real browser flow launches/focuses the app
      and hits the single-instance path (bare-metal checklist).
- [ ] X11 restores window position fully; Wayland matches the documented
      degradation (bare-metal checklist).
- [ ] WebKitGTK 4.1 on the oldest base (Ubuntu 22.04 / Debian 12) renders
      both modes after the CSS pass (bare-metal checklist).
