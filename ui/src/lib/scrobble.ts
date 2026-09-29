import type { ScrobbleServiceStatus } from "./api";

export function serviceLabel(
  status: Pick<ScrobbleServiceStatus, "configured" | "connected" | "username" | "needs_reconnect">,
): string {
  if (!status.configured) return "API key not configured";
  if (status.needs_reconnect) return "Reconnect needed";
  if (status.connected) return `Connected as ${status.username ?? "unknown"}`;
  return "Not connected";
}

export function canToggle(status: Pick<ScrobbleServiceStatus, "connected">): boolean {
  return status.connected;
}

export function queuedLabel(count: number): string {
  return count > 0 ? `${count} queued` : "";
}
