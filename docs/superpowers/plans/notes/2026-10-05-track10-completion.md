# Track 10 completion note (2026-10-05)

- **Status:** Closed 2026-10-05 — owner smoke passed (10.1–10.3, 10.5, 10.6), including the
  post-smoke fixes and enhancements; follow-up fixes through `70d0bc9`
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

## Smoke results (owner, 2026-10-05)

- **10.1 session restore — PASS (7/7).**
- **10.2 sleep timer — PASS (9/9).**
- **10.3 history & stats — PASS (re-checked); step 8 found a bug and it is fixed:** the in-flight
  track stayed in Recently played because `clear()` and `set_enabled(false)` left the tracker's
  active play in place while the page prepends it. Fixed in `1ad2a9d` (reset the in-flight and
  pending play on clear and on disabling; two regression tests); the re-check passed.
- **10.5 content blocking — PASS, including step 13;** follow-up enhancement: `content.show_labels`
  (default on) tags known explicit/AI tracks with muted E/AI chips even when not blocking, with a
  Settings toggle that also stops the lookups.
- **10.6 tray & hotkeys — PASS, including steps 8–9;** follow-up enhancement: recording a chord
  already used by another action opens a Reassign modal (readable names, one click moves the
  binding), and OS-level failures get a readable toast.

Checklist paths were corrected on 2026-10-05 to the shipped locations: the config directory is
`%APPDATA%\OxiTide\config\`, so session is `%APPDATA%\OxiTide\config\session.json` and the flag
cache is `%APPDATA%\OxiTide\config\cache\track_flags.json` (code doc comments updated to match).

## Post-smoke fixes and enhancements (2026-10-05)

- `1ad2a9d` fix(history): drop the in-flight play on clear and disable.
- Content labels: `content.show_labels` + `set_show_labels`; `trackFlags` gains
  `showLabels`/`labels` and its `ensure` enriches for tagging too; TrackTable renders muted
  chips; Settings → Content gets the toggle.
- Hotkey conflicts: `hotkeys::apply_binding` + `set_hotkey(action, accelerator, replace)`;
  `conflictFor` pre-check; Settings Reassign modal; readable OS-failure/backstop toasts.
- Resumed-start quality display: `session.json` now carries the last served `StreamMeta`
  (optional, backward-compatible) and the restore path feeds it into the paused snapshot, so
  Detailed mode shows the real bit-depth/rate readout immediately instead of the preferred-
  quality fallback that looks identical to Compact.
- Gates at this head: `cargo test --workspace` exit 0, clippy clean, 552 vitest pass (59 files),
  svelte-check 0 errors / 0 warnings.

Closed by owner sign-off 2026-10-05, covering the 10.3 step-8 re-check, 10.5 step 13 (content
labels), 10.6 steps 8–9 (reassign modal), and the resumed-start detailed-quality check.

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
