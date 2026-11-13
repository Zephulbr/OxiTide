import type { ScrobbleServiceStatus } from "./api";
import { t } from "./i18n.svelte";

export function serviceLabel(
  status: Pick<ScrobbleServiceStatus, "configured" | "connected" | "username" | "needs_reconnect">,
): string {
  if (!status.configured) return t("scrobble.notConfigured");
  if (status.needs_reconnect) return t("scrobble.reconnect");
  if (status.connected) return t("scrobble.connectedAs", { name: status.username ?? t("scrobble.unknownUser") });
  return t("player.notConnected");
}

export function canToggle(status: Pick<ScrobbleServiceStatus, "connected">): boolean {
  return status.connected;
}

export function queuedLabel(count: number): string {
  return count > 0 ? t("scrobble.queued", { count }) : "";
}
