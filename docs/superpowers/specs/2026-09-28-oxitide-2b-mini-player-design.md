# OxiTide — Track 2b Mini Player

- **Status:** Draft for review
- **Date:** 2026-09-28
- **Owner:** V
- **Parents:** `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (§7.4, §15) ·
  `2026-09-25-oxitide-2b-visual-convergence.md` (§3) ·
  `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 2b)
- **Process note:** scope and decisions were brainstormed live with the owner
  (2026-09-28). This document is the standalone source of truth.

---

## 1. Purpose & Outcome

Ship the last remaining Track 2b slice: **the mini player** — a small, self-contained
playback instrument for when the app is kept small. Foundation §7.4 approved it in
principle and parked sequencing; Track 2b core (Plan 10 Instrument, Plan 11 Atmosphere)
has now landed, so it ships as a follow-up slice.

**Outcome:** a ~300×430 window (resizable to ~260×360 minimum) showing cover, metadata,
seek, transport, favorite, shuffle/repeat, compact volume, a queue shortcut, and a
restore button. It is toggled from the player bar and exited with Esc or the restore
button. It reuses the existing design system with no new mode, no new identity element,
and no new backend endpoints — all UI over the existing player store and shell.

Success = a small window that still reads unmistakably as OxiTide (backdrop, glass,
dynamic color, wave seek in Atmosphere; flat and quiet in Instrument), with full
transport control and a reachable queue, and whose open/close never costs the user
their previous full-window geometry.

## 2. Scope

**In scope:** the mini shell/layout; entry/exit affordances; the queue sheet; a
`start_in_mini` persisted setting; Rust-owned window geometry for mini (size, relaxed
minimum, stash/restore, close-time capture); the `SeekBar` extraction shared with the
player bar; Esc-order changes; tests.

**Out of scope:** Hi-Fi Rack (own design session, convergence §2); light mode; synced
lyrics and other now-playing content; true waveform peak data; a separate always-on-top
window; any change to playback, queue, or catalog behavior; performance-harness work
(Track 3 sets the frame/GPU budgets this slice must respect).

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| MP-D1 | Mini reuses the **existing main window**, shrunk; not a second window. | Reuses the single-window geometry system (`window.rs`), needs no new capability, and matches "window size limits relax." A floating companion window is a different feature. |
| MP-D2 | **Rust owns mini window fate** (`set_mini_mode` command); the frontend holds only the reactive layout flag. | Keeps OS-window behavior next to `window.rs` and startup, and lets close-time capture save the full geometry instead of the shrunk one. |
| MP-D3 | Mini **default size 300×430**, **resizable with a 260×360 minimum**. No fixed maximum; the layout is fluid and centered. | Owner choice: flexible small instrument rather than a rigid tile. |
| MP-D4 | Controls: cover, title/artist, seek, **prev/play/next**, **favorite**, **shuffle + repeat**, **compact volume**, **queue shortcut**, **restore button**. No time readout, no quality/bit-perfect badges. | Owner-selected set; keeps the tile calm and dense. |
| MP-D5 | The queue shortcut opens a **slide-up glass sheet over the mini layout** (reusing `QueueRail`); it never leaves mini mode. | Self-contained instrument; reuses the existing 760px queue-overlay pattern. |
| MP-D6 | Entry via a **player-bar button**; exit via the **restore button** or **Esc**. No new View menu. Esc order becomes menu → mini queue sheet → exit mini → now-playing → queue. | Mirrors the existing now-playing/escape toggle pattern and avoids new chrome. |
| MP-D7 | Mini is **runtime-transient**. A new persisted setting **`start_in_mini` (default off)** optionally relaunches into mini. When off, the app always launches full-size; closing while mini saves the full geometry. | Owner choice: never surprise the user with a tiny launch unless they opt in. |
| MP-D8 | While mini is active, the nav route and now-playing/queue flags are **left untouched**; the mini layout simply replaces the shell. Exiting restores the exact prior shell. | No hidden state mutation; exit is predictable. |
| MP-D9 | `SeekBar` is extracted from `PlayerBar` and shared with Mini. | The seek interaction (pointer + keyboard + hairline/wave) is ~70 lines; duplicating it invites drift. |
| MP-D10 | A **pure escape-target helper** decides which layer Esc closes; the window mutations stay thin and untested. | Keeps the ordering unit-testable without a DOM. |
| MP-D11 | `start_in_mini` **wins over a saved-maximized state** at launch: the app starts mini and restores the maximized state on exit. | "Start in mini" is an explicit user setting; maximizing is not a reason to ignore it. |

## 4. Architecture & components

### 4.1 Frontend state — `ui/src/lib/mini.svelte.ts`

A singleton in the same style as `appearance`/`nav`:

```ts
export interface Mini {
  readonly active: boolean;
  readonly queueOpen: boolean;
  init(): Promise<void>;
  enter(): Promise<void>;
  exit(): Promise<void>;
  toggle(): Promise<void>;
  toggleQueue(): void;
}
export function createMini(): Mini;
export const mini = createMini();
```

- `active` — whether the mini shell renders; `queueOpen` — whether the sheet is shown.
- `enter()` / `exit()` call `api.setMiniMode(true|false)` **then** set `active`; on
  failure the flag is not changed and the error is surfaced (no desync between the
  window and the layout). `toggle()` dispatches on `active`.
- Entering clears `queueOpen`; exiting also clears it.
- `init()` reads `getConfig().ui.start_in_mini` and sets `active` to match the window
  Rust already resized at launch; a config failure leaves `active` false (the default).

### 4.2 Components

| File | Change |
|---|---|
| `ui/src/lib/components/SeekBar.svelte` | **New.** Extract the seek block from `PlayerBar.svelte`: pointer drag with capture, keyboard (←/→ ±5s), hairline or wave (via `appearance.waveSeek`), `role="slider"`, ARIA values, `pendingSeek` handling. Props: `player`, plus optional class/size hooks. |
| `ui/src/lib/components/PlayerBar.svelte` | Consume `SeekBar`; add a **mini-player toggle button** (enter mini) in the control cluster. |
| `ui/src/views/Mini.svelte` | **New.** The compact layout: cover (`Cover.svelte`), title + artist, `SeekBar`, transport (prev/play/next + favorite + shuffle + repeat), compact volume, queue button, restore button; plus the slide-up queue sheet reusing `QueueRail.svelte`. |
| `ui/src/App.svelte` | Within the logged-in branch, render `{#if mini.active}<Mini …/>{:else}<div class="app">…</div>{/if}`; `Backdrop` and the grain stay outside so Atmosphere visuals apply in both. Call `mini.init()` at boot. Update the Esc path to consult the new helper. Exit mini on logout and on `session_expired` (the login view needs room). |
| `ui/src/views/Settings.svelte` | Add a **Start in mini player** toggle in the Appearance section. |

### 4.3 Mini layout & behavior

- Vertical stack, centered, fluid: cover grows with window width (square, `Cover.svelte`,
  existing radius); title (display face) + artist (muted) with ellipsis; `SeekBar`;
  a transport row; a secondary row with favorite, shuffle, repeat, volume, queue,
  restore.
- The shell's `Backdrop` means Atmosphere's backdrop/glass/grain/dynamic color apply
  unchanged; Instrument stays flat. No component branches on mode.
- The queue sheet: glass overlay sliding up over the cover area (`--dur-view`), listing
  `QueueRail`; clicking a row plays it; Esc or the queue button closes it.
- Empty state (nothing playing): a muted "Nothing playing" line, transport disabled,
  as in the player bar.

## 5. Window & geometry lifecycle (Rust)

### 5.1 Constants

| Name | Value |
|---|---|
| Mini default size | 300 × 430 |
| Mini minimum size | 260 × 360 |
| Full (windowed) minimum size | 720 × 520 (unchanged, from `tauri.conf.json`) |

The `tauri.conf.json` minimum stays 720×520; it is relaxed to the mini minimum only
while mini is active.

### 5.2 Geometry stash

`crates/oxitide-app/src/window.rs` gains:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullGeometry {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub maximized: bool,
}

/// The geometry to persist on close: the pre-mini stash when mini is active,
/// otherwise the live window (the caller supplies the captured live state).
pub fn geometry_to_save(pre_mini: Option<FullGeometry>, live: FullGeometry) -> FullGeometry;
```

`AppState` (`state.rs`) holds `pre_mini: Mutex<Option<FullGeometry>>`.

### 5.3 `set_mini_mode(active)` command

- **Enter:** if a stash already exists, no-op (idempotent). Otherwise capture the live
  inner size, outer position, and `is_maximized` into the stash; if maximized,
  `unmaximize`; then `set_min_size(260, 360)`, `set_resizable(true)`, `set_size(300, 430)`.
  The window keeps its current position.
- **Exit:** `set_min_size(720, 520)`, `set_resizable(true)`, then restore the stash (size
  and position; re-`maximize` if the stash was maximized) and clear it. With no stash
  (defensive; should not happen), apply the full minimum and leave size/position as-is.
- Returns `Result<(), String>`; every window op error is propagated.

### 5.4 Startup (`start_in_mini`)

In `main.rs` setup, immediately after `window::apply` and before `window.show()`:

- If `ui.start_in_mini` is true: seed the stash from the config's saved full geometry
  (`window_width/height/x/y/maximized`), then apply the mini enter steps. This wins over
  a saved-maximized state (MP-D11): the stash keeps `maximized = true` so exit restores it.
- If false: unchanged behavior.

### 5.5 Close-time capture

`save_window_state` reads the stash. When it is `Some`, persist
`geometry_to_save(Some(stash), live)` — i.e. the **full** stashed geometry — so closing
while mini still relaunches full-size (MP-D7). When it is `None`, behavior is unchanged.

## 6. Config & IPC contract

### 6.1 Config

`UiConfig` gains `start_in_mini: bool` (serde default, `false`), so an existing
`config.toml` without the key decodes to `false` and a save writes it. Default and
`Default for UiConfig` both set `false`.

### 6.2 Commands (`commands.rs` + `main.rs` `generate_handler!`)

| Command | Args (UI → Rust) | Returns |
|---|---|---|
| `set_mini_mode` | `active: bool` (`active` verbatim) | `Result<(), String>` |
| `set_start_in_mini` | `enabled: bool` (`enabled` verbatim) | `Result<(), String>` |

`set_start_in_mini` persists to `config.toml` through the existing config-write path.

### 6.3 `ui/src/lib/api.ts`

Add `setMiniMode(active: boolean)` and `setStartInMini(enabled: boolean)`; extend the
`getConfig` response type with `ui.start_in_mini`. Field names stay in sync by hand per
the IPC contract in `AGENTS.md`.

## 7. Visual, modes, motion, accessibility

- **Semantic tokens only.** Mini consumes `--surface-*`, `--text`, `--accent`,
  `--on-accent`, `--line`, `--dur-*`, etc.; it never reads base or `--dyn-*` values, and
  never branches on `data-mode`.
- **Identity budget:** mini adds no fifth identity element; it reuses the backdrop, the
  wave seek (Atmosphere), the disc spinner on load, and existing card/control treatments.
- **Motion:** no new ambient motion. The queue sheet uses `--dur-view`; hover/press use
  `--dur-micro`. Only `transform`/`opacity` animate.
- **Reduced motion:** inherited — `appearance.ambientMotion`/`discSpin` already fold in
  `prefers-reduced-motion`; the sheet becomes an instant/120ms fade.
- **Accessibility:** every control labeled; `SeekBar` keeps `role="slider"` with ARIA
  values and keyboard seek; focus-visible ring retained; hit targets ≥24px, including at
  the mini minimum size. Queue rows remain full-row hit areas.

## 8. Error handling & edge cases

| Case | Behavior |
|---|---|
| Window op fails on enter/exit | Command returns `Err`; `mini.active` is not changed and a toast surfaces the error. |
| Enter while maximized | Unmaximize, stash `maximized = true`; exit re-maximizes. |
| Enter while now-playing expanded or docked queue open | Mini replaces the shell; `nav` state is retained; exit restores it. |
| Resize while mini | Clamped by the 260×360 minimum; the resize is discarded on exit (the stashed full size is restored). |
| Mini window moved to another monitor | Position preserved while mini; on exit the stashed full position is restored. Off-screen stashes are guarded by the existing `visible_position` check on the next launch. |
| Close while mini | Persist the stashed full geometry (MP-D7, §5.5). |
| Launch with `start_in_mini` + saved maximized | Start mini; exit restores maximized (MP-D11). |
| `getConfig` failure at boot | `mini.active` stays `false`; the window may still be mini if `start_in_mini` was set — acceptable and documented, since config failures are already tolerated at boot. |
| Logout / session expired while mini | Exit mini before showing the login view so it has room. |
| Repeated `enter` while stashed | No-op. |
| Nothing playing | Muted empty state, transport disabled. |

## 9. Testing

- **Rust (`cargo test --workspace`):**
  - `UiConfig::default().start_in_mini == false`; TOML round-trip writes and reads it.
  - `geometry_to_save(Some(stash), live) == stash` and `geometry_to_save(None, live) == live`.
  - Existing `window.rs` and `config.rs` tests stay green.
  - Window mutations themselves are thin and not unit-tested (no display in CI).
- **UI (`npm --prefix ui test`):**
  - `mini.test.ts`: `enter`/`exit`/`toggle` set `active` and call `api.setMiniMode` with
    the right value; a rejected call leaves `active` unchanged; `toggleQueue` flips
    `queueOpen`; entering clears it.
  - `shortcuts.test.ts`: the new pure escape-target helper resolves the full order
    (menu → mini queue → exit mini → now-playing → queue → nothing), including when
    several layers are open at once.
  - `SeekBar` extraction is covered by preserving existing player-bar behavior; add a
    focused test only if the interaction changes.
  - `appearance`/config types updated so `svelte-check` stays green.
- **Manual smoke (`docs/superpowers/plans/notes/`):** real account, browser login,
  `npm --prefix ui run app:dev`; toggle mini in both modes; queue sheet; Esc order;
  resize to minimum; close-while-mini then relaunch full; `start_in_mini` on/off;
  reduced motion; dynamic color follows the cover in mini.

## 10. Performance (PM-D1)

No per-frame work is added. Resizing happens only on an explicit toggle; the mini layout
is a small static tree with no new observers or timers. `SeekBar` is the same code
already running in the player bar. The Track 3 harness will set the frame/GPU budgets
this slice must respect; it currently adds no measurable cost.

## 11. Open items for the plan

- Exact `SeekBar` prop/class surface so `PlayerBar` and `Mini` can style it differently.
- The mini secondary-row order and its button icons, reusing the inlined Lucide paths in
  `Icon.svelte`; add a path only if a control in §3 (MP-D4) has no existing one.
- Where `seeded` stash comes from when `start_in_mini` is true and there is no saved
  position yet (first run): fall back to the configured default size/centered position.
- Whether the Settings toggle mirrors the live state or only the launch preference
  (spec assumes launch preference only).
- The exact `session_expired`/logout hook point for `mini.exit()`.
