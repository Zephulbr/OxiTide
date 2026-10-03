/// Canonical TIDAL web links for the Share action.
export type ShareKind = "playlist" | "album" | "mix" | "artist";

export function shareUrl(kind: ShareKind, id: string | number): string {
  return `https://tidal.com/${kind}/${id}`;
}
