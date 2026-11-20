import { getContext, setContext } from "svelte";
import type { RemoteActions } from "./actions.svelte";
import type { RemoteApi } from "./api";
import type { RemoteStore } from "./store.svelte";

export interface Remote {
  api: RemoteApi;
  store: RemoteStore;
  actions: RemoteActions;
}

/// Exported for component tests (`render(..., { context })`).
export const REMOTE_KEY = Symbol("oxitide-remote");
const KEY = REMOTE_KEY;

export function provideRemote(remote: Remote) {
  setContext(KEY, remote);
}

export function useRemote(): Remote {
  return getContext<Remote>(KEY);
}
