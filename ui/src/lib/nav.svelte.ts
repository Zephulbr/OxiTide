export type Route =
  | { name: "home" }
  | { name: "search" }
  | { name: "library" }
  | { name: "album"; id: number }
  | { name: "playlist"; id: string }
  | { name: "artist"; id: number }
  | { name: "settings" };

export interface Nav {
  readonly route: Route;
  readonly queueOpen: boolean;
  readonly canBack: boolean;
  readonly canForward: boolean;
  goto(next: Route): void;
  back(): void;
  forward(): void;
  toggleQueue(): void;
}

function sameRoute(a: Route, b: Route): boolean {
  if (a.name !== b.name) return false;
  if ("id" in a && "id" in b) return a.id === b.id;
  return true;
}

/// In-session route history. `goto` on the current route is a no-op; any new
/// navigation truncates forward entries. Queue visibility is orthogonal.
export function createNav(): Nav {
  let history = $state<Route[]>([{ name: "home" }]);
  let index = $state(0);
  let queueOpen = $state(false);

  function goto(next: Route) {
    if (sameRoute(history[index], next)) return;
    history = [...history.slice(0, index + 1), next];
    index = history.length - 1;
  }

  function back() {
    if (index > 0) index -= 1;
  }

  function forward() {
    if (index < history.length - 1) index += 1;
  }

  return {
    get route() {
      return history[index];
    },
    get queueOpen() {
      return queueOpen;
    },
    get canBack() {
      return index > 0;
    },
    get canForward() {
      return index < history.length - 1;
    },
    goto,
    back,
    forward,
    toggleQueue() {
      queueOpen = !queueOpen;
    },
  };
}
