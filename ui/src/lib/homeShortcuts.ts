import type { Mix, PageItem, Playlist } from "./api";
import { t } from "./i18n.svelte";

// Desktop only: the phone remote imports lib/media.ts, never the app catalog.

export interface ShortcutEntry {
  key: string;
  item: PageItem;
  playlist: boolean;
  route: { name: "playlist"; id: string } | { name: "mix"; id: string };
}

/// Home's quick-access strip: owned playlists first, then saved mixes.
export function shortcutEntries(
  playlists: Playlist[],
  mixes: Mix[],
  cap = 6,
): ShortcutEntry[] {
  const entries: ShortcutEntry[] = [];
  for (const p of playlists) {
    entries.push({
      key: `playlist-${p.uuid}`,
      playlist: true,
      route: { name: "playlist", id: p.uuid },
      item: {
        id: p.uuid,
        title: p.title,
        subtitle: t("route.playlist"),
        item_type: "PLAYLIST_LIST",
        artwork: p.squareImage ?? p.image ?? null,
      },
    });
  }
  for (const m of mixes) {
    entries.push({
      key: `mix-${m.id}`,
      playlist: false,
      route: { name: "mix", id: m.id },
      item: {
        id: m.id,
        title: m.title,
        subtitle: t("card.savedMix"),
        item_type: "MIX_LIST",
        artwork: m.image ?? null,
      },
    });
  }
  return entries.slice(0, cap);
}
