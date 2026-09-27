//! Cover-art palette extraction (spec §5.2 three roles, §5.3 contrast chain).
//! Pure functions over a small sRGB bitmap; IO and caching live in artwork.rs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[allow(clippy::excessive_precision)]
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

#[allow(clippy::excessive_precision)]
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

/// Adjust L within the envelope until the role clears min_contrast against the
/// per-track ceiling, moving away from the ceiling. Returns None when the
/// envelope cannot satisfy the floor (§5.3: caller picks a safe color).
fn fit_contrast(mut color: Oklch, lo: f32, hi: f32, ceiling: f32, min_contrast: f32) -> Option<Oklch> {
    for _ in 0..=((hi - lo) / 0.01) as usize {
        let rgb = oklch_to_srgb(color);
        if contrast_ratio(rgb, ceiling) >= min_contrast {
            return Some(color);
        }
        color.l = if luminance(rgb) < ceiling {
            (color.l - 0.01).max(lo)
        } else {
            (color.l + 0.01).min(hi)
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

    /// Review Focus: monochrome art must hit the brand identity floor. The
    /// degradation replaces the three color roles; the per-cover ceiling is a
    /// separate quantity and is intentionally retained (controller ruling R5).
    #[test]
    fn monochrome_cover_yields_the_fallback_roles() {
        let pixels = solid((110, 105, 100), 48 * 48);
        let palette = extract_palette(&pixels, 48, 48);
        let fb = fallback_palette();
        assert_eq!(palette.primary, fb.primary);
        assert_eq!(palette.on_primary, fb.on_primary);
        assert_eq!(palette.secondary, fb.secondary);
        assert_eq!(palette.tertiary, fb.tertiary);
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
    /// Controller ruling R6: with SCRIM = 0.55 the maximum composite ceiling is
    /// ~0.206 and a half-white cover yields ~0.190, where 4.5:1 is mathematically
    /// unreachable even for pure white text (~4.37:1); the spec also pins Primary
    /// to L ∈ [0.70, 0.85] (no dark accent). So this stress case asserts the
    /// achievable intent — an elevated ceiling and a plainer/brighter accent —
    /// not a literal 4.5:1 (documented spec gap, surfaced to the owner).
    #[test]
    fn a_bright_ceiling_forces_a_brighter_or_plainer_primary() {
        let dark = extract_palette(&solid((200, 30, 40), 48 * 48), 48, 48);
        let mut pixels = solid((200, 30, 40), 48 * 48);
        for p in pixels.iter_mut().take(48 * 24) {
            *p = (250, 245, 238); // top half near-white
        }
        let palette = extract_palette(&pixels, 48, 48);
        assert!(palette.ceiling_l > 0.15, "ceiling: {}", palette.ceiling_l);
        assert!(palette.ceiling_l > dark.ceiling_l, "bright ceiling not elevated");
        let bright_lum = luminance(hex_to_rgb(&palette.primary));
        let dark_lum = luminance(hex_to_rgb(&dark.primary));
        assert!(
            bright_lum > dark_lum,
            "bright primary {:?} (lum {bright_lum}) must be plainer/brighter than dark {:?} (lum {dark_lum})",
            palette.primary,
            dark.primary
        );
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
