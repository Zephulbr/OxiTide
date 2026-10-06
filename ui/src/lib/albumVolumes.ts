import type { Track } from "./api";

export interface VolumeSection {
  label: string;
  tracks: Track[];
}

/// Multi-volume albums (spec §6.2): group source-order tracks by
/// `volumeNumber`. Returns null for single-volume albums (or all-zero
/// numbering) so callers keep the flat virtualized table. Volumes appear in
/// first-seen order; zero numbering reads as volume one.
export function volumeSections(tracks: readonly Track[]): VolumeSection[] | null {
  const volumes = new Map<number, Track[]>();
  for (const track of tracks) {
    const volume = track.volumeNumber > 0 ? track.volumeNumber : 1;
    const list = volumes.get(volume);
    if (list) list.push(track);
    else volumes.set(volume, [track]);
  }
  if (![...volumes.keys()].some((volume) => volume > 1)) return null;
  return [...volumes.entries()].map(([volume, list]) => ({
    label: `Volume ${volume}`,
    tracks: list,
  }));
}
