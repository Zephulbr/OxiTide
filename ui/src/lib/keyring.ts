import type { KeyringStatus } from "./api";

export interface KeyringNoticeCopy {
  body: string;
  links: { label: string; url: string }[];
}

/// Copy for the session-only keyring notice, or `null` when persistence is
/// available (or the status has not loaded yet).
export function keyringNotice(status: KeyringStatus | null): KeyringNoticeCopy | null {
  if (!status || status.persistent) return null;
  return {
    body:
      "OxiTide can't reach the system keyring (Secret Service), so your TIDAL login " +
      "will last only for this session and won't be restored after a restart. " +
      "A keyring provider fixes this:",
    links: [
      { label: "gnome-keyring", url: "https://gitlab.gnome.org/GNOME/gnome-keyring" },
      { label: "KWallet", url: "https://invent.kde.org/frameworks/kwallet" },
    ],
  };
}
