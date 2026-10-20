export const FOLDER_META_KEY = "oxitide.folder-meta";
/// The colours-only key this store replaced; read once as a fallback.
const LEGACY_COLORS_KEY = "oxitide.folder-colors";

/// The folder palette: a hue each, drawn through the theme's `--folder-l` /
/// `--folder-c` (tokens.css) so every colour stays readable in both themes.
export const FOLDER_COLORS = [
  { id: "red", label: "Red", hue: 25 },
  { id: "orange", label: "Orange", hue: 60 },
  { id: "yellow", label: "Yellow", hue: 95 },
  { id: "green", label: "Green", hue: 150 },
  { id: "teal", label: "Teal", hue: 190 },
  { id: "blue", label: "Blue", hue: 250 },
  { id: "purple", label: "Purple", hue: 300 },
  { id: "pink", label: "Pink", hue: 350 },
] as const;

export type FolderColorId = (typeof FOLDER_COLORS)[number]["id"];

/// Longest description kept (the edit field enforces it too).
export const DESCRIPTION_MAX = 300;

interface Meta {
  color?: FolderColorId;
  description?: string;
}

type MetaStorage = Pick<Storage, "getItem" | "setItem">;

export interface FolderMeta {
  /// The folder's hue, or null for the default (muted) folder.
  hue(folderId: string): number | null;
  color(folderId: string): FolderColorId | null;
  setColor(folderId: string, color: FolderColorId | null): void;
  /// The folder's description, or "" when it has none.
  description(folderId: string): string;
  setDescription(folderId: string, text: string): void;
}

const VALID = new Set<string>(FOLDER_COLORS.map((c) => c.id));

function parse(raw: string | null): unknown {
  try {
    return JSON.parse(raw ?? "{}");
  } catch {
    return {};
  }
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

function load(storage: MetaStorage): Record<string, Meta> {
  const meta: Record<string, Meta> = {};
  const stored = parse(storage.getItem(FOLDER_META_KEY));
  if (isRecord(stored)) {
    for (const [id, value] of Object.entries(stored)) {
      if (!isRecord(value)) continue;
      const entry: Meta = {};
      if (typeof value.color === "string" && VALID.has(value.color)) entry.color = value.color as FolderColorId;
      if (typeof value.description === "string" && value.description.trim() !== "") {
        entry.description = value.description.slice(0, DESCRIPTION_MAX);
      }
      if (entry.color || entry.description) meta[id] = entry;
    }
  }
  const legacy = parse(storage.getItem(LEGACY_COLORS_KEY));
  if (isRecord(legacy)) {
    for (const [id, value] of Object.entries(legacy)) {
      if (typeof value === "string" && VALID.has(value) && !meta[id]?.color) {
        meta[id] = { ...meta[id], color: value as FolderColorId };
      }
    }
  }
  return meta;
}

/// Folder colour and description, local to this device: TIDAL folders carry
/// only a name, so these live in localStorage keyed by folder id.
export function createFolderMeta(storage: MetaStorage): FolderMeta {
  let meta = $state<Record<string, Meta>>(load(storage));

  function update(id: string, change: Meta) {
    const entry: Meta = { ...meta[id], ...change };
    if (!entry.color) delete entry.color;
    if (!entry.description) delete entry.description;
    const next = { ...meta };
    if (entry.color || entry.description) next[id] = entry;
    else delete next[id];
    meta = next;
    storage.setItem(FOLDER_META_KEY, JSON.stringify(meta));
  }

  return {
    color: (id) => meta[id]?.color ?? null,
    hue: (id) => FOLDER_COLORS.find((c) => c.id === meta[id]?.color)?.hue ?? null,
    setColor: (id, color) => update(id, { color: color ?? undefined }),
    description: (id) => meta[id]?.description ?? "",
    setDescription: (id, text) => update(id, { description: text.trim().slice(0, DESCRIPTION_MAX) || undefined }),
  };
}

let singleton: FolderMeta | null = null;

export function folderMeta(): FolderMeta {
  singleton ??= createFolderMeta(globalThis.localStorage ?? memoryStorage());
  return singleton;
}

/// Where localStorage is unavailable (some test environments): the values
/// still work for the session, they just are not kept.
function memoryStorage(): MetaStorage {
  const items = new Map<string, string>();
  return { getItem: (key) => items.get(key) ?? null, setItem: (key, value) => void items.set(key, value) };
}

/// A palette hue as a CSS colour, through the theme's folder knobs.
export function folderColorCss(hue: number): string {
  return `oklch(var(--folder-l) var(--folder-c) ${hue})`;
}

