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

/// Listening time for History and Stats: "45 s", "42 min", "3 h 05 min",
/// "2 d 4 h". Unlike formatTotalDuration this never reads as a clock time.
export function formatListenTime(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  if (h < 48) return `${h} h ${String(m % 60).padStart(2, "0")} min`;
  return `${Math.floor(h / 24)} d ${h % 24} h`;
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
