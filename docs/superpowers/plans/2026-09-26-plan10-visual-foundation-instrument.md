# Plan 10 — Visual Foundation + Instrument Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the placeholder styling with the Track 2b design system — Warm Graphite tokens, bundled Fraunces/Manrope/Inter, Lucide icons, covers everywhere, the Refined Classic shell, expanded now-playing (static sleeve), and the Settings → Appearance surface — landing it all in Instrument mode (flat, static) so Plan 11 lights up Atmosphere on a de-risked base.

**Architecture:** Three new stylesheets (`tokens.css`, `modes.css`, `fonts.css`) carry the whole design; components consume only semantic tokens and utility classes, never mode names. A singleton `appearance.svelte.ts` owns the five appearance settings, applies them to `document.documentElement`, and persists through six new per-key config commands following the existing `set_quality_display` pattern. Cover art flows from existing UUID fields through a new `artwork.ts` URL builder and `Cover.svelte` (with a warm fallback tile); `HomeItem` gains a best-effort `artwork` field. All testable logic (URL builder, font stack, queue-width clamp, nav state, home parsing, config round-trips) is pure and unit-tested; components are covered by `svelte-check` and the smoke checklist.

**Tech Stack:** Svelte 5 runes, TypeScript, vitest (node environment), Rust (config + DTO changes only; no new crates, one new serde enum family), vendored woff2 fonts (no new npm packages).

**Spec:** `docs/superpowers/specs/2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (foundation; Atmosphere-only systems are Plan 11) · `docs/superpowers/specs/2026-09-25-oxitide-2b-visual-mode-instrument-design.md` (what ships first) · `docs/superpowers/specs/2026-09-25-oxitide-2b-visual-convergence.md` (build order step 1)

**Out of scope (owner decisions, 2026-09-26):** the mini player ships as a follow-up slice after 2b core (spec §7.4/§15); Hi-Fi Rack gets its own later spec (convergence §2); true waveform peaks stay deferred (spec §10.2).

## Global Constraints

- Svelte 5 runes only: no Svelte 4 stores, no `export let`.
- `verbatimModuleSyntax` is on: type-only imports must use `import type` (or inline `type` specifiers).
- No new npm packages and no new crates. Fonts are vendored woff2 files under `ui/src/assets/fonts/`; icons are hand-inlined Lucide path data in `Icon.svelte`.
- Components never branch on mode. Mode lives in `data-mode` + token/flag overrides only (spec §4.2); JS-side feature booleans come from `appearance.svelte.ts`, never from mode checks scattered in components.
- Components consume **semantic tokens only** — never base or dynamic-seed values directly (spec §4.1).
- Compositor-only animation: only `transform` and `opacity` animate (spec §9 / PM-D1).
- Unicode glyphs are retired: no `▶ ⏸ ⏮ ⏭ 🔀 🔁 🔂 ♥ ♡ ⠿ ✕ ＋ ⊕ ↑ ↓ ‹ ›` literals in markup after Task 4.
- Semantic hues (`--danger`/`--success`/`--warning`) are fixed, never cover-derived (spec §5.2).
- Row heights are load-bearing (virtualization): Comfortable 38px / Compact 28px, driven by `appearance.density` everywhere a list windows rows (TrackTable, QueueRail, VirtualList consumers).
- Version stays 0.2.0; no packaging changes beyond `minWidth`/`minHeight` in `tauri.conf.json` (Task 8).
- Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **A custom typeface name containing `"` or `\`** must not break the CSS font stack or inject CSS. Task 6 tests `fontStack` sanitization.
- **A config.toml written before this plan (missing every new key)** must load with documented defaults. Task 1 tests the partial-file path.
- **An empty/missing cover UUID** (artist without picture, home item without art) must render the warm fallback tile, never a broken-image icon. Task 5 tests `coverUrl("")`; Task 12/13 smoke checks the fallback.
- **Queue width dragged past the 260–420px bounds** must clamp on both sides, live and on save. Task 6 tests `clampQueueWidth`; Task 1 tests the Rust-side clamp.
- **Switching density mid-scroll in a long list** must not leave blank space or jump rows: `windowRange` already clamps a shrunk window (plan 9 tests); Task 12 smoke scrolls a 2k playlist and toggles density.
- **Home feed items whose artwork field shape varies by type** (album `cover`, playlist `squareImage`, artist `picture`) must resolve in precedence order and degrade to `None`. Task 5's wiremock tests pin the order; the smoke checklist verifies real covers against the live API.

---

### Task 1: Appearance config keys + setter commands (Rust)

**Files:**
- Modify: `crates/oxitide-app/src/config.rs` (UiConfig + new enums, after line 72)
- Modify: `crates/oxitide-app/src/commands.rs` (new commands after `set_quality_display`, line 681)
- Modify: `crates/oxitide-app/src/main.rs` (handler registration, near line 154)
- Modify: `ui/src/lib/api.ts` (Config interface + api functions)

**Interfaces:**
- Consumes: existing `Config`/`UiConfig` serde-defaults pattern and the `set_quality_display` command shape.
- Produces: `UiMode`/`Typeface`/`Density`/`CoverShape` enums; `UiConfig.mode/typeface/custom_typeface/density/cover_shape/queue_width`; `clamp_queue_width(px: u32) -> u32`; commands `set_ui_mode`, `set_typeface`, `set_custom_typeface`, `set_density`, `set_cover_shape`, `set_queue_width`; TS `api.setUiMode/…` wrappers. Plan 11 adds `UiMode::Atmosphere` and the `dynamic_color` key.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `crates/oxitide-app/src/config.rs`:

```rust
    #[test]
    fn appearance_keys_round_trip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.ui.typeface = Typeface::Inter;
        config.ui.custom_typeface = "Commit Mono".into();
        config.ui.density = Density::Compact;
        config.ui.cover_shape = CoverShape::Sleeve;
        config.ui.queue_width = 400;
        config.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("typeface = \"inter\""), "{text}");
        assert!(text.contains("density = \"compact\""), "{text}");
        assert!(text.contains("cover_shape = \"sleeve\""), "{text}");
        assert!(text.contains("queue_width = 400"), "{text}");
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: a config written before Plan 10 lacks every appearance
    /// key; serde defaults must fill them.
    #[test]
    fn appearance_defaults_fill_a_pre_plan10_file() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[ui]\nquality_display = \"detailed\"\n").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.ui.quality_display, QualityDisplay::Detailed);
        assert_eq!(config.ui.mode, UiMode::Instrument);
        assert_eq!(config.ui.typeface, Typeface::Manrope);
        assert_eq!(config.ui.custom_typeface, "");
        assert_eq!(config.ui.density, Density::Comfortable);
        assert_eq!(config.ui.cover_shape, CoverShape::Disc);
        assert_eq!(config.ui.queue_width, 320);
        let _ = fs::remove_dir_all(dir);
    }

    /// Review Focus: queue width is clamped to the spec range on both sides.
    #[test]
    fn queue_width_clamps_to_260_420() {
        assert_eq!(clamp_queue_width(0), 260);
        assert_eq!(clamp_queue_width(259), 260);
        assert_eq!(clamp_queue_width(260), 260);
        assert_eq!(clamp_queue_width(320), 320);
        assert_eq!(clamp_queue_width(420), 420);
        assert_eq!(clamp_queue_width(9999), 420);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app config`
Expected: FAIL — `Typeface`, `Density`, `CoverShape`, `UiMode`, `clamp_queue_width` unresolved.

- [ ] **Step 3: Implement the config keys**

In `crates/oxitide-app/src/config.rs`, after the `QualityDisplay` enum (line 60):

```rust
/// Visual mode. Plan 10 ships Instrument only; Plan 11 adds `Atmosphere`
/// and flips the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiMode {
    #[default]
    Instrument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Typeface {
    #[default]
    Manrope,
    Inter,
    System,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    #[default]
    Comfortable,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoverShape {
    #[default]
    Disc,
    Sleeve,
}

/// Spec §7.1: the queue rail docks at 260–420px.
pub fn clamp_queue_width(px: u32) -> u32 {
    px.clamp(260, 420)
}
```

Add to `UiConfig` (after `quality_display`):

```rust
    pub mode: UiMode,
    pub typeface: Typeface,
    /// Font family name used when `typeface == Custom`.
    pub custom_typeface: String,
    pub density: Density,
    pub cover_shape: CoverShape,
    pub queue_width: u32,
```

and to `impl Default for UiConfig`:

```rust
            mode: UiMode::Instrument,
            typeface: Typeface::Manrope,
            custom_typeface: String::new(),
            density: Density::Comfortable,
            cover_shape: CoverShape::Disc,
            queue_width: 320,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app config`
Expected: PASS (10 existing + 3 new).

- [ ] **Step 5: Add the commands and register them**

In `crates/oxitide-app/src/commands.rs`, after `set_quality_display` (line 681); import the new types from `crate::config` alongside the existing `QualityDisplay` import:

```rust
#[tauri::command]
pub fn set_ui_mode(state: State<'_, AppState>, mode: UiMode) -> Result<(), String> {
    let mut config = state.config();
    config.ui.mode = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_typeface(state: State<'_, AppState>, typeface: Typeface) -> Result<(), String> {
    let mut config = state.config();
    config.ui.typeface = typeface;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_custom_typeface(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let mut config = state.config();
    config.ui.custom_typeface = name;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_density(state: State<'_, AppState>, density: Density) -> Result<(), String> {
    let mut config = state.config();
    config.ui.density = density;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_cover_shape(state: State<'_, AppState>, shape: CoverShape) -> Result<(), String> {
    let mut config = state.config();
    config.ui.cover_shape = shape;
    config.save()?;
    state.set_config(config);
    Ok(())
}

#[tauri::command]
pub fn set_queue_width(state: State<'_, AppState>, px: u32) -> Result<(), String> {
    let mut config = state.config();
    config.ui.queue_width = clamp_queue_width(px);
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

In `crates/oxitide-app/src/main.rs`, add `commands::set_ui_mode, commands::set_typeface, commands::set_custom_typeface, commands::set_density, commands::set_cover_shape, commands::set_queue_width,` to the `generate_handler![...]` list (next to `commands::set_quality_display`, line 154).

- [ ] **Step 6: Add the TS side**

In `ui/src/lib/api.ts`: extend the `Config` interface's `ui` member (find the existing `quality_display` line) with:

```ts
    mode: "instrument";
    typeface: "manrope" | "inter" | "system" | "custom";
    custom_typeface: string;
    density: "comfortable" | "compact";
    cover_shape: "disc" | "sleeve";
    queue_width: number;
```

and add to the `api` object (after `setQualityDisplay`, line 170):

```ts
  setUiMode: (mode: "instrument") => invoke<void>("set_ui_mode", { mode }),
  setTypeface: (typeface: "manrope" | "inter" | "system" | "custom") =>
    invoke<void>("set_typeface", { typeface }),
  setCustomTypeface: (name: string) => invoke<void>("set_custom_typeface", { name }),
  setDensity: (density: "comfortable" | "compact") => invoke<void>("set_density", { density }),
  setCoverShape: (shape: "disc" | "sleeve") => invoke<void>("set_cover_shape", { shape }),
  setQueueWidth: (px: number) => invoke<void>("set_queue_width", { px }),
```

- [ ] **Step 7: Verify + commit**

Run: `cargo test -p oxitide-app; cargo clippy -p oxitide-app -- -D warnings; npm --prefix ui run check`
Expected: all green.
`git add crates/oxitide-app ui/src/lib/api.ts; git commit -m "feat(app): add appearance config keys and setter commands"`

---

### Task 2: Design tokens, mode stylesheet, app.css rewrite

**Files:**
- Create: `ui/src/styles/tokens.css`
- Create: `ui/src/styles/modes.css`
- Modify: `ui/src/app.css` (full rewrite)
- Modify: `ui/src/main.ts` (line 3 import)

**Interfaces:**
- Consumes: spec §5.1 palette, §8 radii, §9 motion tokens.
- Produces: semantic tokens (`--bg`, `--text`, `--text-muted`, `--accent`, `--on-accent`, `--surface-1/2/3`, `--line`, `--edge-lit`, `--danger`, `--success`, `--warning`), `@property`-registered `--dyn-primary/--dyn-on-primary/--dyn-secondary/--dyn-tertiary`, flag tokens `--flag-*` (all `0`), type tokens `--font-display/--font-ui`, density tokens `--row-height/--thumb-size/--font-body-size`, layout tokens `--queue-width/--player-height/--sidebar-width`, utility classes `.panel`/`.lit`/`.tnum`; `[data-density="compact"]` and `[data-mode="instrument"]` overrides; legacy aliases `--panel/--panel-2/--muted/--accent` kept so untouched components keep compiling until their sweep task. Every later task consumes these.

- [ ] **Step 1: Create `ui/src/styles/tokens.css`**

```css
/* OxiTide design tokens — Track 2b foundation (spec §4–§5, §8–§9).
   Three layers: base (static) → dynamic seed (--dyn-*, Plan 11 drives
   them) → semantic (the only layer components read). */

@property --dyn-primary { syntax: "<color>"; inherits: true; initial-value: #2ee2c4; }
@property --dyn-on-primary { syntax: "<color>"; inherits: true; initial-value: #0b2420; }
@property --dyn-secondary { syntax: "<color>"; inherits: true; initial-value: #17120e; }
@property --dyn-tertiary { syntax: "<color>"; inherits: true; initial-value: #2ee2c4; }

:root {
  /* Base — Warm Graphite (spec §5.1) */
  --bg: #17120e;
  --text: #f2ece6;
  --text-muted: #a99f94;
  --accent-brand: #2ee2c4;
  --on-accent-brand: #0b2420;
  --danger: #ff8a7a;
  --success: #5fd68a;
  --warning: #e0a030;

  /* Semantic (components consume only these) */
  --accent: var(--dyn-primary);
  --on-accent: var(--dyn-on-primary);
  --surface-1: rgba(255, 240, 225, 0.05);
  --surface-2: rgba(255, 240, 225, 0.07);
  --surface-3: rgba(255, 240, 225, 0.09);
  --line: rgba(255, 240, 225, 0.10);
  --edge-lit: linear-gradient(135deg, rgba(255, 240, 225, 0.28), rgba(255, 240, 225, 0.06) 45%, rgba(0, 0, 0, 0.35));

  /* Radii (spec §8) */
  --radius-control: 8px;
  --radius-card: 12px;
  --radius-sheet: 16px;

  /* Motion (spec §9) */
  --dur-micro: 120ms;
  --dur-state: 180ms;
  --dur-view: 240ms;
  --dur-mood: 1200ms;
  --ease-in: cubic-bezier(0.2, 0.7, 0.3, 1);
  --ease-out: cubic-bezier(0.5, 0, 0.75, 0);

  /* Type (spec §6) */
  --font-display: "Fraunces", Georgia, serif;
  --font-ui: "Manrope", system-ui, sans-serif;

  /* Density (comfortable default; spec §7.4) */
  --row-height: 38px;
  --thumb-size: 30px;
  --font-body-size: 14px;

  /* Layout (spec §7.1) */
  --sidebar-width: 232px;
  --queue-width: 320px;
  --player-height: 72px;

  /* Feature flags (spec §4.3) — all off until Plan 11 lights them
     per mode. Components read the JS mirrors in appearance.svelte.ts;
     CSS reads these for pure-presentation gates. */
  --flag-backdrop-art: 0;
  --flag-glass-blur: 0;
  --flag-grain: 0;
  --flag-disc-spin: 0;
  --flag-wave-seek: 0;
  --flag-dynamic-color: 0;
  --flag-ambient-motion: 0;

  /* Glass recipe knobs (mode stylesheets override) */
  --glass-blur: 0px;

  /* Legacy aliases — deleted at the end of Task 14. */
  --panel: var(--surface-1);
  --panel-2: var(--surface-2);
  --muted: var(--text-muted);
}

[data-density="compact"] {
  --row-height: 28px;
  --thumb-size: 22px;
  --font-body-size: 13px;
}

/* prefers-reduced-motion is a hard floor (spec §12): ambient off,
   crossfades instant, slides become 120ms fades. */
@media (prefers-reduced-motion: reduce) {
  :root {
    --dur-micro: 0ms;
    --dur-state: 120ms;
    --dur-view: 120ms;
    --dur-mood: 0ms;
  }
}
```

- [ ] **Step 2: Create `ui/src/styles/modes.css`**

```css
/* Mode deltas. Components never read these names; the stylesheets only
   override semantic tokens and flags (spec §4.2, Instrument doc §2–§4). */

[data-mode="instrument"] {
  --surface-1: #211b15;
  --surface-2: #262019;
  --surface-3: #2b241c;
  --edge-lit: linear-gradient(135deg, var(--line), var(--line));
  --accent: var(--accent-brand);
  --on-accent: var(--on-accent-brand);
  --glass-blur: 0px;
  --flag-backdrop-art: 0;
  --flag-glass-blur: 0;
  --flag-grain: 0;
  --flag-disc-spin: 0;
  --flag-wave-seek: 0;
  --flag-dynamic-color: 0;
  --flag-ambient-motion: 0;
}
```

- [ ] **Step 3: Rewrite `ui/src/app.css`**

```css
:root { color-scheme: dark; }
* { box-sizing: border-box; }
html, body, #app { height: 100%; margin: 0; }
body {
  background: var(--bg);
  color: var(--text);
  font: var(--font-body-size)/1.4 var(--font-ui);
  overflow: hidden;
}

.app {
  display: grid;
  grid-template-columns: var(--sidebar-width) minmax(0, 1fr) auto;
  grid-template-rows: 1fr var(--player-height);
  height: 100%;
}
.app[data-sidebar="rail"] { --sidebar-width: 64px; }

.sidebar {
  background: var(--surface-1);
  border-right: 1px solid var(--line);
  padding: 12px;
  overflow-y: auto;
}
.main { padding: 16px 20px; overflow: auto; }
.queue {
  background: var(--surface-1);
  border-left: 1px solid var(--line);
  width: var(--queue-width);
  overflow: hidden;
}
.player {
  grid-column: 1 / -1;
  background: var(--surface-2);
  border-top: 1px solid var(--line);
}

/* Narrow window: the queue can no longer dock (spec §7.1) — it floats. */
.queue-backdrop {
  position: fixed;
  inset: 0 0 var(--player-height) 0;
  background: rgba(0, 0, 0, 0.35);
  z-index: 19;
}
.queue.floating {
  position: fixed;
  top: 0;
  right: 0;
  bottom: var(--player-height);
  z-index: 20;
  box-shadow: -12px 0 32px rgba(0, 0, 0, 0.45);
}

.muted { color: var(--text-muted); }
.tnum { font-variant-numeric: tabular-nums; }

/* Lit edge: panels read as objects under a top-left key light (spec §10.1). */
.lit {
  border: 1px solid transparent;
  background:
    linear-gradient(var(--surface-2), var(--surface-2)) padding-box,
    var(--edge-lit) border-box;
}

:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
:focus:not(:focus-visible) { outline: none; }
::selection { background: var(--accent); color: var(--on-accent); }

/* Component states (spec §8): hover brighten, press scale, disabled fade. */
button { transition: transform var(--dur-micro) var(--ease-in), opacity var(--dur-micro); }
button:not(:disabled):active { transform: scale(0.98); }
button:disabled { opacity: 0.4; cursor: default; }

/* Thin warm scrollbar; accent on hover (spec §8). */
::-webkit-scrollbar { width: 8px; height: 8px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: rgba(255, 240, 225, 0.14); border-radius: 4px; }
::-webkit-scrollbar-thumb:hover { background: var(--accent); }

button {
  font: inherit;
  color: inherit;
  background: none;
  border: none;
  cursor: pointer;
}
input, select { font: inherit; }
```

- [ ] **Step 4: Wire the stylesheets in `ui/src/main.ts`**

Replace line 3 (`import "./app.css";`) with:

```ts
import "./styles/tokens.css";
import "./styles/modes.css";
import "./styles/fonts.css";
import "./app.css";
```

(`fonts.css` arrives in Task 3; create an empty placeholder file now so the import resolves.)

- [ ] **Step 5: Verify + commit**

Run: `npm --prefix ui run check; npm --prefix ui run build`
Expected: green; the app renders in Warm Graphite with Instrument's solid surfaces.
`git add ui/src; git commit -m "feat(ui): add design tokens, instrument mode stylesheet, and shell css"`

---

### Task 3: Bundled fonts (Fraunces + Manrope + Inter)

**Files:**
- Create: `ui/src/assets/fonts/fraunces-latin-wght-normal.woff2`
- Create: `ui/src/assets/fonts/fraunces-latin-wght-italic.woff2`
- Create: `ui/src/assets/fonts/manrope-latin-wght-normal.woff2`
- Create: `ui/src/assets/fonts/inter-latin-wght-normal.woff2`
- Modify: `ui/src/styles/fonts.css` (placeholder from Task 2)

**Interfaces:**
- Consumes: `--font-display`/`--font-ui` token names from Task 2.
- Produces: font families `"Fraunces"` (variable wght, normal + italic), `"Manrope"`, `"Inter"` (variable wght), latin subsets, ≤250KB total (spec §15).

- [ ] **Step 1: Download the four woff2 files**

Run from the repo root (PowerShell):

```powershell
New-Item -ItemType Directory -Force ui/src/assets/fonts
$base = "https://cdn.jsdelivr.net/npm"
Invoke-WebRequest "$base/@fontsource-variable/fraunces/files/fraunces-latin-wght-normal.woff2" -OutFile ui/src/assets/fonts/fraunces-latin-wght-normal.woff2
Invoke-WebRequest "$base/@fontsource-variable/fraunces/files/fraunces-latin-wght-italic.woff2" -OutFile ui/src/assets/fonts/fraunces-latin-wght-italic.woff2
Invoke-WebRequest "$base/@fontsource-variable/manrope/files/manrope-latin-wght-normal.woff2" -OutFile ui/src/assets/fonts/manrope-latin-wght-normal.woff2
Invoke-WebRequest "$base/@fontsource-variable/inter/files/inter-latin-wght-normal.woff2" -OutFile ui/src/assets/fonts/inter-latin-wght-normal.woff2
Get-ChildItem ui/src/assets/fonts | Measure-Object Length -Sum
```

If a filename 404s, browse `https://www.jsdelivr.com/package/npm/@fontsource-variable/<family>` → `files/` and pick the `latin` variable woff2 for the needed style. Verify the total is ≤ 250KB; if Fraunces normal+italic together blow the budget, re-download from the `standard` subset listing or drop latin-ext remnants (the files above are latin-only) — do not ship over budget (spec §15).

- [ ] **Step 2: Write `ui/src/styles/fonts.css`**

```css
@font-face {
  font-family: "Fraunces";
  src: url("../assets/fonts/fraunces-latin-wght-normal.woff2") format("woff2");
  font-weight: 300 700;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Fraunces";
  src: url("../assets/fonts/fraunces-latin-wght-italic.woff2") format("woff2");
  font-weight: 300 700;
  font-style: italic;
  font-display: swap;
}
@font-face {
  font-family: "Manrope";
  src: url("../assets/fonts/manrope-latin-wght-normal.woff2") format("woff2");
  font-weight: 200 800;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Inter";
  src: url("../assets/fonts/inter-latin-wght-normal.woff2") format("woff2");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}
```

- [ ] **Step 3: Verify bundle size**

Run: `npm --prefix ui run build`
Then: `Get-ChildItem ui/dist/assets -Filter *.woff2 | Measure-Object Length -Sum`
Expected: ≤ 262144 bytes (250KB) of woff2 in the bundle; Vite content-hashes the files into `ui/dist/assets/`.

- [ ] **Step 4: Smoke-check rendering + commit**

Run `npm --prefix ui run app:dev`, open Settings once it exists (or temporarily add `font-family: var(--font-display)` to the sidebar `h1`): the wordmark renders in a serif (Fraunces), body text in Manrope. DevTools → Network shows the two woff2 files loading.
`git add ui/src/assets/fonts ui/src/styles/fonts.css; git commit -m "feat(ui): bundle fraunces, manrope, and inter variable fonts"`

---

### Task 4: Icon.svelte + unicode glyph sweep

**Files:**
- Create: `ui/src/lib/components/Icon.svelte`
- Modify: every file using glyphs — `ui/src/lib/components/PlayerBar.svelte` (⏮⏸▶⏭🔀🔁🔂♥♡), `TrackTable.svelte` (⠿▶＋⊕♥♡↑↓✕), `QueueRail.svelte` (⠿✕), `Sidebar.svelte` (‹›), `ToastHost.svelte` (✕), plus any `Modal.svelte`/`ContextMenu.svelte`/`views/*` matches the grep finds.

**Interfaces:**
- Produces: `<Icon name="play" size={18} />` where `name` is a key of the `ICONS` record; renders an inlined Lucide SVG (24 viewBox, `stroke="currentColor"`, `stroke-width="1.75"`, round caps) so icons follow the dynamic accent for free (spec §8).

- [ ] **Step 1: Find every glyph occurrence**

Run: `rg -n "▶|⏸|⏮|⏭|🔀|🔁|🔂|♥|♡|⠿|✕|＋|⊕|↑|↓|‹|›" ui/src`
Record the full file list; every one gets swept in Step 3.

- [ ] **Step 2: Create `ui/src/lib/components/Icon.svelte`**

```svelte
<script lang="ts">
  /// Inlined Lucide icons (ISC license, https://lucide.dev) — path data copied
  /// verbatim from the Lucide SVG sources; rendered with currentColor so they
  /// follow the accent (spec §8: 1.75px rounded stroke, no icon font).
  const ICONS = {
    // Transport
    play: '<polygon points="6 3 20 12 6 21 6 3"/>',
    pause: '<rect x="14" y="4" width="4" height="16" rx="1"/><rect x="6" y="4" width="4" height="16" rx="1"/>',
    // …copy the inner SVG for each icon below from https://lucide.dev/icons/<name>
  } as const;

  export type IconName = keyof typeof ICONS;
  let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>{@html ICONS[name]}</svg>
```

Fill `ICONS` from lucide.dev (open each icon, copy the inner elements verbatim). Required keys — transport: `play`, `pause`, `skip-back`, `skip-forward`, `shuffle`, `repeat`, `repeat-1`; actions: `heart`, `plus`, `list-plus`, `x`, `check`, `pencil`, `trash-2`, `copy`, `grip-vertical`, `chevron-up`, `chevron-down`, `chevron-left`, `chevron-right`, `arrow-left`, `arrow-right`, `rotate-cw`; navigation: `home`, `search`, `library`, `list-music`, `settings`, `log-out`, `disc-3`, `music`, `volume-2`, `maximize-2`, `minimize-2`, `panel-left-close`, `panel-left-open`; feedback: `alert-triangle`, `info`.

- [ ] **Step 3: Sweep the glyphs**

Replace every occurrence from Step 1 with `<Icon name="…" />` (hearts: `heart` with a `fill: currentColor` style hook when on — add a `.filled :global(svg) { fill: currentColor; }` pattern or a `fill` prop; play glyphs `play`, drag handles `grip-vertical`, remove `x`, reorder arrows `chevron-up`/`chevron-down`, back/forward `chevron-left`/`chevron-right` or `arrow-left`/`arrow-right`). Keep all `aria-label`/`title` attributes on the surrounding buttons — the SVG itself stays `aria-hidden`.

- [ ] **Step 4: Verify the sweep is complete**

Run: `rg -n "▶|⏸|⏮|⏭|🔀|🔁|🔂|♥|♡|⠿|✕|＋|⊕|↑|↓|‹|›" ui/src`
Expected: no matches. Then `npm --prefix ui run check; npm --prefix ui test`.

- [ ] **Step 5: Commit**

`git add ui/src; git commit -m "feat(ui): replace unicode glyphs with inlined lucide icons"`

---

### Task 5: Cover plumbing — artwork.ts, Cover.svelte, HomeItem artwork

**Files:**
- Create: `ui/src/lib/artwork.ts`
- Create: `ui/src/lib/artwork.test.ts`
- Create: `ui/src/lib/components/Cover.svelte`
- Modify: `crates/oxitide-tidal/src/home.rs` (RawItem/HomeItem + mapping, lines 7–16, 54–68, 96–108)
- Modify: `ui/src/lib/api.ts` (HomeItem interface)

**Interfaces:**
- Consumes: existing cover/picture/image UUID fields (`AlbumRef.cover`, `Artist.picture`, `Playlist.image`, `QueueItem.cover`).
- Produces: `coverUrl(uuid: string, size: CoverSize): string` (`CoverSize = 80 | 160 | 320 | 640 | 1280`); `<Cover uuid size radius />` rendering an `<img>` or a warm fallback tile; `HomeItem.artwork: Option<String>` (TS `artwork?: string | null`) with precedence `cover → squareImage → image → picture`. Tasks 12–14 render covers everywhere; Plan 11 Task 2 reuses `coverUrl(uuid, 160)` server-side semantics (same URL shape) for extraction.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/artwork.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { coverUrl } from "./artwork";

describe("coverUrl", () => {
  it("maps a dashed uuid onto the image CDN path", () => {
    expect(coverUrl("abc-def-123", 320)).toBe(
      "https://resources.tidal.com/images/abc/def/123/320x320.jpg",
    );
  });

  it("supports every sanctioned size", () => {
    for (const size of [80, 160, 320, 640, 1280] as const) {
      expect(coverUrl("u", size)).toContain(`/${size}x${size}.jpg`);
    }
  });

  /// Review Focus: an empty uuid must not produce a broken-image request.
  it("returns empty for an empty uuid", () => {
    expect(coverUrl("", 320)).toBe("");
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `npm --prefix ui test -- src/lib/artwork.test.ts`
Expected: FAIL — cannot resolve `./artwork`.

- [ ] **Step 3: Implement `ui/src/lib/artwork.ts`**

```ts
export type CoverSize = 80 | 160 | 320 | 640 | 1280;

/// TIDAL serves artwork from an unauthenticated CDN keyed by the dashed
/// uuid exploded into a path. Verified against the live API in the smoke
/// checklist (plan10-smoke.md).
export function coverUrl(uuid: string, size: CoverSize): string {
  if (!uuid) return "";
  return `https://resources.tidal.com/images/${uuid.replaceAll("-", "/")}/${size}x${size}.jpg`;
}
```

- [ ] **Step 4: Run to verify pass**

Run: `npm --prefix ui test -- src/lib/artwork.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Create `ui/src/lib/components/Cover.svelte`**

```svelte
<script lang="ts">
  import { coverUrl, type CoverSize } from "../artwork";
  import Icon from "./Icon.svelte";

  let {
    uuid,
    size = 320,
    radius = 6,
    alt = "",
  }: { uuid: string | null | undefined; size?: CoverSize; radius?: number; alt?: string } = $props();

  let failed = $state(false);
  $effect(() => {
    uuid;
    failed = false;
  });
  const src = $derived(uuid && !failed ? coverUrl(uuid, size) : "");
</script>

{#if src}
  <img
    class="cover"
    style:border-radius="{radius}px"
    {src}
    {alt}
    loading="lazy"
    draggable="false"
    onerror={() => (failed = true)}
  />
{:else}
  <div class="cover fallback" style:border-radius="{radius}px" role="img" aria-label={alt || "No artwork"}>
    <Icon name="music" size={20} />
  </div>
{/if}

<style>
  .cover {
    display: block;
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    background: var(--surface-2);
  }
  .fallback {
    display: grid;
    place-items: center;
    color: var(--text-muted);
    background: linear-gradient(135deg, var(--surface-2), var(--surface-3));
  }
</style>
```

- [ ] **Step 6: Extend `HomeItem` with artwork**

In `crates/oxitide-tidal/src/home.rs`: add to `HomeItem` (line 13 area) `#[serde(default)] pub artwork: Option<String>,`; add to `RawItem` (line 60 area):

```rust
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default, rename = "squareImage")]
    pub square_image: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
```

and in the `HomeItem` mapping (line 98): `artwork: i.cover.or(i.square_image).or(i.image).or(i.picture),`.

Add a wiremock test next to the existing ones:

```rust
    #[tokio::test]
    async fn home_item_artwork_uses_first_available_field() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/pages/home"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "rows": [{"modules": [{"type": "MIXED", "title": "Mix", "pagedList": {"items": [
                    {"id": 1, "title": "Album", "cover": "c-1"},
                    {"uuid": "p-2", "title": "Playlist", "squareImage": "sq-2", "image": "im-2"},
                    {"id": 3, "title": "Artist", "picture": "pic-3"},
                    {"id": 4, "title": "Nothing"}
                ]}}]}]
            })))
            .mount(&server).await;
        let sections = client(&server).home_page().await.unwrap();
        let items = &sections[0].items;
        assert_eq!(items[0].artwork.as_deref(), Some("c-1"));
        assert_eq!(items[1].artwork.as_deref(), Some("sq-2"));
        assert_eq!(items[2].artwork.as_deref(), Some("pic-3"));
        assert_eq!(items[3].artwork, None);
    }
```

- [ ] **Step 7: Update the TS DTO + run gates**

In `ui/src/lib/api.ts`, `HomeItem` gains `artwork?: string | null;`.
Run: `cargo test -p oxitide-tidal; npm --prefix ui test; npm --prefix ui run check`
Expected: all green.

- [ ] **Step 8: Commit**

`git add crates/oxitide-tidal/src/home.rs ui/src/lib/artwork.ts ui/src/lib/artwork.test.ts ui/src/lib/components/Cover.svelte ui/src/lib/api.ts; git commit -m "feat(ui,tidal): add cover url builder, cover component, and home artwork"`

---

### Task 6: appearance.svelte.ts — settings state + DOM application

**Files:**
- Create: `ui/src/lib/appearance.svelte.ts`
- Create: `ui/src/lib/appearance.test.ts`
- Modify: `ui/src/App.svelte` (init call)

**Interfaces:**
- Consumes: Task 1's `api` setters and `Config`.
- Produces: the singleton `appearance` with reactive `mode/typeface/customTypeface/density/coverShape/queueWidth/reducedMotion`, `init()`, and setters `setMode/setTypeface/setCustomTypeface/setDensity/setCoverShape/setQueueWidth` (each applies locally, then persists; persistence failures propagate to the caller). Pure exports `clampQueueWidth(px)` and `fontStack(typeface, custom)`. `apply()` sets `data-mode`, `data-density`, `--font-ui`, `--queue-width` on `document.documentElement` and mirrors `prefers-reduced-motion`. Tasks 7–14 consume this; Plan 11 extends it with `dynamicColor` and feature booleans.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/appearance.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { clampQueueWidth, fontStack } from "./appearance.svelte";

describe("clampQueueWidth", () => {
  it("clamps to the 260–420 dock range", () => {
    expect(clampQueueWidth(100)).toBe(260);
    expect(clampQueueWidth(320)).toBe(320);
    expect(clampQueueWidth(999)).toBe(420);
    expect(clampQueueWidth(319.6)).toBe(320);
  });
});

describe("fontStack", () => {
  it("bundled and system picks", () => {
    expect(fontStack("manrope", "")).toBe('"Manrope", system-ui, sans-serif');
    expect(fontStack("inter", "")).toBe('"Inter", system-ui, sans-serif');
    expect(fontStack("system", "")).toBe("system-ui, sans-serif");
  });

  it("custom names get quoted and fall back to system-ui", () => {
    expect(fontStack("custom", "Commit Mono")).toBe('"Commit Mono", system-ui, sans-serif');
    expect(fontStack("custom", "  ")).toBe("system-ui, sans-serif");
  });

  /// Review Focus: quotes and backslashes in a custom name must not break
  /// out of the CSS value.
  it("strips css-breaking characters from custom names", () => {
    expect(fontStack("custom", 'Evil"; } * { display:none')).toBe(
      '"Evil; } * { display:none", system-ui, sans-serif',
    );
    expect(fontStack("custom", "a\\b")).toBe('"ab", system-ui, sans-serif');
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `npm --prefix ui test -- src/lib/appearance.test.ts`
Expected: FAIL — cannot resolve `./appearance.svelte`.

- [ ] **Step 3: Implement `ui/src/lib/appearance.svelte.ts`**

```ts
import { api } from "./api";

export type UiMode = "instrument";
export type Typeface = "manrope" | "inter" | "system" | "custom";
export type Density = "comfortable" | "compact";
export type CoverShape = "disc" | "sleeve";

export const QUEUE_WIDTH_MIN = 260;
export const QUEUE_WIDTH_MAX = 420;

export function clampQueueWidth(px: number): number {
  return Math.min(QUEUE_WIDTH_MAX, Math.max(QUEUE_WIDTH_MIN, Math.round(px)));
}

/// The fallback chain always ends in system-ui (spec §6).
export function fontStack(typeface: Typeface, custom: string): string {
  switch (typeface) {
    case "manrope":
      return '"Manrope", system-ui, sans-serif';
    case "inter":
      return '"Inter", system-ui, sans-serif';
    case "system":
      return "system-ui, sans-serif";
    case "custom": {
      const name = custom.trim().replace(/["\\]/g, "");
      return name ? `"${name}", system-ui, sans-serif` : "system-ui, sans-serif";
    }
  }
}

export interface Appearance {
  readonly mode: UiMode;
  readonly typeface: Typeface;
  readonly customTypeface: string;
  readonly density: Density;
  readonly coverShape: CoverShape;
  readonly queueWidth: number;
  readonly reducedMotion: boolean;
  init(): Promise<void>;
  setMode(mode: UiMode): Promise<void>;
  setTypeface(typeface: Typeface): Promise<void>;
  setCustomTypeface(name: string): Promise<void>;
  setDensity(density: Density): Promise<void>;
  setCoverShape(shape: CoverShape): Promise<void>;
  setQueueWidth(px: number): Promise<void>;
}

export function createAppearance(): Appearance {
  let mode = $state<UiMode>("instrument");
  let typeface = $state<Typeface>("manrope");
  let customTypeface = $state("");
  let density = $state<Density>("comfortable");
  let coverShape = $state<CoverShape>("disc");
  let queueWidth = $state(320);
  let reducedMotion = $state(false);

  function apply() {
    const root = document.documentElement;
    root.dataset.mode = mode;
    root.dataset.density = density;
    root.style.setProperty("--font-ui", fontStack(typeface, customTypeface));
    root.style.setProperty("--queue-width", `${queueWidth}px`);
  }

  async function init() {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion = media.matches;
    media.addEventListener("change", (e) => (reducedMotion = e.matches));
    try {
      const config = await api.getConfig();
      mode = config.ui.mode;
      typeface = config.ui.typeface;
      customTypeface = config.ui.custom_typeface;
      density = config.ui.density;
      coverShape = config.ui.cover_shape;
      queueWidth = config.ui.queue_width;
    } catch {
      // Defaults are fine; a config failure must not block boot.
    }
    apply();
  }

  /// Local-first apply, then persist; callers surface persistence errors.
  async function persist(run: () => Promise<void>) {
    apply();
    await run();
  }

  return {
    get mode() { return mode; },
    get typeface() { return typeface; },
    get customTypeface() { return customTypeface; },
    get density() { return density; },
    get coverShape() { return coverShape; },
    get queueWidth() { return queueWidth; },
    get reducedMotion() { return reducedMotion; },
    init,
    setMode: (m) => { mode = m; return persist(() => api.setUiMode(m)); },
    setTypeface: (t) => { typeface = t; return persist(() => api.setTypeface(t)); },
    setCustomTypeface: (n) => { customTypeface = n; return persist(() => api.setCustomTypeface(n)); },
    setDensity: (d) => { density = d; return persist(() => api.setDensity(d)); },
    setCoverShape: (s) => { coverShape = s; return persist(() => api.setCoverShape(s)); },
    setQueueWidth: (px) => { queueWidth = clampQueueWidth(px); return persist(() => api.setQueueWidth(queueWidth)); },
  };
}

export const appearance = createAppearance();
```

- [ ] **Step 4: Run to verify pass**

Run: `npm --prefix ui test -- src/lib/appearance.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Init in `App.svelte`**

In the script, import `appearance` from `"./lib/appearance.svelte"` and add an effect near the existing boot effect (line 91):

```ts
  $effect(() => {
    void appearance.init();
  });
```

(Themed login/boot screens come free since `apply()` runs before login.)

- [ ] **Step 6: Verify + commit**

Run: `npm --prefix ui test; npm --prefix ui run check`
Expected: green.
`git add ui/src/lib/appearance.svelte.ts ui/src/lib/appearance.test.ts ui/src/App.svelte; git commit -m "feat(ui): add appearance state with dom application and persistence"`

---

### Task 7: Settings → Appearance surface

**Files:**
- Modify: `ui/src/views/Settings.svelte` (new section; keep all existing sections)

**Interfaces:**
- Consumes: `appearance` singleton (Task 6), `Icon` (Task 4).
- Produces: the Settings → Appearance section with Mode, Typeface (+Custom with preview and validation), Density, Cover shape — all live-apply, user picks persisted (spec §12; Dynamic color arrives in Plan 11 Task 8).

- [ ] **Step 1: Add the Appearance section**

In `Settings.svelte`, above the existing audio section, add an `Appearance` section matching the spec table:

- **Mode** — segmented control with one enabled option, `Instrument`, plus a muted note: "Atmosphere arrives with the next slice." (Plan 11 Task 8 replaces this with the two-way switch.) Read/write `appearance.mode` via `appearance.setMode`.
- **Typeface** — `<select>` bound to `appearance.typeface` with Manrope (default) / Inter / System / Custom…; on change `await appearance.setTypeface(v)` (wrap in try/catch → `toasts.show(..., { severity: "error" })`; Settings already receives `toasts`). When `custom` is selected, show a text input bound to a local `customName` (initialized from `appearance.customTypeface`), a preview line `The quick brown fox — 0123456789` rendered with `style:font-family={fontStack("custom", customName)}`, and on Apply: `await appearance.setCustomTypeface(customName)` then `appearance.setTypeface("custom")`. After applying, validate with the Font Loading API: `document.fonts.check('12px "' + customName.replace(/["\\]/g, "") + '"')` — when false, show an inline muted note "not found — using fallback" (spec §6). Where `queryLocalFonts` exists (`"queryLocalFonts" in window`), offer it as a "Browse installed fonts…" button whose result populates the input; otherwise the input alone is fine.
- **Density** — segmented Comfortable / Compact → `appearance.setDensity`.
- **Cover shape** — segmented Disc / Sleeve → `appearance.setCoverShape`.

Style rows like the existing settings rows; section heading uses `font-family: var(--font-display)` (Fraunces) as the spec's dialog/heading voice.

- [ ] **Step 2: Verify + smoke**

Run: `npm --prefix ui run check`
Manual (record in plan10-smoke.md): each control live-applies without restart — typeface swaps body text immediately (Fraunces headings unchanged), density re-heights lists, custom font validates a bogus name ("No Such Font" → fallback note), and all picks survive an app restart (check `%APPDATA%\OxiTide\config.toml`).

- [ ] **Step 3: Commit**

`git add ui/src/views/Settings.svelte; git commit -m "feat(ui): add the settings appearance surface"`

---

### Task 8: Shell layout — grid, sidebar rail, min window

**Files:**
- Modify: `crates/oxitide-app/tauri.conf.json` (window min size, line 14)
- Modify: `ui/src/App.svelte` (`.app` data attribute + rail state)
- Modify: `ui/src/lib/components/Sidebar.svelte` (rebuild)

**Interfaces:**
- Consumes: Task 2 grid CSS (`.app[data-sidebar="rail"]`), `Icon` (Task 4).
- Produces: 232px sidebar with manual collapse ↔ 64px icon rail and auto-rail below 1080px (spec §7.1); minimum window 720×520 (spec §7.1). `App.svelte` exposes `sidebarRail` only to `Sidebar` (prop) — no module exports.

- [ ] **Step 1: Set the window minimum**

In `crates/oxitide-app/tauri.conf.json` line 14, add `"minWidth": 720, "minHeight": 520` to the window object.

- [ ] **Step 2: Rail state in `App.svelte`**

```ts
  let sidebarRail = $state(false);
  let railTouched = $state(false);

  $effect(() => {
    const media = window.matchMedia("(max-width: 1080px)");
    const sync = () => {
      if (!railTouched) sidebarRail = media.matches;
    };
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  });
```

`<div class="app" data-sidebar={sidebarRail ? "rail" : "full"}>` and `<Sidebar {nav} rail={sidebarRail} onToggleRail={() => { railTouched = true; sidebarRail = !sidebarRail; }} onLogout={onLogout} />`.

- [ ] **Step 3: Rebuild `Sidebar.svelte`**

Structure: Fraunces wordmark "OxiTide" (`font-family: var(--font-display); font-size: 20px; font-weight: 640`; hidden in rail), back/forward icon buttons (`chevron-left`/`chevron-right`, disabled states preserved, existing titles kept), nav items as icon+label rows — Home `home`, Search `search`, Library `library`, Queue `list-music`, Settings `settings`, Log out `log-out` — each a full-width button, `aria-label` always, label text hidden in rail (icon-only, `title` attr remains), active route highlighted with `color: var(--accent)` + a surface tint background via `color-mix(in srgb, var(--accent) 12%, transparent)`. Collapse toggle at the bottom (`panel-left-close` / `panel-left-open`).

- [ ] **Step 4: Verify + smoke**

Run: `npm --prefix ui run check; cargo tauri build --debug` is unnecessary — `npm --prefix ui run app:dev`, then smoke: window resizes no smaller than 720×520; below 1080px the sidebar collapses to the rail automatically; the toggle overrides until the next breakpoint crossing.

- [ ] **Step 5: Commit**

`git add crates/oxitide-app/tauri.conf.json ui/src/App.svelte ui/src/lib/components/Sidebar.svelte; git commit -m "feat(ui): add collapsible sidebar rail and window minimum"`

---

### Task 9: Queue rail resize + narrow-window overlay

**Files:**
- Modify: `ui/src/App.svelte` (queue markup + drag handle + overlay)

**Interfaces:**
- Consumes: `appearance.queueWidth/setQueueWidth`, `clampQueueWidth` (Task 6); `.queue`/`.queue-backdrop`/`.queue.floating` CSS (Task 2); `nav.queueOpen/toggleQueue` (existing).
- Produces: drag-resizable queue dock 260–420px persisted via `set_queue_width`; below 760px window width the queue renders as `.queue.floating` over a `.queue-backdrop` (spec §7.1).

- [ ] **Step 1: Implement the resize handle and overlay**

In `App.svelte`:

```ts
  let queueOverlay = $state(false);

  $effect(() => {
    const media = window.matchMedia("(max-width: 760px)");
    const sync = () => (queueOverlay = media.matches);
    sync();
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  });

  function startQueueDrag(event: PointerEvent) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = appearance.queueWidth;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    let width = startWidth;
    const onMove = (e: PointerEvent) => {
      // The handle sits on the rail's left edge: dragging left widens.
      // Preview live without touching the config (writes happen once, on release).
      width = clampQueueWidth(startWidth + (startX - e.clientX));
      document.documentElement.style.setProperty("--queue-width", `${width}px`);
    };
    const onUp = () => {
      target.removeEventListener("pointermove", onMove);
      target.removeEventListener("pointerup", onUp);
      appearance.setQueueWidth(width).catch(() => {});
    };
    target.addEventListener("pointermove", onMove);
    target.addEventListener("pointerup", onUp);
  }
```

Import `appearance` and `clampQueueWidth` from `"./lib/appearance.svelte"`. Replace `{#if nav.queueOpen}<aside class="queue"><QueueRail {player} /></aside>{/if}` with:

```svelte
    {#if nav.queueOpen}
      {#if queueOverlay}
        <div class="queue-backdrop" onclick={() => nav.toggleQueue()} aria-hidden="true"></div>
        <aside class="queue floating"><QueueRail {player} /></aside>
      {:else}
        <aside class="queue">
          <div class="queue-resize" onpointerdown={startQueueDrag} role="separator" aria-orientation="vertical" aria-label="Resize queue"></div>
          <QueueRail {player} />
        </aside>
      {/if}
    {/if}
```

Live width: `appearance.setQueueWidth` already applies `--queue-width` through `apply()`; persist once per drag is acceptable (each move persists — config writes are atomic and cheap; if profiling shows churn, switch to persisting only in `onUp` via a direct `api.setQueueWidth` call and keep `setQueueWidth` moves non-persistent — decide during implementation and note the choice).

Add to `app.css`:

```css
.queue { position: relative; }
.queue-resize {
  position: absolute;
  left: -3px;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: col-resize;
  z-index: 5;
}
.queue-resize:hover { background: color-mix(in srgb, var(--accent) 35%, transparent); }
```

- [ ] **Step 2: Verify + smoke**

Run: `npm --prefix ui run check`
Smoke: drag the handle — width follows between 260 and 420, survives restart; shrink the window below 760px — the queue floats and closes via backdrop click or Esc (existing shortcut chain).

- [ ] **Step 3: Commit**

`git add ui/src/App.svelte ui/src/app.css; git commit -m "feat(ui): make the queue rail resizable with a narrow overlay"`

---

### Task 10: Expanded now-playing (static sleeve)

**Files:**
- Modify: `ui/src/lib/nav.svelte.ts` + `ui/src/lib/nav.test.ts` (nowPlayingOpen)
- Create: `ui/src/views/NowPlaying.svelte`
- Modify: `ui/src/App.svelte` (route swap + Esc chain)

**Interfaces:**
- Consumes: `Cover` (Task 5), `Icon` (Task 4), `formatQuality` with `"detailed"` mode + `player.state.stream`/`bit_perfect`, `appearance.coverShape` (Disc renders as sleeve until Plan 11 Task 6), queue state.
- Produces: `nav.nowPlayingOpen`/`toggleNowPlaying()`/`closeNowPlaying()`; `NowPlaying` view replacing the main pane (sidebar + player bar persist, spec §7.2). PlayerBar Task 11 opens it via the 48px thumb.

- [ ] **Step 1: Extend nav state with tests**

In `ui/src/lib/nav.test.ts`, mirror the existing `queueOpen` tests:

```ts
  it("toggles now playing independently of the route", () => {
    const nav = createNav();
    expect(nav.nowPlayingOpen).toBe(false);
    nav.toggleNowPlaying();
    expect(nav.nowPlayingOpen).toBe(true);
    nav.goto({ name: "settings" });
    expect(nav.nowPlayingOpen).toBe(true);
    nav.closeNowPlaying();
    expect(nav.nowPlayingOpen).toBe(false);
  });
```

- [ ] **Step 2: Run to verify failure, then implement**

Run: `npm --prefix ui test -- src/lib/nav.test.ts` → FAIL (`toggleNowPlaying` missing).
In `nav.svelte.ts`: add `let nowPlayingOpen = $state(false);`, expose `get nowPlayingOpen()`, `toggleNowPlaying()` and `closeNowPlaying()` following the `queueOpen` pattern, and extend the `Nav` interface.
Re-run: PASS.

- [ ] **Step 3: Create `ui/src/views/NowPlaying.svelte`**

Layout (spec §7.2 "Editorial Split") — a two-column grid filling `.main`:

- **Left:** `<Cover uuid={track.cover} size={640} radius={12} />` at up to ~55% of the pane width, wrapped in `.lit`. Clicking it calls `nav.closeNowPlaying()` (spec: clicking artwork returns). `appearance.coverShape` is ignored in Plan 10 — the Disc shape arrives with the spin flag in Plan 11 Task 6; render the sleeve unconditionally.
- **Right metadata column:**
  - Eyebrow: `NOW PLAYING` (10px/700, +0.09em, uppercase, `var(--text-muted)`).
  - Title in Fraunces: `font-family: var(--font-display); font-size: 28px; font-weight: 640; letter-spacing: -0.02em`.
  - Byline: `{artist} — {album}` in Fraunces italic 520 (`font-style: italic; font-weight: 520`), muted.
  - Badge row: quality chip `formatQuality(player.state.stream, "compact", player.state.preferred_quality)`, sample-rate/detail chip `formatQuality(player.state.stream, "detailed", player.state.preferred_quality)` (only when a stream exists), and a `Bit-perfect` chip when `player.state.bit_perfect`. Chips: 10px/700 uppercase, bordered (`1px solid var(--line)`, radius 6px, padding 2px 8px), muted text.
  - Divider (`1px solid var(--line)`).
  - **Next Up**: up to 3 rows from `player.state.queue` after the current index (find current via `queue.findIndex(i => i.track_id === player.state.current?.track_id)`), each row: 30px `<Cover size={80}>`, title, muted artist, `.tnum` duration via `formatDuration`. Empty state: muted "Queue is empty".
  - A reserved lyrics slot: an empty container `div.lyrics-slot` with a code comment `// Track 4: synced lyrics land here (spec §7.2)`.

Props: `{ player, nav, qualityDisplay }` as needed. Empty (`player.state.current === null`): muted "Nothing playing" with the same layout skeleton.

- [ ] **Step 4: Wire into `App.svelte`**

Inside `<main class="main">`, wrap the route switch:

```svelte
      {#if nav.nowPlayingOpen}
        <NowPlaying {player} {nav} {qualityDisplay} />
      {:else}
        …existing route switch…
      {/if}
```

Esc handling — in `runShortcut`, extend the `"escape"` case: `if (menu.open) menu.close(); else if (nav.nowPlayingOpen) nav.closeNowPlaying(); else if (nav.queueOpen) nav.toggleQueue();` and update the guard in `onKeydown` (line 195) to also pass when `nav.nowPlayingOpen`.

- [ ] **Step 5: Verify + smoke**

Run: `npm --prefix ui test; npm --prefix ui run check`
Smoke: open via a temporary button (Task 11 wires the real one), Esc closes, history back/forward untouched while open, Next Up shows the following 3 queue rows.

- [ ] **Step 6: Commit**

`git add ui/src/lib/nav.svelte.ts ui/src/lib/nav.test.ts ui/src/views/NowPlaying.svelte ui/src/App.svelte; git commit -m "feat(ui): add expanded now playing view"`

---

### Task 11: PlayerBar rebuild

**Files:**
- Modify: `ui/src/lib/components/PlayerBar.svelte` (full rebuild)

**Interfaces:**
- Consumes: `Cover` (Task 5), `Icon` (Task 4), `nav.toggleNowPlaying` (Task 10), `appearance` for nothing in P10 (flat seek; Plan 11 Task 7 swaps in the wave).
- Produces: the 72px glass player bar — hairline seek across its top edge, 48px cover thumb, transport cluster, badges + volume right (spec §7.1). Behavior contract (seek/volume/transport) unchanged from the current implementation.

- [ ] **Step 1: Rebuild the markup**

Keep the existing script logic (`dragging`/`dragValue`/`shown`/`position`/`duration`/`quality`, `nextRepeat`) and replace the custom hairline seek for the `<input type="range">`:

```svelte
<div class="player">
  {#if player.state.current}
    {@const current = player.state.current}
    <div
      class="seek"
      role="slider"
      aria-label="Playback position"
      aria-valuemin={0}
      aria-valuemax={Math.max(duration, 1)}
      aria-valuenow={Math.round(position)}
      onpointerdown={seekPointer}
    >
      <div class="seek-fill" style:width="{(position / Math.max(duration, 1)) * 100}%"></div>
      <div class="seek-thumb" style:left="{(position / Math.max(duration, 1)) * 100}%"></div>
    </div>
    <button class="thumb" onclick={() => nav.toggleNowPlaying()} aria-label="Open now playing">
      <Cover uuid={current.cover} size={80} radius={6} alt="{current.title} artwork" />
    </button>
    <div class="now">
      <strong class="title">{current.title}</strong>
      <span class="muted artist">{current.artist}</span>
    </div>
    <button class="icon-heart" class:on={favorites.hasTrack(current.track_id)} …existing favorite wiring…><Icon name="heart" /></button>
    <div class="transport">…prev / play-pause / next / shuffle / repeat as Icon buttons, existing handlers…</div>
    <span class="muted tnum">{formatDuration(position / 1000)} / {formatDuration(duration / 1000)}</span>
    <div class="badges">…quality badge + bit-perfect badge as bordered chips (Task 10 chip styling)…</div>
    <input type="range" class="volume" …existing volume wiring… />
    …buffering / error (var(--danger)) / bit_perfect_notice (var(--warning))…
  {:else}
    <span class="muted">Nothing playing</span>
  {/if}
</div>
```

Pass `nav` in from `App.svelte` (`<PlayerBar {player} {qualityDisplay} {favorites} {nav} />`). `seekPointer` implements click/drag seek: `const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();` ratio `clamp((e.clientX - rect.left) / rect.width, 0, 1)` → on pointerdown set `dragging` + `dragValue`, capture the pointer, on move update `dragValue`, on up `player.seek(ratio * duration)` and clear — mirroring the current `oninput`/`onchange` semantics.

Play/pause button: accent circle — `background: var(--accent); color: var(--on-accent); border-radius: 50%; width: 40px; height: 40px` (spec §8 primary button). Seek CSS: `.seek { position: absolute; top: -1px; left: 0; right: 0; height: 12px; cursor: pointer; }` with `.seek-fill { height: 2px; background: var(--accent); transition: height var(--dur-micro); }` growing to 4px + `.seek-thumb` (10px accent circle, opacity 0 → 1) on `.seek:hover`/drag — transform/opacity only.

- [ ] **Step 2: Verify + smoke**

Run: `npm --prefix ui run check; npm --prefix ui test`
Smoke: seek by click and by drag (position follows, releases to the engine), thumb opens now-playing, volume persists on release (existing `setVolume(v, true)` path), all glyphs are icons.

- [ ] **Step 3: Commit**

`git add ui/src/lib/components/PlayerBar.svelte ui/src/App.svelte; git commit -m "feat(ui): rebuild the player bar with hairline seek and cover thumb"`

---

### Task 12: TrackTable covers + density-reactive rows (and QueueRail)

**Files:**
- Modify: `ui/src/lib/components/TrackTable.svelte`
- Modify: `ui/src/lib/components/QueueRail.svelte`
- Modify: `ui/src/views/Album.svelte` (pass `showCovers={false}`)

**Interfaces:**
- Consumes: `Cover` (Task 5), `appearance.density` (Task 6), `windowRange(total, scrollTop, viewport, rowHeight, overscan?)` (existing, plan 9).
- Produces: `TrackTable` prop `showCovers?: boolean` (default `true`); rows at `var(--row-height)` with a `var(--thumb-size)` cover cell (spec §7.3); QueueRail rows follow the same density heights. `rowHeight` values consumed: Comfortable 38 / Compact 28 everywhere.

- [ ] **Step 1: Density-reactive heights**

In `TrackTable.svelte`: replace the `ROW_HEIGHT = 36` constant with

```ts
  import { appearance } from "../appearance.svelte";
  const rowHeight = $derived(appearance.density === "compact" ? 28 : 38);
```

and feed `rowHeight` to every `windowRange(...)` call, spacer style, and row style that used the constant. Same in `QueueRail.svelte` for `QUEUE_ROW_HEIGHT` (40 → 38/28): `const queueRowHeight = $derived(appearance.density === "compact" ? 28 : 38);` — pass it to `VirtualList` and to `queueDrag.rowHeight: () => queueRowHeight`.

- [ ] **Step 2: Cover cell in `TrackTable`**

Add `let { showCovers = true, ... } = $props()` destructure addition. Between the action cell and the `#` cell, when `showCovers`:

```svelte
          <td class="thumb-cell">
            <span class="thumb">
              <Cover uuid={t.album?.cover} size={80} radius={6} alt="" />
              <span class="thumb-play" aria-hidden="true"><Icon name="play" size={14} /></span>
            </span>
          </td>
```

CSS: `.thumb { display: block; width: var(--thumb-size); position: relative; }` `.thumb-play { position: absolute; inset: 0; display: grid; place-items: center; background: rgba(0,0,0,.45); border-radius: 6px; opacity: 0; transition: opacity var(--dur-micro); color: #fff; }` `tr:hover .thumb-play { opacity: 1; }`. Playing row: `tr.playing .thumb :global(img) { box-shadow: 0 0 0 1.5px var(--accent); }`. Playing/selected row background + accent title: `tr.playing { background: color-mix(in srgb, var(--accent) 12%, transparent); } tr.playing .title-text { color: var(--accent); }` — no left bars, no equalizers (spec §7.3 veto).

Row height styles switch from hardcoded 36px to `var(--row-height)`; keep the spacer math in px via `rowHeight`.

- [ ] **Step 3: Album view opts out**

In `ui/src/views/Album.svelte`, pass `showCovers={false}` to `TrackTable` (spec §7.3: redundant in single-album view).

- [ ] **Step 4: Queue row thumbs**

In `QueueRail.svelte` rows, add `<Cover uuid={item.cover} size={80} radius={6} alt="" />` at `var(--thumb-size)` before the title span; current-row highlight switches from `background:var(--panel)` to `background: color-mix(in srgb, var(--accent) 12%, transparent)`.

- [ ] **Step 5: Verify + smoke**

Run: `npm --prefix ui test; npm --prefix ui run check`
Smoke: thumbnails render in playlist/search/library/queue but not the album view; hover shows the play glyph; the playing row is tinted with an accent title and ringed thumb; **Review Focus check** — open a 2k playlist, scroll to the middle, toggle density in Settings: rows re-height with no blank gap and the same tracks stay in view.

- [ ] **Step 6: Commit**

`git add ui/src/lib/components/TrackTable.svelte ui/src/lib/components/QueueRail.svelte ui/src/views/Album.svelte; git commit -m "feat(ui): add cover thumbnails and density-reactive rows to track lists"`

---

### Task 13: Cards and view headers (Home, Search, Library, Album, Playlist, Artist)

**Files:**
- Modify: `ui/src/views/Home.svelte` (rebuild cards)
- Modify: `ui/src/views/Search.svelte` (result rows get covers)
- Modify: `ui/src/views/Library.svelte` (rows get thumbs)
- Modify: `ui/src/views/Album.svelte` / `Playlist.svelte` / `Artist.svelte` (headers)

**Interfaces:**
- Consumes: `Cover` (Task 5), `HomeItem.artwork` (Task 5), `routeForHomeItem` (existing).
- Produces: the card treatment (12px radius, lit edge, hover brighten + cover zoom 1.03, spec §8) and Fraunces view titles everywhere. All existing click/route behavior unchanged.

- [ ] **Step 1: Home cards**

Replace the inline-styled text cards with a `.card-grid` (`display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 16px;`) of `.card` buttons for routeable items (`routeForHomeItem(item) !== null`) and inert `.card` divs otherwise: `<Cover uuid={item.artwork} size={320} radius={12} alt="" />` inside a `.card-art` wrapper (`overflow: hidden; border-radius: var(--radius-card)`), title (14px/500), subtitle (11.5px muted). Hover: `.card:hover .card-art :global(img) { transform: scale(1.03); }` with `transition: transform var(--dur-state) var(--ease-in)` and a surface brighten on the card. Card chrome: `.lit`.

- [ ] **Step 2: Search results**

Album rows: `<Cover uuid={album.cover} size={80}>` thumbs; artist rows: `artist.picture`; playlist rows: `playlist.image` — 30px (`var(--thumb-size)`) thumbs in the existing `<ul>` rows, titles keep their routes.

- [ ] **Step 3: Library rows**

Playlist rows get `playlist.image` thumbs, album rows `album.cover`, artist rows `artist.picture` — same thumb pattern. Favorite-tracks `TrackTable` already has covers (Task 12).

- [ ] **Step 4: View headers**

Album view: 160px `<Cover uuid={album.cover} size={320} radius={12}>` beside a Fraunces title (28px/640) + muted artist/year line. Playlist view: `playlist.image` the same. Artist view: `artist.picture` (160px, radius 50% — artists are portraits) + Fraunces name. Home/Search/Library/Settings view titles switch to Fraunces 28px.

- [ ] **Step 5: Verify + smoke**

Run: `npm --prefix ui run check`
Smoke: home shows cover cards (fallback tile where `artwork` is null — **Review Focus**), cards zoom on hover, search/library thumbs render, album/playlist/artist headers show artwork.

- [ ] **Step 6: Commit**

`git add ui/src/views; git commit -m "feat(ui): add cover cards and artwork headers to the browse views"`

---

### Task 14: Feedback, overlays, spinner, and the hardcoded-color sweep

**Files:**
- Modify: `ui/src/lib/components/Loading.svelte` (disc spinner)
- Modify: `ui/src/lib/components/ToastHost.svelte` (glass + severity glyph)
- Modify: `ui/src/lib/components/Modal.svelte`, `ConfirmDialog.svelte`, `PlaylistPicker.svelte` (sheet styling)
- Modify: `ui/src/lib/components/ContextMenu.svelte`
- Modify: `ui/src/lib/components/EmptyState.svelte`, `ErrorState.svelte`
- Modify: `ui/src/App.svelte` (two `#ff6b6b` sweeps), `ui/src/views/Settings.svelte` and `ui/src/views/Login.svelte` (`#e0a030`/`#ff6b6b` sweeps)
- Modify: `ui/src/styles/tokens.css` (drop legacy aliases)

**Interfaces:**
- Consumes: `Icon` (Task 4), tokens (Task 2).
- Produces: spec §8 treatments — disc spinner, glass toasts with severity glyph, 16px sheets with lit edges and Fraunces titles, glass context menu, Fraunces empty states; zero hardcoded hexes outside `styles/`.

- [ ] **Step 1: Disc spinner in `Loading.svelte`**

Replace the border spinner with a record (spec §8: small record with an accent tick):

```css
  .disc {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--line);
    position: relative;
    animation: spin 1.2s linear infinite;
  }
  .disc::after {
    content: "";
    position: absolute;
    left: calc(50% - 1px);
    top: 1px;
    width: 2px;
    height: 5px;
    background: var(--accent);
    border-radius: 1px;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
```

(`@media (prefers-reduced-motion: reduce)` the animation stays — it is functional feedback, not ambient motion, and the spec keeps the disc spinner in Instrument.)

- [ ] **Step 2: Toast severity glyphs**

In `ToastHost.svelte`: add an `Icon` before `.message` — severity map `success → check` in `var(--success)`, `error → alert-triangle` in `var(--danger)`, default `info` in `var(--text-muted)`. Restyle `.toast` as glass: `background: var(--surface-3); backdrop-filter: blur(var(--glass-blur)); border-radius: var(--radius-control);` with the `.lit` recipe, message text back to `var(--text)` (severity lives on the glyph only — no stripes, no colored borders, spec §8/§14). Slide-in with slight overshoot: `animation: toast-in var(--dur-view) cubic-bezier(.2,.9,.3,1.2);` from `transform: translateY(-8px); opacity: 0` (the one sanctioned micro-interaction, spec §10.3).

- [ ] **Step 3: Sheets, menu, states**

`Modal.svelte`/`ConfirmDialog`/`PlaylistPicker`: panel gets `border-radius: var(--radius-sheet)` + `.lit`; titles get `font-family: var(--font-display)`. `ContextMenu.svelte`: glass + `.lit`, radius control; danger items `color: var(--danger)`. `EmptyState.svelte`: headline in Fraunces 28px + muted body + primary action button (accent fill, `--on-accent`). `ErrorState.svelte`: `#ff6b6b` → `var(--danger)`.

- [ ] **Step 4: Sweep the remaining hardcoded colors**

Run: `rg -n "#[0-9a-fA-F]{3,8}" ui/src --glob '!styles/**' --glob '!*.test.ts'`
Expected hits: `App.svelte` (×2 `#ff6b6b`), `Settings.svelte` (`#e0a030`), `Login.svelte`, `PlayerBar.svelte` if any remain. Replace `#ff6b6b` → `var(--danger)`, `#e0a030` → `var(--warning)`. Then delete the legacy aliases block (`--panel/--panel-2/--muted`) from `tokens.css` and rename their remaining consumers to `--surface-*`/`--text-muted` (`rg -n "var\(--panel|var\(--muted" ui/src`). Re-run the hex grep: only `styles/` matches remain.

- [ ] **Step 5: Verify + smoke**

Run: `npm --prefix ui test; npm --prefix ui run check`
Smoke: trigger a toast (favorite a track) — glyph + slide-in; open a dialog — lit edge + Fraunces title; loading states show the disc; empty search shows the Fraunces empty state.

- [ ] **Step 6: Commit**

`git add ui/src; git commit -m "feat(ui): restyle feedback surfaces and sweep hardcoded colors"`

---

### Task 15: Docs, gates, and smoke checklist

**Files:**
- Modify: `AGENTS.md`
- Create: `docs/superpowers/plans/notes/plan10-smoke.md`

- [ ] **Step 1: Update `AGENTS.md`**

- Replace the stale "Playback is not implemented" line with the truth: playback, favorites, playlists, UX foundations, and data interaction (plans 1–9) are shipped; `crates/oxitide-audio` exists.
- Under Architecture `ui/` add: design tokens live in `ui/src/styles/` (`tokens.css` semantic layer, `modes.css` per-mode overrides, `fonts.css` vendored faces); components consume semantic tokens only; appearance settings flow through `ui/src/lib/appearance.svelte.ts` → `data-mode`/`data-density` + CSS vars; icons are inlined Lucide paths in `Icon.svelte` (no unicode glyphs); covers render through `Cover.svelte`/`coverUrl` (TIDAL image CDN UUIDs).

- [ ] **Step 2: Write `docs/superpowers/plans/notes/plan10-smoke.md`**

```markdown
# Plan 10 smoke — Visual foundation + Instrument mode

Requires: real account, browser login, `npm --prefix ui run app:dev`.

- [ ] Boots into Warm Graphite (warm dark bg, off-white text); no Spotify-gray remains.
- [ ] Settings → Appearance: typeface swaps live (Manrope/Inter/System); Custom "No Such Font" shows the fallback note; a real installed font (e.g. "Georgia") previews and applies.
- [ ] Density Compact re-heights track rows (28px) and queue rows; Comfortable restores (38px); mid-scroll toggle on a 2k playlist keeps position.
- [ ] Sidebar collapses to the icon rail below 1080px and via the toggle; window will not shrink below 720×520.
- [ ] Queue drags 260–420px and persists across restart; below 760px it floats over a backdrop.
- [ ] Covers render: track rows (not in album view), queue, home cards, search/library rows, album/playlist/artist headers; missing art shows the warm tile.
- [ ] Player bar: hairline seek (grows + thumb on hover), 48px cover opens now-playing; Esc and artwork click close it; Next Up lists 3 rows.
- [ ] Playing row is accent-tinted with accent title; hover shows the thumb play glyph.
- [ ] Toasts have severity glyphs; dialogs have lit edges + Fraunces titles; loading shows the disc spinner.
- [ ] `config.toml` round-trips all six new `[ui]` keys with correct TOML names.
- [ ] Reduced motion (Windows setting on): state transitions become 120ms fades.
```

- [ ] **Step 3: Run the full gates**

Run: `cargo test --workspace; cargo clippy --workspace -- -D warnings; npm --prefix ui test; npm --prefix ui run check`
Expected: all green.

- [ ] **Step 4: Commit**

`git add AGENTS.md docs/superpowers/plans/notes/plan10-smoke.md; git commit -m "docs: refresh agents.md and record the plan 10 smoke checklist"`
