import type { Playlist } from "./api";
import type { DropZone } from "./dragDrop.svelte";
import { payloadSourcePlaylist, resolveTracks } from "./dragPayload";
import type { PlaylistActions } from "./playlists.svelte";
import { t } from "./i18n.svelte";

/// The drop-zone key of a playlist, shared by its tree row and its grid card
/// so both show the same target and pending state.
export function playlistDropKey(uuid: string): string {
  return `pl:${uuid}`;
}

/// A playlist as a drop target (drag-to-playlist spec §4.2): owned playlists
/// take the dragged tracks through `addTracks`; someone else's playlist, or
/// the one the tracks came from, refuses with the reason (DD-D6). Tracks are
/// fetched only on drop (DD-D3); more than one asks first (DD-D15).
export function playlistDropZone(
  playlist: Pick<Playlist, "uuid" | "title" | "owned">,
  actions: PlaylistActions,
  open: () => void,
): DropZone {
  return {
    key: playlistDropKey(playlist.uuid),
    accepts(payload) {
      if (!playlist.owned) return { ok: false, reason: t("playlist.drop.notEditable") };
      if (payloadSourcePlaylist(payload) === playlist.uuid) return { ok: false, reason: t("playlist.drop.alreadyHere") };
      return { ok: true, hint: t("playlist.drop.addTo", { playlist: playlist.title }) };
    },
    drop: (payload) =>
      actions.addTracks(playlist, () => resolveTracks(payload), {
        onOpen: open,
        confirm: true,
        from: payload.kind === "collection" ? payload.target.title || undefined : undefined,
      }),
  };
}
