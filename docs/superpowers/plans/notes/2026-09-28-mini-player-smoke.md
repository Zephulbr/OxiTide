# Mini player smoke — 2026-09-28

Requires: real account, browser login, `npm --prefix ui run app:dev`.

Implemented: `ui/src/views/Mini.svelte` + `ui/src/lib/mini.svelte.ts`; Rust window
geometry in `crates/oxitide-app/src/window.rs` + `set_mini_mode` / `set_start_in_mini`
commands. Spec: `docs/superpowers/specs/2026-09-28-oxitide-2b-mini-player-design.md`.

## Enter/exit

- [ ] Player-bar mini button shrinks the window to 300×430; the mini layout renders (cover, title/artist, seek, transport, secondary row).
- [ ] Restore button returns the exact previous size/position; a maximized window comes back maximized.
- [ ] Esc exits mini; with the queue sheet open, Esc closes the sheet first and a second press exits.
- [ ] Entering while Now Playing was open and the docked queue visible: mini replaces the shell; exit restores both.
- [ ] Resize mini to 260×360: every control stays reachable (the layout scrolls if needed), hit targets ≥24px.
- [ ] A resize while mini is discarded on exit (the stashed full size returns).

## Controls

- [ ] Cover, title, artist correct; long titles/artists ellipsize.
- [ ] Seek drag + ←/→ ±5 s; wave variant in Atmosphere, hairline in Instrument.
- [ ] Prev/play/next, shuffle, repeat, favorite, and volume all behave; favoriting fills the heart.
- [ ] Queue button slides up the glass sheet; clicking a row plays it; Esc or the button closes it; Clear still confirms.
- [ ] Nothing playing: muted empty state, transport no-ops.
- [ ] A playback error shows with Retry inside mini.

## Launch preference and session

- [ ] Settings → Appearance → "Start in mini player" on: next launch starts mini; toggling off returns to a full-size launch.
- [ ] Launch with `start_in_mini` + a saved maximized state: starts mini, exit restores maximized.
- [ ] Close while mini: relaunch is full-size (full geometry persisted).
- [ ] Log out while mini: the login view has room (mini exits first).
- [ ] Cold launch with `start_in_mini` on and no/expired session: the login view gets a full-size window.

## Modes, motion, color

- [ ] Atmosphere: backdrop, glass, and dynamic color follow the cover in mini; Instrument stays flat.
- [ ] Reduced motion: the queue sheet fades instead of sliding.
- [ ] `npm --prefix ui run check`, `npm --prefix ui test`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` all green.
