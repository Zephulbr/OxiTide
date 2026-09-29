# OxiTide — Oneko (desktop cat)

- **Status:** Draft for review
- **Date:** 2026-09-29
- **Owner:** V
- **References:** [adryd325/oneko.js](https://github.com/adryd325/oneko.js) (MIT) ·
  [kyrie25/spicetify-oneko](https://github.com/kyrie25/spicetify-oneko) (MIT)
- **Process note:** scope, behaviors, placement, and look were brainstormed live with
  the owner (2026-09-29). This document is the standalone source of truth.

---

## 1. Purpose & Outcome

Bring the classic **oneko cat** to OxiTide: a 32×32 pixel cat that chases the mouse
cursor around the window, with the original idle behaviors (sleeping, scratching) and
the spicetify additions the owner picked — **drag her around** and **double-click to
put her to sleep on the now-playing seek bar**.

**Outcome:** when enabled in Settings, one cat roams the whole (logged-in) app,
including the mini player. She never blocks clicks in practice (she keeps a 48px
standoff from the cursor), never covers dialogs, menus, or toasts, and can be switched
off instantly from Settings. The enabled flag lives in the TOML config and survives
restarts; her position survives restarts too. The sprite and behavior are ported from
the two MIT reference repos, credited in Settings and in a `LICENSE` file beside the
vendored sprite.

Success = she follows naturally at the reference cadence (10 ticks/s), respects
reduced-motion by not appearing at all, and the whole feature is a leaf: no changes to
playback, IPC beyond one config command, or existing component behavior except one
data attribute on the seek bar.

## 2. Scope

**In scope:** a DOM-free, unit-tested engine (`oneko.ts`) ported from oneko.js +
spicetify-oneko; a thin Svelte shell component (`Neko.svelte`) owning RAF, listeners,
and direct style writes; a vendored `oneko.gif` + its MIT `LICENSE`; the seek-bar
perch attribute; the `oneko` config flag end-to-end (Rust config, command, handler
registration, `api.ts`, appearance singleton); the Settings checkbox with attribution;
unit tests, Rust round-trip test, and a human GUI smoke checklist.

**Out of scope:** cat variants / picker / keyword easter egg / right-click kuroneko
(explicitly deferred, §11); interactions with UI elements other than the seek-bar
perch; sound; any network fetch (sprite is bundled); persistence of sleep state across
restarts; any change to playback, queue, catalog, or auth behavior.

## 3. Decisions (owner-confirmed)

| # | Decision | Rationale |
|---|---|---|
| N-D1 | **Scope: core + drag + wake/sleep.** Chase, alert, idle sleep/scratch; drag her with the left mouse button; double-click toggles sleep. No variants, no picker, no keyword. | Owner choice among three scopes; keeps v1 a leaf feature. |
| N-D2 | **Placement: everywhere in the app once logged in** — main shell and mini player, one cat. Not on the login screen. | Owner choice; mini included with full size. |
| N-D3 | **Sleep perch: the now-playing seek bar.** Double-click sends her to the seek bar, where she curls up about 60% of the way along it (`PERCH_SETTLE = 0.6` of the bar width; owner-tuned after smoke, not the right end); second double-click wakes her. If no seek bar is rendered (nothing playing), she sleeps in place. | Owner choice, spicetify parity; the seek bar is tagged with a data attribute rather than hardcoding CSS classes. |
| N-D4 | **Look: classic colors in both modes.** No mode branching (AGENTS.md). | Owner choice; recognizability over theme tinting. |
| N-D5 | **Implementation: DOM-free engine + Svelte shell.** Engine state machine with injectable RNG is unit-tested in vitest; the shell only touches DOM/RAF/localStorage. | Owner choice (approach A); matches the repo's pure-module + vitest convention. |
| N-D6 | **Toggle: Settings checkbox**, persisted as `ui.oneko` in the TOML config (default off), via a `set_oneko` command mirroring `set_start_in_mini`. | Owner note; matches existing appearance plumbing. |
| N-D7 | **Reduced motion: she does not appear.** When the OS reduced-motion preference is on, the component is not mounted and the Settings row shows a muted note. | Mirrors oneko.js's own bail; consistent with `discSpin`/`ambientMotion`. |
| N-D8 | **Layering: fixed `z-index: 8`** — above app content (1) and grain (5), below modals (10), queue overlay (19/20), toasts (30), and the context menu (50). | She must never cover real UI. |
| N-D9 | **Credits:** a `LICENSE` file beside the vendored sprite and an attribution line with links below the Settings option. The two GitHub repo URLs are appended to the `opener` capability allowlist. | Owner note; MIT compliance and honest credit. |
| N-D10 | **Persistence: enabled flag in TOML; position in `localStorage` (`oxitide:oneko`), clamped on restore; sleep state not persisted.** | Flag is a real setting; position is ephemeral UI state, mirroring oneko.js. |

## 4. Research summary

- **oneko.js** (adryd325, MIT): a single IIFE placing a 32×32 fixed div with
  `image-rendering: pixelated` and a 256×128 gif sprite sheet (8×4 cells of 32px).
  A `requestAnimationFrame` loop throttled to 100ms drives `frame()`: chase the mouse
  at 10px/tick, stop inside 48px, then idle. Idle rolls (after `idleTime > 10`, 1-in-200
  per tick) pick `sleeping`, `scratchSelf`, or a wall scratch if near an edge.
  Direction sprites are chosen from `diffX`/`diffY` ratios (`> 0.5` / `< -0.5`).
  Resuming movement after idling shows `alert` for a countdown of ticks. Position is
  clamped to `[16, viewport - 16]`. State persists in `localStorage` on unload.
- **spicetify-oneko** (kyrie25, MIT): same core plus drag (mousedown on the cat,
  scratch sprite opposite the drag direction past 10px, nudge on release), double-click
  `sleep()` that retargets the cat to the progress bar's right end (`x = right - 16`,
  `y = top - 8`), sleeping loops until woken, and a resize handler that recomputes the
  perch. It drops `pointer-events: none` so the cat can receive mouse events.
- **OxiTide integration points verified:** `appearance.svelte.ts` pattern
  (L46–137), `set_start_in_mini` command (commands.rs L1102) and handler list
  (main.rs L281), `api.ts` Config.ui (L107–122), Settings Appearance tab toggle
  (Settings.svelte L299–309), App.svelte logged-in mount point beside `.grain`
  (L406), `SeekBar.svelte` root element (L51–63), z-index map (app.css/Modal/ToastHost/
  ContextMenu), `opener` allowlist (capabilities/default.json), and Vite asset imports
  covered by `vite/client` types.

## 5. Architecture & components

### 5.1 Engine — `ui/src/lib/oneko.ts` (new, pure)

No DOM, no timers, no globals; deterministic under an injected RNG.

```ts
export const NEKO_SIZE = 32;
export const NEKO_SPEED = 10;   // px per tick, reference value
export const TICK_MS = 100;     // reference cadence
export const IDLE_RADIUS = 48;  // stop distance
export const PERCH_SETTLE = 0.6; // fraction along the seek bar where she sleeps

export interface Sprite { col: number; row: number }   // 8x4 gif grid
export interface NekoFrame { x: number; y: number; sprite: Sprite }

export function createNekoEngine(opts?: { rng?: () => number }): NekoEngine;

interface NekoEngine {
  setMouse(x: number, y: number): void;
  setViewport(w: number, h: number): void;
  setPosition(x: number, y: number): NekoFrame; // restore; clamps, returns the idle frame
  startDrag(x: number, y: number): void;    // left button pressed on the cat; snapshots anchor + cat pos
  dragTo(x: number, y: number): NekoFrame;  // move while held; frame applied at pointer rate
  endDrag(): void;                          // release -> nudge
  toggleSleep(perch: PerchRect | null): void;
  setPerch(perch: PerchRect | null): void;  // resize while asleep
  tick(): NekoFrame;                        // advance one 100ms step
}
export interface PerchRect { left: number; right: number; top: number }
export function clampPosition(x: number, y: number, w: number, h: number): { x: number; y: number };
```

The sprite table is the oneko.js table transcribed to positive grid coordinates
(`idle`, `alert`, `tired`, `sleeping`, `scratchSelf`, `scratchWallN/S/E/W`, and the
eight compass walk rows). `frame()` logic ports as-is: alert countdown, direction
selection, 2-frame walk alternation on `frameCount`, idle roll + animation state
machine, forced sleep target walk/snap with nudge handling, drag overrides, and viewport
clamping. Viewport-size defaults are `0×0` until the shell sets them.

### 5.2 Shell — `ui/src/lib/components/Neko.svelte` (new)

- Renders one `aria-hidden` 32×32 fixed div: imported gif as `background-image`,
  `image-rendering: pixelated`, `cursor: grab`/`grabbing` while dragging, `z-index: 8`,
  `pointer-events: auto` (required for drag).
- One `$effect` owns everything and cleans up on destroy: starts a RAF loop that calls
  `engine.tick()` at most once per 100ms (DiscArt pattern), attaches window
  `mousemove`/`resize`, and subscribes to the cat's `mousedown` (button 0) / `dblclick`.
- After each `tick()`, writes `left`, `top`, and `background-position` directly to the
  bound element — no per-frame Svelte state.
- Drag: `mousedown` → `engine.startDrag(e.clientX, e.clientY)`, window `mousemove` →
  `engine.dragTo`, window `mouseup` → `engine.endDrag()`; listeners are removed when
  the drag ends or the component is destroyed.
- Sleep: `dblclick` queries `[data-oneko-perch]`; found → `toggleSleep({right, top})`
  from `getBoundingClientRect()`; absent → `toggleSleep(null)` (sleep in place).
- Resize: `engine.setViewport(innerWidth, innerHeight)`; if asleep, re-query the perch
  and call `setPerch(...)` (or `null` to sleep in place).
- Persistence: reads `oxitide:oneko` on mount and restores through
  `engine.setPosition` (which clamps); writes `{x, y}` at most once per second while
  she moves and once on destroy; all storage access is try/caught (storage failure =
  no persistence, never an error).

### 5.3 Perch — `ui/src/lib/components/SeekBar.svelte` (touch)

Add `data-oneko-perch` to the `.seek` root element. One attribute covers both the main
player bar (`variant="edge"`) and the mini player (`variant="inline"`). The sleep
target is `x = rect.left + (rect.right - rect.left) * 0.6`, `y = rect.top - 8`
(owner-tuned settle fraction after the smoke test; the y offset remains pixel-tuned
there).

### 5.4 Config & IPC (mirrors `start_in_mini`)

- `crates/oxitide-app/src/config.rs`: `pub oneko: bool` on `UiConfig` (default
  `false`); update the `UiConfig` literal in the save/load round-trip test and add a
  round-trip test modeled on `start_in_mini_defaults_false_and_round_trips`.
- `crates/oxitide-app/src/commands.rs`: `#[tauri::command] set_oneko(state, enabled)`
  next to `set_start_in_mini`; register `commands::set_oneko` in
  `main.rs generate_handler![]`.
- `ui/src/lib/api.ts`: `oneko: boolean` in `Config.ui`; `setOneko(enabled)`.
- `ui/src/lib/appearance.svelte.ts`: `oneko` `$state` read in `init()`, readonly
  getter on the `Appearance` interface, `setOneko` via `persist(() => api.setOneko(...))`.

### 5.5 Settings & attribution — `ui/src/views/Settings.svelte` (touch)

Appearance tab, directly under the Mini player row:

- Checkbox labelled **"Cat"** (no explanation — owner choice), optimistically patched
  and persisted through `appearance.setOneko`; failures surface via `apply()`.
- When `appearance.reducedMotion`: muted note "Paused while your system's
  reduced-motion setting is on."
- Below the option, a credits line with openable links:
  "Credits: oneko.js by adryd325 & spicetify-oneko by kyrie25". Links use `openUrl`
  from `@tauri-apps/plugin-opener`, and the two repo URLs are added to
  `capabilities/default.json`'s `opener:allow-open-url` allowlist. The MIT text ships
  in `ui/src/assets/oneko/LICENSE`.

### 5.6 Mount — `ui/src/App.svelte` (touch)

Inside the logged-in branch, beside `.grain`:

```svelte
{#if appearance.oneko && !appearance.reducedMotion}<Neko />{/if}
```

## 6. Behavior specification

- **Cadence:** one engine tick per 100ms; RAF is throttled, never catches up multiple
  ticks after a gap (backgrounded window freezes her, she resumes in place).
- **Chase:** move 10px toward the cursor per tick; direction sprite from the
  `diffX`/`diffY` ratio thresholds; walk frames alternate each tick; position clamped
  to `[16, viewport - 16]` on both axes.
- **Idle:** inside 48px she stops and idles (default `idle` sprite). After `idleTime >
  10` (~1s), each tick has a 1-in-200 chance to start one of `sleeping`, `scratchSelf`,
  or a wall scratch when within 16px of the corresponding edge; sleeping runs
  `tired` (8 ticks) → `sleeping` loop (~19s) then resets unless forced.
- **Alert:** resuming movement after an idle shows `alert` and pauses movement for the
  reference countdown (up to 7 ticks), so she visibly "notices" you.
- **Drag:** holding the left button shows `alert`; moving more than 10px shows the wall
  scratch facing against the drag; releasing nudges her (wakes from idle) and she
  resumes chasing or returns to the perch if she was told to sleep.
- **Sleep:** double-click toggles. Asleep: she walks to the perch target, snaps onto it
  within one tick of arrival, plays `tired` → `sleeping` and loops there; while asleep,
  mouse movement does not retarget her; double-click again wakes her. Dragging her while
  asleep repositions her, and on release she walks back to the perch and resumes
  sleeping. No perch (nothing playing): she sleeps where she stands.
- **Resize:** viewport updates clamp her; while asleep the perch is re-queried, and if
  it disappeared she sleeps in place instead.
- **Reduced motion:** if the preference turns on while she is out, the component
  unmounts and she disappears; turning it off with the flag still on brings her back.
- **Restart:** enabled flag from TOML; position restored from `localStorage` and
  clamped; she starts awake.

## 7. Error handling & degradation

- `localStorage` unavailable or corrupt: persistence silently disabled; cat works.
- Engine never throws: unknown state falls back to `idle`; no frame is skipped.
- Perch query returns an element with a degenerate rect (hidden/zero): treated as
  absent → sleep in place.
- Config save failure surfaces through the existing `apply()` toast, as with every
  other appearance setting.
- If the gif fails to load, the div stays invisible but harmless (no crash); a smoke
  check covers the happy path.

## 8. Performance

One 32×32 composited div, style writes only when values change, 10 ticks/s — far below
the cost of existing ambient animation. No network, no extra threads, no polling. The
RAF loop is torn down with the component, so a disabled cat costs nothing.

## 9. Testing & verification

**Unit (vitest, `ui/src/lib/oneko.test.ts`, injected RNG):** chase closes distance;
stop at the 48px radius; all eight direction mappings; walk frame alternation; idle
roll triggers with a forced RNG and picks sleep; forced sleep walks, snaps, tires, and
loops; drag scratch + nudge; alert countdown; viewport clamping; `clampPosition` on
restore; `setPerch(null)` falls back to sleeping in place.

**Rust:** `oneko` config default/round-trip test.

**Checks:** `cargo test --workspace`; `cargo clippy --workspace -- -D warnings`;
`npm --prefix ui run check`; `npm --prefix ui test`.

**Human GUI smoke** (`docs/superpowers/plans/notes/oneko-smoke.md`): follows cursor;
idle sleep/scratch; drag; double-click sleep on the seek bar in wave mode, line mode,
and mini; wake; toggle off removes her immediately; restart restores position and flag;
reduced-motion note + no cat; she does not block clicks on a button she crosses.

## 10. Risks & open verifications

- **Sleep y-offset** depends on the sprite's visual paw line; initial `top - 8` is a
  reference value and is tuned in the smoke check.
- **Tauri close may skip `beforeunload`;** position persistence is throttled to ≤1s, so
  at worst the restored spot is one second stale. Acceptable.
- **Tauri `innerWidth/innerHeight` vs. device pixel ratio:** all engine math is in CSS
  pixels, matching `clientX/clientY`; no scaling is applied.
- **Svelte 5 teardown ordering** must cancel RAF before removing listeners; verified by
  the component's `$effect` cleanup (smoke-checked by toggling rapidly).

## 11. Future / explicitly deferred

Cat variants and the "oneko" keyword picker; right-click black cat; petting; sleep
position on other panes; persisted sleep state; sound. None of these affect the engine
API beyond adding sprite rows and one shell listener.

## 12. Licensing

- `oneko.gif` and the ported engine behavior: adryd325/oneko.js, MIT — ship the full
  MIT text as `ui/src/assets/oneko/LICENSE`.
- Drag/sleep behaviors: kyrie25/spicetify-oneko, MIT — credited in Settings and in this
  document; no code copied verbatim beyond the shared upstream oneko logic.
- OxiTide itself is an unofficial personal project with no license file; no change.
