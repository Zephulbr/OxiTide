import { keysFor, type TableKind } from "./trackSort";

/// Optional track-table columns (MS-D12), in display order.
export type OptionalColumn = "added" | "released" | "popularity" | "plays";
export const OPTIONAL_COLUMNS: readonly OptionalColumn[] = ["added", "released", "popularity", "plays"];

export const COLUMNS_KEY = "oxitide.track-columns";
/// Shown until the user changes a table kind's columns (where available).
const DEFAULT_COLUMNS: readonly OptionalColumn[] = ["added"];

export interface TrackColumns {
  /// Optional columns this table kind offers.
  available(kind: TableKind): OptionalColumn[];
  /// The user's chosen columns for this kind (sorted keys show regardless).
  chosen(kind: TableKind): OptionalColumn[];
  toggle(kind: TableKind, column: OptionalColumn): void;
}

type ColumnStorage = Pick<Storage, "getItem" | "setItem">;

function load(storage: ColumnStorage): Partial<Record<TableKind, OptionalColumn[]>> {
  try {
    const parsed: unknown = JSON.parse(storage.getItem(COLUMNS_KEY) ?? "{}");
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return {};
    const out: Partial<Record<TableKind, OptionalColumn[]>> = {};
    for (const [kind, value] of Object.entries(parsed)) {
      if (!Array.isArray(value)) continue;
      out[kind as TableKind] = value.filter((column): column is OptionalColumn =>
        OPTIONAL_COLUMNS.includes(column as OptionalColumn),
      );
    }
    return out;
  } catch {
    return {};
  }
}

export function createTrackColumns(storage: ColumnStorage): TrackColumns {
  let choices = $state<Partial<Record<TableKind, OptionalColumn[]>>>(load(storage));

  function available(kind: TableKind): OptionalColumn[] {
    const keys = keysFor(kind);
    return OPTIONAL_COLUMNS.filter((column) => keys.includes(column));
  }

  function chosen(kind: TableKind): OptionalColumn[] {
    const picked = choices[kind] ?? DEFAULT_COLUMNS;
    return available(kind).filter((column) => picked.includes(column));
  }

  return {
    available,
    chosen,
    toggle(kind, column) {
      const current = chosen(kind);
      const next = current.includes(column)
        ? current.filter((c) => c !== column)
        : OPTIONAL_COLUMNS.filter((c) => c === column || current.includes(c));
      choices = { ...choices, [kind]: next };
      storage.setItem(COLUMNS_KEY, JSON.stringify(choices));
    },
  };
}

let singleton: TrackColumns | null = null;

/// In-memory stand-in where `localStorage` is unavailable (some test DOMs).
function memoryStorage(): ColumnStorage {
  const data = new Map<string, string>();
  return { getItem: (key) => data.get(key) ?? null, setItem: (key, value) => void data.set(key, value) };
}

export function trackColumns(): TrackColumns {
  const storage = typeof window !== "undefined" ? window.localStorage : undefined;
  singleton ??= createTrackColumns(storage ?? memoryStorage());
  return singleton;
}
