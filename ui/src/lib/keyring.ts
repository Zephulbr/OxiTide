import type { KeyringStatus } from "./api";
import { t } from "./i18n.svelte";

export interface KeyringNoticeCopy {
  body: string;
  links: { label: string; url: string }[];
}

/// Copy for the session-only keyring notice, or `null` when persistence is
/// available (or the status has not loaded yet).
export function keyringNotice(status: KeyringStatus | null): KeyringNoticeCopy | null {
  if (!status || status.persistent) return null;
  return {
    body: t("keyring.sessionOnly"),
    links: [
      { label: "gnome-keyring", url: "https://gitlab.gnome.org/GNOME/gnome-keyring" },
      { label: "KWallet", url: "https://invent.kde.org/frameworks/kwallet" },
    ],
  };
}
