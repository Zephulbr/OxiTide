# OxiTide — Taskbar thumbnail toolbar

Date: 2026-10-16. Status: implemented (`crates/oxitide-app/src/thumbbar.rs`); owner smoke pending
(`docs/superpowers/plans/notes/2026-10-16-thumbbar-smoke.md`).

## Goal

Previous / Play-Pause / Next buttons under the window's taskbar preview on Windows, like the
official TIDAL client and other media players.

## Design

- **API.** `ITaskbarList3::ThumbBarAddButtons` (once per taskbar button) and
  `ThumbBarUpdateButtons` (every later change). The shell accepts buttons only after it has
  created the window's taskbar button, which it announces with the registered
  `TaskbarButtonCreated` message — re-sent after an Explorer restart, so the add runs on
  every such message (falling back to an update when the buttons already exist). Install also
  tries an add straight away, since the window may already be shown when the subclass lands.
  `ChangeWindowMessageFilterEx` lets both messages through for an elevated process.
- **Clicks.** `WM_COMMAND` with `HIWORD(wParam) == THBN_CLICKED`, button id in `LOWORD`, caught
  by a `SetWindowSubclass` hook on the main window (alongside the Snap Layouts one in
  `titlebar.rs`). Ids map to `TrayAction::{Prev, PlayPause, Next}` and run through
  `tray::run` — no new transport path, no new Tauri command, nothing for the phone remote's
  allowlist.
- **State.** The engine subscriber in `commands.rs` turns each `EngineEvent::State` into a
  `thumbbar::View { playing, has_track }`. Unchanged views are dropped; changes are applied on
  the main thread via `run_on_main_thread`, because the COM object is apartment-bound. Play
  shows Pause while playing. Without a current track all three buttons are disabled.
- **Icons.** No icon assets: the four glyphs (Lucide-grid polygons, filled) are rasterized in
  pure Rust at `GetSystemMetricsForDpi(SM_CXSMICON)` with 4×4 supersampling, encoded as PNG,
  and loaded with `CreateIconFromResourceEx`. They are rebuilt on each add and on
  `WM_DPICHANGED`, and destroyed on `WM_NCDESTROY`.
- **Re-entrancy.** The per-window state (not `Send`: it holds the COM interface) is a
  main-thread `thread_local`. It is moved out of its slot for the duration of each COM call, so
  a message re-entering the window proc during the call skips instead of hitting a held borrow.

## Scope / non-goals

- Windows only; other platforms get no-op stubs (Linux desktops get transport via MPRIS).
- Hidden to tray means no taskbar button and therefore no toolbar — expected.
- No like/shuffle buttons (could be added later; the toolbar allows up to seven, but the set is
  fixed at first add, so additions need hidden placeholders).
