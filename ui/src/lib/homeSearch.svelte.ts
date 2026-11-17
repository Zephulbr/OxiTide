/// Whether Home shows its search field (spec 2026-11-17 SD-D4). On by
/// default; UI-only, so it lives in localStorage like the sort memory.
export const HOME_SEARCH_KEY = "oxitide.home-search";

type FlagStorage = Pick<Storage, "getItem" | "setItem">;

export interface HomeSearch {
  readonly shown: boolean;
  setShown(shown: boolean): void;
}

export function createHomeSearch(storage: FlagStorage | null): HomeSearch {
  let stored: string | null = null;
  try {
    stored = storage?.getItem(HOME_SEARCH_KEY) ?? null;
  } catch {
    // Unavailable storage: the default.
  }
  let shown = $state(stored !== "off");

  return {
    get shown() {
      return shown;
    },
    setShown(next) {
      shown = next;
      try {
        storage?.setItem(HOME_SEARCH_KEY, next ? "on" : "off");
      } catch {
        // A storage failure only costs persistence.
      }
    },
  };
}

let singleton: HomeSearch | null = null;

/// The app-wide flag, bound to localStorage when there is one.
export function homeSearch(): HomeSearch {
  singleton ??= createHomeSearch(typeof window !== "undefined" ? window.localStorage : null);
  return singleton;
}
