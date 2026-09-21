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
  goto(next: Route): void;
  toggleQueue(): void;
}

export function createNav(): Nav {
  let route = $state<Route>({ name: "home" });
  let queueOpen = $state(false);
  return {
    get route() { return route; },
    get queueOpen() { return queueOpen; },
    goto(next: Route) { route = next; },
    toggleQueue() { queueOpen = !queueOpen; },
  };
}
