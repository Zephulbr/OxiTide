export type CoverSize = 80 | 160 | 320 | 640 | 1280;

/// TIDAL serves artwork from an unauthenticated CDN keyed by the dashed
/// uuid exploded into a path. Verified against the live API in the smoke
/// checklist (plan10-smoke.md).
export function coverUrl(uuid: string, size: CoverSize): string {
  if (!uuid) return "";
  return `https://resources.tidal.com/images/${uuid.replace(/-/g, "/")}/${size}x${size}.jpg`;
}

export type VideoCoverSize = "160x90" | "480x270" | "640x360";

/// TIDAL video artwork is 16:9 and lives on the same CDN keyed by `imageId`.
export function videoCoverUrl(uuid: string, size: VideoCoverSize): string {
  if (!uuid) return "";
  return `https://resources.tidal.com/images/${uuid.replace(/-/g, "/")}/${size}.jpg`;
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
