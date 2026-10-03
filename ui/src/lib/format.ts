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

/// Compact fan/listener counts: 999, 9.7K, 12K, 1.1M, 1B.
export function formatCount(n: number): string {
  const value = Math.max(0, Math.floor(n));
  if (value < 1000) return String(value);
  if (value >= 1e9) return compact(value / 1e9, "B");
  if (value >= 1e6) {
    if (Math.round(value / 1e6) >= 1000) return compact(value / 1e9, "B");
    return compact(value / 1e6, "M");
  }
  const thousands = value / 1e3;
  if (Math.round(thousands) >= 1000) return compact(value / 1e6, "M");
  return compact(thousands, "K");
}

function compact(scaled: number, suffix: string): string {
  const rounded = scaled >= 100 ? Math.round(scaled) : Math.round(scaled * 10) / 10;
  return `${rounded}${suffix}`;
}
