/// Pointer-to-ratio mapping for the seek control, clamped to [0, 1].
export function seekRatio(clientX: number, left: number, width: number): number {
  if (width <= 0) return 0;
  return Math.min(1, Math.max(0, (clientX - left) / width));
}

/// Keyboard seek target: arrows ±5 s, Page keys ±10 % of the duration (or
/// ±10 s when the duration is unknown), Home/End to the bounds. Returns null
/// for keys the control ignores (End is null while the duration is unknown).
export function seekByKey(key: string, position: number, duration: number): number | null {
  const known = duration > 0;
  const clamp = (value: number) => Math.max(0, known ? Math.min(duration, value) : value);
  const page = known ? Math.round(duration / 10) : 10000;
  switch (key) {
    case "ArrowLeft":
      return clamp(position - 5000);
    case "ArrowRight":
      return clamp(position + 5000);
    case "PageUp":
      return clamp(position + page);
    case "PageDown":
      return clamp(position - page);
    case "Home":
      return 0;
    case "End":
      return known ? duration : null;
    default:
      return null;
  }
}
