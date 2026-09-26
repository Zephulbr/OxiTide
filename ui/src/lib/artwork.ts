export type CoverSize = 80 | 160 | 320 | 640 | 1280;

/// TIDAL serves artwork from an unauthenticated CDN keyed by the dashed
/// uuid exploded into a path. Verified against the live API in the smoke
/// checklist (plan10-smoke.md).
export function coverUrl(uuid: string, size: CoverSize): string {
  if (!uuid) return "";
  return `https://resources.tidal.com/images/${uuid.replace(/-/g, "/")}/${size}x${size}.jpg`;
}
