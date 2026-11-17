import type { PlaylistFilter, PlaylistSortKey } from "./playlistSort";
import type { MenuItem } from "./menu.svelte";
import type { PlaylistViewPrefs } from "./playlistView.svelte";
import { t, type MessageKey } from "./i18n.svelte";

export interface PlaylistViewMenuHandlers {
  setSort(key: PlaylistSortKey): void;
  setFilter(filter: PlaylistFilter): void;
}

const SORT_ROWS: readonly { key: PlaylistSortKey; label: MessageKey }[] = [
  { key: "recent", label: "playlists.sort.recent" },
  { key: "created", label: "playlists.sort.created" },
  { key: "updated", label: "playlists.sort.updated" },
  { key: "alpha", label: "playlists.sort.alpha" },
];

const FILTER_ROWS: readonly { key: PlaylistFilter; label: MessageKey }[] = [
  { key: "all", label: "playlists.filter.all" },
  { key: "owned", label: "playlists.filter.owned" },
  { key: "followed", label: "playlists.filter.followed" },
];

/// The sidebar header's combined Sort & Filter menu. The active sort row shows
/// the direction arrow; picking it again flips direction (handled by the
/// prefs store).
export function playlistViewMenuItems(
  prefs: PlaylistViewPrefs,
  handlers: PlaylistViewMenuHandlers,
): MenuItem[] {
  const sortItems = SORT_ROWS.map(({ key, label }): MenuItem => {
    const selected = prefs.sort === key;
    return {
      id: `sort-${key}`,
      label: t(label),
      section: t("playlists.sortSection"),
      selected,
      icon: selected ? "check" : undefined,
      trailing: selected ? (prefs.dir === "desc" ? "arrow-down" : "arrow-up") : undefined,
      run: () => handlers.setSort(key),
    };
  });
  const filterItems = FILTER_ROWS.map(
    ({ key, label }): MenuItem => ({
      id: `filter-${key}`,
      label: t(label),
      section: t("playlists.filterSection"),
      selected: prefs.filter === key,
      icon: prefs.filter === key ? "check" : undefined,
      run: () => handlers.setFilter(key),
    }),
  );
  return [...sortItems, ...filterItems];
}
