import { getContext, setContext } from "svelte";
import type { RemoteActions } from "./actions.svelte";
import type { RemoteApi } from "./api";
import type { RemoteStore } from "./store.svelte";

export interface Remote {
  api: RemoteApi;
  store: RemoteStore;
  actions: RemoteActions;
}

const KEY = Symbol("oxitide-remote");

export function provideRemote(remote: Remote) {
  setContext(KEY, remote);
}

export function useRemote(): Remote {
  return getContext<Remote>(KEY);
}
