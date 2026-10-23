import { SvelteSet } from "svelte/reactivity";
import { api, type LaterKind } from "./api";
import { errorMessage } from "./errors";
import type { Toasts } from "./toasts.svelte";

/// TIDAL's Save for Later list for the signed-in account (spec
/// 2026-10-22-oxitide-save-for-later-collaborators). Saved ids are hydrated
/// once per session, on first use, so menus can label Save / Remove without a
/// request; toggles are optimistic and revert on failure, like `favorites`.
export interface Later {
  readonly hydrated: boolean;
  /// Bumps on every confirmed change, so a mounted page can drop removals.
  readonly revision: number;
  /// Saved videos (shown only as a count; OxiTide plays none).
  readonly videos: number;
  hydrate(): Promise<void>;
  has(kind: LaterKind, id: number | string): boolean;
  count(kind: LaterKind): number;
  toggle(kind: LaterKind, id: number | string): Promise<boolean | null>;
  /// Toasts and the "Open" action target, given once by the app shell.
  connect(deps: { toasts: Toasts; open(): void }): void;
  reset(): void;
}

export function createLater(): Later {
  const sets: Record<LaterKind, SvelteSet<string>> = {
    track: new SvelteSet(),
    album: new SvelteSet(),
    artist: new SvelteSet(),
    playlist: new SvelteSet(),
    mix: new SvelteSet(),
  };
  const pending = new Set<string>();
  /// Keys toggled this session; a hydration snapshot taken earlier must not
  /// override them.
  const touched = new Set<string>();
  let hydrated = $state(false);
  let revision = $state(0);
  let videos = $state(0);
  let hydratePromise: Promise<void> | null = null;
  let generation = 0;
  let toasts: Toasts | null = null;
  let open: (() => void) | null = null;

  async function hydrate(): Promise<void> {
    if (hydrated) return;
    if (hydratePromise) return hydratePromise;
    const gen = generation;
    hydratePromise = (async () => {
      try {
        const ids = await api.getSaveForLaterIds();
        if (gen !== generation) return;
        const fill = (kind: LaterKind, list: readonly (number | string)[]) => {
          for (const id of list) {
            const key = String(id);
            if (!touched.has(`${kind}:${key}`)) sets[kind].add(key);
          }
        };
        fill("track", ids.tracks);
        fill("album", ids.albums);
        fill("artist", ids.artists);
        fill("playlist", ids.playlists);
        fill("mix", ids.mixes);
        videos = ids.videos;
        hydrated = true;
      } catch (e) {
        // Menus fall back to "Save for later"; the page reports its own load.
        if (gen === generation) console.warn("save for later ids failed", e);
      } finally {
        if (gen === generation) hydratePromise = null;
      }
    })();
    return hydratePromise;
  }

  async function toggle(kind: LaterKind, rawId: number | string): Promise<boolean | null> {
    const id = String(rawId);
    const set = sets[kind];
    const key = `${kind}:${id}`;
    if (pending.has(key)) return set.has(id);
    const was = set.has(id);
    const next = !was;
    if (next) set.add(id);
    else set.delete(id);
    pending.add(key);
    try {
      await api.setSaveForLater(kind, id, next);
      touched.add(key);
      revision += 1;
      if (next) {
        const go = open;
        toasts?.show("Saved for later", {
          severity: "success",
          action: go ? { label: "Open", run: go } : undefined,
        });
      } else {
        toasts?.show("Removed from Save for later", {
          severity: "success",
          action: {
            label: "Undo",
            run: () => {
              if (!set.has(id)) void toggle(kind, id);
            },
          },
        });
      }
      return next;
    } catch (e) {
      if (was) set.add(id);
      else set.delete(id);
      toasts?.show(errorMessage(e), { severity: "error" });
      return null;
    } finally {
      pending.delete(key);
    }
  }

  return {
    get hydrated() { return hydrated; },
    get revision() { return revision; },
    get videos() { return videos; },
    hydrate,
    has: (kind, id) => sets[kind].has(String(id)),
    count: (kind) => sets[kind].size,
    toggle,
    connect: (deps) => {
      toasts = deps.toasts;
      open = deps.open;
    },
    reset: () => {
      generation += 1;
      hydratePromise = null;
      for (const set of Object.values(sets)) set.clear();
      pending.clear();
      touched.clear();
      hydrated = false;
      revision = 0;
      videos = 0;
    },
  };
}

export const later = createLater();
