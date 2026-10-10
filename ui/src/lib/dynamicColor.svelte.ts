import { api, type CoverPalette as ApiPalette } from "./api";
import { appearance } from "./appearance.svelte";
import { applyToDocument, createPaletteApplier } from "./coverPalette";
import type { Player } from "./player.svelte";

export type CoverPalette = ApiPalette;
export { paletteTokens, createPaletteApplier, type DynamicColorApply } from "./coverPalette";

export function createDynamicColor(player: Player) {
  const applier = createPaletteApplier(applyToDocument, api.analyzeCover);
  const cover = $derived(player.state.current?.cover ?? null);
  const theme = $derived(appearance.resolvedTheme);
  $effect(() => {
    void applier.onTrack(cover, appearance.dynamicColor, theme);
  });
}
