import { api } from "./api";
import { readThemeMirror, resolveTheme, writeThemeMirror, type ResolvedTheme, type ThemeSetting } from "./theme";

export type UiMode = "instrument" | "atmosphere";
export type Typeface = "manrope" | "inter" | "system" | "custom";
export type TypefaceSetting = "auto" | Typeface;
export type DynamicColorSetting = "auto" | "full" | "accent" | "off";
export type DynamicColorMode = "full" | "accent" | "off";
export type Density = "comfortable" | "compact";
export type CoverShape = "disc" | "sleeve";

export const QUEUE_WIDTH_MIN = 260;
export const QUEUE_WIDTH_MAX = 420;

/// An embedder may throw on the `localStorage` property getter itself, before
/// theme.ts's item-level guards can run; the mirror is best-effort either way.
function themeStorage(): Storage | undefined {
  try {
    return globalThis.localStorage;
  } catch {
    return undefined;
  }
}

export function clampQueueWidth(px: number): number {
  return Math.min(QUEUE_WIDTH_MAX, Math.max(QUEUE_WIDTH_MIN, Math.round(px)));
}

/// Mode defaults: Manrope in Atmosphere, Inter in Instrument (spec §4/§6).
export function resolveTypeface(t: TypefaceSetting, mode: UiMode): Typeface {
  if (t !== "auto") return t;
  return mode === "atmosphere" ? "manrope" : "inter";
}

/// `Auto` is Full in Atmosphere and Off in Instrument (spec §5.4).
export function resolveDynamicColor(s: DynamicColorSetting, mode: UiMode): DynamicColorMode {
  if (s !== "auto") return s;
  return mode === "atmosphere" ? "full" : "off";
}

/// The fallback chain always ends in system-ui (spec §6).
export function fontStack(typeface: Typeface, custom: string): string {
  switch (typeface) {
    case "manrope":
      return '"Manrope", system-ui, sans-serif';
    case "inter":
      return '"Inter", system-ui, sans-serif';
    case "system":
      return "system-ui, sans-serif";
    case "custom": {
      const name = custom.trim().replace(/["\\]/g, "");
      return name ? `"${name}", system-ui, sans-serif` : "system-ui, sans-serif";
    }
  }
}

export interface Appearance {
  readonly mode: UiMode;
  readonly theme: ThemeSetting;
  readonly resolvedTheme: ResolvedTheme;
  readonly typeface: TypefaceSetting;
  readonly customTypeface: string;
  readonly density: Density;
  readonly coverShape: CoverShape;
  readonly queueWidth: number;
  readonly reducedMotion: boolean;
  readonly oneko: boolean;
  readonly dynamicColor: DynamicColorMode;
  readonly backdropArt: boolean;
  readonly glassPanels: boolean;
  readonly grain: boolean;
  readonly discSpin: boolean;
  readonly waveSeek: boolean;
  readonly ambientMotion: boolean;
  init(): Promise<void>;
  setMode(mode: UiMode): Promise<void>;
  setTheme(theme: ThemeSetting): Promise<void>;
  setTypeface(typeface: TypefaceSetting): Promise<void>;
  setCustomTypeface(name: string): Promise<void>;
  setDensity(density: Density): Promise<void>;
  setCoverShape(shape: CoverShape): Promise<void>;
  setDynamicColor(setting: DynamicColorSetting): Promise<void>;
  setQueueWidth(px: number): Promise<void>;
  setOneko(enabled: boolean): Promise<void>;
}

export function createAppearance(): Appearance {
  let mode = $state<UiMode>("atmosphere");
  let theme = $state<ThemeSetting>("dark");
  let prefersDark = $state(false);
  let typefaceSetting = $state<TypefaceSetting>("auto");
  let customTypeface = $state("");
  let density = $state<Density>("comfortable");
  let coverShape = $state<CoverShape>("disc");
  let dynamicColorSetting = $state<DynamicColorSetting>("auto");
  let queueWidth = $state(320);
  let reducedMotion = $state(false);
  let oneko = $state(false);

  function apply() {
    const root = document.documentElement;
    const resolved = resolveTheme(theme, prefersDark);
    root.dataset.mode = mode;
    root.dataset.theme = resolved;
    root.style.colorScheme = resolved;
    root.dataset.density = density;
    root.style.setProperty("--font-ui", fontStack(resolveTypeface(typefaceSetting, mode), customTypeface));
    root.style.setProperty("--queue-width", `${queueWidth}px`);
  }

  async function init() {
    theme = readThemeMirror(themeStorage());
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion = media.matches;
    media.addEventListener("change", (e) => (reducedMotion = e.matches));
    const themeMedia = window.matchMedia("(prefers-color-scheme: dark)");
    prefersDark = themeMedia.matches;
    themeMedia.addEventListener("change", (e) => {
      prefersDark = e.matches;
      apply();
    });
    try {
      const config = await api.getConfig();
      mode = config.ui.mode;
      theme = config.ui.theme;
      typefaceSetting = config.ui.typeface;
      customTypeface = config.ui.custom_typeface;
      density = config.ui.density;
      coverShape = config.ui.cover_shape;
      dynamicColorSetting = config.ui.dynamic_color;
      queueWidth = config.ui.queue_width;
      oneko = config.ui.oneko;
    } catch {
      // Defaults are fine; a config failure must not block boot.
    }
    writeThemeMirror(themeStorage(), theme);
    apply();
  }

  /// Local-first apply, then persist; callers surface persistence errors.
  async function persist(run: () => Promise<void>) {
    apply();
    await run();
  }

  /// Local-first like the other setters, but a failed persist reverts the
  /// flag so the UI never disagrees with the stored config.
  async function setOneko(enabled: boolean): Promise<void> {
    const previous = oneko;
    oneko = enabled;
    try {
      await api.setOneko(enabled);
    } catch (error) {
      oneko = previous;
      throw error;
    }
  }

  return {
    get mode() { return mode; },
    get theme() { return theme; },
    get resolvedTheme() { return resolveTheme(theme, prefersDark); },
    get typeface() { return typefaceSetting; },
    get customTypeface() { return customTypeface; },
    get density() { return density; },
    get coverShape() { return coverShape; },
    get queueWidth() { return queueWidth; },
    get reducedMotion() { return reducedMotion; },
    get oneko() { return oneko; },
    get dynamicColor() { return resolveDynamicColor(dynamicColorSetting, mode); },
    get backdropArt() { return mode === "atmosphere"; },
    get glassPanels() { return mode === "atmosphere"; },
    get grain() { return mode === "atmosphere"; },
    get discSpin() { return mode === "atmosphere" && !reducedMotion; },
    get waveSeek() { return mode === "atmosphere"; },
    get ambientMotion() { return mode === "atmosphere" && !reducedMotion; },
    init,
    setMode: (m) => { mode = m; return persist(() => api.setUiMode(m)); },
    setTheme: (t) => { theme = t; writeThemeMirror(themeStorage(), t); return persist(() => api.setTheme(t)); },
    setTypeface: (t) => { typefaceSetting = t; return persist(() => api.setTypeface(t)); },
    setCustomTypeface: (n) => { customTypeface = n; return persist(() => api.setCustomTypeface(n)); },
    setDensity: (d) => { density = d; return persist(() => api.setDensity(d)); },
    setCoverShape: (s) => { coverShape = s; return persist(() => api.setCoverShape(s)); },
    setDynamicColor: (s) => { dynamicColorSetting = s; return persist(() => api.setDynamicColor(s)); },
    setQueueWidth: (px) => { queueWidth = clampQueueWidth(px); return persist(() => api.setQueueWidth(queueWidth)); },
    setOneko,
  };
}

export const appearance = createAppearance();
