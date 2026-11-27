import type { CollectionEntry } from "./api";
import { formsMatch, searchForms, searchNeedle } from "./searchText";

/// The Playlists page's filter (issue #14): a query searches the page's
/// folder and every folder beneath it, so a playlist filed in a folder is
/// found from All playlists.

/// One entry the walk found, with the folder it sits in.
export interface FoundEntry {
  entry: CollectionEntry;
  /// The folder holding it ("root": in no folder).
  parentId: string;
  /// That folder's name; null for the folder the walk started in.
  parentName: string | null;
}

export interface CollectionWalk {
  found: FoundEntry[];
  /// False when a folder failed to load or the caps below cut the walk short:
  /// a match may be missing.
  complete: boolean;
}

/// Bounds a broken or enormous tree: folders fetched per walk, and levels
/// below the start (the Move dialog lists folders five deep).
export const MAX_WALK_FOLDERS = 200;
export const MAX_WALK_DEPTH = 5;
/// Folder requests in flight at once.
export const WALK_CONCURRENCY = 6;

interface Level {
  id: string;
  name: string | null;
  entries: readonly CollectionEntry[];
}

/// Every entry in the start folder (already loaded by the page) and in the
/// folders beneath it, one level at a time. A folder that fails to load is
/// skipped, and the walk reports itself incomplete.
export async function walkCollection(
  start: Level,
  fetchFolder: (folderId: string) => Promise<CollectionEntry[]>,
): Promise<CollectionWalk> {
  const found: FoundEntry[] = [];
  const seen = new Set<string>([start.id]);
  let complete = true;
  let fetched = 0;
  let level: Level[] = [start];
  for (let depth = 0; level.length > 0; depth++) {
    const next: { id: string; name: string }[] = [];
    for (const folder of level) {
      for (const entry of folder.entries) {
        found.push({ entry, parentId: folder.id, parentName: folder.name });
        if (entry.kind === "folder" && !seen.has(entry.id)) {
          seen.add(entry.id);
          next.push({ id: entry.id, name: entry.name });
        }
      }
    }
    if (next.length === 0) break;
    if (depth >= MAX_WALK_DEPTH) {
      complete = false;
      break;
    }
    const batch = next.slice(0, Math.max(0, MAX_WALK_FOLDERS - fetched));
    if (batch.length < next.length) complete = false;
    fetched += batch.length;
    level = [];
    for (let i = 0; i < batch.length; i += WALK_CONCURRENCY) {
      const chunk = batch.slice(i, i + WALK_CONCURRENCY);
      const results = await Promise.allSettled(chunk.map((folder) => fetchFolder(folder.id)));
      results.forEach((result, j) => {
        if (result.status === "fulfilled") level.push({ ...chunk[j], entries: result.value });
        else complete = false;
      });
    }
  }
  return { found, complete };
}

/// The key a grid cell is keyed by: folders and playlists have separate ids.
export function entryKey(entry: CollectionEntry): string {
  return entry.kind === "folder" ? `f:${entry.id}` : `p:${entry.uuid}`;
}

/// Found entries whose name matches `query` (case, accents and punctuation
/// folded, `lib/searchText.ts`), each once, in walk order. Empty for a query
/// with nothing to match.
export function matchCollection(found: readonly FoundEntry[], query: string): FoundEntry[] {
  const needle = searchNeedle(query);
  if (needle === "") return [];
  const seen = new Set<string>();
  return found.filter(({ entry }) => {
    const key = entryKey(entry);
    if (seen.has(key)) return false;
    const name = entry.kind === "folder" ? entry.name : entry.title;
    if (!formsMatch(searchForms(name), needle)) return false;
    seen.add(key);
    return true;
  });
}
