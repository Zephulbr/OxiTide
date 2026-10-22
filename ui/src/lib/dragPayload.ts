import { api, type Track } from "./api";
import type { CardTarget } from "./cardActions";
import { loadTracks } from "./collectionTracks";

/// Where a drag started, for the refusal rules: a table only reorders its own
/// rows, and nothing is added back to the playlist it came from.
export interface DragSourceRef {
  tableId?: string;
  playlistId?: string;
}

/// What a drag carries (drag-to-playlist spec §6.2). Collections and lone
/// track ids resolve to tracks only on drop (DD-D3).
export type DragPayload =
  | { kind: "tracks"; tracks: Track[]; source?: DragSourceRef }
  | { kind: "track-id"; id: number; title: string; cover?: string | null }
  | { kind: "collection"; target: CardTarget; cover?: string | null };

const COLLECTION_NOUN: Record<CardTarget["kind"], string> = {
  album: "Album",
  playlist: "Playlist",
  mix: "Mix",
};

/// The ghost's label: the track or collection name, or a count.
export function payloadLabel(payload: DragPayload): string {
  switch (payload.kind) {
    case "tracks":
      if (payload.tracks.length === 1) return payload.tracks[0].title;
      return `${payload.tracks.length} tracks`;
    case "track-id":
      return payload.title;
    case "collection":
      return payload.target.title || COLLECTION_NOUN[payload.target.kind];
  }
}

/// Cover ids for the ghost thumbs (up to two, for the stacked look).
export function payloadCovers(payload: DragPayload): (string | null)[] {
  switch (payload.kind) {
    case "tracks":
      return payload.tracks.slice(0, 2).map((t) => t.album?.cover ?? null);
    case "track-id":
    case "collection":
      return [payload.cover ?? null];
  }
}

/// How many tracks the payload holds, when known before resolving.
export function payloadCount(payload: DragPayload): number | null {
  if (payload.kind === "tracks") return payload.tracks.length;
  if (payload.kind === "track-id") return 1;
  return null;
}

/// The playlist the payload came from, if any: a playlist's rows, or the
/// playlist itself dragged from a card.
export function payloadSourcePlaylist(payload: DragPayload): string | null {
  if (payload.kind === "tracks") return payload.source?.playlistId ?? null;
  if (payload.kind === "collection" && payload.target.kind === "playlist") return payload.target.id;
  return null;
}

/// The table a track drag started in, for in-table reorder.
export function payloadSourceTable(payload: DragPayload): string | null {
  return payload.kind === "tracks" ? (payload.source?.tableId ?? null) : null;
}

/// The payload's tracks, fetched now for collections and lone track ids.
export async function resolveTracks(payload: DragPayload): Promise<Track[]> {
  switch (payload.kind) {
    case "tracks":
      return payload.tracks;
    case "track-id":
      return [await api.getTrack(payload.id)];
    case "collection":
      return loadTracks(payload.target);
  }
}
