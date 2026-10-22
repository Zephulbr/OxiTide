import { SvelteSet } from "svelte/reactivity";

/// Modifier keys of a selecting click (Ctrl stands for Meta on macOS too).
export interface SelectMods {
  ctrl: boolean;
  shift: boolean;
}

/// Row multi-select keyed by row identity, not index or id: playlists repeat
/// track ids, and indices move on sort/filter while row objects stay stable
/// across pager appends (drag-to-playlist spec DD-D4). Order always comes
/// from the caller's current `items`.
export interface Selection<T> {
  readonly size: number;
  has(item: T): boolean;
  /// Plain: select only `item`. Ctrl: toggle it. Shift: the range from the
  /// anchor to `item` (replacing the selection, or adding with Ctrl+Shift).
  click(item: T, items: readonly T[], mods: SelectMods): void;
  /// Shift+arrow: the range from the anchor to `item`.
  extend(item: T, items: readonly T[]): void;
  /// Space: toggle one row, making it the anchor.
  toggle(item: T): void;
  selectAll(items: readonly T[]): void;
  clear(): void;
  /// The selected items in `items` order; rows no longer present drop out.
  ordered(items: readonly T[]): T[];
  /// Indices into `items` of the selected rows.
  indices(items: readonly T[]): number[];
}

/// The rows from `a` to `b` inclusive, in `items` order, whichever comes
/// first. Empty when either is missing.
export function rangeBetween<T>(items: readonly T[], a: T, b: T): T[] {
  const from = items.indexOf(a);
  const to = items.indexOf(b);
  if (from < 0 || to < 0) return [];
  return items.slice(Math.min(from, to), Math.max(from, to) + 1);
}

export function createSelection<T>(): Selection<T> {
  const selected = new SvelteSet<T>();
  let anchor: T | null = null;

  function only(item: T) {
    selected.clear();
    selected.add(item);
    anchor = item;
  }

  function toggle(item: T) {
    if (selected.has(item)) selected.delete(item);
    else selected.add(item);
    anchor = item;
  }

  /// Select the anchor→item range; an anchor that left the list (filtered,
  /// reloaded) starts over at `item`.
  function range(item: T, items: readonly T[], add: boolean) {
    const rows = anchor === null ? [] : rangeBetween(items, anchor, item);
    if (rows.length === 0) {
      if (add) toggle(item);
      else only(item);
      return;
    }
    if (!add) selected.clear();
    for (const row of rows) selected.add(row);
  }

  return {
    get size() {
      return selected.size;
    },
    has: (item) => selected.has(item),
    click(item, items, mods) {
      if (mods.shift) range(item, items, mods.ctrl);
      else if (mods.ctrl) toggle(item);
      else only(item);
    },
    extend: (item, items) => range(item, items, false),
    toggle,
    selectAll(items) {
      selected.clear();
      for (const item of items) selected.add(item);
      anchor = items[0] ?? null;
    },
    clear() {
      selected.clear();
      anchor = null;
    },
    ordered: (items) => items.filter((item) => selected.has(item)),
    indices(items) {
      const out: number[] = [];
      items.forEach((item, index) => {
        if (selected.has(item)) out.push(index);
      });
      return out;
    },
  };
}
