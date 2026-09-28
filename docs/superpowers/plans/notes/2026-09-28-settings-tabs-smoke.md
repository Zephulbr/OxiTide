# Settings tabs smoke — 2026-09-28

Requires: real account, browser login, `npm --prefix ui run app:dev`.

Implemented: `Settings.svelte` is a tabbed view (Appearance / Playback /
Connections / Storage / System); tab order and helpers live in
`ui/src/lib/settingsTabs.ts` (unit-tested in `settingsTabs.test.ts`).

- [ ] Tab bar renders under the Settings heading; Appearance is active on entry and carries the accent underline (navigation reads differently from the pill segmented controls below).
- [ ] Clicking each tab swaps the panel; the long single-page scroll is gone.
- [ ] Keyboard: focus a tab, Left/Right moves and wraps through the categories, Home/End jump to first/last, focus follows the selected tab; Enter/Space activates.
- [ ] Appearance: Mode, Dynamic color, Typeface (incl. custom font apply + preview), Density, Cover shape still read and persist across a Settings revisit.
- [ ] Playback: output device dropdown lists devices; bit-perfect toggle and fallback select persist; streaming quality and allow-fallback persist; Quality display switches the player-bar chips; the bit-perfect notice (if triggered) shows in this panel.
- [ ] Connections: four "Planned" rows (Last.fm, ListenBrainz, Discord Rich Presence, Windows media controls); checkboxes are disabled; nothing errors.
- [ ] Storage: cache used/limit and pending-download numbers match disk; max size persists; clear-on-exit persists; Refresh updates numbers; Clear cache empties it and the numbers update.
- [ ] System: logs directory shown and Open logs folder opens Explorer; the keyboard-shortcuts list matches the app's actual shortcuts.
- [ ] Check both Atmosphere and Instrument modes.
- [ ] Intended change: when config fails to load, only `ErrorState` renders (tabs/panels hidden), so shortcuts are no longer reachable from that state.
- [ ] `npm --prefix ui run check`, `npm --prefix ui test` green.
