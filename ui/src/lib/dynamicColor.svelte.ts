import { api, type CoverPalette as ApiPalette } from "./api";
import { appearance } from "./appearance.svelte";
import type { Player } from "./player.svelte";

export type CoverPalette = ApiPalette;
export type DynamicColorApply = Record<string, string>;

/// Token mapping per dynamic-color mode (spec §5.4): accent-only follows the
/// cover for Primary but keeps Secondary/Tertiary at brand; off is static.
/// The backdrop wash is not a role: the visible backdrop art follows the cover
/// in accent-only too, so the player bar's shaped top edge must track it there
/// as well.
export function paletteTokens(p: CoverPalette, mode: "full" | "accent" | "off"): DynamicColorApply {
  if (mode === "off") return {};
  const base = {
    "--dyn-primary": p.primary,
    "--dyn-on-primary": p.on_primary,
    "--dyn-backdrop": p.backdrop,
  };
  if (mode === "accent") return base;
  return { ...base, "--dyn-secondary": p.secondary, "--dyn-tertiary": p.tertiary };
}

/// Latest-wins application: a new call supersedes any in-flight fetch, so
/// skip-storms never apply a stale palette (spec §5.2 interrupt-and-restart).
export function createPaletteApplier(
  apply: (tokens: DynamicColorApply) => void,
  fetcher: (uuid: string) => Promise<CoverPalette>,
) {
  let request = 0;
  let lastKey: string | undefined;
  return {
    async onTrack(cover: string | null, mode: "full" | "accent" | "off") {
      const key = `${mode}|${cover ?? ""}`;
      if (key === lastKey) return;
      lastKey = key;
      const id = ++request;
      if (mode === "off" || !cover) {
        apply({});
        return;
      }
      try {
        const palette = await fetcher(cover);
        if (id !== request) return;
        apply(paletteTokens(palette, mode));
      } catch {
        // Keep the current colors; the next track retries.
      }
    },
  };
}

function applyToDocument(tokens: DynamicColorApply) {
  const style = document.documentElement.style;
  for (const key of [
    "--dyn-primary",
    "--dyn-on-primary",
    "--dyn-secondary",
    "--dyn-tertiary",
    "--dyn-backdrop",
  ]) {
    const value = tokens[key];
    if (value) style.setProperty(key, value);
    else style.removeProperty(key);
  }
}

export function createDynamicColor(player: Player) {
  const applier = createPaletteApplier(applyToDocument, api.analyzeCover);
  const cover = $derived(player.state.current?.cover ?? null);
  $effect(() => {
    void applier.onTrack(cover, appearance.dynamicColor);
  });
}
