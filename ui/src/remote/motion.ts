/// Svelte transition durations that honour `prefers-reduced-motion` (the CSS
/// side does this through the shared `--dur-*` tokens).
export function motion(ms: number): number {
  if (typeof window === "undefined" || !window.matchMedia) return ms;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : ms;
}

/// TIDAL image ids only; mix artwork can be an absolute URL, which the PC's
/// palette analysis does not take.
export function isCoverId(cover: string | null | undefined): cover is string {
  return !!cover && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(cover);
}
