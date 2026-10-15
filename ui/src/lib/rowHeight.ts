import type { Density } from "./appearance.svelte";

/// JS mirror of the `--row-height` density tokens in styles/tokens.css
/// (spec §7.4). The virtualizer and playlist drag math need the row height as
/// a number; rowHeight.test.ts guards that this mirror matches the CSS.
export function rowHeightFor(density: Density): number {
  return density === "compact" ? 38 : 46;
}

/// Queue rows stack title over artist beside a larger cover, so they run
/// taller than track-table rows. Pure JS (no CSS token): QueueRail sets the
/// row height inline from this value.
export function queueRowHeightFor(density: Density): number {
  return density === "compact" ? 48 : 58;
}
