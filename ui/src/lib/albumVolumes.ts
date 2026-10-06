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

/// One rendered row of a grouped multi-volume table: a heading when `label`
/// is non-null, otherwise a track row. `index` is always a position in the
/// source-order list the `sourceIndex` map was built from, so every index
/// consumer (play, menu, drag) addresses the track the row displays, even
/// when an interleaved payload makes display order differ from source order.
/// `number` is the disc-scoped 1-based display number.
export interface VolumeDisplayRow {
  key: string;
  label: string | null;
  track?: Track;
  index?: number;
  number?: number;
}

/// Interleave volume headings with their source-order tracks. `sourceIndex`
/// maps each track to its position in the source-order list (built once by
/// the caller from its rows).
export function volumeDisplayRows(
  sections: readonly VolumeSection[],
  sourceIndex: ReadonlyMap<Track, number>,
): VolumeDisplayRow[] {
  const rows: VolumeDisplayRow[] = [];
  let display = 0;
  for (const section of sections) {
    rows.push({ key: `section-${section.label}`, label: section.label });
    section.tracks.forEach((track, i) => {
      rows.push({
        key: `${track.id}-${display}`,
        label: null,
        track,
        index: sourceIndex.get(track) ?? 0,
        number: i + 1,
      });
      display += 1;
    });
  }
  return rows;
}
