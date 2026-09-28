# Mini Player Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the last Track 2b slice — a ~300×430 mini player window that reuses the existing shell, player store, and design system, with Rust-owned window geometry, Esc ordering, and a persisted `start_in_mini` launch preference.

**Architecture:** The main window shrinks (MP-D1); Rust owns the geometry stash/restore behind two new commands (`set_mini_mode`, `set_start_in_mini`), while the UI mirrors only a reactive `active` flag after the command succeeds. `SeekBar` is extracted from `PlayerBar` and shared with a new `Mini.svelte` view; the queue sheet reuses `QueueRail`.

**Tech Stack:** Rust (Tauri 2, serde, toml), Svelte 5 runes, TypeScript, vitest, wiremock-free (no HTTP changes).

**Spec:** `docs/superpowers/specs/2026-09-28-oxitide-2b-mini-player-design.md`

## Global Constraints

- Windows-first; no new Cargo or npm dependencies.
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails.
- `invoke` sends camelCase args; Rust params stay snake_case (`set_mini_mode` takes `active` verbatim; `set_start_in_mini` takes `enabled` verbatim).
- UI uses Svelte 5 runes only; no Svelte 4 stores or `export let`.
- Styles consume semantic tokens only (`--surface-*`, `--text`, `--accent`, `--line`, `--dur-*`, …); never base or `--dyn-*` values directly, and components never branch on `data-mode`. Icons are inlined Lucide paths in `Icon.svelte` — no unicode glyphs.
- Rust DTO/config field names are the source of truth; keep `ui/src/lib/api.ts` in sync by hand.
- Never log token values.
- Gates for every task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test` (UI gates only where the task touches `ui/`).
- Commit after each task; keep unrelated working-tree changes out of task commits.

## Preflight — working tree

The tree currently carries the **uncommitted 2026-09-28 polish pass** (player bar
three-zone layout, tabbed Settings, sidebar scale + playing marker, track row rework).
Automated gates are green (`npm --prefix ui run check` 0 errors, `npm --prefix ui test`
242 passed, `cargo test --workspace` passed). Mini-player Tasks 8, 11, and 12 edit
`PlayerBar.svelte`, `App.svelte`, and `Settings.svelte`, which the polish pass also
edits, so the polish must be **committed or stashed before Task 1** — otherwise its
changes get swept into mini-player commits and violate the one-task-one-commit rule.
Owner decision at plan review (recommended: commit it as its own commit first).

## Review Focus

- A failed `set_mini_mode` must leave `mini.active` and the real window in agreement — no tiny window showing the full shell, no full window showing the mini shell (mini store test).
- Closing while mini must persist the **pre-mini full geometry**, never the 300×430 size, so the next launch is full-size (MP-D7; `geometry_to_save` test + `save_window_state` wiring).
- Launching with `start_in_mini` while a maximized state is saved must start mini and restore maximized on exit (MP-D11; stash keeps `maximized = true`, `enter_mini` unmaximizes first).
- Esc with several layers open must resolve to exactly one target in the documented order menu → mini queue → exit mini → now playing → queue (pure helper test).
- At the mini minimum (260×360) every control must stay reachable; the layout scrolls rather than clipping (manual smoke; fluid layout).

---

## Phase A — Rust window & config

### Task 1: `start_in_mini` config key

**Files:**
- Modify: `crates/oxitide-app/src/config.rs` (`UiConfig`, its `Default`, tests)

**Interfaces:**
- Consumes: nothing new.
- Produces: `UiConfig::start_in_mini: bool` (serde default `false`), serialized as `start_in_mini`.

- [ ] **Step 1: Write the failing tests**

In `config.rs`'s test module, append:

```rust
#[test]
fn start_in_mini_defaults_false_and_round_trips() {
    let dir = temp_dir();
    let path = dir.join("config.toml");
    assert!(!Config::default().ui.start_in_mini);
    let mut config = Config::default();
    config.ui.start_in_mini = true;
    config.save_to(&path).unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("start_in_mini = true"));
    assert!(Config::load_from(&path).ui.start_in_mini);
    let _ = fs::remove_dir_all(dir);
}
```

In `partial_file_fills_defaults`, add after `assert_eq!(config.ui.queue_width, 320);`:

```rust
    assert!(!config.ui.start_in_mini);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app start_in_mini`
Expected: FAIL to compile — no field `start_in_mini` on `UiConfig`.

- [ ] **Step 3: Implement**

In `UiConfig` add the field after `pub queue_width: u32,`:

```rust
    /// Launch the window in the mini player (spec MP-D7); default off.
    pub start_in_mini: bool,
```

In `impl Default for UiConfig` add after `queue_width: 320,`:

```rust
            start_in_mini: false,
```

In `save_and_load_round_trip`, add `start_in_mini: false,` to the `UiConfig { … }` literal (after `queue_width: 320,`).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app config`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-app/src/config.rs
git commit -m "feat(app): add the start_in_mini launch preference"
```

---

### Task 2: Window geometry helpers

**Files:**
- Modify: `crates/oxitide-app/src/window.rs`

**Interfaces:**
- Consumes: `tauri::{PhysicalPosition, PhysicalSize, Size, WebviewWindow}`.
- Produces:
  - `window::MINI_WIDTH = 300`, `MINI_HEIGHT = 430`, `MINI_MIN_WIDTH = 260`, `MINI_MIN_HEIGHT = 360`, `FULL_MIN_WIDTH = 720`, `FULL_MIN_HEIGHT = 520`.
  - `window::FullGeometry { width: u32, height: u32, x: Option<i32>, y: Option<i32>, maximized: bool }` (derives `Debug, Clone, Copy, PartialEq, Eq`).
  - `window::geometry_to_save(pre_mini: Option<FullGeometry>, live: FullGeometry) -> FullGeometry`.
  - `window::read_geometry(&WebviewWindow) -> tauri::Result<FullGeometry>`.
  - `window::apply_geometry(&WebviewWindow, FullGeometry) -> tauri::Result<()>`.
  - `window::enter_mini(&WebviewWindow, FullGeometry) -> tauri::Result<()>`.
  - `window::exit_mini(&WebviewWindow, Option<FullGeometry>) -> tauri::Result<()>`.

- [ ] **Step 1: Write the failing tests**

In `window.rs`'s test module, append:

```rust
#[test]
fn geometry_to_save_prefers_the_stash() {
    let live = FullGeometry {
        width: 1200,
        height: 800,
        x: Some(10),
        y: Some(20),
        maximized: false,
    };
    let stash = FullGeometry {
        width: 1000,
        height: 700,
        x: Some(30),
        y: Some(40),
        maximized: true,
    };
    assert_eq!(geometry_to_save(Some(stash), live), stash);
    assert_eq!(geometry_to_save(None, live), live);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app geometry_to_save`
Expected: FAIL to compile — no `FullGeometry`.

- [ ] **Step 3: Implement**

Change the import at the top of `window.rs` to:

```rust
use tauri::{PhysicalPosition, PhysicalSize, Size, WebviewWindow};
```

Add below `MIN_VISIBLE_HEIGHT` (before `visible_position`):

```rust
/// Mini-player and full-window size limits (spec §5.1).
pub const MINI_WIDTH: u32 = 300;
pub const MINI_HEIGHT: u32 = 430;
pub const MINI_MIN_WIDTH: u32 = 260;
pub const MINI_MIN_HEIGHT: u32 = 360;
/// The windowed minimum from `tauri.conf.json`; re-applied on mini exit.
pub const FULL_MIN_WIDTH: u32 = 720;
pub const FULL_MIN_HEIGHT: u32 = 520;

/// The full-window geometry stashed while the mini player is active.
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
pub fn geometry_to_save(pre_mini: Option<FullGeometry>, live: FullGeometry) -> FullGeometry {
    pre_mini.unwrap_or(live)
}

/// Read the live window geometry for the pre-mini stash.
pub fn read_geometry(window: &WebviewWindow) -> tauri::Result<FullGeometry> {
    let size = window.inner_size()?;
    let position = window.outer_position().ok();
    Ok(FullGeometry {
        width: size.width,
        height: size.height,
        x: position.map(|p| p.x),
        y: position.map(|p| p.y),
        maximized: window.is_maximized().unwrap_or(false),
    })
}

/// Apply a stashed geometry: size, position when known, then maximize.
pub fn apply_geometry(window: &WebviewWindow, geometry: FullGeometry) -> tauri::Result<()> {
    window.set_size(PhysicalSize::new(geometry.width, geometry.height))?;
    if let (Some(x), Some(y)) = (geometry.x, geometry.y) {
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    if geometry.maximized {
        window.maximize()?;
    }
    Ok(())
}

/// Enter the mini window. Unmaximizing first keeps the stash authoritative
/// (MP-D11): exit restores the maximized state from the stash.
pub fn enter_mini(window: &WebviewWindow, stash: FullGeometry) -> tauri::Result<()> {
    if stash.maximized {
        window.unmaximize()?;
    }
    window.set_min_size(Some(Size::Physical(PhysicalSize::new(
        MINI_MIN_WIDTH,
        MINI_MIN_HEIGHT,
    ))))?;
    window.set_resizable(true)?;
    window.set_size(PhysicalSize::new(MINI_WIDTH, MINI_HEIGHT))?;
    Ok(())
}

/// Leave the mini window: restore the full minimum, then the stash when there
/// is one (a defensive `None` leaves size/position untouched).
pub fn exit_mini(window: &WebviewWindow, stash: Option<FullGeometry>) -> tauri::Result<()> {
    window.set_min_size(Some(Size::Physical(PhysicalSize::new(
        FULL_MIN_WIDTH,
        FULL_MIN_HEIGHT,
    ))))?;
    window.set_resizable(true)?;
    if let Some(geometry) = stash {
        apply_geometry(window, geometry)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app window && cargo clippy -p oxitide-app -- -D warnings`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-app/src/window.rs
git commit -m "feat(app): add mini window geometry stash and restore helpers"
```

---

### Task 3: Pre-mini geometry stash on `AppState`

**Files:**
- Modify: `crates/oxitide-app/src/state.rs`

**Interfaces:**
- Consumes: `crate::window::FullGeometry` (Task 2).
- Produces: `AppState::pre_mini(&self) -> Option<FullGeometry>`, `set_pre_mini(&self, FullGeometry)`, `take_pre_mini(&self) -> Option<FullGeometry>`.

- [ ] **Step 1: Write the failing test**

In `state.rs`'s test module, append:

```rust
#[test]
fn pre_mini_stash_is_set_and_taken_once() {
    let state = AppState::default();
    assert_eq!(state.pre_mini(), None);
    let geometry = crate::window::FullGeometry {
        width: 1200,
        height: 800,
        x: Some(1),
        y: Some(2),
        maximized: false,
    };
    state.set_pre_mini(geometry);
    assert_eq!(state.pre_mini(), Some(geometry));
    assert_eq!(state.take_pre_mini(), Some(geometry));
    assert_eq!(state.pre_mini(), None);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p oxitide-app pre_mini`
Expected: FAIL to compile — no method `pre_mini`.

- [ ] **Step 3: Implement**

Add to the `use crate::config::Config;` area at the top:

```rust
use crate::window::FullGeometry;
```

Add the field to `AppState` after `pub config: Mutex<Config>,`:

```rust
    /// The full-window geometry stashed while the mini player is active.
    pub pre_mini: Mutex<Option<FullGeometry>>,
```

Add the accessors after `set_config`:

```rust
    pub fn pre_mini(&self) -> Option<FullGeometry> {
        *self.pre_mini.lock().unwrap()
    }

    pub fn set_pre_mini(&self, geometry: FullGeometry) {
        *self.pre_mini.lock().unwrap() = Some(geometry);
    }

    pub fn take_pre_mini(&self) -> Option<FullGeometry> {
        self.pre_mini.lock().unwrap().take()
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app state`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/oxitide-app/src/state.rs
git commit -m "feat(app): stash the full geometry while mini is active"
```

---

### Task 4: `set_mini_mode` / `set_start_in_mini` commands, startup, close capture

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs` (append after `set_queue_width`)
- Modify: `crates/oxitide-app/src/main.rs` (`save_window_state`, setup block, `generate_handler!`)

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces: Tauri commands `set_mini_mode(active: bool)` and `set_start_in_mini(enabled: bool)`, both `Result<(), String>`; startup honors `ui.start_in_mini`; close-time capture prefers the stash.

- [ ] **Step 1: Add the commands**

In `commands.rs`, after the `set_queue_width` function, add:

```rust
#[tauri::command]
pub fn set_mini_mode(
    state: State<'_, AppState>,
    window: tauri::WebviewWindow,
    active: bool,
) -> Result<(), String> {
    if active {
        // Idempotent (spec §5.3): a repeated enter keeps the original stash.
        if state.pre_mini().is_some() {
            return Ok(());
        }
        let geometry = crate::window::read_geometry(&window).map_err(|e| e.to_string())?;
        state.set_pre_mini(geometry);
        crate::window::enter_mini(&window, geometry).map_err(|e| e.to_string())
    } else {
        let stash = state.take_pre_mini();
        crate::window::exit_mini(&window, stash).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn set_start_in_mini(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = state.config();
    config.ui.start_in_mini = enabled;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

- [ ] **Step 2: Register the commands**

In `main.rs`, add to `generate_handler![...]` after `commands::set_queue_width,`:

```rust
            commands::set_mini_mode,
            commands::set_start_in_mini,
```

- [ ] **Step 3: Persist the stash on close**

Replace the body of `save_window_state` with:

```rust
fn save_window_state(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let mut config = state.config();
    window::capture(&window, &mut config);
    let live = window::FullGeometry {
        width: config.ui.window_width,
        height: config.ui.window_height,
        x: config.ui.window_x,
        y: config.ui.window_y,
        maximized: config.ui.window_maximized,
    };
    let saved = window::geometry_to_save(state.pre_mini(), live);
    config.ui.window_width = saved.width;
    config.ui.window_height = saved.height;
    config.ui.window_x = saved.x;
    config.ui.window_y = saved.y;
    config.ui.window_maximized = saved.maximized;
    if let Err(e) = config.save() {
        tracing::warn!("failed to save window geometry: {e}");
    }
    state.set_config(config);
}
```

- [ ] **Step 4: Honor `start_in_mini` at launch**

In `main.rs`'s setup block, replace the window block with:

```rust
            if let Some(window) = app.get_webview_window("main") {
                let config = app.state::<AppState>().config();
                if let Err(e) = window::apply(&window, &config) {
                    tracing::warn!("failed to apply window geometry: {e}");
                }
                if config.ui.start_in_mini {
                    // MP-D11: the stash keeps the saved full geometry (including
                    // a saved maximized state) so exiting mini restores it.
                    let stash = window::FullGeometry {
                        width: config.ui.window_width,
                        height: config.ui.window_height,
                        x: config.ui.window_x,
                        y: config.ui.window_y,
                        maximized: config.ui.window_maximized,
                    };
                    app.state::<AppState>().set_pre_mini(stash);
                    if let Err(e) = window::enter_mini(&window, stash) {
                        tracing::warn!("failed to start in mini player: {e}");
                    }
                }
                if let Err(e) = window.show() {
                    tracing::warn!("failed to show the main window: {e}");
                }
            }
```

- [ ] **Step 5: Run the Rust gates**

Run: `cargo test --workspace && cargo clippy --workspace -- -D warnings`
Expected: PASS. (The window mutations themselves are thin and have no display in CI; the stash logic is covered by Tasks 1–3.)

- [ ] **Step 6: Commit**

```powershell
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(app): add mini window mode commands and launch preference"
```

---

### Task 5: API types and invoke wrappers

**Files:**
- Modify: `ui/src/lib/api.ts`

**Interfaces:**
- Consumes: Task 4 command names.
- Produces: `Config.ui.start_in_mini: boolean`; `api.setMiniMode(active: boolean)`, `api.setStartInMini(enabled: boolean)`.

- [ ] **Step 1: Extend the config type**

In `ui/src/lib/api.ts`, in `Config.ui`, add after `queue_width: number;`:

```ts
    start_in_mini: boolean;
```

- [ ] **Step 2: Add the invoke wrappers**

In the `api` object, after `setQueueWidth`, add:

```ts
  setMiniMode: (active: boolean) => invoke<void>("set_mini_mode", { active }),
  setStartInMini: (enabled: boolean) => invoke<void>("set_start_in_mini", { enabled }),
```

- [ ] **Step 3: Run the UI gates**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS (svelte-check only; no test asserts the new fields yet).

- [ ] **Step 4: Commit**

```powershell
git add ui/src/lib/api.ts
git commit -m "feat(ui): add mini mode API wrappers and config field"
```

---

## Phase B — UI foundations

### Task 6: Pure escape-target helper

**Files:**
- Modify: `ui/src/lib/shortcuts.ts`
- Modify: `ui/src/lib/shortcuts.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `export type EscapeTarget = "menu" | "mini-queue" | "exit-mini" | "now-playing" | "queue" | null;`
  - `export function escapeTarget(layers: { menuOpen: boolean; miniActive: boolean; miniQueueOpen: boolean; nowPlayingOpen: boolean; queueOpen: boolean }): EscapeTarget`.

- [ ] **Step 1: Write the failing tests**

In `shortcuts.test.ts`, change the import line to:

```ts
import { escapeTarget, isShortcutHandled, isTypingTarget, matchShortcut, SHORTCUTS } from "./shortcuts";
```

Append:

```ts
describe("escapeTarget", () => {
  const none = {
    menuOpen: false,
    miniActive: false,
    miniQueueOpen: false,
    nowPlayingOpen: false,
    queueOpen: false,
  };

  it("resolves the documented order, most modal first", () => {
    expect(escapeTarget({ ...none, queueOpen: true })).toBe("queue");
    expect(escapeTarget({ ...none, nowPlayingOpen: true, queueOpen: true })).toBe("now-playing");
    expect(
      escapeTarget({ ...none, miniActive: true, nowPlayingOpen: true, queueOpen: true }),
    ).toBe("exit-mini");
    expect(
      escapeTarget({ ...none, miniActive: true, miniQueueOpen: true, nowPlayingOpen: true }),
    ).toBe("mini-queue");
    expect(
      escapeTarget({ ...none, menuOpen: true, miniActive: true, miniQueueOpen: true }),
    ).toBe("menu");
  });

  it("returns null when nothing owns Esc", () => {
    expect(escapeTarget(none)).toBeNull();
  });

  it("ignores a stale mini queue flag when mini is not active", () => {
    expect(escapeTarget({ ...none, miniQueueOpen: true })).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/shortcuts.test.ts`
Expected: FAIL — `escapeTarget` is not exported.

- [ ] **Step 3: Implement**

In `shortcuts.ts`, add after the `ShortcutAction` type:

```ts
export type EscapeTarget = "menu" | "mini-queue" | "exit-mini" | "now-playing" | "queue" | null;

export interface EscapeLayers {
  menuOpen: boolean;
  miniActive: boolean;
  miniQueueOpen: boolean;
  nowPlayingOpen: boolean;
  queueOpen: boolean;
}

/// Which layer Esc closes (MP-D6/MP-D10), most modal first. `null` means Esc is
/// a no-op and the key keeps its native behavior.
export function escapeTarget(layers: EscapeLayers): EscapeTarget {
  if (layers.menuOpen) return "menu";
  if (layers.miniActive && layers.miniQueueOpen) return "mini-queue";
  if (layers.miniActive) return "exit-mini";
  if (layers.nowPlayingOpen) return "now-playing";
  if (layers.queueOpen) return "queue";
  return null;
}
```

Update the last `SHORTCUTS` row:

```ts
  { keys: "Esc", description: "Close the menu, mini queue, mini player, now playing, or queue" },
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/shortcuts.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add ui/src/lib/shortcuts.ts ui/src/lib/shortcuts.test.ts
git commit -m "feat(ui): add the pure escape-target helper"
```

---

### Task 7: Pure seek math helpers

**Files:**
- Create: `ui/src/lib/seek.ts`
- Create: `ui/src/lib/seek.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: `seekRatio(clientX: number, left: number, width: number): number`; `seekByKey(key: "ArrowLeft" | "ArrowRight", position: number, duration: number): number | null`.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/seek.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { seekByKey, seekRatio } from "./seek";

describe("seekRatio", () => {
  it("maps a pointer position to a 0..1 ratio", () => {
    expect(seekRatio(150, 100, 100)).toBe(0.5);
    expect(seekRatio(100, 100, 100)).toBe(0);
    expect(seekRatio(200, 100, 100)).toBe(1);
  });

  it("clamps outside the track", () => {
    expect(seekRatio(50, 100, 100)).toBe(0);
    expect(seekRatio(250, 100, 100)).toBe(1);
  });

  it("returns 0 for a zero-width track", () => {
    expect(seekRatio(150, 100, 0)).toBe(0);
  });
});

describe("seekByKey", () => {
  it("steps ±5 s", () => {
    expect(seekByKey("ArrowLeft", 30000, 60000)).toBe(25000);
    expect(seekByKey("ArrowRight", 30000, 60000)).toBe(35000);
  });

  it("clamps to the track bounds", () => {
    expect(seekByKey("ArrowLeft", 2000, 60000)).toBe(0);
    expect(seekByKey("ArrowRight", 59000, 60000)).toBe(60000);
  });

  it("steps past the end when the duration is unknown", () => {
    expect(seekByKey("ArrowRight", 10000, 0)).toBe(15000);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/seek.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

Create `ui/src/lib/seek.ts`:

```ts
/// Pointer-to-ratio mapping for the seek control, clamped to [0, 1].
export function seekRatio(clientX: number, left: number, width: number): number {
  if (width <= 0) return 0;
  return Math.min(1, Math.max(0, (clientX - left) / width));
}

/// Keyboard seek target for ←/→: ±5 s, clamped to the track bounds when the
/// duration is known.
export function seekByKey(
  key: "ArrowLeft" | "ArrowRight",
  position: number,
  duration: number,
): number | null {
  if (key === "ArrowLeft") return Math.max(0, position - 5000);
  if (key === "ArrowRight") {
    const limit = duration > 0 ? duration : position + 5000;
    return Math.min(limit, position + 5000);
  }
  return null;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/seek.test.ts`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```powershell
git add ui/src/lib/seek.ts ui/src/lib/seek.test.ts
git commit -m "feat(ui): add pure seek math helpers"
```

---

### Task 8: Extract `SeekBar` and consume it in `PlayerBar`

**Files:**
- Create: `ui/src/lib/components/SeekBar.svelte`
- Modify: `ui/src/lib/components/PlayerBar.svelte`

**Interfaces:**
- Consumes: `seekRatio`/`seekByKey` (Task 7), `appearance.waveSeek`, `wavePath`, `Player`.
- Produces: `SeekBar.svelte` props `{ player: Player; variant?: "edge" | "inline" }` (default `"edge"`); renders nothing when there is no current track.

- [ ] **Step 1: Create the component**

Create `ui/src/lib/components/SeekBar.svelte`:

```svelte
<script lang="ts">
  import { appearance } from "../appearance.svelte";
  import type { Player } from "../player.svelte";
  import { seekByKey, seekRatio } from "../seek";
  import { wavePath } from "../wave";

  let {
    player,
    variant = "edge",
  }: { player: Player; variant?: "edge" | "inline" } = $props();

  let dragging = $state(false);
  let dragValue = $state(0);

  const shown = $derived(player.pendingSeek ?? player.state.position_ms);
  const position = $derived(dragging ? dragValue : shown);
  const duration = $derived(player.state.duration_ms);
  const progress = $derived((position / Math.max(duration, 1)) * 100);

  function seekPointer(event: PointerEvent) {
    const el = event.currentTarget as HTMLElement;
    const rect = el.getBoundingClientRect();
    const ratio = (clientX: number) => seekRatio(clientX, rect.left, rect.width);
    dragging = true;
    dragValue = ratio(event.clientX) * duration;
    el.setPointerCapture(event.pointerId);
    const onMove = (e: PointerEvent) => {
      dragValue = ratio(e.clientX) * duration;
    };
    const onUp = (e: PointerEvent) => {
      dragging = false;
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", onUp);
      player.seek(Math.round(ratio(e.clientX) * duration));
    };
    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", onUp);
  }

  function seekKeydown(event: KeyboardEvent) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    const target = seekByKey(event.key, position, duration);
    if (target === null) return;
    player.seek(target);
    event.preventDefault();
  }
</script>

{#if player.state.current}
  {@const current = player.state.current}
  <div
    class="seek {variant}"
    role="slider"
    tabindex="0"
    aria-label="Playback position"
    aria-valuemin={0}
    aria-valuemax={Math.max(duration, 1)}
    aria-valuenow={Math.round(position)}
    onpointerdown={seekPointer}
    onkeydown={seekKeydown}
  >
    {#if appearance.waveSeek}
      {@const pct = (position / Math.max(duration, 1)) * 100}
      {@const d = wavePath(current.track_id, 800, 14)}
      <div class="seek-wave">
        <svg viewBox="0 0 800 14" preserveAspectRatio="none" aria-hidden="true">
          <path d={d} fill="none" stroke="var(--line)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
          <clipPath id="seek-played"><rect x="0" y="0" width="{pct}%" height="100%" /></clipPath>
          <path d={d} fill="none" stroke="var(--accent)" stroke-width="1.5" vector-effect="non-scaling-stroke" clip-path="url(#seek-played)" />
        </svg>
      </div>
    {:else}
      <div class="seek-line">
        <div class="seek-fill" style:width="{progress}%"></div>
        <div class="seek-thumb" style:left="{progress}%"></div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .seek {
    cursor: pointer;
    display: flex;
    align-items: center;
  }
  /* Edge: hugs the player bar's top border; thin at rest, grows on hover. */
  .seek.edge {
    position: absolute;
    left: 0;
    right: 0;
    top: -1px;
    height: 12px;
    align-items: flex-start;
  }
  /* Inline (mini): a visible control at rest with a full 24px hit height. */
  .seek.inline {
    width: 100%;
    height: 24px;
  }
  .seek-line {
    position: relative;
    width: 100%;
    height: 2px;
    background: var(--line);
    transition: height var(--dur-micro) var(--ease-in);
  }
  .seek.inline .seek-line {
    height: 4px;
  }
  .seek-fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent);
  }
  .seek-thumb {
    position: absolute;
    top: 50%;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    transform: translate(-50%, -50%) scale(0);
    transition: transform var(--dur-micro) var(--ease-in);
  }
  .seek.edge:hover .seek-line,
  .seek.edge:focus-visible .seek-line {
    height: 4px;
  }
  .seek.edge:hover .seek-thumb,
  .seek.edge:focus-visible .seek-thumb,
  .seek.inline .seek-thumb {
    transform: translate(-50%, -50%) scale(1);
  }
  /* Stylized wave (Atmosphere): squashed + faint at rest, grows on hover. */
  .seek-wave {
    width: 100%;
    height: 10px;
    opacity: 0.35;
    transform: scaleY(0.4);
    transform-origin: top;
    transition:
      opacity var(--dur-micro) var(--ease-in),
      transform var(--dur-micro) var(--ease-in);
  }
  .seek.edge:hover .seek-wave,
  .seek.edge:focus-visible .seek-wave {
    opacity: 1;
    transform: scaleY(1);
  }
  .seek.inline .seek-wave {
    opacity: 1;
    transform: none;
  }
  .seek-wave svg {
    display: block;
    width: 100%;
    height: 100%;
  }
</style>
```

- [ ] **Step 2: Consume it in `PlayerBar`**

In `PlayerBar.svelte`:

1. Remove the imports `import { appearance } from "../appearance.svelte";` and `import { wavePath } from "../wave";`; add `import SeekBar from "./SeekBar.svelte";`.
2. Replace the state/handler block (`let dragging` … through the end of `seekKeydown`) with:

```ts
  const position = $derived(player.pendingSeek ?? player.state.position_ms);
  const duration = $derived(player.state.duration_ms);
```

3. Replace the whole `.seek` markup block (from `<div class="seek"` through its closing `</div>`, just after `<div class="player">`) with:

```svelte
  <SeekBar {player} />
```

4. Delete the seek styles from `<style>`: the `.seek`, `.seek-line`, `.seek-fill`, `.seek-thumb`, `.seek:hover …`, and `.seek-wave …` rules (everything between the three-zone comment and `.thumb {`).

- [ ] **Step 3: Run the UI gates**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS. (`position` and `duration` still feed the elapsed/total readout; the seek interaction is now shared.)

- [ ] **Step 4: Commit**

```powershell
git add ui/src/lib/components/SeekBar.svelte ui/src/lib/components/PlayerBar.svelte
git commit -m "refactor(ui): extract the seek control into SeekBar"
```

---

### Task 9: Mini store

**Files:**
- Create: `ui/src/lib/mini.svelte.ts`
- Create: `ui/src/lib/mini.test.ts`

**Interfaces:**
- Consumes: `api.setMiniMode` (Task 5), `errorMessage`, `Toasts`.
- Produces: `Mini` interface and `createMini(toasts: Toasts): Mini` (spec §4.1; the factory takes `toasts` because errors surface as toasts, matching `createFavorites`/`createCollectionTree`).

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/mini.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { createMini } from "./mini.svelte";
import type { ToastSeverity, Toasts } from "./toasts.svelte";

vi.mock("./api", () => ({
  api: { setMiniMode: vi.fn(), getConfig: vi.fn() },
}));

function fakeToasts() {
  const calls: { message: string; severity: ToastSeverity }[] = [];
  const toasts: Toasts = {
    get toasts() {
      return [];
    },
    show(message, options) {
      calls.push({ message, severity: options?.severity ?? "info" });
      return calls.length;
    },
    dismiss() {},
  };
  return { toasts, calls };
}

describe("mini store", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("enter sets active after the window command succeeds", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);

    await mini.enter();

    expect(api.setMiniMode).toHaveBeenCalledWith(true);
    expect(mini.active).toBe(true);
  });

  it("exit clears active and any open queue sheet", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);
    await mini.enter();
    mini.toggleQueue();

    await mini.exit();

    expect(api.setMiniMode).toHaveBeenLastCalledWith(false);
    expect(mini.active).toBe(false);
    expect(mini.queueOpen).toBe(false);
  });

  it("a rejected window command keeps the flag and toasts", async () => {
    vi.mocked(api.setMiniMode).mockRejectedValue("no window");
    const { toasts, calls } = fakeToasts();
    const mini = createMini(toasts);

    await mini.enter();

    expect(mini.active).toBe(false);
    expect(calls[0]?.severity).toBe("error");
  });

  it("toggle dispatches on the current state", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);

    await mini.toggle();
    expect(mini.active).toBe(true);

    await mini.toggle();
    expect(mini.active).toBe(false);
    expect(api.setMiniMode).toHaveBeenNthCalledWith(1, true);
    expect(api.setMiniMode).toHaveBeenNthCalledWith(2, false);
  });

  it("entering closes an open queue sheet", async () => {
    vi.mocked(api.setMiniMode).mockResolvedValue(undefined);
    const mini = createMini(fakeToasts().toasts);
    mini.toggleQueue();

    await mini.enter();

    expect(mini.queueOpen).toBe(false);
  });

  it("init mirrors start_in_mini without touching the window", async () => {
    vi.mocked(api.getConfig).mockResolvedValue({ ui: { start_in_mini: true } } as never);
    const mini = createMini(fakeToasts().toasts);

    await mini.init();

    expect(mini.active).toBe(true);
    expect(api.setMiniMode).not.toHaveBeenCalled();
  });

  it("a config failure leaves the default off", async () => {
    vi.mocked(api.getConfig).mockRejectedValue("offline");
    const mini = createMini(fakeToasts().toasts);

    await mini.init();

    expect(mini.active).toBe(false);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm --prefix ui test -- src/lib/mini.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

Create `ui/src/lib/mini.svelte.ts`:

```ts
import { api } from "./api";
import { errorMessage } from "./errors";
import type { Toasts } from "./toasts.svelte";

export interface Mini {
  readonly active: boolean;
  readonly queueOpen: boolean;
  init(): Promise<void>;
  enter(): Promise<void>;
  exit(): Promise<void>;
  toggle(): Promise<void>;
  toggleQueue(): void;
}

/// Mini-player layout state (MP-D2): Rust owns the window; this store mirrors
/// it only after `set_mini_mode` succeeds, so the layout can never disagree
/// with the real window size. `init` trusts the launch resize Rust already did.
export function createMini(toasts: Toasts): Mini {
  let active = $state(false);
  let queueOpen = $state(false);

  async function setMode(next: boolean): Promise<void> {
    try {
      await api.setMiniMode(next);
      active = next;
      queueOpen = false;
    } catch (e) {
      toasts.show(errorMessage(e), { severity: "error" });
    }
  }

  async function init(): Promise<void> {
    try {
      const config = await api.getConfig();
      active = config.ui.start_in_mini;
    } catch {
      active = false;
    }
  }

  return {
    get active() {
      return active;
    },
    get queueOpen() {
      return queueOpen;
    },
    init,
    enter: () => setMode(true),
    exit: () => setMode(false),
    toggle: () => setMode(!active),
    toggleQueue: () => {
      queueOpen = !queueOpen;
    },
  };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/mini.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```powershell
git add ui/src/lib/mini.svelte.ts ui/src/lib/mini.test.ts
git commit -m "feat(ui): add the mini player store"
```

---

### Task 10: `Mini.svelte` view and the minimize icon

**Files:**
- Create: `ui/src/views/Mini.svelte`
- Modify: `ui/src/lib/components/Icon.svelte`

**Interfaces:**
- Consumes: `SeekBar` (Task 8), `Mini` (Task 9), `QueueRail`, `Cover`, `Icon`, `Player`, `Favorites`.
- Produces: `Mini.svelte` props `{ player: Player; favorites: Favorites; mini: Mini }`; icon `minimize-2`.

- [ ] **Step 1: Add the icon**

In `Icon.svelte`, add before the closing `} as const` (after `"arrow-up-down"`):

```ts
    "minimize-2": '<path d="m14 10 7-7" /> <path d="M20 10h-6V4" /> <path d="m3 21 7-7" /> <path d="M4 14h6v6" />',
```

- [ ] **Step 2: Create the view**

Create `ui/src/views/Mini.svelte`:

```svelte
<script lang="ts">
  import type { RepeatMode } from "../lib/api";
  import type { Favorites } from "../lib/favorites.svelte";
  import type { Mini } from "../lib/mini.svelte";
  import type { Player } from "../lib/player.svelte";
  import Cover from "../lib/components/Cover.svelte";
  import Icon from "../lib/components/Icon.svelte";
  import QueueRail from "../lib/components/QueueRail.svelte";
  import SeekBar from "../lib/components/SeekBar.svelte";

  let {
    player,
    favorites,
    mini,
  }: { player: Player; favorites: Favorites; mini: Mini } = $props();

  function nextRepeat(mode: RepeatMode): RepeatMode {
    if (mode === "off") return "all";
    if (mode === "all") return "one";
    return "off";
  }
</script>

<div class="mini">
  {#if player.state.current}
    {@const current = player.state.current}
    {@const fav = favorites.hasTrack(current.track_id)}
    <div class="art">
      <Cover uuid={current.cover} size={320} radius={10} alt={`${current.title} artwork`} />
    </div>
    <div class="meta">
      <strong class="title">{current.title}</strong>
      <span class="muted artist">{current.artist}</span>
    </div>
    <SeekBar {player} variant="inline" />
    <div class="transport">
      <button class="ibtn" onclick={() => player.prev()} aria-label="Previous">
        <Icon name="skip-back" size={20} />
      </button>
      <button class="play" onclick={() => player.playPause()} aria-label="Play/Pause">
        <span class="play-ico" class:show={!player.state.playing}><Icon name="play" size={20} /></span>
        <span class="play-ico" class:show={player.state.playing}><Icon name="pause" size={20} /></span>
      </button>
      <button class="ibtn" onclick={() => player.next()} aria-label="Next">
        <Icon name="skip-forward" size={20} />
      </button>
    </div>
    <div class="secondary">
      <button
        class="ibtn icon-heart"
        class:on={fav}
        onclick={() => favorites.toggleTrack(current.track_id)}
        aria-label={fav ? "Unfavorite" : "Favorite"}
        title={fav ? "Unfavorite" : "Favorite"}
      ><Icon name="heart" size={18} /></button>
      <button
        class="ibtn"
        class:active={player.state.shuffle}
        onclick={() => player.setShuffle(!player.state.shuffle)}
        aria-label="Shuffle"
        title="Shuffle the rest of the queue"
      ><Icon name="shuffle" size={18} /></button>
      <button
        class="ibtn"
        class:active={player.state.repeat !== "off"}
        onclick={() => player.setRepeat(nextRepeat(player.state.repeat))}
        aria-label="Repeat"
        title={player.state.repeat === "one"
          ? "Repeat one"
          : player.state.repeat === "all"
            ? "Repeat all"
            : "Repeat off"}
      ><Icon name={player.state.repeat === "one" ? "repeat-1" : "repeat"} size={18} /></button>
      <label class="volume">
        <Icon name="volume-2" size={16} />
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={player.state.volume}
          oninput={(e) => player.setVolume(Number((e.currentTarget as HTMLInputElement).value))}
          onchange={(e) =>
            player.setVolume(Number((e.currentTarget as HTMLInputElement).value), true)}
          aria-label="Volume"
        />
      </label>
      <button
        class="ibtn"
        class:active={mini.queueOpen}
        onclick={() => mini.toggleQueue()}
        aria-label="Queue"
        title="Queue"
        aria-pressed={mini.queueOpen}
      ><Icon name="list-music" size={18} /></button>
      <button
        class="ibtn"
        onclick={() => void mini.exit()}
        aria-label="Restore full window"
        title="Restore full window"
      ><Icon name="maximize-2" size={18} /></button>
    </div>
  {:else}
    <div class="empty">
      <Icon name="music" size={40} />
      <p class="muted">Nothing playing</p>
    </div>
  {/if}

  {#if player.state.error}
    <div class="error">
      <span style="color:var(--danger)">{player.state.error}</span>
      <button onclick={() => player.retry()} aria-label="Retry playback">Retry</button>
    </div>
  {/if}

  {#if mini.queueOpen}
    <div class="sheet-backdrop" onclick={() => mini.toggleQueue()} aria-hidden="true"></div>
    <div class="queue-sheet">
      <QueueRail {player} />
    </div>
  {/if}
</div>

<style>
  .mini {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    height: 100%;
    padding: 14px;
    position: relative;
    z-index: 1;
    overflow: auto;
  }
  /* The cover is fluid: it gives up width before the controls run out of room. */
  .art {
    flex: 0 0 auto;
    width: min(100%, 190px, 42vh);
    border-radius: var(--radius-card);
    overflow: hidden;
  }
  .meta {
    width: 100%;
    min-width: 0;
    text-align: center;
  }
  .title {
    display: block;
    font-family: var(--font-display);
    font-size: 16px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .artist {
    display: block;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .transport {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }
  .secondary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: center;
    gap: 4px;
    width: 100%;
  }
  .empty {
    margin: auto;
    display: grid;
    place-items: center;
    gap: 8px;
    color: var(--text-muted);
  }
  .error {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  button {
    display: inline-grid;
    place-items: center;
  }
  .ibtn {
    position: relative;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    color: var(--text-muted);
    transition:
      color var(--dur-micro),
      background-color var(--dur-micro),
      transform var(--dur-micro) var(--ease-in);
  }
  .ibtn:hover {
    background: var(--surface-3);
    color: var(--text);
  }
  .ibtn:active {
    transform: scale(0.92);
  }
  .ibtn.active {
    color: var(--accent);
  }
  .icon-heart.on {
    color: var(--accent);
  }
  .icon-heart.on :global(svg) {
    fill: currentColor;
  }
  .play {
    position: relative;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    transition:
      transform var(--dur-micro) var(--ease-in),
      box-shadow var(--dur-state) var(--ease-in);
  }
  .play:hover {
    transform: scale(1.06);
    box-shadow: 0 0 16px color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .play-ico {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity var(--dur-state) var(--ease-in),
      transform var(--dur-state) var(--ease-in);
  }
  .play-ico.show {
    opacity: 1;
    transform: scale(1);
  }
  .volume {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--text-muted);
  }
  .volume input[type="range"] {
    width: 56px;
    accent-color: var(--accent);
  }
  /* Queue sheet (MP-D5): slides up over the mini layout, never leaves mini. */
  .sheet-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    z-index: 10;
  }
  .queue-sheet {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 11;
    max-height: 85vh;
    overflow: auto;
    background: var(--surface-1);
    backdrop-filter: blur(var(--glass-blur));
    border-top: 1px solid var(--line);
    border-radius: var(--radius-sheet) var(--radius-sheet) 0 0;
    animation: sheet-in var(--dur-view) var(--ease-in) both;
  }
  @keyframes sheet-in {
    from {
      transform: translateY(100%);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .queue-sheet {
      animation-name: sheet-fade;
    }
    @keyframes sheet-fade {
      from {
        opacity: 0;
      }
    }
  }
</style>
```

- [ ] **Step 3: Run the UI gates**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS. (The view is not imported yet; this only checks syntax/types.)

- [ ] **Step 4: Commit**

```powershell
git add ui/src/views/Mini.svelte ui/src/lib/components/Icon.svelte
git commit -m "feat(ui): add the mini player view"
```

---

### Task 11: Wire mini into the shell — PlayerBar button and App

**Files:**
- Modify: `ui/src/lib/components/PlayerBar.svelte`
- Modify: `ui/src/App.svelte`

**Interfaces:**
- Consumes: `Mini` store (Task 9), `Mini` view (Task 10), `escapeTarget` (Task 6).
- Produces: `PlayerBar` gains a required `mini: Mini` prop and an entry button; `App` creates the store, calls `mini.init()`, renders the mini shell, and routes Esc through `escapeTarget`.

- [ ] **Step 1: Add the mini entry button to `PlayerBar`**

In `PlayerBar.svelte`:

1. Add `import type { Mini } from "../mini.svelte";` to the imports.
2. Extend the props type and destructuring:

```ts
  let {
    player,
    qualityDisplay,
    favorites,
    nav,
    mini,
  }: {
    player: Player;
    qualityDisplay: QualityDisplay;
    favorites: Favorites;
    nav: Nav;
    mini: Mini;
  } = $props();
```

3. In the right zone, after the `.badges` div and before `{#if player.state.bit_perfect_notice}`, add:

```svelte
      <button
        class="ibtn"
        onclick={() => void mini.enter()}
        aria-label="Mini player"
        title="Mini player"
      ><Icon name="minimize-2" size={18} /></button>
```

- [ ] **Step 2: Create the store and render the mini shell in `App.svelte`**

In `App.svelte`:

1. Add imports: `import Mini from "./views/Mini.svelte";` (after the `NowPlaying` import) and `import { createMini } from "./lib/mini.svelte";` (with the other store imports).
2. Change the shortcuts import to include `escapeTarget`:

```ts
  import { escapeTarget, isShortcutHandled, matchShortcut, type ShortcutAction } from "./lib/shortcuts";
```

3. After `const toasts = createToasts();` add:

```ts
  const mini = createMini(toasts);
```

4. Add an effect next to the appearance init effect:

```ts
  $effect(() => {
    void mini.init();
  });
```

5. In the `session_expired` listener, after `collectionActions.reset();` add:

```ts
      if (mini.active) void mini.exit();
```

6. In `onLogout`, before `loggedIn = false;` add:

```ts
    if (mini.active) void mini.exit();
```

7. Add the escape helper before `runShortcut`:

```ts
  function currentEscapeTarget() {
    return escapeTarget({
      menuOpen: menu.open,
      miniActive: mini.active,
      miniQueueOpen: mini.queueOpen,
      nowPlayingOpen: nav.nowPlayingOpen,
      queueOpen: nav.queueOpen,
    });
  }
```

8. Replace the `case "escape":` body in `runShortcut` with:

```ts
      case "escape":
        switch (currentEscapeTarget()) {
          case "menu":
            menu.close();
            break;
          case "mini-queue":
            mini.toggleQueue();
            break;
          case "exit-mini":
            void mini.exit();
            break;
          case "now-playing":
            nav.closeNowPlaying();
            break;
          case "queue":
            nav.toggleQueue();
            break;
        }
        break;
```

9. Replace the escape guard in `onKeydown`:

```ts
    if (action === "escape" && currentEscapeTarget() === null) return;
```

10. Wrap the `.app` shell in the mini branch. The logged-in block becomes:

```svelte
{:else}
  <Backdrop {player} />
  {#if mini.active}
    <Mini {player} {favorites} {mini} />
  {:else}
    <div class="app" data-sidebar={sidebarRail ? "rail" : "full"}>
      <!-- existing Sidebar, main, queue, PlayerBar, PlaylistPicker, CollectionDialogs stay here -->
    </div>
  {/if}
  <div class="grain" aria-hidden="true"></div>
{/if}
```

11. Pass the store to the player bar: `<PlayerBar {player} {qualityDisplay} {favorites} {nav} {mini} />`.

- [ ] **Step 3: Run the UI gates**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add ui/src/lib/components/PlayerBar.svelte ui/src/App.svelte
git commit -m "feat(ui): enter the mini player from the player bar"
```

---

### Task 12: Settings "Start in mini player" toggle

**Files:**
- Modify: `ui/src/views/Settings.svelte`

**Interfaces:**
- Consumes: `api.setStartInMini` (Task 5), `Config.ui.start_in_mini`.
- Produces: a checkbox in the Appearance panel that persists the launch preference (not the live state, per spec §11).

- [ ] **Step 1: Add the setter**

In `Settings.svelte`, after `setDisplay`:

```ts
  function setStartInMini(enabled: boolean) {
    if (!config) return;
    config = { ...config, ui: { ...config.ui, start_in_mini: enabled } };
    api.setStartInMini(enabled).catch(apply);
  }
```

- [ ] **Step 2: Add the field**

In the Appearance panel, after the Cover shape `.field` div (before its closing `</div>`), add:

```svelte
      <div class="field">
        <span class="field-label">Mini player:</span>
        <label>
          <input
            type="checkbox"
            checked={config.ui.start_in_mini}
            onchange={(e) => setStartInMini((e.target as HTMLInputElement).checked)}
          />
          Start in mini player
        </label>
      </div>
```

- [ ] **Step 3: Run the UI gates**

Run: `npm --prefix ui run check && npm --prefix ui test`
Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add ui/src/views/Settings.svelte
git commit -m "feat(ui): add the start-in-mini setting"
```

---

### Task 13: Smoke checklist and final gates

**Files:**
- Create: `docs/superpowers/plans/notes/2026-09-28-mini-player-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the manual GUI checklist required by the spec §9 (a human with a real account runs it).

- [ ] **Step 1: Write the checklist**

Create `docs/superpowers/plans/notes/2026-09-28-mini-player-smoke.md`:

```markdown
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

## Modes, motion, color

- [ ] Atmosphere: backdrop, glass, and dynamic color follow the cover in mini; Instrument stays flat.
- [ ] Reduced motion: the queue sheet fades instead of sliding.
- [ ] `npm --prefix ui run check`, `npm --prefix ui test`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` all green.
```

- [ ] **Step 2: Run every gate**

Run:

```powershell
cargo test --workspace; cargo clippy --workspace -- -D warnings; npm --prefix ui run check; npm --prefix ui test
```

Expected: all PASS.

- [ ] **Step 3: Commit**

```powershell
git add docs/superpowers/plans/notes/2026-09-28-mini-player-smoke.md
git commit -m "test: record the mini player smoke checklist"
```

---

## Self-Review Notes

- **Spec coverage:** §3 decisions MP-D1–D11 map to Tasks 2–4 (window fate, geometry, startup, close capture), 6 (Esc order), 8 (SeekBar), 9–11 (store, view, entry/exit), 12 (`start_in_mini`). §6 config/IPC is Tasks 1, 4, 5. §8 edge cases: idempotent enter (Task 4), rejected command (Task 9 test), maximized stash (Tasks 2, 4), logout/session (Task 11), nothing playing (Task 10). §9 testing is Tasks 1–3, 6–7, 9 plus the Task 13 checklist. §4.1's `createMini()` singleton is realized as `createMini(toasts)` to match the codebase's toast-injected stores.
- **Type consistency:** `Mini` (store) vs `Mini.svelte` (view) share only the prop name; `FullGeometry` fields are used verbatim in Tasks 3–4 and `save_window_state`; `escapeTarget` keys match `EscapeLayers`.
- **Known limitation:** window mutations themselves have no automated test (no display in CI), per spec §9; the stash decision logic and config are covered.
