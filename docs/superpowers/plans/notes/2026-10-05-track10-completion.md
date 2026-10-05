# Track 10 completion note (2026-10-05)

- **Status:** Implementation complete and merged — owner smoke verification pending
- **Owner:** V
- **Scope:** Plans 10.1–10.3, 10.5, 10.6 (daily-driver QoL); 10.4 (scrobble love) deferred 2026-10-05
- **Merged:** `main` at `446e054` (branch `track10-tray-hotkeys`, fast-forward, 2026-10-05)
- **Authority:** `docs/superpowers/specs/2026-10-04-oxitide-track10-qol-design.md` §10

## What shipped

- **10.1 Session restore** — `session.rs` + engine `current_index`/`RestoreSession` (paused, exact
  order, no audio blip); `session.json` cleared on logout, `.bak` on malformed.
- **10.2 Sleep timer** — `EngineCmd::FadeAndPause` (~5 s), player-bar durations + end-of-track;
  in-memory, resets on restart.
- **10.3 History and stats** — `history.rs` (`history.jsonl`, threshold + retention, in-flight
  prepend), History view with Recently-played and Stats tabs.
- **10.5 Content blocking** — `track_flags.rs` (public v2 `ai` + DTO `explicit`, ≤20/request,
  fail-open, `cache/track_flags.json`), Content settings tab, gray-out/badges, enqueue + restore
  filtering, toggle clear/prune.
- **10.6 Tray and hotkeys** — tray menu/tooltip/close-to-tray, single-instance re-show, `hotkeys.rs`
  + `[hotkeys]` user-defined accelerators with in-app duplicate rejection, conflict rollback, and
  recorder suspension. The plan's post-review amendment removes the focused-skip: on Windows/X11 a
  registered chord never reaches the webview, so the global handler is the only delivery path.

## Verified (scripted, merged tree `446e054`, 2026-10-05)

- `cargo test --workspace` exit 0; `cargo clippy --workspace -- -D warnings` exit 0.
- `npm --prefix ui test` 543 passed (59 files); `npm --prefix ui run check` 0 errors / 0 warnings.
- Every plan ran TDD with per-task review; 10.6 passed a whole-branch review and a scoped
  re-review after the fix wave (`3d9af22`, `a830ce9`, `446e054`).

## Pending — owner smoke run (Windows, real account)

Run in plan order; record results in each checklist (and mirror the outcome here):

1. `plans/notes/2026-10-04-track10-1-session-restore-smoke.md` — restart paused, position/queue/
   marker, logout, corrupt file, mini player, empty queue.
2. `plans/notes/2026-10-04-track10-2-sleep-timer-smoke.md` — fade at expiry and end-of-track,
   cancel semantics, restart reset, exclusive mode if configured.
3. `plans/notes/2026-10-04-track10-3-history-stats-smoke.md` — recently played, skip threshold,
   in-flight row, stats/chart, clear/disable, retention.
4. `plans/notes/2026-10-05-track10-5-content-blocking-smoke.md` — badges, queue clear/prune,
   restore filtering, offline fail-open, flag cache file.
5. `plans/notes/2026-10-05-track10-6-tray-hotkeys-smoke.md` — tray/menu/tooltip, close-to-tray,
   single-instance, recorder/conflicts, volume persistence, known edges (step 13).

Checklist paths corrected on 2026-10-05 to the shipped locations: the config directory is
`%APPDATA%\OxiTide\config\`, so session is `%APPDATA%\OxiTide\config\session.json` and the flag
cache is `%APPDATA%\OxiTide\config\cache\track_flags.json` (code doc comments updated to match).

When the run is complete, append the results here and flip this note's status to closed.

## Linux / WSL2

Track 10 is Windows-first; no WSL2 run was performed. Tray and hotkeys carry documented platform
caveats (code-review verified only): Linux tray needs libappindicator; GNOME/Wayland may need an
extension; Wayland global shortcuts need an XDG portal; `show_menu_on_left_click(false)` is
unsupported on Linux (menu on left click) and the tray tooltip is Windows/macOS-only. These follow
the Track 8 exemption pattern and join the resume list for the first bare-metal session.

## Follow-ups (not closure-gating)

- 10.4 scrobble love remains deferred; the design stays in spec §6.4.
- Parked from the 10.6 final review: a rare capture-release/save re-registration race (recovers on
  the next save or restart), and the plan's Self-Review prose still cites the removed focused-skip
  (superseded by its Post-review amendment).
- `set_close_to_tray` updates optimistically with a toast but no UI rollback (matches
  `set_smtc`/`set_oneko`); `toggle_window`'s doc wording vs a minimized window is imprecise.
- Historical docs (pre-2026-10-05 plans/specs/smoke notes) keep the older `%APPDATA%\OxiTide\...`
  paths; the live Track 10 checklists and code doc comments were corrected.
