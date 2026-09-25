# Plan 11 — Atmosphere Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Light up Atmosphere — the default mode — on top of Plan 10's foundation: the cover-reactive backdrop, dynamic color extraction with the contrast chain, glass and lit edges, the spinning disc, and the waveform seek, all behind the mode flags.

**Architecture:** A pure Rust module (`palette.rs`) extracts the three color roles and the §5.3 composite ceiling from a decoded cover bitmap, fully unit-tested on synthetic bitmaps; `artwork.rs` wraps it with CDN fetch, an in-memory + JSON disk cache, and the `analyze_cover` command (fetch failures degrade to the brand fallback, never error). On the UI side, `dynamicColor.svelte.ts` applies palettes to `@property`-registered tokens with latest-wins semantics so skip-storms interrupt-and-restart natively; `Backdrop.svelte` crossfades two blurred layers. Mode-aware `auto` defaults for typeface and dynamic color resolve in `appearance.svelte.ts`, which also exposes the feature-flag booleans components read (components never branch on mode).

**Tech Stack:** Rust (`image` crate — the only new crate dependency), Svelte 5 runes, TypeScript, vitest, wiremock (already a dev-dependency).

**Spec:** `docs/superpowers/specs/2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (Atmosphere systems: §5 dynamic color + contrast chain, §9 motion, §10 signature treatments, §12 settings) · `docs/superpowers/specs/2026-09-25-oxitide-2b-visual-mode-instrument-design.md` (per-mode defaults) · `docs/superpowers/specs/2026-09-25-oxitide-2b-visual-convergence.md` (build order step 2)

**Prerequisite:** Plan 10 (`2026-09-26-plan10-visual-foundation-instrument.md`) fully landed — tokens, flags, `appearance.svelte.ts`, `Cover.svelte`/`coverUrl`, now-playing view, player bar.

## Global Constraints

- Svelte 5 runes only: no Svelte 4 stores, no `export let`; `verbatimModuleSyntax` (`import type`).
- One new crate dependency only: `image` (default-features off, `jpeg` + `png`) in `oxitide-app`. No new npm packages.
- Components never branch on mode — they read flag booleans from `appearance.svelte.ts` or flag CSS vars; only `modes.css` and `appearance.svelte.ts` know mode names (spec §4.2, Instrument §2).
- Components consume semantic tokens only, never `--dyn-*` directly; the only exceptions are the disc label ring (`--dyn-tertiary`, spec §5.2) and dynamic surfaces whose whole job is the dynamic seed.
- Compositor-only animation: `transform`/`opacity` only; the disc spin is the one rAF exception (transform-only, runs solely while now-playing is open).
- `backdrop-filter` stays on the fixed, countable panel set (sidebar, queue, player, overlays) — never nested or stacked glass (spec §9).
- Ambient motion exists only while playing; `prefers-reduced-motion` forces ambient off and instant crossfades regardless of mode or settings — a hard floor, not a suggestion (spec §12).
- Semantic hues (`--danger`/`--success`/`--warning`) never go dynamic (spec §5.2).
- `analyze_cover` never fails the UI: empty uuid, network error, or undecodable bytes all yield the brand fallback palette (spec §5.2 degradation paths).
- Version stays 0.2.0. Gates: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui test`, `npm --prefix ui run check`.

## Review Focus

- **Skip-storm (rapid track changes):** an older `analyze_cover` response must never overwrite a newer track's palette, and backdrop layers must interrupt-and-restart without flashing a stale cover. Task 4 tests latest-wins; Task 5 smoke spams Next.
- **Monochrome cover art** (no candidate above the chroma floor) must yield the brand-teal fallback — the identity floor holds. Task 2 tests it.
- **Offline playback of cached tracks** (cover fetch fails) must keep the app on the fallback palette with no error UI. Task 3 tests the 404/network path.
- **A cover with bright regions** (high composite ceiling) must clamp Primary/Tertiary to ≥4.5:1 against that ceiling, not against an idealized constant. Task 2 tests the clamp on a half-white bitmap.
- **`prefers-reduced-motion` user on Atmosphere:** crossfades instant (`--dur-mood: 0` from Plan 10's media query), disc static, backdrop still. Task 9 smoke verifies.
- **A config written by Plan 10** (`mode = "instrument"`, `typeface = "manrope"`) must load untouched — those were explicit values then and stay honored; only fresh configs get the new `atmosphere`/`auto` defaults. Task 1 tests it.

---

### Task 1: Atmosphere variant + `auto` defaults in config

**Files:**
- Modify: `crates/oxitide-app/src/config.rs` (UiMode/Typeface/DynamicColor + keys)
- Modify: `crates/oxitide-app/src/commands.rs` (`set_dynamic_color` after the Plan 10 setters)
- Modify: `crates/oxitide-app/src/main.rs` (handler registration)
- Modify: `ui/src/lib/api.ts` (union widenings + `setDynamicColor`)
- Modify: `ui/src/lib/appearance.svelte.ts` + `ui/src/lib/appearance.test.ts` (resolution + flag booleans)

**Interfaces:**
- Consumes: Plan 10 Task 1 enums and Task 6 module.
- Produces: `UiMode::Atmosphere` (new default), `Typeface::Auto` (new default), `DynamicColor { Auto, Full, Accent, Off }` + `UiConfig.dynamic_color`; command `set_dynamic_color`; TS `DynamicColorSetting = "auto" | "full" | "accent" | "off"`; pure `resolveTypeface(typeface, mode)` and `resolveDynamicColor(setting, mode)`; appearance getters `dynamicColor` (resolved), `backdropArt`, `glassPanels`, `grain`, `discSpin`, `waveSeek`, `ambientMotion` — the JS flag surface for Tasks 4–8.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `crates/oxitide-app/src/config.rs`:

```rust
    #[test]
    fn atmosphere_defaults_for_fresh_configs() {
        let config = Config::default();
        assert_eq!(config.ui.mode, UiMode::Atmosphere);
        assert_eq!(config.ui.typeface, Typeface::Auto);
        assert_eq!(config.ui.dynamic_color, DynamicColor::Auto);
    }

    /// Review Focus: Plan 10 wrote explicit values; they stay honored.
    #[test]
    fn plan10_configs_load_untouched() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        fs::write(&path, "[ui]\nmode = \"instrument\"\ntypeface = \"manrope\"\n").unwrap();
        let config = Config::load_from(&path);
        assert_eq!(config.ui.mode, UiMode::Instrument);
        assert_eq!(config.ui.typeface, Typeface::Manrope);
        assert_eq!(config.ui.dynamic_color, DynamicColor::Auto);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dynamic_color_round_trips() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.ui.dynamic_color = DynamicColor::Accent;
        config.save_to(&path).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("dynamic_color = \"accent\""));
        assert_eq!(Config::load_from(&path), config);
        let _ = fs::remove_dir_all(dir);
    }
```

Append to `ui/src/lib/appearance.test.ts`:

```ts
import { resolveDynamicColor, resolveTypeface } from "./appearance.svelte";

describe("resolveTypeface", () => {
  it("auto follows the mode default", () => {
    expect(resolveTypeface("auto", "atmosphere")).toBe("manrope");
    expect(resolveTypeface("auto", "instrument")).toBe("inter");
  });
  it("explicit picks always win", () => {
    expect(resolveTypeface("system", "atmosphere")).toBe("system");
    expect(resolveTypeface("custom", "instrument")).toBe("custom");
  });
});

describe("resolveDynamicColor", () => {
  it("auto is full in atmosphere and off in instrument", () => {
    expect(resolveDynamicColor("auto", "atmosphere")).toBe("full");
    expect(resolveDynamicColor("auto", "instrument")).toBe("off");
  });
  it("explicit picks always win, including off in atmosphere", () => {
    expect(resolveDynamicColor("accent", "atmosphere")).toBe("accent");
    expect(resolveDynamicColor("off", "atmosphere")).toBe("off");
    expect(resolveDynamicColor("full", "instrument")).toBe("full");
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p oxitide-app config; npm --prefix ui test -- src/lib/appearance.test.ts`
Expected: FAIL — `UiMode::Atmosphere`, `Typeface::Auto`, `DynamicColor`, `resolveTypeface` unresolved.

- [ ] **Step 3: Implement the Rust side**

In `config.rs`: add `Atmosphere` to `UiMode` and move the default; add `Auto` to `Typeface` as the default; add the new enum and key:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiMode {
    Instrument,
    #[default]
    Atmosphere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Typeface {
    /// Mode default: Manrope in Atmosphere, Inter in Instrument.
    #[default]
    Auto,
    Manrope,
    Inter,
    System,
    Custom,
}

/// Dynamic cover color (spec §5.4). `Auto` = Full in Atmosphere, Off in Instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DynamicColor {
    #[default]
    Auto,
    Full,
    Accent,
    Off,
}
```

`UiConfig` gains `pub dynamic_color: DynamicColor,` and its `Default` impl gains `dynamic_color: DynamicColor::Auto,` plus `mode: UiMode::Atmosphere,` / `typeface: Typeface::Auto,`. Update the Plan 10 `appearance_defaults_fill_a_pre_plan10_file` test expectations to the new defaults where they changed (`mode` was Instrument for pre-Plan-10 files — keep that expectation; `typeface`/`dynamic_color` fill as `Auto`).

In `commands.rs` (after `set_cover_shape`):

```rust
#[tauri::command]
pub fn set_dynamic_color(state: State<'_, AppState>, mode: DynamicColor) -> Result<(), String> {
    let mut config = state.config();
    config.ui.dynamic_color = mode;
    config.save()?;
    state.set_config(config);
    Ok(())
}
```

Register `commands::set_dynamic_color` in `main.rs`.

- [ ] **Step 4: Implement the TS side**

In `api.ts`: widen `mode` to `"instrument" | "atmosphere"`, `typeface` to include `"auto"`, add `dynamic_color: "auto" | "full" | "accent" | "off";` to the Config `ui` member, and add `setDynamicColor: (mode: "auto" | "full" | "accent" | "off") => invoke<void>("set_dynamic_color", { mode }),` to `api`.

In `appearance.svelte.ts`:

```ts
export type UiMode = "instrument" | "atmosphere";
export type TypefaceSetting = "auto" | "manrope" | "inter" | "system" | "custom";
export type DynamicColorSetting = "auto" | "full" | "accent" | "off";
export type DynamicColorMode = "full" | "accent" | "off";

export function resolveTypeface(t: TypefaceSetting, mode: UiMode): Typeface {
  if (t !== "auto") return t;
  return mode === "atmosphere" ? "manrope" : "inter";
}

export function resolveDynamicColor(s: DynamicColorSetting, mode: UiMode): DynamicColorMode {
  if (s !== "auto") return s;
  return mode === "atmosphere" ? "full" : "off";
}
```

(Rename the existing `Typeface` used by `fontStack` to the resolved union `"manrope" | "inter" | "system" | "custom"` and keep `fontStack` unchanged.) Add state `dynamicColorSetting` (default `"auto"`, hydrated in `init` from `config.ui.dynamic_color`) plus `setDynamicColor` persisting via `api.setDynamicColor`. New getters:

```ts
    get dynamicColor() { return resolveDynamicColor(dynamicColorSetting, mode); },
    get backdropArt() { return mode === "atmosphere"; },
    get glassPanels() { return mode === "atmosphere"; },
    get grain() { return mode === "atmosphere"; },
    get discSpin() { return mode === "atmosphere" && !reducedMotion; },
    get waveSeek() { return mode === "atmosphere"; },
    get ambientMotion() { return mode === "atmosphere" && !reducedMotion; },
```

`apply()` switches to `fontStack(resolveTypeface(typeface, customTypeface-mode-resolved), customTypeface)` — pass `resolveTypeface(typefaceSetting, mode)` so an `auto` pick follows the mode live.

- [ ] **Step 5: Run to verify pass + commit**

Run: `cargo test -p oxitide-app; npm --prefix ui test -- src/lib/appearance.test.ts; npm --prefix ui run check; cargo clippy -p oxitide-app -- -D warnings`
Expected: green.
`git add crates/oxitide-app ui/src/lib; git commit -m "feat(app,ui): add atmosphere mode and auto appearance defaults"`

---

### Task 2: palette.rs — pure color extraction

**Files:**
- Create: `crates/oxitide-app/src/palette.rs`
- Modify: `crates/oxitide-app/src/main.rs` (`mod palette;`)

**Interfaces:**
- Consumes: nothing (pure).
- Produces: `pub struct Palette { pub primary: String, pub on_primary: String, pub secondary: String, pub tertiary: String, pub ceiling_l: f32 }` (hex strings + luminance); `pub fn extract_palette(pixels: &[(u8, u8, u8)], width: usize, height: usize) -> Palette`; `pub fn fallback_palette() -> Palette`; `srgb_to_oklch`/`oklch_to_srgb` (pub(crate) for tests). Task 3 wraps it with IO.

- [ ] **Step 1: Write the failing tests**

Create the `#[cfg(test)]` module content for `palette.rs` (the file itself arrives in Step 3 — write tests first in the new file with only `use super::*;` and stubs commented, run, confirm compile failure, then implement):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn solid(color: (u8, u8, u8), n: usize) -> Vec<(u8, u8, u8)> {
        vec![color; n]
    }

    fn hue_diff(a: f32, b: f32) -> f32 {
        (a - b).abs().min(360.0 - (a - b).abs())
    }

    #[test]
    fn oklch_bounds_for_extremes() {
        let white = srgb_to_oklch(255, 255, 255);
        let black = srgb_to_oklch(0, 0, 0);
        assert!(white.l > 0.99 && white.c < 0.01);
        assert!(black.l < 0.01);
    }

    #[test]
    fn oklch_round_trip_within_a_couple_of_units() {
        for (r, g, b) in [(46, 226, 196), (200, 30, 40), (23, 18, 14), (120, 120, 120)] {
            let back = oklch_to_srgb(srgb_to_oklch(r, g, b));
            assert!((back.0 as i32 - r as i32).abs() <= 3, "r: {back:?} vs {r}");
            assert!((back.1 as i32 - g as i32).abs() <= 3, "g: {back:?} vs {g}");
            assert!((back.2 as i32 - b as i32).abs() <= 3, "b: {back:?} vs {b}");
        }
    }

    /// Review Focus: monochrome art must hit the brand identity floor.
    #[test]
    fn monochrome_cover_yields_the_fallback_palette() {
        let pixels = solid((110, 105, 100), 48 * 48);
        assert_eq!(extract_palette(&pixels, 48, 48), fallback_palette());
    }

    #[test]
    fn saturated_cover_drives_a_contrasting_primary() {
        let pixels = solid((200, 30, 40), 48 * 48); // saturated red cover
        let palette = extract_palette(&pixels, 48, 48);
        assert_ne!(palette.primary, fallback_palette().primary);
        let primary = hex_to_rgb(&palette.primary);
        let contrast = contrast_ratio(primary, palette.ceiling_l);
        assert!(contrast >= 4.5, "primary vs ceiling: {contrast}");
    }

    /// Review Focus: bright regions must tighten the clamp (§5.3).
    #[test]
    fn a_bright_ceiling_forces_a_brighter_or_plainer_primary() {
        let mut pixels = solid((200, 30, 40), 48 * 48);
        for p in pixels.iter_mut().take(48 * 24) {
            *p = (250, 245, 238); // top half near-white
        }
        let palette = extract_palette(&pixels, 48, 48);
        assert!(palette.ceiling_l > 0.25, "ceiling: {}", palette.ceiling_l);
        let contrast = contrast_ratio(hex_to_rgb(&palette.primary), palette.ceiling_l);
        assert!(contrast >= 4.5, "primary vs bright ceiling: {contrast}");
    }

    #[test]
    fn tertiary_stays_thirty_degrees_from_primary() {
        let pixels = solid((200, 30, 40), 48 * 48);
        let palette = extract_palette(&pixels, 48, 48);
        let p = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        let t = srgb_to_oklch_tuple(hex_to_rgb(&palette.tertiary));
        assert!(hue_diff(p.h, t.h) >= 30.0, "hues {} vs {}", p.h, t.h);
    }

    #[test]
    fn ceiling_is_the_p90_of_the_composited_bitmap() {
        let mut pixels = solid((0, 0, 0), 48 * 48);
        for p in pixels.iter_mut().take(48 * 12) {
            *p = (255, 255, 255); // quarter white, three quarters black
        }
        let palette = extract_palette(&pixels, 48, 48);
        assert!(palette.ceiling_l > 0.2, "P90 should sit in the white quarter");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p oxitide-app palette`
Expected: FAIL — module/functions missing.

- [ ] **Step 3: Implement `crates/oxitide-app/src/palette.rs`**

```rust
//! Cover-art palette extraction (spec §5.2 three roles, §5.3 contrast chain).
//! Pure functions over a small sRGB bitmap; IO and caching live in artwork.rs.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub primary: String,
    pub on_primary: String,
    pub secondary: String,
    pub tertiary: String,
    /// P90 luminance of the blurred + scrimmed backdrop (§5.3 composite ceiling).
    pub ceiling_l: f32,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

pub(crate) fn srgb_to_oklch(r: u8, g: u8, b: u8) -> Oklch {
    let r = srgb_to_linear(r as f32 / 255.0);
    let g = srgb_to_linear(g as f32 / 255.0);
    let b = srgb_to_linear(b as f32 / 255.0);
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let big_l = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
    let a = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
    let bb = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
    Oklch {
        l: big_l,
        c: (a * a + bb * bb).sqrt(),
        h: bb.atan2(a).to_degrees().rem_euclid(360.0),
    }
}

pub(crate) fn oklch_to_srgb(color: Oklch) -> (u8, u8, u8) {
    let a = color.c * color.h.to_radians().cos();
    let b = color.c * color.h.to_radians().sin();
    let l = (color.l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (color.l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (color.l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    let to_u8 = |v: f32| (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8;
    (
        to_u8(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
        to_u8(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
        to_u8(-0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s),
    )
}

fn hex(rgb: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
}

#[cfg(test)]
fn hex_to_rgb(value: &str) -> (u8, u8, u8) {
    let v = u32::from_str_radix(value.trim_start_matches('#'), 16).unwrap();
    (((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8)
}

#[cfg(test)]
fn srgb_to_oklch_tuple(rgb: (u8, u8, u8)) -> Oklch {
    srgb_to_oklch(rgb.0, rgb.1, rgb.2)
}

fn luminance(rgb: (u8, u8, u8)) -> f32 {
    0.2126 * srgb_to_linear(rgb.0 as f32 / 255.0)
        + 0.7152 * srgb_to_linear(rgb.1 as f32 / 255.0)
        + 0.0722 * srgb_to_linear(rgb.2 as f32 / 255.0)
}

/// WCAG contrast of a text color against a background luminance (§5.3: the
/// ceiling is a luminance, not a color).
pub(crate) fn contrast_ratio(text: (u8, u8, u8), bg_luminance: f32) -> f32 {
    let tl = luminance(text);
    let (hi, lo) = if tl >= bg_luminance { (tl, bg_luminance) } else { (bg_luminance, tl) };
    (hi + 0.05) / (lo + 0.05)
}

/// The room background (spec §5.1), used for the scrim composite.
const BG: (u8, u8, u8) = (0x17, 0x12, 0x0e);
/// Structural floor (§5.3 step 1): the scrim is at least 55% room.
const SCRIM: f32 = 0.55;

fn composite(pixel: (u8, u8, u8)) -> (u8, u8, u8) {
    let blend = |src: u8, bg: u8| (src as f32 * (1.0 - SCRIM) + bg as f32 * SCRIM).round() as u8;
    (blend(pixel.0, BG.0), blend(pixel.1, BG.1), blend(pixel.2, BG.2))
}

/// Blur → scrim → luminance map → P90 (§5.3 step 2). The input is already
/// downscaled (~48px), so a wide box blur approximates the CSS backdrop blur.
fn composite_ceiling(pixels: &[(u8, u8, u8)], width: usize, height: usize) -> f32 {
    let radius = (width / 8).max(1);
    let blurred = box_blur(pixels, width, height, radius);
    let mut ls: Vec<f32> = blurred.iter().map(|&p| luminance(composite(p))).collect();
    ls.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ls[(ls.len() as f32 * 0.9) as usize].min(1.0)
}

fn box_blur(pixels: &[(u8, u8, u8)], width: usize, height: usize, radius: usize) -> Vec<(u8, u8, u8)> {
    let mut out = vec![(0u8, 0u8, 0u8); pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for dy in -(radius as isize)..=radius as isize {
                for dx in -(radius as isize)..=radius as isize {
                    let (xx, yy) = (x as isize + dx, y as isize + dy);
                    if xx >= 0 && xx < width as isize && yy >= 0 && yy < height as isize {
                        let p = pixels[yy as usize * width + xx as usize];
                        r += p.0 as u32;
                        g += p.1 as u32;
                        b += p.2 as u32;
                        n += 1;
                    }
                }
            }
            out[y * width + x] = ((r / n) as u8, (g / n) as u8, (b / n) as u8);
        }
    }
    out
}

/// k-means (k = 6) in Oklab (l, a, b), deterministic: seeds stride the input.
/// Returns (Oklch centroid, population weight 0..1).
fn cluster(pixels: &[(u8, u8, u8)], k: usize) -> Vec<(Oklch, f32)> {
    let to_lab = |p: (u8, u8, u8)| {
        let c = srgb_to_oklch(p.0, p.1, p.2);
        (c.l, c.c * c.h.to_radians().cos(), c.c * c.h.to_radians().sin())
    };
    let labs: Vec<(f32, f32, f32)> = pixels.iter().map(|&p| to_lab(p)).collect();
    let stride = (labs.len() / k.max(1)).max(1);
    let mut centroids: Vec<(f32, f32, f32)> = (0..k).map(|i| labs[(i * stride) % labs.len()]).collect();
    let mut weights = vec![0f32; k];
    for _ in 0..8 {
        let mut sums = vec![(0f32, 0f32, 0f32, 0u32); k];
        for &p in &labs {
            let mut best = 0;
            let mut best_d = f32::MAX;
            for (i, &c) in centroids.iter().enumerate() {
                let d = (p.0 - c.0).powi(2) + (p.1 - c.1).powi(2) + (p.2 - c.2).powi(2);
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            sums[best].0 += p.0;
            sums[best].1 += p.1;
            sums[best].2 += p.2;
            sums[best].3 += 1;
        }
        for (i, c) in centroids.iter_mut().enumerate() {
            if sums[i].3 > 0 {
                *c = (sums[i].0 / sums[i].3 as f32, sums[i].1 / sums[i].3 as f32, sums[i].2 / sums[i].3 as f32);
            }
            weights[i] = sums[i].3 as f32 / labs.len() as f32;
        }
    }
    centroids
        .iter()
        .zip(weights)
        .map(|(&(l, a, b), w)| {
            (Oklch { l, c: (a * a + b * b).sqrt(), h: b.atan2(a).to_degrees().rem_euclid(360.0) }, w)
        })
        .collect()
}

fn clamp(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

/// Adjust L within the envelope until the role clears `min_contrast` against
/// the per-track ceiling; lighten first (dark-room UI). Returns `None` when
/// the envelope cannot satisfy the floor (§5.3: caller picks a safe color).
fn fit_contrast(mut color: Oklch, lo: f32, hi: f32, ceiling: f32, min_contrast: f32) -> Option<Oklch> {
    for _ in 0..=((hi - lo) / 0.01) as usize {
        let rgb = oklch_to_srgb(color);
        if contrast_ratio(rgb, ceiling) >= min_contrast {
            return Some(color);
        }
        color.l = if contrast_ratio(rgb, ceiling) < min_contrast && luminance(rgb) < ceiling {
            (color.l + 0.01).min(hi)
        } else {
            (color.l - 0.01).max(lo)
        };
    }
    None
}

const TEXT: (u8, u8, u8) = (0xf2, 0xec, 0xe6);
const ON_ACCENT_DARK: (u8, u8, u8) = (0x0b, 0x24, 0x20);

/// The identity floor (spec §5.2 degradation paths): brand teal primary,
/// warm-neutral secondary, teal-tint tertiary.
pub fn fallback_palette() -> Palette {
    Palette {
        primary: hex(oklch_to_srgb(Oklch { l: 0.81, c: 0.12, h: 172.0 })),
        on_primary: hex(ON_ACCENT_DARK),
        secondary: hex(oklch_to_srgb(Oklch { l: 0.35, c: 0.02, h: 60.0 })),
        tertiary: hex(oklch_to_srgb(Oklch { l: 0.72, c: 0.10, h: 172.0 })),
        ceiling_l: luminance(BG),
    }
}

pub fn extract_palette(pixels: &[(u8, u8, u8)], width: usize, height: usize) -> Palette {
    if pixels.is_empty() || width == 0 || height == 0 {
        return fallback_palette();
    }
    let ceiling = composite_ceiling(pixels, width, height);
    let candidates = cluster(pixels, 6);

    // Primary: the most chromatic candidate; below the chroma floor the cover
    // is monochrome and the whole palette falls back (spec §5.2).
    let Some(seed) = candidates.iter().map(|(c, _)| *c).reduce(|a, b| if a.c >= b.c { a } else { b }) else {
        return fallback_palette();
    };
    if seed.c < 0.04 {
        return Palette { ceiling_l: ceiling, ..fallback_palette() };
    }

    let mut primary = Oklch { l: clamp(seed.l, 0.70, 0.85), c: clamp(seed.c, 0.08, 0.17), h: seed.h };
    primary = fit_contrast(primary, 0.70, 0.85, ceiling, 4.5).unwrap_or(Oklch { l: 0.95, c: 0.01, h: seed.h });
    let primary_rgb = oklch_to_srgb(primary);
    let on_primary = if contrast_ratio(ON_ACCENT_DARK, luminance(primary_rgb))
        >= contrast_ratio(TEXT, luminance(primary_rgb))
    {
        hex(ON_ACCENT_DARK)
    } else {
        hex(TEXT)
    };

    // Secondary: the most populous dark candidate, de-chromed (never text).
    let secondary = candidates
        .iter()
        .filter(|(c, _)| c.l < 0.5)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(c, _)| Oklch { l: clamp(c.l, 0.25, 0.45), c: c.c.min(0.05), h: c.h })
        .unwrap_or(Oklch { l: 0.35, c: 0.02, h: 60.0 });

    // Tertiary: informational only — ≥30° hue separation from Primary, else a
    // rotation so it never reads as interactive (spec §5.2).
    let sep = |a: f32, b: f32| (a - b).abs().min(360.0 - (a - b).abs());
    let tertiary_seed = candidates
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| sep(c.h, primary.h) >= 30.0)
        .reduce(|a, b| if a.c >= b.c { a } else { b })
        .unwrap_or(Oklch { h: (primary.h + 150.0).rem_euclid(360.0), ..primary });
    let tertiary = Oklch {
        l: clamp(tertiary_seed.l, 0.65, 0.80),
        c: clamp(tertiary_seed.c.max(0.06), 0.06, 0.14),
        h: tertiary_seed.h,
    };
    let tertiary = fit_contrast(tertiary, 0.65, 0.80, ceiling, 4.5).unwrap_or(tertiary);

    Palette {
        primary: hex(primary_rgb),
        on_primary,
        secondary: hex(oklch_to_srgb(secondary)),
        tertiary: hex(oklch_to_srgb(tertiary)),
        ceiling_l: ceiling,
    }
}
```

Add `mod palette;` to `main.rs` (with the other module declarations).

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p oxitide-app palette`
Expected: PASS (7 tests).

- [ ] **Step 5: Clippy + commit**

Run: `cargo clippy -p oxitide-app -- -D warnings`
`git add crates/oxitide-app/src/palette.rs crates/oxitide-app/src/main.rs; git commit -m "feat(app): add cover palette extraction with the contrast chain"`

---

### Task 3: artwork.rs — fetch, cache, `analyze_cover` command

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml` (`image` dep)
- Create: `crates/oxitide-app/src/artwork.rs`
- Modify: `crates/oxitide-app/src/main.rs` (`mod artwork;` + handler registration)
- Modify: `crates/oxitide-app/src/state.rs` (ArtworkService field)
- Modify: `ui/src/lib/api.ts` (`CoverPalette` + `analyzeCover`)

**Interfaces:**
- Consumes: `palette::extract_palette`/`fallback_palette` (Task 2), `default_cache_dir()` (config.rs), the existing `reqwest` workspace dep.
- Produces: `ArtworkService::new(http: reqwest::Client, base_url: String, disk_path: PathBuf)` and `async fn palette(&self, uuid: &str) -> Palette` (never fails); command `analyze_cover(uuid) -> PaletteDto { primary, on_primary, secondary, tertiary, ceiling_l }`; TS `api.analyzeCover(uuid: string): Promise<CoverPalette>`. Task 4 consumes the TS wrapper.

- [ ] **Step 1: Add the dependency**

In `crates/oxitide-app/Cargo.toml` `[dependencies]`:

```toml
image = { version = "0.25", default-features = false, features = ["jpeg", "png"] }
```

Run `cargo check -p oxitide-app` to fetch and compile it.

- [ ] **Step 2: Write the failing tests**

Create `crates/oxitide-app/src/artwork.rs` with the test module first (implementation stubs in Step 3). Tests use `wiremock` (already a dev-dependency) and encode fixtures with the `image` crate:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn png_bytes(color: [u8; 3]) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(8, 8, image::Rgb(color));
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        buf.into_inner()
    }

    fn service(server: &MockServer, dir: &std::path::Path) -> ArtworkService {
        ArtworkService::new(reqwest::Client::new(), server.uri(), dir.join("palettes.json"))
    }

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("oxitide-artwork-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn fetches_and_extracts_a_real_cover() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
            .expect(1)
            .mount(&server).await;
        let dir = temp_dir();
        let palette = service(&server, &dir).palette("ab-cd").await;
        assert_ne!(palette.primary, fallback_palette().primary);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Review Focus: offline / 404 / garbage must degrade to the fallback.
    #[tokio::test]
    async fn failures_yield_the_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server).await;
        let dir = temp_dir();
        let service = service(&server, &dir);
        assert_eq!(service.palette("missing-uuid").await.primary, fallback_palette().primary);
        assert_eq!(service.palette("").await.primary, fallback_palette().primary);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_memory_cache_serves_repeats_without_http() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
            .expect(1) // second call must not hit the server
            .mount(&server).await;
        let dir = temp_dir();
        let service = service(&server, &dir);
        let first = service.palette("ab-cd").await;
        let second = service.palette("ab-cd").await;
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn the_disk_cache_survives_a_restart() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ab/cd/160x160.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes([200, 30, 40])))
            .expect(1)
            .mount(&server).await;
        let dir = temp_dir();
        let first = service(&server, &dir).palette("ab-cd").await;
        drop(server); // server gone; a fresh service must read the disk cache
        let second = service(&MockServer::start().await, &dir).palette("ab-cd").await;
        assert_eq!(first, second);
        let _ = std::fs::remove_dir_all(dir);
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p oxitide-app artwork`
Expected: FAIL — `ArtworkService` missing.

- [ ] **Step 4: Implement `artwork.rs`**

```rust
//! Cover fetch + palette cache for `analyze_cover` (spec §13: extraction
//! happens off the UI critical path, from already-decoded artwork bytes).
//! Every failure degrades to the brand fallback (spec §5.2) — never errors.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::palette::{extract_palette, fallback_palette, Palette};

const ANALYSIS_SIZE: u32 = 160;
const EXTRACT_SIDE: u32 = 48;

#[derive(Default, Serialize, Deserialize)]
struct DiskCache {
    palettes: HashMap<String, PaletteDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteDto {
    pub primary: String,
    pub on_primary: String,
    pub secondary: String,
    pub tertiary: String,
    pub ceiling_l: f32,
}

impl From<Palette> for PaletteDto {
    fn from(p: Palette) -> Self {
        Self {
            primary: p.primary,
            on_primary: p.on_primary,
            secondary: p.secondary,
            tertiary: p.tertiary,
            ceiling_l: p.ceiling_l,
        }
    }
}
impl From<PaletteDto> for Palette {
    fn from(d: PaletteDto) -> Self {
        Self {
            primary: d.primary,
            on_primary: d.on_primary,
            secondary: d.secondary,
            tertiary: d.tertiary,
            ceiling_l: d.ceiling_l,
        }
    }
}

pub struct ArtworkService {
    http: reqwest::Client,
    base_url: String,
    memory: Mutex<HashMap<String, PaletteDto>>,
    disk_path: PathBuf,
}

impl ArtworkService {
    pub fn new(http: reqwest::Client, base_url: String, disk_path: PathBuf) -> Self {
        let memory = std::fs::read_to_string(&disk_path)
            .ok()
            .and_then(|text| serde_json::from_str::<DiskCache>(&text).ok())
            .map(|cache| cache.palettes)
            .unwrap_or_default();
        Self { http, base_url: base_url.trim_end_matches('/').to_string(), memory: Mutex::new(memory), disk_path }
    }

    fn url(base: &str, uuid: &str) -> String {
        format!("{base}/{}/{ANALYSIS_SIZE}x{ANALYSIS_SIZE}.jpg", uuid.replace('-', "/"))
    }

    /// Never fails: empty uuid, HTTP error, or undecodable bytes all yield
    /// the brand fallback (spec §5.2 degradation paths).
    pub async fn palette(&self, uuid: &str) -> Palette {
        if uuid.is_empty() {
            return fallback_palette();
        }
        if let Some(hit) = self.memory.lock().unwrap().get(uuid) {
            return hit.clone().into();
        }
        let palette = match self.fetch(uuid).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("cover analysis failed for {uuid}: {e}");
                fallback_palette()
            }
        };
        self.memory.lock().unwrap().insert(uuid.to_string(), palette.clone().into());
        self.save_disk();
        palette
    }

    async fn fetch(&self, uuid: &str) -> Result<Palette, String> {
        let bytes = self
            .http
            .get(Self::url(&self.base_url, uuid))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;
        let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
        let small = image::imageops::resize(
            &image.to_rgb8(),
            EXTRACT_SIDE,
            EXTRACT_SIDE,
            image::imageops::FilterType::Triangle,
        );
        let pixels: Vec<(u8, u8, u8)> = small.pixels().map(|p| (p[0], p[1], p[2])).collect();
        Ok(extract_palette(&pixels, EXTRACT_SIDE as usize, EXTRACT_SIDE as usize))
    }

    fn save_disk(&self) {
        let cache = DiskCache { palettes: self.memory.lock().unwrap().clone() };
        let Ok(text) = serde_json::to_string_pretty(&cache) else { return };
        let tmp = self.disk_path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &self.disk_path);
        }
    }
}
```

`Palette` needs `Serialize`/`Deserialize` derives — add them in `palette.rs` (`#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]` + `use serde::{Deserialize, Serialize};`).

- [ ] **Step 5: Wire the state, command, and TS wrapper**

In `state.rs`: add `pub artwork: ArtworkService` to `AppState` (follow the existing field/accessor pattern); construct it in `main.rs` setup with `ArtworkService::new(reqwest::Client::new(), "https://resources.tidal.com/images".into(), default_cache_dir().unwrap_or_default().join("palettes.json"))` (import `default_cache_dir` from config).

In `commands.rs`:

```rust
#[tauri::command]
pub async fn analyze_cover(state: State<'_, AppState>, uuid: String) -> Result<PaletteDto, String> {
    Ok(state.artwork().palette(&uuid).await.into())
}
```

Register `commands::analyze_cover` in `main.rs`. In `api.ts`:

```ts
export interface CoverPalette {
  primary: string;
  on_primary: string;
  secondary: string;
  tertiary: string;
  ceiling_l: number;
}
```

and `analyzeCover: (uuid: string) => invoke<CoverPalette>("analyze_cover", { uuid }),` in the `api` object.

- [ ] **Step 6: Run to verify pass + commit**

Run: `cargo test -p oxitide-app artwork; cargo clippy -p oxitide-app -- -D warnings; npm --prefix ui run check`
Expected: green.
`git add crates/oxitide-app ui/src/lib/api.ts; git commit -m "feat(app): add analyze_cover with fetch and a two-level palette cache"`

---

### Task 4: dynamicColor.svelte.ts — palette application with latest-wins

**Files:**
- Create: `ui/src/lib/dynamicColor.svelte.ts`
- Create: `ui/src/lib/dynamicColor.test.ts`
- Modify: `ui/src/styles/tokens.css` (crossfade transition)
- Modify: `ui/src/App.svelte` (create the watcher)

**Interfaces:**
- Consumes: `api.analyzeCover`/`CoverPalette` (Task 3), `appearance.dynamicColor` (Task 1), Plan 10's `@property` tokens.
- Produces: `paletteTokens(palette, mode)` (mode-token mapping, spec §5.4), `createPaletteApplier(apply, fetcher)` (latest-wins), `createDynamicColor(player)` (effect wiring). Tasks 5+ rely on the `--dyn-*` tokens it sets.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/dynamicColor.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createPaletteApplier, paletteTokens, type CoverPalette } from "./dynamicColor.svelte";

const palette: CoverPalette = {
  primary: "#ff0000",
  on_primary: "#000000",
  secondary: "#222222",
  tertiary: "#00ff00",
  ceiling_l: 0.1,
};

describe("paletteTokens", () => {
  it("off applies nothing", () => {
    expect(paletteTokens(palette, "off")).toEqual({});
  });
  it("accent-only applies primary and on-primary (spec §5.4)", () => {
    expect(paletteTokens(palette, "accent")).toEqual({
      "--dyn-primary": "#ff0000",
      "--dyn-on-primary": "#000000",
    });
  });
  it("full applies all four roles", () => {
    expect(paletteTokens(palette, "full")).toEqual({
      "--dyn-primary": "#ff0000",
      "--dyn-on-primary": "#000000",
      "--dyn-secondary": "#222222",
      "--dyn-tertiary": "#00ff00",
    });
  });
});

describe("createPaletteApplier", () => {
  function deferred<T>() {
    let resolve!: (v: T) => void;
    return { promise: new Promise<T>((r) => (resolve = r)), resolve };
  }

  /// Review Focus: a stale response must never clobber a newer track.
  it("ignores an older response that resolves after a newer track", async () => {
    const applied: Record<string, string>[] = [];
    const slow = deferred<CoverPalette>();
    const fast = deferred<CoverPalette>();
    const fetcher = (uuid: string) => (uuid === "slow" ? slow.promise : fast.promise);
    const applier = createPaletteApplier((t) => applied.push(t), fetcher);

    void applier.onTrack("slow", "full");
    void applier.onTrack("fast", "full");
    fast.resolve({ ...palette, primary: "#111111" });
    await Promise.resolve();
    slow.resolve({ ...palette, primary: "#999999" });
    await Promise.resolve();

    expect(applied).toEqual([expect.objectContaining({ "--dyn-primary": "#111111" })]);
  });

  it("off and missing covers clear the tokens synchronously", async () => {
    const applied: Record<string, string>[] = [];
    const applier = createPaletteApplier((t) => applied.push(t), () => Promise.resolve(palette));
    await applier.onTrack(null, "full");
    await applier.onTrack("uuid", "off");
    expect(applied).toEqual([{}, {}]);
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `npm --prefix ui test -- src/lib/dynamicColor.test.ts`
Expected: FAIL — cannot resolve `./dynamicColor.svelte`.

- [ ] **Step 3: Implement `ui/src/lib/dynamicColor.svelte.ts`**

```ts
import { api, type CoverPalette as ApiPalette } from "./api";
import { appearance } from "./appearance.svelte";
import type { Player } from "./player.svelte";

export type CoverPalette = ApiPalette;
export type DynamicColorApply = Record<string, string>;

/// Token mapping per dynamic-color mode (spec §5.4): accent-only follows the
/// cover for Primary but keeps Secondary/Tertiary at brand; off is static.
export function paletteTokens(p: CoverPalette, mode: "full" | "accent" | "off"): DynamicColorApply {
  if (mode === "off") return {};
  const base = { "--dyn-primary": p.primary, "--dyn-on-primary": p.on_primary };
  if (mode === "accent") return base;
  return { ...base, "--dyn-secondary": p.secondary, "--dyn-tertiary": p.tertiary };
}

/// Latest-wins application: a new call supersedes any in-flight fetch, so
/// skip-storms never apply a stale palette (spec §5.2 interrupt-and-restart).
export function createPaletteApplier(
  apply: (tokens: DynamicColorApply) => void,
  fetcher: (uuid: string) => Promise<CoverPalette>,
) {
  let request = 0;
  return {
    async onTrack(cover: string | null, mode: "full" | "accent" | "off") {
      const id = ++request;
      if (mode === "off" || !cover) {
        apply({});
        return;
      }
      try {
        const palette = await fetcher(cover);
        if (id !== request) return;
        apply(paletteTokens(palette, mode));
      } catch {
        // Keep the current colors; the next track retries.
      }
    },
  };
}

function applyToDocument(tokens: DynamicColorApply) {
  const style = document.documentElement.style;
  for (const key of ["--dyn-primary", "--dyn-on-primary", "--dyn-secondary", "--dyn-tertiary"]) {
    const value = tokens[key];
    if (value) style.setProperty(key, value);
    else style.removeProperty(key);
  }
}

export function createDynamicColor(player: Player) {
  const applier = createPaletteApplier(applyToDocument, api.analyzeCover);
  $effect(() => {
    const cover = player.state.current?.cover ?? null;
    const mode = appearance.dynamicColor;
    void applier.onTrack(cover, mode);
  });
}
```

- [ ] **Step 4: Add the crossfade to `tokens.css`**

Append to `:root`:

```css
  /* Dynamic-color crossfade (spec §5.2): @property-registered tokens
     interpolate natively; a new value interrupts the in-flight transition
     and starts from the current interpolated color. */
  transition:
    --dyn-primary var(--dur-mood) linear,
    --dyn-on-primary var(--dur-mood) linear,
    --dyn-secondary var(--dur-mood) linear,
    --dyn-tertiary var(--dur-mood) linear;
```

- [ ] **Step 5: Wire in `App.svelte`**

Import `createDynamicColor` from `"./lib/dynamicColor.svelte"` and call it once at the top level of the component script (next to `const player = createPlayer();`):

```ts
  createDynamicColor(player);
```

The internal `$effect` watches `player.state.current?.cover` and simply never fires a fetch while nothing is playing; calling it at component init is the standard Svelte 5 pattern (no `loggedIn` gate — `analyze_cover` needs no session).

- [ ] **Step 6: Run to verify pass + commit**

Run: `npm --prefix ui test; npm --prefix ui run check`
Expected: green.
`git add ui/src/lib/dynamicColor.svelte.ts ui/src/lib/dynamicColor.test.ts ui/src/styles/tokens.css ui/src/App.svelte; git commit -m "feat(ui): apply cover palettes with latest-wins dynamic color"`

---

### Task 5: Backdrop.svelte — blurred crossfade, scrim, grain

**Files:**
- Create: `ui/src/lib/components/Backdrop.svelte`
- Modify: `ui/src/styles/modes.css` (atmosphere overrides)
- Modify: `ui/src/App.svelte` (render backdrop + grain)

**Interfaces:**
- Consumes: `coverUrl(uuid, 640)` (Plan 10 Task 5), `appearance.backdropArt` (Task 1), flag CSS vars (Plan 10 Task 2).
- Produces: the fixed backdrop layer (two crossfading `<img>`s, blur, ≥55% scrim) and the grain overlay; `[data-mode="atmosphere"]` token/flag values. Glass panel work (Task 6) consumes `--glass-blur`.

- [ ] **Step 1: Create `Backdrop.svelte`**

```svelte
<script lang="ts">
  import { coverUrl } from "../artwork";
  import type { Player } from "../player.svelte";

  let { player }: { player: Player } = $props();

  // Two stacked layers: the new cover loads on the inactive one, then the
  // active flag flips and the opacity transition runs (spec §5.2 crossfade).
  let layers = $state<[string, string]>(["", ""]);
  let active = $state<0 | 1>(0);

  $effect(() => {
    const cover = player.state.current?.cover ?? null;
    const url = cover ? coverUrl(cover, 640) : "";
    if (url === layers[active]) return;
    const next = active === 0 ? 1 : 0;
    layers[next] = url;
    active = next;
  });
</script>

<div class="backdrop" aria-hidden="true">
  {#each [0, 1] as i}
    {#if layers[i]}
      <img class:active={active === i} src={layers[i]} alt="" draggable="false" />
    {/if}
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 0;
    overflow: hidden;
    background: linear-gradient(160deg, #1d1712, var(--bg)); /* no-art room (§5.2) */
    opacity: var(--flag-backdrop-art);
  }
  .backdrop img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: blur(80px) saturate(1.15);
    transform: scale(1.25); /* hide the blurred edge falloff */
    opacity: 0;
    transition: opacity var(--dur-mood) linear;
  }
  .backdrop img.active { opacity: 1; }
  /* Structural floor (§5.3): the scrim never drops below 55%. */
  .backdrop::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(23, 18, 14, 0.55), rgba(23, 18, 14, 0.78));
  }
</style>
```

- [ ] **Step 2: Atmosphere overrides in `modes.css`**

```css
[data-mode="atmosphere"] {
  --surface-1: rgba(255, 240, 225, 0.05);
  --surface-2: rgba(255, 240, 225, 0.07);
  --surface-3: rgba(255, 240, 225, 0.09);
  --glass-blur: 18px;
  --flag-backdrop-art: 1;
  --flag-glass-blur: 1;
  --flag-grain: 1;
  --flag-disc-spin: 1;
  --flag-wave-seek: 1;
  --flag-dynamic-color: 1;
  --flag-ambient-motion: 1;
}
```

- [ ] **Step 3: Render backdrop + grain in `App.svelte`**

Import `Backdrop` and place it as a sibling before `.app`, plus the grain overlay after `.app` (grain sits over the backdrop but under toasts; panels keep their own backgrounds):

```svelte
  <Backdrop {player} />
  <div class="app" data-sidebar={sidebarRail ? "rail" : "full"}> … </div>
  <div class="grain" aria-hidden="true"></div>
```

`.app` needs `position: relative; z-index: 1;` (add to `app.css`) so it stacks over the fixed backdrop. Add to `app.css`:

```css
/* Film grain: one static feTurbulence tile at a fixed ~4.5% (spec §10.1 —
   deliberately never a user setting). */
.grain {
  position: fixed;
  inset: 0;
  z-index: 15;
  pointer-events: none;
  opacity: calc(var(--flag-grain) * 0.045);
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='160' height='160' filter='url(%23n)'/%3E%3C/svg%3E");
}
```

- [ ] **Step 4: Verify + smoke**

Run: `npm --prefix ui run check`
Smoke (requires `mode = "atmosphere"` in config or the Task 9 switch — do this smoke after Task 9 and record it there; the interim check is `npm run check` + a temporary `data-mode` flip in DevTools): blurred cover behind the app, scrim keeps panels readable, grain barely visible.

- [ ] **Step 5: Commit**

`git add ui/src/lib/components/Backdrop.svelte ui/src/styles/modes.css ui/src/App.svelte ui/src/app.css; git commit -m "feat(ui): add the cover backdrop with scrim and grain"`

---

### Task 6: Glass panels — blur and lit edges on the fixed set

**Files:**
- Modify: `ui/src/app.css` (`.sidebar`/`.queue`/`.player` glass)
- Modify: `ui/src/lib/components/ToastHost.svelte`, `Modal.svelte`, `ContextMenu.svelte` (blur on the Plan 10 surfaces)

**Interfaces:**
- Consumes: `--glass-blur` (0px Instrument / 18px Atmosphere, Task 5) and Plan 10's `.lit` recipe.
- Produces: the fixed glass set — sidebar, queue rail, player bar, toasts, dialogs, context menu — with `backdrop-filter` only there (spec §9 rule).

- [ ] **Step 1: Apply the glass recipe**

In `app.css`, add `backdrop-filter: blur(var(--glass-blur));` to `.sidebar`, `.queue`, `.player`, and `.queue.floating`. In the overlay components (Plan 10 gave them `--surface-3` backgrounds + `.lit`), add the same `backdrop-filter` line: `.toast`, Modal's panel, `.context-menu`. The playing-row/selection tints (`color-mix` accents from Plan 10) need no change — they sit over glass and pick up the mood for free.

Badges (spec §8): in `PlayerBar.svelte` and `NowPlaying.svelte`, the MAX-quality chip gets `border-color: var(--dyn-tertiary); color: var(--dyn-tertiary);` (informational emphasis — early mockups showing it in Primary are superseded); the BIT-PERFECT and sample-rate chips stay muted. Match the chip condition to the compact label: tertiary styling only when `formatQuality(..., "compact", ...) === "MAX"`.

- [ ] **Step 2: Verify + smoke**

Run: `npm --prefix ui run check`
Smoke: in Atmosphere the sidebar/queue/player/toasts/dialogs frost over the backdrop; in Instrument everything stays solid with zero blur (DevTools: `backdrop-filter: blur(0px)` computes but renders nothing — Track 3 measures; the flag is the documented kill-switch if it ever costs).

- [ ] **Step 3: Commit**

`git add ui/src; git commit -m "feat(ui): frost the fixed panel set in atmosphere"`

---

### Task 7: DiscArt.svelte — disc shape, grooves, sheen, eased spin

**Files:**
- Create: `ui/src/lib/components/DiscArt.svelte`
- Modify: `ui/src/views/NowPlaying.svelte` (shape switch)

**Interfaces:**
- Consumes: `Cover` (Plan 10), `appearance.discSpin`/`coverShape`/`reducedMotion` (Task 1), `player.state.playing`.
- Produces: `<DiscArt uuid playing />` — circular artwork with groove + sheen overlays and the tertiary label ring (spec §7.2, §10.1); spins at ~12s/rev only while playing, easing to a stop in ~600ms; static when reduced-motion or Instrument.

- [ ] **Step 1: Create `DiscArt.svelte`**

```svelte
<script lang="ts">
  import { appearance } from "../appearance.svelte";
  import Cover from "./Cover.svelte";

  let { uuid, playing }: { uuid: string | null | undefined; playing: boolean } = $props();
  let disc: HTMLDivElement;

  // One transform-only rAF loop while mounted; speed eases toward the target
  // (~30°/s ≈ 12s/rev, a design value per spec §7.2) with a ~150ms time
  // constant, so pause settles in about 600ms instead of stopping dead.
  $effect(() => {
    let raf = 0;
    let last = performance.now();
    let angle = 0;
    let speed = 0;
    const tick = (now: number) => {
      const dt = Math.min((now - last) / 1000, 0.1);
      last = now;
      const target = playing && appearance.ambientMotion ? 30 : 0;
      speed += (target - speed) * Math.min(1, dt / 0.15);
      if (Math.abs(speed) < 0.001) speed = 0;
      angle = (angle + speed * dt) % 360;
      disc.style.transform = `rotate(${angle}deg)`;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });
</script>

<div class="disc-wrap">
  <div class="disc" bind:this={disc}>
    <Cover {uuid} size={640} radius={0} alt="" />
    <div class="grooves"></div>
    <div class="sheen"></div>
    <div class="label-ring"></div>
  </div>
</div>

<style>
  .disc-wrap { display: grid; place-items: center; }
  .disc {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 50%;
    overflow: hidden;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
    will-change: transform;
  }
  .grooves {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: repeating-radial-gradient(circle, rgba(0, 0, 0, 0.22) 0 1px, transparent 1px 4px);
  }
  .sheen {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: conic-gradient(from 200deg, transparent 0deg, rgba(255, 255, 255, 0.10) 40deg, transparent 100deg);
  }
  /* Tertiary ring: informational emphasis (spec §5.2). */
  .label-ring {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 24%;
    aspect-ratio: 1;
    transform: translate(-50%, -50%);
    border-radius: 50%;
    border: 3px solid var(--dyn-tertiary);
    background: var(--bg);
  }
</style>
```

- [ ] **Step 2: Switch the shape in `NowPlaying.svelte`**

Replace the unconditional sleeve `<Cover>` with:

```svelte
    {#if appearance.coverShape === "disc" && appearance.discSpin}
      <DiscArt uuid={track.cover} playing={player.state.playing} />
    {:else}
      <button class="art-sleeve lit" onclick={() => nav.closeNowPlaying()} aria-label="Close now playing">
        <Cover uuid={track.cover} size={640} radius={12} alt="{track.title} artwork" />
      </button>
    {/if}
```

Import `appearance` and `DiscArt`. (The disc reads as decorative while the sleeve stays the explicit close affordance; spec §7.2's "clicking artwork returns" applies to the sleeve.)

- [ ] **Step 3: Verify + smoke**

Run: `npm --prefix ui run check`
Smoke: disc spins only while playing and eases to a stop on pause; sleeve mode and reduced-motion render a static sleeve; grooves/sheen read as a record without competing with the title.

- [ ] **Step 4: Commit**

`git add ui/src/lib/components/DiscArt.svelte ui/src/views/NowPlaying.svelte; git commit -m "feat(ui): add the spinning disc to expanded now playing"`

---

### Task 8: Waveform seek

**Files:**
- Create: `ui/src/lib/wave.ts`
- Create: `ui/src/lib/wave.test.ts`
- Modify: `ui/src/lib/components/PlayerBar.svelte` (swap the hairline for the wave in Atmosphere)

**Interfaces:**
- Consumes: `appearance.waveSeek` (Task 1), Plan 11 Task 4's accent tokens, the Plan 10 seek pointer logic.
- Produces: `waveSamples(seed, count): number[]` and `wavePath(seed, width, height, count?): string` (deterministic stylized wave — true peaks are a later upgrade, spec §10.2); the seek strip crossfades hairline ↔ wave on hover, played portion in accent.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/wave.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { wavePath, waveSamples } from "./wave";

describe("waveSamples", () => {
  it("is deterministic for a seed", () => {
    expect(waveSamples(42, 32)).toEqual(waveSamples(42, 32));
  });
  it("varies with the seed", () => {
    expect(waveSamples(1, 32)).not.toEqual(waveSamples(2, 32));
  });
  it("stays within 0..1", () => {
    for (const v of waveSamples(7, 96)) {
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThanOrEqual(1);
    }
  });
});

describe("wavePath", () => {
  it("builds an svg path inside the viewbox", () => {
    const path = wavePath(42, 800, 14);
    expect(path.startsWith("M")).toBe(true);
    const coords = path.match(/-?\d+\.?\d*/g)!.map(Number);
    const ys = coords.filter((_, i) => i % 2 === 1);
    for (const y of ys) {
      expect(y).toBeGreaterThanOrEqual(0);
      expect(y).toBeLessThanOrEqual(14);
    }
  });
});
```

- [ ] **Step 2: Run to verify failure**

Run: `npm --prefix ui test -- src/lib/wave.test.ts`
Expected: FAIL — cannot resolve `./wave`.

- [ ] **Step 3: Implement `ui/src/lib/wave.ts`**

```ts
/// Stylized wave (spec §10.2): a soft deterministic line derived from the
/// track id — deliberately not real peak data (that upgrade is deferred).

export function waveSamples(seed: number, count: number): number[] {
  const p1 = (seed % 628) / 100;
  const p2 = (seed % 314) / 50;
  const p3 = (seed % 157) / 25;
  const out: number[] = [];
  for (let i = 0; i < count; i++) {
    const x = count <= 1 ? 0 : i / (count - 1);
    const v =
      Math.sin(x * 6.3 + p1) * 0.5 +
      Math.sin(x * 11.9 + p2) * 0.3 +
      Math.sin(x * 23.7 + p3) * 0.2;
    out.push(Math.min(1, Math.abs(v)));
  }
  return out;
}

export function wavePath(seed: number, width: number, height: number, count = 96): string {
  const samples = waveSamples(seed, count);
  const step = count <= 1 ? 0 : width / (count - 1);
  return samples
    .map((v, i) => {
      const y = height - v * height * 0.9 - height * 0.05;
      return `${i === 0 ? "M" : "L"}${(i * step).toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
}
```

- [ ] **Step 4: Run to verify pass**

Run: `npm --prefix ui test -- src/lib/wave.test.ts`
Expected: PASS (4 tests).

- [ ] **Step 5: Integrate into the PlayerBar seek**

In `PlayerBar.svelte`, inside the `.seek` container, add the wave variant rendered when `appearance.waveSeek` (import `appearance` and `wavePath`):

```svelte
    {#if appearance.waveSeek}
      {@const pct = (position / Math.max(duration, 1)) * 100}
      <svg class="seek-wave" viewBox="0 0 800 14" preserveAspectRatio="none" aria-hidden="true">
        <path d={wavePath(current.track_id, 800, 14)} fill="none" stroke="var(--line)" stroke-width="1.5" vector-effect="non-scaling-stroke" />
        <clipPath id="seek-played"><rect x="0" y="0" width="{pct}%" height="100%" /></clipPath>
        <path d={wavePath(current.track_id, 800, 14)} fill="none" stroke="var(--accent)" stroke-width="1.5" vector-effect="non-scaling-stroke" clip-path="url(#seek-played)" />
      </svg>
    {/if}
```

Behavior: at rest the wave renders as a thin soft line (the `.seek` strip stays 12px tall; the svg sits at its top edge, 14px viewBox scaled to ~10px visible). On `.seek:hover`/drag the hairline scales up — wrap the svg in a container that transitions `opacity` (0.35 → 1) and `transform: scaleY(1)` from `scaleY(0.4)` with `transform-origin: top` — transform/opacity only (Global Constraints). The flat hairline from Plan 10 remains the Instrument rendering (`appearance.waveSeek` false). `clipPath` `id` needs uniqueness if several seeks ever mount — PlayerBar is a singleton; fine.

- [ ] **Step 6: Verify + smoke**

Run: `npm --prefix ui test; npm --prefix ui run check`
Smoke: Atmosphere shows the soft wave with the played portion in accent, following seek drags; the wave changes per track; Instrument keeps the flat hairline.

- [ ] **Step 7: Commit**

`git add ui/src/lib/wave.ts ui/src/lib/wave.test.ts ui/src/lib/components/PlayerBar.svelte; git commit -m "feat(ui): add the stylized waveform seek"`

---

### Task 9: Settings mode switch, docs, and smoke

**Files:**
- Modify: `ui/src/views/Settings.svelte`
- Modify: `AGENTS.md`
- Create: `docs/superpowers/plans/notes/plan11-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the full Settings → Appearance surface (spec §12) and the recorded smoke pass.

- [ ] **Step 1: Finish the Settings surface**

In the Appearance section from Plan 10 Task 7:

- Replace the Instrument-only mode control with the two-way segmented **Mode**: Atmosphere / Instrument → `appearance.setMode`. Under Instrument add the muted note "Instrument minimizes animation and visual effects." (Instrument doc §8).
- Add **Dynamic color**: segmented Default / Full / Accent-only / Off → `appearance.setDynamicColor` (maps to `auto/full/accent/off`), per spec §5.4 ("Accent-only = Primary follows the cover, Secondary/Tertiary stay static brand").
- Typeface select gains a **Default (per mode)** option mapped to `"auto"`.

- [ ] **Step 2: Update `AGENTS.md`**

Under the `ui/` architecture notes: modes are `atmosphere` (default) / `instrument` via `data-mode`; dynamic color flows `analyze_cover` (Rust, `artwork.rs` + pure `palette.rs`, cached in `cache/palettes.json`) → `--dyn-*` `@property` tokens; JS flag booleans live on `appearance` (`discSpin`, `waveSeek`, …) — components must not branch on mode; Hi-Fi Rack is deferred (convergence doc §2).

- [ ] **Step 3: Write `docs/superpowers/plans/notes/plan11-smoke.md`**

```markdown
# Plan 11 smoke — Atmosphere mode

Requires: real account, browser login, `npm --prefix ui run app:dev`. If the
config still says `mode = "instrument"` from Plan 10, switch Mode to
Atmosphere in Settings once (also set Typeface to Default).

- [ ] Mode switch live-applies: Atmosphere (blur backdrop, glass, wave seek) ↔ Instrument (flat, solid, hairline) with no restart.
- [ ] Backdrop follows the playing cover with a 1200ms crossfade; no artwork → the static warm room gradient.
- [ ] **Skip-storm:** spam Next 6+ times — colors and backdrop drift continuously, never flash a stale cover, and settle on the final track.
- [ ] Accent follows the cover: play button, active nav, seek fill, playing-row tint, focus rings.
- [ ] **Monochrome cover** (e.g. a grayscale album): brand teal accent, app keeps its identity.
- [ ] **Bright cover:** title/playing-row accent stays readable (≥4.5:1) over the bright blur.
- [ ] Dynamic color Accent-only: accent follows the cover, backdrop/scrollbar stay brand; Off: fully static brand.
- [ ] Disc spins only while playing, eases to a stop on pause; Sleeve setting shows the static sleeve.
- [ ] Wave seek: played portion fills accent, wave changes per track.
- [ ] Instrument + Dynamic color Full: accent recolors work on the flat surfaces.
- [ ] Reduced motion (Windows setting on): instant recolors, static disc, no backdrop crossfade — in both modes.
- [ ] Offline replay of cached tracks: fallback teal palette, no error toasts.
- [ ] `cache/palettes.json` exists and grows; restart with network off still colors previously-seen covers from the disk cache.
```

- [ ] **Step 4: Run the full gates**

Run: `cargo test --workspace; cargo clippy --workspace -- -D warnings; npm --prefix ui test; npm --prefix ui run check`
Expected: all green.

- [ ] **Step 5: Commit**

`git add ui/src/views/Settings.svelte AGENTS.md docs/superpowers/plans/notes/plan11-smoke.md; git commit -m "feat(ui): ship the atmosphere mode switch and smoke checklist"`
