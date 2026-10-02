export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const m = Math.floor(s / 60);
  const r = s % 60;
  return `${m}:${r.toString().padStart(2, "0")}`;
}

/// Total runtime for headers: minutes below an hour, otherwise hours.
export function formatTotalDuration(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  if (s < 3600) return formatDuration(s);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = s % 60;
  return `${h}:${m.toString().padStart(2, "0")}:${r.toString().padStart(2, "0")}`;
}
