/// Pure helpers for Settings → Remote (Track 11).

/// "just now", "5 min ago", "3 h ago", "2 days ago"; never for future stamps.
export function lastSeenLabel(lastSeenMs: number, nowMs: number): string {
  const age = Math.max(0, nowMs - lastSeenMs);
  const minutes = Math.floor(age / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "yesterday" : `${days} days ago`;
}

/// `m:ss` for the pairing QR countdown.
export function countdownLabel(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, "0")}`;
}

/// Port field input → a usable port, or null to keep the current one.
export function parsePort(text: string): number | null {
  const value = Number(text.trim());
  if (!Number.isInteger(value) || value < 1024 || value > 65526) return null;
  return value;
}

/// Firewall guidance for the host OS (spec §6.6).
export function firewallHint(userAgent: string, port: number): string {
  if (/linux/i.test(userAgent) && !/android/i.test(userAgent)) {
    return `If a phone cannot connect, allow the port through your firewall (for example: sudo ufw allow ${port}/tcp).`;
  }
  return "Windows may ask to allow OxiTide through the firewall — choose Private networks. If a phone cannot connect, check Windows Security → Firewall & network protection → Allow an app through firewall.";
}
