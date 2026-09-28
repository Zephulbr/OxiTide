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
