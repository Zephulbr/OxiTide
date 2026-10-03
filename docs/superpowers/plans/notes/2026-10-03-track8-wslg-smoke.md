# Track 8 WSLg smoke checklist (2026-10-03)

Human-in-the-loop checks for the WSL2-verifiable half of Track 8, per
`docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` §5.3.
Run inside the WSL2 Ubuntu 22.04 environment built by Plan 8.1
(`docs/linux-build.md`), with WSLg for GUI windows and shared audio. These
items may complete in WSL2 (spec §5.4); the bare-metal items live in
`2026-10-03-track8-bare-metal-checklist.md`.

Record each item as PASS/FAIL with date, distro/kernel, and evidence
(screenshot, command output, log excerpt). A FAIL becomes an issue; do not
silently skip an item.

## Shell, login, and keyring

- [ ] **GUI launch (8.1–8.3).** Start Vite (`npm --prefix ui run dev`) and
      launch the debug binary from the repo root under WSLg; the OxiTide
      window renders and the terminal shows no GTK/WebKit init errors.
- [ ] **Login flow (8.1).** With no Secret Service running, Settings/Account
      and the login view show the session-only notice; browser PKCE login
      (WSLg `xdg-open`) + paste-back completes and the shell loads home.
- [ ] **Session-only keyring notice (8.1).** Quit and relaunch → still logged
      out (tokens were memory-only) and the notice explains why. Start
      `gnome-keyring-daemon` (or another Secret Service), log in again,
      relaunch → still logged in.
- [ ] **Shared audio (8.2).** Play a track; WSLg routes through PulseAudio
      (`PULSE_SERVER`), audio is audible on the Windows host, position
      advances, and pause/resume/seek/next work. WSLg exposes no `hw:` PCM:
      expect Shared or `ExclusiveFallback` notices, never a crash.
- [ ] **MPRIS via playerctl (8.4).** With a session D-Bus,
      `playerctl -l` lists the player; `status`, `metadata`, `play-pause`,
      `next`, `previous`, and `stop` mirror the app, and `Raise` focuses the
      window. (Check once Plan 8.4 has landed.)

## Packaging launch (8.3)

- [ ] **deb.** `sudo apt install ./target/release/bundle/deb/*.deb` in WSL2;
      the installed binary launches under WSLg and its `.desktop` entry
      carries `MimeType=x-scheme-handler/tidal;`.
- [ ] **AppImage.** `chmod +x` the AppImage and run it under WSLg; the window
      renders and the version matches the build.
- [ ] **`tidal://` best effort.** If `xdg-open tidal://...` reaches the app
      under WSLg, record it; full registration/launch verification is on the
      bare-metal checklist.

Close-out: when every item is PASS (or explicitly recorded as exempt), note
the result here and proceed to the completion gate in
`2026-10-03-track8-bare-metal-checklist.md` / spec §5.4.
