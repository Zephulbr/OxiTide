import { api, type Track } from "./api";
import type { CardTarget } from "./cardActions";

/// A collection's full track list (album, playlist, or mix), as the card
/// actions and a dropped collection both play or add it.
export async function loadTracks(target: CardTarget): Promise<Track[]> {
  if (target.kind === "album") return api.getAlbumTracks(target.id);
  if (target.kind === "playlist") return api.getPlaylistTracks(target.id);
  return (await api.getMix(target.id)).tracks;
}
