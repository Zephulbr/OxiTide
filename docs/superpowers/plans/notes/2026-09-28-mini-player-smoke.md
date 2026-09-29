# Mini player smoke — 2026-09-28

Requires: real account, browser login, `npm --prefix ui run app:dev`.

Implemented: `ui/src/views/Mini.svelte` + `ui/src/lib/mini.svelte.ts`; Rust window
geometry in `crates/oxitide-app/src/window.rs` + `set_mini_mode` / `set_start_in_mini`
commands. Spec: `docs/superpowers/specs/2026-09-28-oxitide-2b-mini-player-design.md`.

## Enter/exit

- [x] Player-bar mini button shrinks the window to 300×430; the mini layout renders (cover, title/artist, seek, transport, secondary row).
- [x] Restore button returns the exact previous size/position; a maximized window comes back maximized.
- [x] Esc exits mini; with the queue sheet open, Esc closes the sheet first and a second press exits.
- [x] Entering while Now Playing was open and the docked queue visible: mini replaces the shell; exit restores both.
- [x] Resize mini to 260×360: every control stays reachable (the layout scrolls if needed), hit targets ≥24px.
- [x] A resize while mini is discarded on exit (the stashed full size returns).

## Controls

- [x] Cover, title, artist correct; long titles/artists ellipsize.
- [x] Seek drag + ←/→ ±5 s; wave variant in Atmosphere, hairline in Instrument.
- [x] Prev/play/next, shuffle, repeat, favorite, and volume all behave; favoriting fills the heart.
- [x] Queue button slides up the glass sheet; clicking a row plays it; Esc or the button closes it; Clear still confirms.
- [x] Nothing playing: muted empty state, transport no-ops.
- [x] A playback error shows with Retry inside mini.

## Launch preference and session

- [x] Settings → Appearance → "Start in mini player" on: next launch starts mini; toggling off returns to a full-size launch.
- [x] Launch with `start_in_mini` + a saved maximized state: starts mini, exit restores maximized.
- [x] Close while mini: relaunch is full-size (full geometry persisted).
- [x] Log out while mini: the login view has room (mini exits first).
- [x] Cold launch with `start_in_mini` on and no/expired session: the login view gets a full-size window.

## Modes, motion, color

- [x] Atmosphere: backdrop, glass, and dynamic color follow the cover in mini; Instrument stays flat.
- [x] Reduced motion: the queue sheet fades instead of sliding.
- [x] `npm --prefix ui run check`, `npm --prefix ui test`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` all green.
