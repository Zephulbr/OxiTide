//! Cover-art palette extraction (spec §5.2 three roles, §5.3 contrast chain).
//! Pure functions over a small sRGB bitmap; IO and caching live in artwork.rs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

/// Visual room the palette is extracted for (Track 9 spec §5.2). `Dark` is
/// Warm Graphite; `Light` is its warm-paper counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Room {
    Dark,
    Light,
}

impl Room {
    #[cfg_attr(not(test), expect(dead_code, reason = "the room-scoped cache key lands in Task 7"))]
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
    plain_secondary_l: f32,
    plain_tertiary_l: f32,
}

fn room_consts(room: Room) -> RoomConsts {
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
            plain_secondary_l: 0.35,
            plain_tertiary_l: 0.72,
        },
        Room::Light => RoomConsts {
            bg: (0xf6, 0xf1, 0xea),
            room_avg: (0xee, 0xe7, 0xdd),
            text: (0x2a, 0x21, 0x1a),
            ink: (0xff, 0xff, 0xff),
            scrim: 0.55,
            primary_l: (0.15, 0.24),
            primary_c: (0.08, 0.21),
            secondary_l: (0.80, 0.92),
            secondary_c_cap: 0.04,
            secondary_dark: false,
            tertiary_l: (0.30, 0.52),
            tertiary_c: (0.06, 0.14),
            plain_l: 0.30,
            plain_secondary_l: 0.80,
            plain_tertiary_l: 0.30,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub primary: String,
    pub on_primary: String,
    pub secondary: String,
    pub tertiary: String,
    /// Mean of the blurred cover (pre-scrim): the backdrop wash the player bar
    /// shows. The seek cap re-composites it with the backdrop scrim and the
    /// bar's surface tint, so the cap matches the bar per cover (including
    /// monochrome art, where the three roles fall back).
    pub backdrop: String,
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
fn oklch_to_linear(color: Oklch) -> (f32, f32, f32) {
    let a = color.c * color.h.to_radians().cos();
    let b = color.c * color.h.to_radians().sin();
    let l = (color.l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (color.l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (color.l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )
}

pub(crate) fn oklch_to_srgb(color: Oklch) -> (u8, u8, u8) {
    let (r, g, b) = oklch_to_linear(color);
    let to_u8 = |v: f32| (linear_to_srgb(v.clamp(0.0, 1.0)) * 255.0).round() as u8;
    (to_u8(r), to_u8(g), to_u8(b))
}

/// sRGB gamut test on the unclamped linear channels.
fn oklch_in_gamut(color: Oklch) -> bool {
    let (r, g, b) = oklch_to_linear(color);
    [r, g, b].iter().all(|v| (-0.0005..=1.0005).contains(v))
}

/// The largest chroma at (l, h) that stays inside sRGB, capped at `cap`.
fn gamut_chroma(l: f32, h: f32, cap: f32) -> f32 {
    let (mut lo, mut hi) = (0.0f32, cap.max(0.0));
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if oklch_in_gamut(Oklch { l, c: mid, h }) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
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

/// Below this chroma a candidate carries no usable hue (spec §5.2).
const CHROMA_FLOOR: f32 = 0.04;
/// A candidate at or above this lightness with at most this chroma reads as
/// white and must not drive the interactive Primary (owner ruling 2026-10-02).
const NEAR_WHITE_L: f32 = 0.90;
const NEAR_WHITE_C: f32 = 0.06;
/// A cover whose overall mean chroma is at or below this is fully neutral —
/// shadow-only casts and JPEG chroma noise sit below it — and takes the
/// plain-light Primary; above it the broad cast is boosted into the accent
/// (owner ruling 2026-10-02).
const TINT_FLOOR: f32 = 0.012;

fn composite(pixel: (u8, u8, u8), r: &RoomConsts) -> (u8, u8, u8) {
    let blend = |src: u8, bg: u8| (src as f32 * (1.0 - r.scrim) + bg as f32 * r.scrim).round() as u8;
    (blend(pixel.0, r.bg.0), blend(pixel.1, r.bg.1), blend(pixel.2, r.bg.2))
}

/// Wide box blur approximating the CSS backdrop blur over the already
/// downscaled (~48px) bitmap.
fn blur_cover(pixels: &[(u8, u8, u8)], width: usize, height: usize) -> Vec<(u8, u8, u8)> {
    let radius = (width / 8).max(1);
    box_blur(pixels, width, height, radius)
}

/// Mean of the blurred pixels (the pre-scrim backdrop wash). A solid cover
/// blurs to itself, so its exact value survives.
fn blurred_average(blurred: &[(u8, u8, u8)], r: &RoomConsts) -> (u8, u8, u8) {
    if blurred.is_empty() {
        return r.room_avg;
    }
    let n = blurred.len() as u32;
    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
    for p in blurred {
        r += p.0 as u32;
        g += p.1 as u32;
        b += p.2 as u32;
    }
    ((r / n) as u8, (g / n) as u8, (b / n) as u8)
}

/// Scrim → luminance map → P90 (§5.3 step 2).
fn composite_ceiling(blurred: &[(u8, u8, u8)], r: &RoomConsts) -> f32 {
    let mut ls: Vec<f32> = blurred.iter().map(|&p| luminance(composite(p, r))).collect();
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
    color.c = color.c.min(gamut_chroma(color.l, color.h, color.c));
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
        // Desaturate into gamut at each step: clipping would distort the
        // lightness the search depends on.
        color.c = color.c.min(gamut_chroma(color.l, color.h, color.c));
    }
    None
}

fn is_near_white(color: &Oklch) -> bool {
    color.l >= NEAR_WHITE_L && color.c <= NEAR_WHITE_C
}

/// Pick the on-accent ink with the better contrast against the Primary fill.
fn on_accent(primary: (u8, u8, u8), r: &RoomConsts) -> String {
    if contrast_ratio(r.ink, luminance(primary)) >= contrast_ratio(r.text, luminance(primary)) {
        hex(r.ink)
    } else {
        hex(r.text)
    }
}

/// The identity floor (spec §5.2 degradation paths): brand teal primary,
/// warm-neutral secondary, teal-tint tertiary.
pub fn fallback_palette(room: Room) -> Palette {
    let r = room_consts(room);
    let dark = room == Room::Dark;
    Palette {
        primary: hex(oklch_to_srgb(Oklch {
            l: if dark { 0.81 } else { 0.45 },
            c: 0.12,
            h: 172.0,
        })),
        on_primary: hex(r.ink),
        secondary: hex(oklch_to_srgb(Oklch { l: r.plain_secondary_l, c: 0.02, h: 60.0 })),
        tertiary: hex(oklch_to_srgb(Oklch {
            l: if dark { 0.72 } else { 0.42 },
            c: 0.10,
            h: 172.0,
        })),
        backdrop: hex(r.room_avg),
        ceiling_l: luminance(r.bg),
    }
}

pub fn extract_palette(pixels: &[(u8, u8, u8)], width: usize, height: usize, room: Room) -> Palette {
    let r = room_consts(room);
    if pixels.is_empty() || width == 0 || height == 0 {
        return fallback_palette(room);
    }
    let blurred = blur_cover(pixels, width, height);
    let ceiling = composite_ceiling(&blurred, &r);
    let mean = blurred_average(&blurred, &r);
    let backdrop = hex(mean);
    let candidates = cluster(pixels, 6);

    // Primary: the most chromatic candidate that is neither near-white nor
    // below the chroma floor (spec §5.2). Without one, the cover-wide mean
    // decides: a broad cast above the noise floor keeps the cover's own hue
    // (boosted into the envelope) while a lower mean — shadow-only casts,
    // JPEG chroma noise — keeps a plain-light Primary with neutral
    // Secondary/Tertiary. The backdrop wash still tracks the image itself, so
    // the player bar stays matched either way.
    let usable = candidates
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| !is_near_white(c) && c.c >= CHROMA_FLOOR)
        .reduce(|a, b| if a.c >= b.c { a } else { b });
    let seed = match usable {
        Some(seed) => seed,
        None => {
            let mean = srgb_to_oklch(mean.0, mean.1, mean.2);
            if mean.c > TINT_FLOOR {
                mean
            } else {
                let primary = oklch_to_srgb(Oklch { l: r.plain_l, c: 0.01, h: 60.0 });
                return Palette {
                    primary: hex(primary),
                    on_primary: on_accent(primary, &r),
                    secondary: hex(oklch_to_srgb(Oklch { l: r.plain_secondary_l, c: 0.02, h: 60.0 })),
                    tertiary: hex(oklch_to_srgb(Oklch { l: r.plain_tertiary_l, c: 0.02, h: 60.0 })),
                    backdrop,
                    ceiling_l: ceiling,
                };
            }
        }
    };

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

    // Secondary: the most populous dark candidate, de-chromed (never text).
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
        .unwrap_or(Oklch { l: r.plain_secondary_l, c: 0.02, h: 60.0 });

    // Tertiary: informational only — ≥30° hue separation from Primary, else a
    // rotation so it never reads as interactive (spec §5.2).
    let sep = |a: f32, b: f32| (a - b).abs().min(360.0 - (a - b).abs());
    let tertiary_seed = candidates
        .iter()
        .map(|(c, _)| *c)
        .filter(|c| sep(c.h, primary.h) >= 30.0)
        .reduce(|a, b| if a.c >= b.c { a } else { b })
        // No separated hue in the art: collapse to a Primary tint rather than
        // inventing one (a red cover must not yield a teal badge ring).
        .unwrap_or(Oklch { l: primary.l - 0.10, c: primary.c, h: primary.h });
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

    Palette {
        primary: hex(primary_rgb),
        on_primary,
        secondary: hex(oklch_to_srgb(secondary)),
        tertiary: hex(oklch_to_srgb(tertiary)),
        backdrop,
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

    /// Gamut-aware chroma: a red at L 0.62 can carry the full widened cap,
    /// but the same hue at L 0.85 is desaturated into sRGB instead of clipped.
    #[test]
    fn gamut_chroma_desaturates_instead_of_clipping() {
        let wide = gamut_chroma(0.62, 25.0, 0.21);
        assert!(wide > 0.19, "red at L 0.62 should stay vivid: {wide}");
        let high = gamut_chroma(0.85, 25.0, 0.21);
        assert!(high < 0.16, "red at L 0.85 must desaturate: {high}");
        assert!(oklch_in_gamut(Oklch { l: 0.85, c: high, h: 25.0 }));
        assert!(!oklch_in_gamut(Oklch { l: 0.85, c: 0.21, h: 25.0 }));
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

    /// A monochrome cover with a faint cast keeps its own hue instead of brand
    /// teal: the tint is boosted into the Primary envelope (owner ruling
    /// 2026-10-02).
    #[test]
    fn warm_monochrome_cover_boosts_its_tint() {
        let pixels = solid((120, 110, 95), 48 * 48);
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        assert_ne!(palette.primary, fallback_palette(Room::Dark).primary);
        let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        // The envelope floor is 0.08; 8-bit quantization costs a hair.
        assert!(primary.c >= 0.075, "boosted primary chroma {}", primary.c);
        assert!(hue_diff(primary.h, 78.0) < 15.0, "hue should stay warm: {}", primary.h);
        let tertiary = srgb_to_oklch_tuple(hex_to_rgb(&palette.tertiary));
        assert!(hue_diff(tertiary.h, primary.h) < 10.0, "tertiary should echo Primary");
    }

    #[test]
    fn saturated_cover_drives_a_contrasting_primary() {
        let pixels = solid((200, 30, 40), 48 * 48); // saturated red cover
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        assert_ne!(palette.primary, fallback_palette(Room::Dark).primary);
        let primary = hex_to_rgb(&palette.primary);
        let contrast = contrast_ratio(primary, palette.ceiling_l);
        assert!(contrast >= 4.5, "primary vs ceiling: {contrast}");
    }

    /// Review Focus: bright regions must tighten the clamp (§5.3), but when
    /// 4.5:1 is unreachable the fallback keeps the art's own lightness and
    /// chroma — vibrancy over the unreachable text floor (owner ruling
    /// 2026-10-02) — instead of the old pale L 0.85 tone.
    #[test]
    fn a_bright_ceiling_keeps_a_vivid_primary() {
        let dark = extract_palette(&solid((200, 30, 40), 48 * 48), 48, 48, Room::Dark);
        let mut pixels = solid((200, 30, 40), 48 * 48);
        for p in pixels.iter_mut().take(48 * 24) {
            *p = (250, 245, 238); // top half near-white
        }
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        assert!(palette.ceiling_l > 0.15, "ceiling: {}", palette.ceiling_l);
        assert!(palette.ceiling_l > dark.ceiling_l, "bright ceiling not elevated");
        let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        assert!(!is_near_white(&primary), "bright primary {:?} must stay chromatic", palette.primary);
        assert!(primary.l <= 0.70, "fallback must keep the art's lightness: L {}", primary.l);
        assert!(primary.c >= 0.15, "fallback must stay vivid: C {}", primary.c);
        assert!(hue_diff(primary.h, 30.0) < 30.0, "hue should track the art: {}", primary.h);
    }

    /// White-dominant art with no usable color keeps a plain-light Primary
    /// instead of the brand-teal identity floor (owner ruling 2026-10-02).
    #[test]
    fn pure_white_cover_keeps_a_plain_light_primary() {
        let palette = extract_palette(&solid((255, 255, 255), 48 * 48), 48, 48, Room::Dark);
        let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        assert!(primary.l >= 0.9, "plain-light primary L {}", primary.l);
        assert!(primary.c <= 0.02, "plain-light primary C {}", primary.c);
        assert_ne!(palette.primary, fallback_palette(Room::Dark).primary);
        assert_eq!(palette.on_primary, hex(room_consts(Room::Dark).ink));
        assert_eq!(palette.secondary, fallback_palette(Room::Dark).secondary);
        assert_ne!(palette.tertiary, fallback_palette(Room::Dark).tertiary);
        assert!(srgb_to_oklch_tuple(hex_to_rgb(&palette.tertiary)).c <= 0.03);
    }

    /// A white-dominant cover that still contains a real color uses the color:
    /// near-white candidates only win when nothing else is usable.
    #[test]
    fn white_dominant_cover_with_one_color_uses_the_color() {
        let mut pixels = solid((255, 255, 255), 48 * 48);
        for p in pixels.iter_mut().take((48 * 48) / 20) {
            *p = (30, 80, 220); // 5% vivid blue
        }
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        assert!(!is_near_white(&primary), "primary {:?} must be the blue", palette.primary);
        assert!(primary.c >= 0.08, "primary chroma {}", primary.c);
        assert!(hue_diff(primary.h, 264.0) < 40.0, "hue should be blue: {}", primary.h);
    }

    /// Fully neutral art (no tint above the noise floor) keeps a plain-light
    /// Primary with neutral Secondary/Tertiary — never brand teal.
    #[test]
    fn neutral_monochrome_cover_keeps_a_plain_light_primary() {
        for color in [(128, 128, 128), (30, 30, 30)] {
            let palette = extract_palette(&solid(color, 48 * 48), 48, 48, Room::Dark);
            let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
            assert!(primary.l >= 0.9, "{color:?} primary L {}", primary.l);
            assert!(primary.c <= 0.02, "{color:?} primary C {}", primary.c);
            let tertiary = srgb_to_oklch_tuple(hex_to_rgb(&palette.tertiary));
            assert!(tertiary.c <= 0.03, "{color:?} tertiary C {}", tertiary.c);
            assert_ne!(palette.tertiary, fallback_palette(Room::Dark).tertiary);
        }
        let mut pixels = solid((230, 230, 230), 48 * 48); // mixed neutral greys
        for p in pixels.iter_mut().take((48 * 48) * 2 / 5) {
            *p = (120, 118, 115);
        }
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        assert!(primary.l >= 0.9 && primary.c <= 0.02, "mixed neutral primary {:?}", palette.primary);
    }

    #[test]
    fn faint_cast_below_the_noise_floor_stays_neutral() {
        let palette = extract_palette(&solid((128, 127, 126), 48 * 48), 48, 48, Room::Dark);
        let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        assert!(primary.l >= 0.9 && primary.c <= 0.02, "primary {:?}", palette.primary);
    }

    /// Regression (The Perfect Girl): a bright B&W halftone whose dark dots
    /// carry a faint cast must read as monochrome — the shadow cast is not a
    /// cover-wide tint (owner ruling 2026-10-02).
    #[test]
    fn halftone_shadow_cast_stays_neutral() {
        let mut pixels = solid((245, 245, 245), 48 * 48);
        for p in pixels.iter_mut().take((48 * 48) * 2 / 5) {
            *p = (27, 43, 35); // dark dots with a faint cyan cast
        }
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        let primary = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        assert!(primary.l >= 0.9 && primary.c <= 0.02, "primary {:?}", palette.primary);
        assert_ne!(palette.primary, fallback_palette(Room::Dark).primary);
    }

    /// Spec §5.2: tertiary takes a ≥30° hue only when the art provides one.
    #[test]
    fn tertiary_uses_a_separated_hue_when_available() {
        let mut pixels = solid((200, 30, 40), 48 * 48);
        for p in pixels.iter_mut().take(48 * 24) {
            *p = (30, 80, 220); // blue half
        }
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        let p = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        let t = srgb_to_oklch_tuple(hex_to_rgb(&palette.tertiary));
        assert!(hue_diff(p.h, t.h) >= 30.0, "hues {} vs {}", p.h, t.h);
    }

    /// Monochrome art must not invent a tertiary hue (a red cover used to
    /// rotate to teal); it collapses to a Primary tint instead (owner ruling
    /// 2026-10-02, spec §5.2).
    #[test]
    fn tertiary_collapses_to_a_primary_tint_without_a_separated_hue() {
        let palette = extract_palette(&solid((200, 30, 40), 48 * 48), 48, 48, Room::Dark);
        let p = srgb_to_oklch_tuple(hex_to_rgb(&palette.primary));
        let t = srgb_to_oklch_tuple(hex_to_rgb(&palette.tertiary));
        assert!(hue_diff(p.h, t.h) < 10.0, "tertiary must echo Primary: {} vs {}", p.h, t.h);
        assert_ne!(palette.primary, palette.tertiary);
    }

    #[test]
    fn ceiling_is_the_p90_of_the_composited_bitmap() {
        let mut pixels = solid((0, 0, 0), 48 * 48);
        for p in pixels.iter_mut().take(48 * 12) {
            *p = (255, 255, 255); // quarter white, three quarters black
        }
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        assert!(palette.ceiling_l > 0.2, "P90 should sit in the white quarter");
    }

    #[test]
    fn backdrop_is_the_blurred_cover_mean() {
        let bright = extract_palette(&solid((250, 245, 238), 48 * 48), 48, 48, Room::Dark);
        let dark = extract_palette(&solid((10, 12, 14), 48 * 48), 48, 48, Room::Dark);
        assert_eq!(bright.backdrop, "#faf5ee");
        assert_eq!(dark.backdrop, "#0a0c0e");
        assert!(
            luminance(hex_to_rgb(&bright.backdrop)) > luminance(hex_to_rgb(&dark.backdrop)),
            "a bright cover must yield a lighter backdrop wash"
        );
    }

    /// The seek cap must match the bar even on monochrome art, where the three
    /// roles degrade to the identity fallback (spec §5.2).
    #[test]
    fn monochrome_backdrop_keeps_the_cover_gray() {
        let pixels = solid((110, 105, 100), 48 * 48);
        let palette = extract_palette(&pixels, 48, 48, Room::Dark);
        assert_eq!(palette.backdrop, "#6e6964");
        assert_ne!(palette.backdrop, fallback_palette(Room::Dark).backdrop);
    }

    #[test]
    fn no_art_uses_the_room_tone() {
        assert_eq!(fallback_palette(Room::Dark).backdrop, "#1a1510");
        assert_eq!(extract_palette(&[], 0, 0, Room::Dark).backdrop, "#1a1510");
    }

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
        assert_eq!(Room::Dark.as_str(), "dark");
        assert_eq!(Room::Light.as_str(), "light");
        let light = fallback_palette(Room::Light);
        assert_eq!(light.backdrop, "#eee7dd");
        assert_ne!(light.primary, fallback_palette(Room::Dark).primary);
    }

    /// The pre-refactor dark-room defaults are pinned so the room refactor
    /// cannot drift them (identity fallback and plain-neutral branch).
    #[test]
    fn dark_room_defaults_stay_pinned() {
        let fallback = fallback_palette(Room::Dark);
        assert_eq!(fallback.secondary, "#433830");
        assert_eq!(fallback.tertiary, "#5ab99d");
        let neutral = extract_palette(&solid((128, 128, 128), 48 * 48), 48, 48, Room::Dark);
        assert_eq!(neutral.secondary, fallback.secondary);
        assert_eq!(neutral.tertiary, "#aea298");
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
}
