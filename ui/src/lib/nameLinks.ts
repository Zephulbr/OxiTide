import type { ArtistRef } from "./api";
import type { Route } from "./nav.svelte";

/// The album/artist the current page shows, so names pointing at it render as
/// plain text instead of linking to themselves (name-links spec NL-D4).
export interface HereTargets {
  albumId: number | null;
  artistId: number | null;
}

export function hereTargets(route: Route): HereTargets {
  return {
    albumId: route.name === "album" ? route.id : null,
    artistId: route.name === "artist" || route.name === "artist-all" ? route.id : null,
  };
}

/// Artists worth a "Go to" menu entry: not the current page, first three
/// (NL-D6), deduplicated by id (some credits repeat an artist).
export function menuArtists(artists: ArtistRef[], here: HereTargets | null): ArtistRef[] {
  const seen = new Set<number>();
  const out: ArtistRef[] = [];
  for (const artist of artists) {
    if (artist.id === here?.artistId || seen.has(artist.id)) continue;
    seen.add(artist.id);
    out.push(artist);
    if (out.length === 3) break;
  }
  return out;
}
