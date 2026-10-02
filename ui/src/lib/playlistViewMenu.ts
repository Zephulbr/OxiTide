import type { PlaylistFilter, PlaylistSortKey } from "./playlistSort";
import type { MenuItem } from "./menu.svelte";
import type { PlaylistViewPrefs } from "./playlistView.svelte";

export interface PlaylistViewMenuHandlers {
  setSort(key: PlaylistSortKey): void;
  setFilter(filter: PlaylistFilter): void;
}

const SORT_ROWS: readonly { key: PlaylistSortKey; label: string }[] = [
  { key: "recent", label: "Recently played" },
  { key: "created", label: "Created date" },
  { key: "updated", label: "Updated date" },
  { key: "alpha", label: "Alphabetical" },
];

const FILTER_ROWS: readonly { key: PlaylistFilter; label: string }[] = [
  { key: "all", label: "All playlists" },
  { key: "owned", label: "Your playlists" },
  { key: "followed", label: "Followed playlists" },
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
      label,
      section: "Sort",
      selected,
      trailing: selected ? (prefs.dir === "desc" ? "arrow-down" : "arrow-up") : undefined,
      run: () => handlers.setSort(key),
    };
  });
  const filterItems = FILTER_ROWS.map(
    ({ key, label }): MenuItem => ({
      id: `filter-${key}`,
      label,
      section: "Filter",
      selected: prefs.filter === key,
      run: () => handlers.setFilter(key),
    }),
  );
  return [...sortItems, ...filterItems];
}
