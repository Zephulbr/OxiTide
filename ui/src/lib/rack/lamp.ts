import type { CoverPalette, LampColor } from "../api";

/// A lamp colour is an OKLCH hue (degrees) + chroma; lightness is fixed per
/// role in rack.css, so any choice still reads as a lit screen (RK-D8).
export interface Lamp {
  h: number;
  c: number;
}

export const LAMP_PRESETS: Record<Exclude<LampColor, "cover">, Lamp> = {
  amber: { h: 64, c: 0.16 },
  blue: { h: 235, c: 0.14 },
  green: { h: 165, c: 0.15 },
  red: { h: 27, c: 0.19 },
  white: { h: 80, c: 0.025 },
};

/// Cover-derived lamps use one chroma; anything greyer than this falls back
/// to amber instead of a muddy glow.
export const COVER_CHROMA = 0.15;
export const COVER_MIN_CHROMA = 0.05;

/// `#rrggbb` → [L, C, h] in OKLCH (h in degrees). Null for anything else.
export function hexToOklch(hex: string): [number, number, number] | null {
  const m = /^#([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  const lin = (v: number) => {
    const s = v / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  };
  const r = lin((n >> 16) & 255);
  const g = lin((n >> 8) & 255);
  const b = lin(n & 255);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const mm = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const L = 0.2104542553 * l + 0.793617785 * mm - 0.0040720468 * s;
  const A = 1.9779984951 * l - 2.428592205 * mm + 0.4505937099 * s;
  const B = 0.0259040371 * l + 0.7827717662 * mm - 0.808675766 * s;
  const h = ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360;
  return [L, Math.hypot(A, B), h];
}

/// OKLCH → linear-clamped sRGB channels in 0..1.
export function oklchToSrgb(L: number, C: number, h: number): [number, number, number] {
  const a = C * Math.cos((h * Math.PI) / 180);
  const b = C * Math.sin((h * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const lin = [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ];
  return lin.map((v) => {
    const c = Math.min(1, Math.max(0, v));
    return c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055;
  }) as [number, number, number];
}

export function relativeLuminance([r, g, b]: [number, number, number]): number {
  const lin = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

export function contrastRatio(a: [number, number, number], b: [number, number, number]): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/// The lamp for a cover palette: the hue of its most chromatic role at a fixed
/// chroma; near-grey covers (or no palette) keep amber.
export function coverLamp(palette: CoverPalette | null | undefined): Lamp {
  if (!palette) return LAMP_PRESETS.amber;
  const best = [palette.primary, palette.secondary, palette.tertiary, palette.backdrop]
    .map((hex) => hexToOklch(hex))
    .filter((v): v is [number, number, number] => v !== null)
    .sort((x, y) => y[1] - x[1])[0];
  if (!best || best[1] < COVER_MIN_CHROMA) return LAMP_PRESETS.amber;
  return { h: Math.round(best[2] * 10) / 10, c: COVER_CHROMA };
}

/// Lamp brightness (`--lamp-dim`, 0.4–1) only pulls lit lightness down by 0.2
/// at the floor, so dimmed VFD text still clears 4.5:1 (mirrors rack.css).
export function vfdLightness(dim: number): number {
  return 0.6 + (dim - 0.4) / 3;
}

export function lampFor(setting: LampColor, palette: CoverPalette | null | undefined): Lamp {
  return setting === "cover" ? coverLamp(palette) : LAMP_PRESETS[setting];
}
