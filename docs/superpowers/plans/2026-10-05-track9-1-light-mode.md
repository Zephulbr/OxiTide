# Track 9.1 — Light Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a Dark / Light / System theme with full dynamic color, no boot flash, in both Atmosphere and Instrument modes.

**Architecture:** A new `UiConfig.theme` (Rust) flows to `appearance.svelte.ts`, which resolves `system` via `matchMedia`, sets `data-theme` on the root, and mirrors the setting to `localStorage` for a pre-paint boot script. CSS inversion lives in a new `themes.css` that overrides the semantic token layer; `palette.rs` gains a `Room` (`Dark`/`Light`) so `analyze_cover` and the palette cache become theme-aware.

**Tech Stack:** Rust (serde, Tauri commands, wiremock), Svelte 5 runes, TypeScript, Vitest, CSS custom properties, `@property` dynamic tokens.

**Spec:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` (§5.1, §5.2, §6.1, §8, §9)

## Global Constraints

- Theme default is `dark`; a config written before this plan backfills to `dark` with zero visual change.
- `data-theme` (`dark`/`light`) is orthogonal to `data-mode`; components must never branch on theme — only tokens do.
- Dynamic color `Auto` semantics are unchanged: full in Atmosphere, off in Instrument, in **both** themes.
- The palette cache key becomes cover+theme; bump `CACHE_VERSION` (5 → 6) so old entries discard cleanly.
- PM-D1: Track 3 budgets are unchanged; S5 gains a light variant and S1/S2 are re-run.
- IPC: `invoke` args camelCase in TS, snake_case params in Rust (`theme` is identical in both).
- New strings stay component-local (i18n is its own later track).
- Scripted gates after every task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.

## Review Focus

1. **Missing/corrupt `localStorage` mirror** (cleared storage, privacy mode, malformed value): boot must still resolve `dark`, never throw, and the app must reconcile from config once `init()` runs.
2. **OS theme changes while `system` is selected**: `data-theme` updates live and, with dynamic color on, the current cover re-analyzes for the new room.
3. **Old `config.toml` / old `palettes.json`** from previous versions: config backfills `theme = "dark"`; stale palettes discard on the version bump; no wrong colors or flash.
4. **Dynamic color off or fetch-failed after a theme switch**: inline `--dyn-*` properties are cleared and the **light** CSS fallbacks apply — never the dark `@property` initial values.
5. **Bright cover in light mode where 4.5:1 is unreachable**: the accent must stay the art's hue and darken to the envelope's best point (usable on light), never stay a pale/near-white accent.

---

### Task 1: Theme config, command, and IPC types

**Files:**
- Modify: `crates/oxitide-app/src/config.rs` (add `Theme`, `UiConfig.theme`, defaults; tests)
- Modify: `crates/oxitide-app/src/commands.rs:1814-1821` (add `set_theme` next to `set_ui_mode`)
- Modify: `crates/oxitide-app/src/main.rs:416-423` (register command)
- Modify: `ui/src/lib/api.ts:192-209` (Config.ui) and `:405` (setTheme)
- Test: `crates/oxitide-app/src/config.rs` (existing `mod tests`)

**Interfaces:**
- Produces: `Theme { Dark, Light, System }` (serde lowercase, `Default = Dark`), `UiConfig.theme: Theme`, command `set_theme(theme: Theme)`, TS type `theme: "dark" | "light" | "system"`, `api.setTheme(theme)`.

- [ ] **Step 1: Write the failing tests**

Add to `config.rs` `mod tests` (next to `appearance_keys_round_trip`):

```rust
#[test]
fn theme_round_trips_and_defaults_to_dark() {
    let dir = temp_dir();
    let path = dir.join("config.toml");
    let mut config = Config::default();
    assert_eq!(config.ui.theme, Theme::Dark);
    config.ui.theme = Theme::System;
    config.save_to(&path).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("theme = \"system\""), "{text}");
    assert_eq!(Config::load_from(&path), config);
    let _ = fs::remove_dir_all(dir);
}
```

In `appearance_defaults_fill_a_pre_plan10_file`, add after the mode assertion:

```rust
assert_eq!(config.ui.theme, Theme::Dark);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-app theme_round_trips_and_defaults_to_dark`
Expected: FAIL — `Theme` not found.

- [ ] **Step 3: Implement the enum, field, default, and command**

In `config.rs`, after the `UiMode` enum (~line 75):

```rust
/// Color theme. Orthogonal to `UiMode`; `System` follows the OS preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}
```

Add `pub theme: Theme,` to `UiConfig` (after `mode`) and `theme: Theme::Dark,` to its `Default` impl.

In `commands.rs`, after `set_ui_mode`:

```rust
#[tauri::command]
pub fn set_theme(state: State<'_, AppState>, theme: Theme) -> Result<(), String> {
    let mut config = state.config();
    config.ui.theme = theme;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

Check `config.rs` imports in `commands.rs` include `Theme` (the file already imports sibling config types in the same `use crate::config::{...}` list as `UiMode`).

In `main.rs`, add `commands::set_theme,` directly after `commands::set_ui_mode,`.

In `api.ts`, add to `Config["ui"]` after `mode`:

```ts
    theme: "dark" | "light" | "system";
```

and after `setUiMode`:

```ts
  setTheme: (theme: "dark" | "light" | "system") => invoke<void>("set_theme", { theme }),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app theme_round_trips_and_defaults_to_dark`, `cargo test -p oxitide-app appearance_defaults_fill_a_pre_plan10_file`, then `cargo clippy --workspace -- -D warnings`.
Expected: PASS, clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/config.rs crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs ui/src/lib/api.ts
git commit -m "feat(config): add the theme setting and set_theme command"
```

---

### Task 2: Pure theme resolution + mirror helpers

**Files:**
- Create: `ui/src/lib/theme.ts`
- Test: `ui/src/lib/theme.test.ts`

**Interfaces:**
- Produces: `ThemeSetting = "dark" | "light" | "system"`, `ResolvedTheme = "dark" | "light"`, `THEME_STORAGE_KEY = "oxitide.theme"`, `resolveTheme(setting, prefersDark)`, `readThemeMirror(storage?)`, `writeThemeMirror(storage?, setting)`.

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/theme.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import {
  readThemeMirror,
  resolveTheme,
  THEME_STORAGE_KEY,
  writeThemeMirror,
} from "./theme";

describe("resolveTheme", () => {
  it("returns explicit settings unchanged", () => {
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
  });
  it("system follows the OS preference", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });
});

describe("theme mirror", () => {
  /// Review Focus: a missing or corrupt mirror must fall back to dark without
  /// throwing (cleared storage, privacy mode, hand-edited value).
  it("reads valid settings, defaults anything else to dark", () => {
    const get = (value: string | null) => ({ getItem: () => value, setItem: () => {} });
    expect(readThemeMirror(get("light"))).toBe("light");
    expect(readThemeMirror(get("system"))).toBe("system");
    expect(readThemeMirror(get(null))).toBe("dark");
    expect(readThemeMirror(get("evil"))).toBe("dark");
  });

  it("survives a throwing storage", () => {
    const storage = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(readThemeMirror(storage)).toBe("dark");
    expect(() => writeThemeMirror(storage, "light")).not.toThrow();
    expect(readThemeMirror(undefined)).toBe("dark");
  });

  it("writes under the documented key", () => {
    const setItem = vi.fn();
    writeThemeMirror({ getItem: () => null, setItem }, "system");
    expect(setItem).toHaveBeenCalledWith(THEME_STORAGE_KEY, "system");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npm --prefix ui test -- src/lib/theme.test.ts`
Expected: FAIL — cannot resolve `./theme`.

- [ ] **Step 3: Implement `theme.ts`**

```ts
export type ThemeSetting = "dark" | "light" | "system";
export type ResolvedTheme = "dark" | "light";

export const THEME_STORAGE_KEY = "oxitide.theme";

export interface ThemeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/// `System` follows the OS preference; explicit settings win (spec §6.1).
export function resolveTheme(setting: ThemeSetting, prefersDark: boolean): ResolvedTheme {
  if (setting === "system") return prefersDark ? "dark" : "light";
  return setting;
}

/// The pre-paint script reads the same mirror; an absent or unreadable value
/// means the config default (dark) so first paint is never wrong for
/// existing installs (Review Focus 1/3).
export function readThemeMirror(storage?: ThemeStorage): ThemeSetting {
  try {
    const value = storage?.getItem(THEME_STORAGE_KEY);
    if (value === "dark" || value === "light" || value === "system") return value;
  } catch {
    // Storage may be unavailable; the config default is the fallback.
  }
  return "dark";
}

export function writeThemeMirror(storage: ThemeStorage | undefined, setting: ThemeSetting): void {
  try {
    storage?.setItem(THEME_STORAGE_KEY, setting);
  } catch {
    // Non-fatal: the next boot falls back to dark and init reconciles.
  }
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `npm --prefix ui test -- src/lib/theme.test.ts`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/theme.ts ui/src/lib/theme.test.ts
git commit -m "feat(ui): add theme resolution and the boot mirror"
```

---

### Task 3: Wire theme into `appearance`

**Files:**
- Modify: `ui/src/lib/appearance.svelte.ts:1-146`
- Test: `ui/src/lib/appearance.test.ts`

**Interfaces:**
- Consumes: `theme.ts` from Task 2; `api.getConfig().ui.theme` and `api.setTheme` from Task 1.
- Produces: `appearance.theme: ThemeSetting`, `appearance.resolvedTheme: ResolvedTheme`, `appearance.setTheme(t)`; `apply()` writes `root.dataset.theme` and `root.style.colorScheme`.

- [ ] **Step 1: Write the failing test**

Replace the `vi.mock("./api", ...)` block and add a test to `appearance.test.ts`:

```ts
vi.mock("./api", () => ({
  api: {
    getConfig: vi.fn(),
    setOneko: vi.fn(),
    setTheme: vi.fn(),
  },
}));
```

Add a new `describe`:

```ts
describe("theme", () => {
  beforeEach(() => {
    vi.stubGlobal("window", {
      matchMedia: () => ({ matches: false, addEventListener: () => {} }),
    });
    vi.stubGlobal("document", {
      documentElement: { dataset: {}, style: { setProperty: () => {} } },
    });
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: vi.fn() });
    vi.mocked(api.getConfig).mockResolvedValue({ ui: { oneko: false, theme: "system" } } as never);
    vi.mocked(api.setTheme).mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it("init resolves system, applies data-theme, and mirrors the setting", async () => {
    const appearance = createAppearance();
    await appearance.init();
    expect(appearance.theme).toBe("system");
    expect(appearance.resolvedTheme).toBe("light"); // matchMedia matches: false
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(localStorage.setItem).toHaveBeenCalledWith("oxitide.theme", "system");
  });

  it("setTheme applies and persists", async () => {
    const appearance = createAppearance();
    await appearance.init();
    await appearance.setTheme("dark");
    expect(appearance.resolvedTheme).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(api.setTheme).toHaveBeenCalledWith("dark");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npm --prefix ui test -- src/lib/appearance.test.ts`
Expected: FAIL — `appearance.theme` undefined / `setTheme` not a function.

- [ ] **Step 3: Implement the appearance changes**

In `appearance.svelte.ts`:

```ts
import { readThemeMirror, resolveTheme, writeThemeMirror, type ResolvedTheme, type ThemeSetting } from "./theme";
```

Add to the `Appearance` interface:

```ts
  readonly theme: ThemeSetting;
  readonly resolvedTheme: ResolvedTheme;
  setTheme(theme: ThemeSetting): Promise<void>;
```

Inside `createAppearance()`:

```ts
  let theme = $state<ThemeSetting>(readThemeMirror(globalThis.localStorage));
  let prefersDark = $state(false);
```

Extend `apply()`:

```ts
  function apply() {
    const root = document.documentElement;
    const resolved = resolveTheme(theme, prefersDark);
    root.dataset.mode = mode;
    root.dataset.theme = resolved;
    root.style.colorScheme = resolved;
    root.dataset.density = density;
    root.style.setProperty("--font-ui", fontStack(resolveTypeface(typefaceSetting, mode), customTypeface));
    root.style.setProperty("--queue-width", `${queueWidth}px`);
  }
```

In `init()`, before the config read:

```ts
    const themeMedia = window.matchMedia("(prefers-color-scheme: dark)");
    prefersDark = themeMedia.matches;
    themeMedia.addEventListener("change", (e) => {
      prefersDark = e.matches;
      apply();
    });
```

and inside the `try` block after `mode = config.ui.mode;`:

```ts
      theme = config.ui.theme;
```

after the `try/catch`, before `apply()`:

```ts
    writeThemeMirror(globalThis.localStorage, theme);
```

Add the getters and setter to the returned object:

```ts
    get theme() { return theme; },
    get resolvedTheme() { return resolveTheme(theme, prefersDark); },
```

```ts
    setTheme: (t) => { theme = t; writeThemeMirror(globalThis.localStorage, t); return persist(() => api.setTheme(t)); },
```

- [ ] **Step 4: Run it to verify it passes**

Run: `npm --prefix ui test -- src/lib/appearance.test.ts`, then `npm --prefix ui run check`.
Expected: PASS, 0 errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/appearance.svelte.ts ui/src/lib/appearance.test.ts
git commit -m "feat(ui): resolve and apply the color theme"
```

---

### Task 4: Pre-paint boot script

**Files:**
- Modify: `ui/index.html`
- Test: `ui/src/lib/themeBoot.test.ts`

**Interfaces:**
- Consumes: `THEME_STORAGE_KEY` from Task 2.
- Produces: `data-theme` + `style.colorScheme` set before the app module loads.

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/themeBoot.test.ts` (source guard, modeled on `rowHeight.test.ts`):

```ts
import { describe, expect, it } from "vitest";
// @ts-expect-error — the svelte tsconfig has no node types; vitest runs in node.
import { readFileSync } from "node:fs";

describe("theme boot script", () => {
  it("applies the stored theme before the module script", () => {
    const html = readFileSync(new URL("../../index.html", import.meta.url), "utf8");
    const boot = html.indexOf("oxitide.theme");
    const module = html.indexOf('type="module"');
    expect(boot).toBeGreaterThan(-1);
    expect(module).toBeGreaterThan(-1);
    expect(boot).toBeLessThan(module);
    expect(html).toContain("documentElement.dataset.theme");
    expect(html).toContain("prefers-color-scheme: dark");
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npm --prefix ui test -- src/lib/themeBoot.test.ts`
Expected: FAIL — `oxitide.theme` not found.

- [ ] **Step 3: Add the inline script to `index.html`**

Inside `<head>`, before the closing tag:

```html
    <script>
      (function () {
        var t = "dark";
        try {
          t = localStorage.getItem("oxitide.theme") || "dark";
        } catch (e) {}
        if (t !== "dark" && t !== "light") {
          t =
            window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches
              ? "dark"
              : "light";
        }
        document.documentElement.dataset.theme = t;
        document.documentElement.style.colorScheme = t;
      })();
    </script>
```

- [ ] **Step 4: Run it to verify it passes**

Run: `npm --prefix ui test -- src/lib/themeBoot.test.ts`
Expected: PASS.

- [ ] **Step 5: Manual check (record in the plan's smoke note later)**

Run: `npm --prefix ui run app:dev`, set Theme to Light in Settings, quit, relaunch. Expected: no dark flash before the light UI appears. Also try setting it to a hand-corrupted `localStorage["oxitide.theme"] = "junk"` in the WebView devtools, relaunch, and confirm a clean dark boot (Review Focus 1).

- [ ] **Step 6: Commit**

```bash
git add ui/index.html ui/src/lib/themeBoot.test.ts
git commit -m "feat(ui): apply the theme mirror before first paint"
```

---

### Task 5: Light token layer + raw-color fixes

**Files:**
- Create: `ui/src/styles/themes.css`
- Modify: `ui/src/main.ts:3-6` (import order)
- Modify: `ui/src/styles/tokens.css:21-104` (room/scrim/grain tokens; semantic relocation)
- Modify: `ui/src/app.css:1` (drop hard-coded `color-scheme`), `:44` (grain), `:225` (scrollbar thumb)
- Modify: `ui/src/lib/components/Backdrop.svelte:65,85`
- Modify: `ui/src/views/NowPlaying.svelte:459-463`
- Modify: `ui/src/views/Profiles.svelte:215-220`
- Test: `ui/src/lib/themeTokens.test.ts`

**Interfaces:**
- Produces: `[data-theme="light"]` overrides for every semantic color token; `--room-top`, `--scrim-top`, `--scrim-bottom`, `--grain-alpha` tokens; light `--dyn-*` fallbacks.

- [ ] **Step 1: Write the failing test**

Create `ui/src/lib/themeTokens.test.ts`:

```ts
import { describe, expect, it } from "vitest";
// @ts-expect-error — the svelte tsconfig has no node types; vitest runs in node.
import { readFileSync } from "node:fs";

const tokens = readFileSync(new URL("../styles/tokens.css", import.meta.url), "utf8");
const themes = readFileSync(new URL("../styles/themes.css", import.meta.url), "utf8");

function hexFor(css: string, name: string): string {
  const match = css.match(new RegExp(`--${name}:\\s*(#[0-9a-fA-F]{6})`));
  if (!match) throw new Error(`missing hex token --${name}`);
  return match[1];
}

function luminance(hex: string): number {
  const srgb = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const linear = srgb.map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const light = themes.slice(themes.indexOf('[data-theme="light"]'));

describe("light token layer", () => {
  /// Review Focus 4: every semantic color token in :root must have a light
  /// counterpart, including the --dyn-* fallbacks that apply when dynamic
  /// color is off or a fetch failed.
  it("overrides every semantic color token", () => {
    for (const name of [
      "bg", "text", "text-muted", "accent-brand", "on-accent-brand",
      "danger", "success", "warning",
      "surface-1", "surface-2", "surface-3", "surface-float", "line",
      "edge-lit", "glare-alpha",
      "dyn-primary", "dyn-on-primary", "dyn-secondary", "dyn-tertiary", "dyn-backdrop",
    ]) {
      expect(light, name).toMatch(new RegExp(`--${name}:`));
    }
  });

  it("keeps text and semantic colors at 4.5:1 on the light room", () => {
    const bg = hexFor(light, "bg");
    for (const name of ["text", "text-muted", "danger", "success", "warning", "dyn-primary"]) {
      const ratio = contrast(hexFor(light, name), bg);
      expect(ratio, `${name} on light bg`).toBeGreaterThanOrEqual(4.5);
    }
    expect(contrast(hexFor(light, "dyn-on-primary"), hexFor(light, "dyn-primary"))).toBeGreaterThanOrEqual(4.5);
  });

  it("defines the theme-paired room and scrim tokens in the dark layer", () => {
    for (const name of ["room-top", "scrim-top", "scrim-bottom", "grain-alpha"]) {
      expect(tokens, name).toMatch(new RegExp(`--${name}:`));
    }
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `npm --prefix ui test -- src/lib/themeTokens.test.ts`
Expected: FAIL — cannot read `themes.css`.

- [ ] **Step 3: Add the token hooks to `tokens.css`**

Inside `:root`, after `--glare-alpha`:

```css
  /* Theme-paired room + scrim (Backdrop reads these; themes.css flips them). */
  --room-top: #1d1712;
  --scrim-top: rgba(23, 18, 14, 0.55);
  --scrim-bottom: rgba(23, 18, 14, 0.78);
  /* Film-grain opacity multiplier (spec §10.1); drops in light (§4.4). */
  --grain-alpha: 0.045;

  /* Static fallbacks for the dynamic tokens, so the brand pair has one
     source; themes.css re-declares the light values. Dynamic color overrides
     these inline when active. */
  --dyn-primary: var(--accent-brand);
  --dyn-on-primary: var(--on-accent-brand);
  --dyn-secondary: #17120e;
  --dyn-tertiary: var(--accent-brand);
  --dyn-backdrop: #1a1510;
```

Move `--bg`, `--text`, `--text-muted` out of the "Base" comment block into the semantic block (values unchanged) so the layer contract matches the docs.

- [ ] **Step 4: Create `themes.css`**

```css
/* Theme deltas. `data-theme` is orthogonal to `data-mode` (spec §6.1):
   components never read these names; only semantic tokens change.
   The dark recipe lives in :root; this file carries the light inversion. */

[data-theme="light"] {
  /* Warm paper counterpart to Warm Graphite (foundation §4.4). */
  --bg: #f6f1ea;
  --text: #2a211a;
  --text-muted: #665a4f;
  --accent-brand: #0e7a67;
  --on-accent-brand: #ffffff;
  --danger: #b3261e;
  --success: #1e6b3a;
  --warning: #8a5a00;

  /* Static fallbacks for cleared inline --dyn-* values (Review Focus 4). */
  --dyn-primary: #0e7a67;
  --dyn-on-primary: #ffffff;
  --dyn-secondary: #e8dfd3;
  --dyn-tertiary: #8a4a1f;
  --dyn-backdrop: #ece4da;

  --surface-1: rgba(42, 33, 26, 0.045);
  --surface-2: rgba(42, 33, 26, 0.065);
  --surface-3: rgba(42, 33, 26, 0.085);
  --surface-float: rgba(250, 246, 240, 0.94);
  --line: rgba(42, 33, 26, 0.14);
  --edge-lit: linear-gradient(135deg, rgba(255, 255, 255, 0.85), rgba(42, 33, 26, 0.05) 45%, rgba(42, 33, 26, 0.18));
  --glare-alpha: 0.10;
  --grain-alpha: 0.025;

  --room-top: #efe9e0;
  --scrim-top: rgba(246, 241, 234, 0.55);
  --scrim-bottom: rgba(246, 241, 234, 0.78);
}

[data-mode="atmosphere"][data-theme="light"] {
  --surface-1: rgba(42, 33, 26, 0.045);
  --surface-2: rgba(42, 33, 26, 0.065);
  --surface-3: rgba(42, 33, 26, 0.085);
  --surface-float: rgba(250, 246, 240, 0.94);
  --glass-veil: rgba(246, 241, 234, 0.30);
}

[data-mode="instrument"][data-theme="light"] {
  --surface-1: #fbf8f3;
  --surface-2: #f2ece3;
  --surface-3: #eae2d7;
  --surface-float: #fffdf9;
  --edge-lit: linear-gradient(135deg, var(--line), var(--line));
  --glare-alpha: 0.08;
}
```

Import it in `main.ts` between `modes.css` and `fonts.css`:

```ts
import "./styles/themes.css";
```

- [ ] **Step 5: Fix the raw-color sites**

`app.css`: delete line 1 (`:root { color-scheme: dark; }`) — `appearance.apply()` and the boot script own `color-scheme` now. Change the grain to `opacity: calc(var(--flag-grain) * var(--grain-alpha));`. Change the scrollbar thumb to `background: color-mix(in srgb, var(--text) 14%, transparent);`.

`Backdrop.svelte`: line 65 becomes `background: linear-gradient(160deg, var(--room-top), var(--bg)); /* no-art room (§5.2) */` and line 85 becomes `background: linear-gradient(180deg, var(--scrim-top), var(--scrim-bottom));`.

`NowPlaying.svelte:459-463`: replace both hard-coded warm-white stops with the token-driven form:

```css
    background: radial-gradient(
      circle at var(--tilt-gx, 50%) var(--tilt-gy, 50%),
      rgba(255, 255, 255, var(--glare-alpha)),
      rgba(255, 255, 255, 0) 40%
    );
```

`Profiles.svelte:215-220`: replace `rgba(255, 240, 225, 0.16)` with `color-mix(in srgb, var(--text) 16%, transparent)`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/themeTokens.test.ts`, then `npm --prefix ui test` and `npm --prefix ui run check`.
Expected: all pass; 0 svelte-check errors.

- [ ] **Step 7: Commit**

```bash
git add ui/src/styles/themes.css ui/src/main.ts ui/src/styles/tokens.css ui/src/app.css ui/src/lib/components/Backdrop.svelte ui/src/views/NowPlaying.svelte ui/src/views/Profiles.svelte ui/src/lib/themeTokens.test.ts
git commit -m "feat(ui): add the light token layer and theme the raw color sites"
```

---

### Task 6: `palette.rs` light room

**Files:**
- Modify: `crates/oxitide-app/src/palette.rs` (room constants + extraction)
- Modify: `crates/oxitide-app/src/artwork.rs:12,162` (call sites to `Room::Dark` for now)
- Test: `crates/oxitide-app/src/palette.rs` (`mod tests`)

**Interfaces:**
- Produces: `Room { Dark, Light }` (serde lowercase, `as_str()`), `extract_palette(pixels, width, height, room: Room)`, `fallback_palette(room: Room)`. Task 7 threads `Room` through the command and cache.

- [ ] **Step 1: Write the failing light-room tests**

Add to `palette.rs` `mod tests`:

```rust
#[test]
fn light_room_saturated_cover_clears_the_light_ceiling() {
    let pixels = solid((30, 80, 220), 48 * 48);
    let palette = extract_palette(&pixels, 48, 48, Room::Light);
    let primary = hex_to_rgb(&palette.primary);
    let contrast = contrast_ratio(primary, palette.ceiling_l);
    assert!(contrast >= 4.5, "primary vs light ceiling: {contrast}");
    let oklch = srgb_to_oklch_tuple(primary);
    assert!(hue_diff(oklch.h, 264.0) < 40.0, "hue should stay blue: {}", oklch.h);
}

/// Review Focus 5: when 4.5:1 is unreachable on light surfaces, the accent
/// darkens to the envelope's best point instead of staying pale.
#[test]
fn light_room_unreachable_floor_keeps_a_dark_accent() {
    let pixels = solid((200, 30, 40), 48 * 48);
    let palette = extract_palette(&pixels, 48, 48, Room::Light);
    let oklch = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
    assert!(oklch.l <= 0.25, "light-room fallback must be dark: L {}", oklch.l);
    assert!(hue_diff(oklch.h, 30.0) < 40.0, "hue should track the art: {}", oklch.h);
}

#[test]
fn light_room_backdrop_keeps_the_cover_mean() {
    let bright = extract_palette(&solid((250, 245, 238), 48 * 48), 48, 48, Room::Light);
    assert_eq!(bright.backdrop, "#faf5ee");
}

#[test]
fn light_room_fallback_uses_the_light_room_tone() {
    let light = fallback_palette(Room::Light);
    assert_eq!(light.backdrop, "#eee7dd");
    assert_ne!(light.primary, fallback_palette(Room::Dark).primary);
}

/// The same cover must not share a palette across rooms.
#[test]
fn rooms_produce_different_primaries_for_the_same_cover() {
    let pixels = solid((200, 30, 40), 48 * 48);
    assert_ne!(
        extract_palette(&pixels, 48, 48, Room::Dark).primary,
        extract_palette(&pixels, 48, 48, Room::Light).primary
    );
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p oxitide-app light_room`
Expected: FAIL — `extract_palette` takes 3 args.

- [ ] **Step 3: Parameterize `palette.rs`**

Add near the top (after `Oklch`):

```rust
/// Visual room the palette is extracted for (Track 9 spec §5.2). `Dark` is
/// Warm Graphite; `Light` is its warm-paper counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Room {
    Dark,
    Light,
}

impl Room {
    pub fn as_str(self) -> &'static str {
        match self {
            Room::Dark => "dark",
            Room::Light => "light",
        }
    }
}

/// Per-room extraction constants. Everything that used to be a module const
/// and assumed a dark room lives here.
struct RoomConsts {
    bg: (u8, u8, u8),
    room_avg: (u8, u8, u8),
    text: (u8, u8, u8),
    ink: (u8, u8, u8),
    scrim: f32,
    primary_l: (f32, f32),
    primary_c: (f32, f32),
    secondary_l: (f32, f32),
    secondary_c_cap: f32,
    secondary_dark: bool,
    tertiary_l: (f32, f32),
    tertiary_c: (f32, f32),
    plain_l: f32,
}

fn room(room: Room) -> RoomConsts {
    match room {
        Room::Dark => RoomConsts {
            bg: (0x17, 0x12, 0x0e),
            room_avg: (0x1a, 0x15, 0x10),
            text: (0xf2, 0xec, 0xe6),
            ink: (0x0b, 0x24, 0x20),
            scrim: 0.55,
            primary_l: (0.62, 0.85),
            primary_c: (0.08, 0.21),
            secondary_l: (0.25, 0.45),
            secondary_c_cap: 0.05,
            secondary_dark: true,
            tertiary_l: (0.65, 0.80),
            tertiary_c: (0.06, 0.14),
            plain_l: 0.95,
        },
        Room::Light => RoomConsts {
            bg: (0xf6, 0xf1, 0xea),
            room_avg: (0xee, 0xe7, 0xdd),
            text: (0x2a, 0x21, 0x1a),
            ink: (0xff, 0xff, 0xff),
            scrim: 0.55,
            primary_l: (0.15, 0.55),
            primary_c: (0.08, 0.21),
            secondary_l: (0.80, 0.92),
            secondary_c_cap: 0.04,
            secondary_dark: false,
            tertiary_l: (0.30, 0.52),
            tertiary_c: (0.06, 0.14),
            plain_l: 0.30,
        },
    }
}
```

Then apply these substitutions (keep all other logic identical):

| Old | New |
|---|---|
| `const BG`, `const ROOM_AVG`, `const SCRIM`, `const TEXT`, `const ON_ACCENT_DARK` | deleted; use `r.bg`, `r.room_avg`, `r.scrim`, `r.text`, `r.ink` |
| `const PRIMARY_L_LO/HI`, `PRIMARY_C_LO/HI` | `r.primary_l.0/.1`, `r.primary_c.0/.1` |
| `NEAR_WHITE_L/C`, `CHROMA_FLOOR`, `TINT_FLOOR` | unchanged module consts |
| `fn composite(pixel)` | `fn composite(pixel, r: &RoomConsts)` using `r.bg`, `r.scrim` |
| `blurred_average(blurred)` | `blurred_average(blurred, r: &RoomConsts)` fallback `r.room_avg` |
| `composite_ceiling(blurred)` | `composite_ceiling(blurred, r: &RoomConsts)` |
| `fn on_accent(primary)` | `fn on_accent(primary, r: &RoomConsts)` comparing `r.ink` vs `r.text` |
| `fallback_palette()` | `fallback_palette(room: Room)`: primary `Oklch { l: if dark { 0.81 } else { 0.45 }, c: 0.12, h: 172.0 }`, on_primary `r.ink`, secondary `Oklch { l: r.secondary_l.0, c: 0.02, h: 60.0 }`, tertiary `Oklch { l: if dark { 0.72 } else { 0.42 }, c: 0.10, h: 172.0 }`, backdrop `r.room_avg`, ceiling `luminance(r.bg)` |
| `extract_palette(pixels, width, height)` | add `room: Room` param and `let r = room(room);` |

Plain-neutral branch (`mean.c <= TINT_FLOOR`): use `Oklch { l: r.plain_l, c: 0.01, h: 60.0 }` for primary, `Oklch { l: r.secondary_l.0, c: 0.02, h: 60.0 }` for secondary, `Oklch { l: r.tertiary_l.0, c: 0.02, h: 60.0 }` for tertiary.

Primary envelope and fit:

```rust
    let mut primary = Oklch {
        l: clamp(seed.l, r.primary_l.0, r.primary_l.1),
        c: clamp(seed.c, r.primary_c.0, r.primary_c.1),
        h: seed.h,
    };
    primary.c = primary.c.min(gamut_chroma(primary.l, primary.h, primary.c));
    primary = fit_contrast(primary, r.primary_l.0, r.primary_l.1, ceiling, 4.5).unwrap_or_else(|| {
        if room == Room::Light {
            // Best effort on light surfaces: the darkest in-gamut envelope
            // point keeps the accent usable when 4.5:1 is unreachable.
            let mut fallback = Oklch { l: r.primary_l.0, c: primary.c, h: primary.h };
            fallback.c = fallback.c.min(gamut_chroma(fallback.l, fallback.h, fallback.c));
            fallback
        } else {
            primary
        }
    });
    let primary_rgb = oklch_to_srgb(primary);
    let on_primary = on_accent(primary_rgb, &r);
```

Secondary:

```rust
    let secondary = candidates
        .iter()
        .filter(|(c, _)| {
            if r.secondary_dark { c.l < 0.5 } else { c.l > 0.5 }
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(c, _)| Oklch {
            l: clamp(c.l, r.secondary_l.0, r.secondary_l.1),
            c: c.c.min(r.secondary_c_cap),
            h: c.h,
        })
        .unwrap_or(Oklch { l: r.secondary_l.0, c: 0.02, h: 60.0 });
```

Tertiary envelope and fallback:

```rust
    let tertiary = Oklch {
        l: clamp(tertiary_seed.l, r.tertiary_l.0, r.tertiary_l.1),
        c: clamp(tertiary_seed.c.max(r.tertiary_c.0), r.tertiary_c.0, r.tertiary_c.1),
        h: tertiary_seed.h,
    };
    let tertiary = fit_contrast(tertiary, r.tertiary_l.0, r.tertiary_l.1, ceiling, 4.5)
        .unwrap_or_else(|| {
            if room == Room::Light {
                let mut fallback =
                    Oklch { l: r.tertiary_l.0, c: tertiary.c, h: tertiary.h };
                fallback.c = fallback.c.min(gamut_chroma(fallback.l, fallback.h, fallback.c));
                fallback
            } else {
                tertiary
            }
        });
```

Also thread `room` through `let blurred_average(...)`, `composite_ceiling(...)`, and the fallback return at the no-pixels check.

Update every existing test call: `extract_palette(..., 48, 48)` → `extract_palette(..., 48, 48, Room::Dark)`; `fallback_palette()` → `fallback_palette(Room::Dark)`.

In `artwork.rs`, update the two `extract_palette`/`fallback_palette` call sites to `Room::Dark` (Task 7 replaces them with the parameter). Import `palette::Room`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p oxitide-app palette`, then `cargo test --workspace`, then `cargo clippy --workspace -- -D warnings`.
Expected: all pass; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/palette.rs crates/oxitide-app/src/artwork.rs
git commit -m "feat(palette): extract for a dark or light room"
```

---

### Task 7: Theme-aware `analyze_cover`, cache key, and UI re-analysis

**Files:**
- Modify: `crates/oxitide-app/src/artwork.rs` (`palette`, `fetch`, cache key, `CACHE_VERSION`; tests)
- Modify: `crates/oxitide-app/src/commands.rs:2027-2033` (`analyze_cover` signature)
- Modify: `ui/src/lib/api.ts:430` (`analyzeCover`)
- Modify: `ui/src/lib/dynamicColor.svelte.ts:26-77` (theme in the key and the effect)
- Test: `crates/oxitide-app/src/artwork.rs`, `ui/src/lib/dynamicColor.test.ts`

**Interfaces:**
- Consumes: `Room` from Task 6; `appearance.resolvedTheme` from Task 3.
- Produces: `ArtworkService::palette(&self, uuid: &str, room: Room)`, cache key `"{room}|{uuid}"`, `CACHE_VERSION = 6`, command `analyze_cover(uuid, theme)`, `api.analyzeCover(uuid, theme)`, `createPaletteApplier.onTrack(cover, mode, theme)`.

- [ ] **Step 1: Write the failing tests**

In `artwork.rs` tests, add:

```rust
/// The same cover fetched for both rooms must produce separate cache entries
/// and different palettes (Review Focus 3/5).
#[tokio::test]
async fn rooms_cache_separately_and_extract_differently() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
        .expect(2) // once per room; neither may serve the other's entry
        .mount(&server).await;
    let dir = temp_dir();
    let service = service(&server, &dir);
    let dark = service.palette("ab-cd", Room::Dark).await.unwrap();
    let light = service.palette("ab-cd", Room::Light).await.unwrap();
    assert_ne!(dark.primary, light.primary);
    let _ = std::fs::remove_dir_all(dir);
}
```

Update every existing `service(...).palette("...")` call in these tests to pass `Room::Dark`.

In `dynamicColor.test.ts`, update the `createPaletteApplier` calls and add a theme test:

```ts
  /// Review Focus 2: a theme change with the same cover/mode must re-analyze.
  it("re-fetches when only the theme changes", async () => {
    const applied: Record<string, string>[] = [];
    let fetches = 0;
    const fetcher = async () => {
      fetches += 1;
      return { ...palette, primary: fetches === 1 ? "#111111" : "#222222" };
    };
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);
    await applier.onTrack("uuid", "full", "dark");
    await applier.onTrack("uuid", "full", "light");
    expect(fetches).toBe(2);
    expect(applied.at(-1)).toEqual(expect.objectContaining({ "--dyn-primary": "#222222" }));
  });
```

Update the existing `onTrack(...)` calls in `dynamicColor.test.ts` to the 3-argument signature (append `"dark"`).

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p oxitide-app rooms_cache_separately_and_extract_differently` and `npm --prefix ui test -- src/lib/dynamicColor.test.ts`
Expected: FAIL — signature mismatches.

- [ ] **Step 3: Implement**

`artwork.rs`:

```rust
const CACHE_VERSION: u32 = 6; // 5 → 6: palettes are keyed by room now
```

```rust
    pub async fn palette(&self, uuid: &str, room: Room) -> Result<Palette, String> {
        if uuid.is_empty() {
            return Err("empty cover uuid".into());
        }
        let key = format!("{}|{uuid}", room.as_str());
        if let Some(hit) = self.memory().lock().unwrap().get(&key) {
            return Ok(hit.clone().into());
        }
        let palette = match self.fetch(uuid, room).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("cover analysis failed for {uuid}: {e}");
                return Err(e);
            }
        };
        let mut dto = PaletteDto::from(palette.clone());
        dto.fetched_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.memory().lock().unwrap().insert(key, dto);
        self.save_disk();
        Ok(palette)
    }

    async fn fetch(&self, uuid: &str, room: Room) -> Result<Palette, String> {
        // ... unchanged until the extract call:
        Ok(extract_palette(&pixels, EXTRACT_SIDE as usize, EXTRACT_SIDE as usize, room))
    }
```

`commands.rs`:

```rust
#[tauri::command]
pub async fn analyze_cover(
    state: State<'_, AppState>,
    uuid: String,
    theme: crate::palette::Room,
) -> Result<PaletteDto, String> {
    let Some(service) = state.artwork() else {
        return Err("artwork service unavailable".into());
    };
    service.palette(&uuid, theme).await.map(PaletteDto::from)
}
```

`api.ts`:

```ts
  analyzeCover: (uuid: string, theme: "dark" | "light") =>
    invoke<CoverPalette>("analyze_cover", { uuid, theme }),
```

`dynamicColor.svelte.ts`:

```ts
    async onTrack(cover: string | null, mode: "full" | "accent" | "off", theme: "dark" | "light") {
      const key = `${mode}|${theme}|${cover ?? ""}`;
      // ... unchanged body
```

```ts
export function createDynamicColor(player: Player) {
  const applier = createPaletteApplier(applyToDocument, api.analyzeCover);
  const cover = $derived(player.state.current?.cover ?? null);
  const theme = $derived(appearance.resolvedTheme);
  $effect(() => {
    void applier.onTrack(cover, appearance.dynamicColor, theme);
  });
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/artwork.rs crates/oxitide-app/src/commands.rs ui/src/lib/api.ts ui/src/lib/dynamicColor.svelte.ts ui/src/lib/dynamicColor.test.ts
git commit -m "feat(palette): key cover analysis by room and re-analyze on theme change"
```

---

### Task 8: Settings control and smoke checklist

**Files:**
- Modify: `ui/src/views/Settings.svelte:559-569` (Theme control above Mode)
- Create: `docs/superpowers/plans/notes/2026-10-05-track9-1-light-mode-smoke.md`
- Test: manual GUI smoke (human, real account)

**Interfaces:**
- Consumes: `appearance.theme`, `appearance.setTheme` from Task 3.

- [ ] **Step 1: Add the Theme control**

In `Settings.svelte`, directly before the Mode field:

```svelte
      <div class="field">
        <span class="field-label">Theme:</span>
        <div class="segmented" role="group" aria-label="Theme">
          <button class:active={appearance.theme === "dark"} onclick={() => appearance.setTheme("dark").catch(apply)}>Dark</button>
          <button class:active={appearance.theme === "light"} onclick={() => appearance.setTheme("light").catch(apply)}>Light</button>
          <button class:active={appearance.theme === "system"} onclick={() => appearance.setTheme("system").catch(apply)}>System</button>
        </div>
      </div>
      {#if appearance.theme === "system"}
        <p class="muted">Follows the operating system's app theme.</p>
      {/if}
```

- [ ] **Step 2: Verify scripted gates**

Run: `npm --prefix ui run check`, `npm --prefix ui test`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`.
Expected: all green.

- [ ] **Step 3: Write the smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-05-track9-1-light-mode-smoke.md`:

```markdown
# Track 9.1 light mode — owner smoke checklist

- **Status:** Pending owner run (Windows, real account)
- **Authority:** `docs/superpowers/specs/2026-10-05-track9-ui-completion-design.md` §6.1

## Theme switching
- [ ] Settings → Appearance shows Dark / Light / System; each selection applies immediately.
- [ ] Light + Atmosphere: warm paper room, translucent surfaces, glass header/player, cover-art backdrop, no washed-out text.
- [ ] Light + Instrument: solid light surfaces, flat edges, no blur, no grain.
- [ ] Dark + both modes look unchanged from before this plan.
- [ ] System follows a Windows theme flip while the app is running (toggle Windows app mode and watch OxiTide follow).
- [ ] Restart in each setting; no dark/light flash before first paint.
- [ ] Hand-corrupt `localStorage["oxitide.theme"]` via devtools and restart: clean dark boot, no errors.

## Dynamic color
- [ ] Light + Atmosphere, dynamic color Default: nav/link/focus/playing-row colors follow the cover and stay readable.
- [ ] Switch a bright/warm cover and a dark/blue cover; no pale unreadable accents.
- [ ] Toggle theme mid-playback with dynamic color on: colors re-analyze once, no stale dark-room palette.
- [ ] Dynamic color Off in light theme: static light-safe teal accent (not dark-mode teal).
- [ ] Menus/overlays over bright art remain legible in light mode.

## Regression
- [ ] Player bar, seek, lyrics, queue, modals, toasts legible in all four mode×theme combinations.
```

- [ ] **Step 4: Commit**

```bash
git add ui/src/views/Settings.svelte docs/superpowers/plans/notes/2026-10-05-track9-1-light-mode-smoke.md
git commit -m "feat(ui): add the theme control and light-mode smoke checklist"
```

---

### Task 9: Perf light pin and scenario runs

**Files:**
- Modify: `scripts/perf/drive.mjs:155-199` (theme pin + restore)
- Modify: `scripts/perf/budgets.json` (add `S5-atmosphere-light`)
- Modify: `docs/superpowers/plans/notes/2026-10-05-track9-1-light-mode-smoke.md` (append scenario results)

**Interfaces:**
- Consumes: `set_theme` command from Task 1.

- [ ] **Step 1: Add the theme pin to `drive.mjs`**

Add next to `setUiMode`:

```js
const setTheme = async (theme) => {
  await evaluate(
    `window.__TAURI_INTERNALS__.invoke("set_theme", { theme: ${JSON.stringify(theme)} })`,
  );
};
```

In `scenarioFallback`, extend the pin block:

```js
  const targetTheme = id.endsWith("-light") ? "light" : null;
  let originalTheme = null;
  let themeApplied = false;
  const applyScenarioTheme = async () => {
    if (!targetTheme || themeApplied) return;
    try {
      originalTheme = await evaluate(
        'window.__TAURI_INTERNALS__.invoke("get_config").then((c) => c.ui.theme)',
      );
    } catch {
      originalTheme = null;
    }
    if (!originalTheme || originalTheme === targetTheme) return;
    themeApplied = true;
    try {
      await setTheme(targetTheme);
    } catch (e) {
      console.error(`theme switch to ${targetTheme} failed: ${e.message}`);
    }
  };
```

Call `await applyScenarioTheme();` immediately after each `await applyScenarioMode();` (S5 branch), and in the `finally`-style cleanup where the mode is restored, add:

```js
    if (themeApplied) {
      try {
        await setTheme(originalTheme);
      } catch (e) {
        console.error(`theme restore to ${originalTheme} failed: ${e.message}`);
      }
    }
```

- [ ] **Step 2: Add the budget entry**

In `budgets.json`, add a sibling to `"S5-atmosphere"` with the same rows (atmosphere frame budgets apply; theme is orthogonal to mode):

```json
  "S5-atmosphere-light": [
    { "metric": "frame_p95_ms:atmosphere", "max": 20 },
    { "metric": "frame_p99_ms:atmosphere", "max": 33 }
  ],
```

- [ ] **Step 3: Run the scenarios (Vite warm, single app, foreground)**

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5-atmosphere-light -Runs 2
node scripts/perf/check.mjs --scenario S5-atmosphere-light
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S1 -Runs 3
powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S2 -Runs 3
node scripts/perf/check.mjs
```

Expected: `S5-atmosphere-light` passes its frame budgets; S1/S2 pass unchanged (the boot script adds no measurable startup step); no other budget regresses.

- [ ] **Step 4: Record results and commit**

Append the run results to `docs/superpowers/plans/notes/2026-10-05-track9-1-light-mode-smoke.md` (scripted results section above the owner checklist), then:

```bash
git add scripts/perf/drive.mjs scripts/perf/budgets.json docs/superpowers/plans/notes/2026-10-05-track9-1-light-mode-smoke.md
git commit -m "perf: pin S5 to a light-theme variant and record the runs"
```

---

## Plan Self-Review

- **Spec coverage:** §5.1 state/resolution/mirror/settings → Tasks 1–4, 8; §5.1 CSS inversion + token hygiene + raw-color fixes → Task 5; §5.2 room extraction + cache + `analyze_cover` → Tasks 6–7; §6.1 tests (config round-trip, `resolveTheme`, `apply()`, light-room palette, token parity) → Tasks 1/2/3/5/6; §8 perf light pin + S1/S2 → Task 9; §9 boot-flash and contrast risks → Tasks 4/5/6. No spec item is unassigned.
- **Placeholders:** none; all steps carry code or exact substitutions.
- **Type consistency:** `Room`/`as_str` (Task 6) is used by Task 7; `ThemeSetting`/`resolvedTheme` (Tasks 2–3) is used by Tasks 7–8; `analyzeCover(uuid, theme)` matches the command signature.
- **Review Focus coverage:** (1) mirror robustness → Task 2 test + Task 4 manual; (2) live OS flip → Task 3 listener + Task 7 re-fetch test; (3) old config/cache → Task 1 backfill assert + Task 6/7 tests; (4) cleared `--dyn-*` → Task 5 parity test; (5) bright-cover accent → Task 6 fallback test.
