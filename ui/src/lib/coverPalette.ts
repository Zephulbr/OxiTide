import type { CoverPalette } from "./api";

/// Pure cover-palette helpers shared by the desktop shell and the phone remote
/// (no Tauri imports, so `ui/src/remote` may run them).

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
  fetcher: (uuid: string, theme: "dark" | "light") => Promise<CoverPalette>,
) {
  let request = 0;
  let lastKey: string | undefined;
  let appliedTheme: "dark" | "light" | undefined;
  /// Clearing back to the room's static CSS is correct only when the applied
  /// palette belongs to the other room; a same-theme failure keeps current
  /// colors (spec §5.2).
  const clearIfStale = (theme: "dark" | "light") => {
    if (appliedTheme !== undefined && appliedTheme !== theme) {
      apply({});
      appliedTheme = theme;
    }
  };
  return {
    async onTrack(cover: string | null, mode: "full" | "accent" | "off", theme: "dark" | "light") {
      const key = `${mode}|${theme}|${cover ?? ""}`;
      if (key === lastKey) return;
      lastKey = key;
      const id = ++request;
      if (mode === "off") {
        apply({});
        appliedTheme = theme;
        return;
      }
      // No track or no artwork: keep the last applied palette (owner ruling
      // 2026-10-02), unless it belongs to the other theme. The counter bump
      // above cancels any in-flight older fetch.
      if (!cover) {
        clearIfStale(theme);
        return;
      }
      try {
        const palette = await fetcher(cover, theme);
        if (id !== request) return;
        apply(paletteTokens(palette, mode));
        appliedTheme = theme;
      } catch {
        if (id !== request) return;
        // Keep the current colors; the next track retries. A theme switch
        // failed before re-analysis: the stale other-room palette must not
        // survive, so drop to the room's static fallbacks instead.
        clearIfStale(theme);
      }
    },
  };
}

/// Write (or clear) the `--dyn-*` tokens inline on the root element.
export function applyToDocument(tokens: DynamicColorApply) {
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
  // The logo follows the cover only while a cover colour is applied (else
  // it keeps the brand teal from tokens.css).
  if (tokens["--dyn-primary"]) style.setProperty("--logo-accent", "var(--dyn-primary)");
  else style.removeProperty("--logo-accent");
}

