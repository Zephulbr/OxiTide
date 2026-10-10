import { applyToDocument, createPaletteApplier } from "../lib/coverPalette";
import type { RemoteApi } from "./api";
import { isCoverId } from "./motion";
import type { RemoteStore } from "./store.svelte";
import { theme } from "./theme.svelte";

/// Atmosphere dynamic colour on the phone: the PC analyses the current cover
/// (cached there) and the page applies the same `--dyn-*` tokens the desktop
/// does; components keep reading semantic tokens. Call from a component.
export function startDynamicColor(api: RemoteApi, store: RemoteStore) {
  const applier = createPaletteApplier(applyToDocument, (uuid, room) => api.analyzeCover(uuid, room));
  const cover = $derived(store.state?.current?.cover ?? null);
  $effect(() => {
    void applier.onTrack(isCoverId(cover) ? cover : null, "full", theme.current);
  });
}
