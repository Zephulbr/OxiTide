import type { Density } from "./appearance.svelte";

/// JS mirror of the `--row-height` density tokens in styles/tokens.css
/// (spec §7.4). The virtualizer and playlist drag math need the row height as
/// a number; rowHeight.test.ts guards that this mirror matches the CSS.
export function rowHeightFor(density: Density): number {
  return density === "compact" ? 38 : 46;
}
