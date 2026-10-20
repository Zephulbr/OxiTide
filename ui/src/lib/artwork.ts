export type CoverSize = 80 | 160 | 320 | 480 | 640 | 1280;

/// Artist pictures 403 at 640 on the CDN; the profileArt pipeline serves
/// 160/320/480/750 (spike: 2026-10-03-artist-page-spike.md). Album covers
/// keep using 640.
export const ARTIST_PICTURE_SIZE: CoverSize = 480;

/// An artist's image: the picture, else the album cover TIDAL falls back to.
/// The fallback is an album image (403 at 480), so it pairs with 640.
export function artistArt(artist: {
  picture?: string | null;
  cover_fallback?: string | null;
}): { uuid: string | null; size: CoverSize } {
  if (artist.picture) return { uuid: artist.picture, size: ARTIST_PICTURE_SIZE };
  return { uuid: artist.cover_fallback ?? null, size: 640 };
}

/// TIDAL serves artwork from an unauthenticated CDN keyed by the dashed
/// uuid exploded into a path. Mix rows from `pages/*` endpoints instead carry
/// a ready-made absolute url (the `images` object); pass those through.
/// Verified against the live API in the smoke checklist (plan10-smoke.md).
export function coverUrl(uuid: string, size: CoverSize): string {
  if (!uuid) return "";
  if (uuid.startsWith("http")) return uuid;
  return `https://resources.tidal.com/images/${uuid.replace(/-/g, "/")}/${size}x${size}.jpg`;
}

/// Pick the tiles for an imageless playlist's collage: album covers in track
/// order, deduplicated, repeat-filled up to `tiles` entries. One distinct
/// album stays a single tile; none yields an empty list (caller falls back).
export function collageCovers(
  covers: (string | null | undefined)[],
  tiles = 4,
): string[] {
  const distinct: string[] = [];
  for (const cover of covers) {
    if (!cover || distinct.includes(cover)) continue;
    distinct.push(cover);
    if (distinct.length === tiles) break;
  }
  if (distinct.length <= 1) return distinct;
  return Array.from({ length: tiles }, (_, i) => distinct[i % distinct.length]);
}
