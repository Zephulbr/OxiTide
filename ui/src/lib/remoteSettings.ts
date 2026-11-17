/// Pure helpers for Settings → Remote (Track 11).

import { t } from "./i18n.svelte";

/// "just now", "5 min ago", "3 h ago", "2 days ago"; never for future stamps.
export function lastSeenLabel(lastSeenMs: number, nowMs: number): string {
  const age = Math.max(0, nowMs - lastSeenMs);
  const minutes = Math.floor(age / 60_000);
  if (minutes < 1) return t("remoteSettings.seen.now");
  if (minutes < 60) return t("remoteSettings.seen.minutes", { count: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("remoteSettings.seen.hours", { count: hours });
  const days = Math.floor(hours / 24);
  return t("remoteSettings.seen.days", { count: days });
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

/// What Settings says about a detected host firewall (Linux): fine when its
/// rules allow the port, otherwise why a phone needs the port and the
/// command that opens it. Null when no firewall was detected.
export function firewallNotice(
  firewall: { name: string; port_open: boolean | null; command: string } | null,
  port: number,
): { ok: boolean; text: string; command: string | null } | null {
  if (!firewall) return null;
  if (firewall.port_open === true) {
    return { ok: true, text: t("remoteSettings.firewallOpen", { firewall: firewall.name, port }), command: null };
  }
  return {
    ok: false,
    text: t(firewall.port_open === null ? "remoteSettings.firewallUnknown" : "remoteSettings.firewallBlocks", {
      firewall: firewall.name,
      port,
    }),
    command: firewall.command,
  };
}

/// Firewall guidance for the host OS (spec §6.6).
export function firewallHint(userAgent: string, port: number): string {
  if (/linux/i.test(userAgent) && !/android/i.test(userAgent)) {
    return t("remoteSettings.firewallHintLinux", { command: `sudo ufw allow ${port}/tcp` });
  }
  return t("remoteSettings.firewallHintWindows");
}
