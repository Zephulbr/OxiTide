import { hexToOklch } from "./rack/lamp";

/// How hard the artist hero's scrim works (0..1, the `--scrim` multiplier).
///
/// The hero paints its local palette as a background, but the roles are tuned
/// as *text* colours against the room: light in the dark theme, dark in the
/// light one — the same side as the hero's own text. So the scrim scales with
/// how far the palette sits on that side: a deep palette in the dark theme gets
/// only the floor (kept for the blurred photo, whose brightness is unknown), a
/// pale one the full scrim. The worst role decides, since the gradient spans
/// all three.

/// OKLCH lightness where the scrim starts to ramp, and where it is full.
const RAMP: Record<"dark" | "light", [number, number]> = {
  dark: [0.45, 0.8],
  light: [0.85, 0.55],
};
export const SCRIM_FLOOR = 0.3;
/// No palette (still loading, failed, or no picture): a middle strength.
export const SCRIM_UNKNOWN = 0.6;

export function heroScrim(colors: string[] | null, theme: "dark" | "light"): number {
  const ls = (colors ?? [])
    .map((color) => hexToOklch(color)?.[0])
    .filter((l): l is number => l !== undefined);
  if (ls.length === 0) return SCRIM_UNKNOWN;
  const worst = theme === "dark" ? Math.max(...ls) : Math.min(...ls);
  const [start, full] = RAMP[theme];
  const t = (worst - start) / (full - start);
  return Math.max(SCRIM_FLOOR, Math.min(1, t));
}
