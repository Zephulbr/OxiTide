# Track 8.1 keyring smoke checklist (2026-10-03)

Human-in-the-loop checks for the session-only keyring fallback (spec T8-D2,
plan 8.1). Run in WSL2 Ubuntu 22.04 under WSLg with a real TIDAL account and
`npm --prefix ui run app:dev`.

## Automated gates (WSL2 Ubuntu 22.04)

- [x] `cargo test --workspace` — all pass.
- [ ] `cargo clippy --workspace -- -D warnings` — clean.
- [x] `cargo build -p oxitide-app` — builds.
- [x] `npm --prefix ui run check` — 0 errors.
- [x] `npm --prefix ui test` — all pass.
- [x] `cargo tree -p oxitide-tidal -e features -i keyring` shows
      `sync-secret-service` + `crypto-rust` + `dbus-secret-service` (not the
      mock store).

## No Secret Service (the WSL2 default)

- [ ] The login screen shows the notice: cause ("can't reach the system keyring
      (Secret Service)"), consequence ("last only for this session"), and both
      provider links.
- [ ] Clicking each provider link opens the browser.
- [ ] Log in and play a track; no keyring error surfaces.
- [ ] Quit and relaunch: the app returns to the login screen (nothing
      persisted) and the notice is still present.
- [ ] After login, Settings → System → Account shows the same notice.
- [ ] Settings → Connections: connect Last.fm; it reports connected for the
      session (and disconnected after a restart).

## With a Secret Service (persistent path)

- [ ] `sudo apt install -y gnome-keyring`
- [ ] `dbus-run-session -- bash -lc 'echo -n "" | gnome-keyring-daemon --unlock --components=secrets; npm --prefix ui run app:dev'`
- [ ] Log in; relaunch; the session restores automatically and the notice is
      absent.

## Recorded results (2026-10-03)

- Passed: `cargo test --workspace` (WSL all green; Windows 212 app + 166 audio
  with 1 ignored + 1 pipeline + 179 tidal), `cargo build -p oxitide-app` (WSL),
  `npm --prefix ui run check` (0 errors), `npm --prefix ui test` (504 passed),
  and `cargo tree -p oxitide-tidal -e features -i keyring` (shows
  `sync-secret-service`, `crypto-rust`, and `dbus-secret-service`; no mock
  store).
- clippy: the `unused import: Error` blocker in
  `crates/oxitide-audio/src/output.rs` was fixed by cfg-gating the Windows-only
  `Error` import while keeping the cross-platform `Result` import; WSL clippy
  for `oxitide-audio` is clean. The `--workspace` gate is still red: with audio
  clean, clippy proceeds to `oxitide-app` and reports 14 pre-existing Linux
  dead-code/unused warnings in code untouched by plan 8.1 (`scheme.rs`,
  `media_controls.rs`, `perf.rs`, `state.rs`). Those files belong to later
  Linux bring-up work (Track 8.4 owns `media_controls.rs` and pins `scheme.rs`
  unchanged), so the box above stays unchecked pending a scope ruling.
- Environment: the WSL cargo gates ran as root on Ubuntu 26.04 (the provisioned
  dev substitute for 22.04); the UI npm gates ran on Windows because this
  worktree's `ui/node_modules` is Windows-native.
- Note: a first Windows workspace test run (concurrent with the WSL build) had
  two timing-flaky `oxitide-audio` engine failures; both passed standalone, the
  audio suite passed twice consecutively, and the full workspace suite passed
  when rerun serially, with no source changes.
- The human WSLg checks above are pending a human with a real TIDAL account;
  their boxes remain unchecked.
