import { api, type CoverPalette, type LampColor } from "../api";
import { appearance } from "../appearance.svelte";
import type { Player } from "../player.svelte";
import { lampFor, type Lamp } from "./lamp";

const PROPS = ["--lamp-h", "--lamp-c", "--lamp-dim"] as const;

/// Latest-wins resolver: a cover change supersedes an in-flight palette fetch,
/// and the palette is only fetched while the lamp follows the cover (RK-D9).
export function createLampResolver(
  apply: (lamp: Lamp) => void,
  fetcher: (uuid: string) => Promise<CoverPalette>,
) {
  let request = 0;
  let lastKey: string | undefined;
  return {
    async update(setting: LampColor, cover: string | null) {
      const key = setting === "cover" ? `cover|${cover ?? ""}` : setting;
      if (key === lastKey) return;
      lastKey = key;
      const id = ++request;
      if (setting !== "cover") {
        apply(lampFor(setting, null));
        return;
      }
      // No artwork: keep the current lamp (like dynamic colour keeps its last
      // palette) — nothing to apply.
      if (!cover) return;
      try {
        const palette = await fetcher(cover);
        if (id === request) apply(lampFor("cover", palette));
      } catch {
        if (id === request) apply(lampFor("cover", null));
      }
    },
    reset() {
      lastKey = undefined;
      request++;
    },
  };
}

/// Drives `--lamp-h`/`--lamp-c`/`--lamp-dim` on <html> while Rack is active
/// and clears them otherwise. Rack palettes are analysed in the dark room.
export function createLamp(player: Player) {
  const root = () => document.documentElement.style;
  const resolver = createLampResolver(
    (lamp) => {
      root().setProperty("--lamp-h", String(lamp.h));
      root().setProperty("--lamp-c", String(lamp.c));
    },
    (uuid) => api.analyzeCover(uuid, "dark"),
  );
  const cover = $derived(player.state.current?.cover ?? null);
  $effect(() => {
    if (!appearance.rackShell) {
      for (const prop of PROPS) root().removeProperty(prop);
      resolver.reset();
      return;
    }
    root().setProperty("--lamp-dim", String(appearance.rack.lamp_brightness / 100));
    void resolver.update(appearance.rack.lamp, cover);
  });
}
