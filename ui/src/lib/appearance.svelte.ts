import {
  api, type AnalyzerStyle, type CatjamCoat, type InstrumentTone, type PanelStyle, type RackSettings,
} from "./api";
import { DEFAULT_CATJAM_SCALE, DEFAULT_COAT, clampCatjamScale, isCoat } from "./catjamSprite";
import {
  MODE_STORAGE_KEY, readMirror, readThemeMirror, resolveTheme, TONE_STORAGE_KEY, writeMirror,
  writeThemeMirror, type ResolvedTheme, type ThemeSetting,
} from "./theme";

export type UiMode = "instrument" | "atmosphere" | "rack";
const UI_MODES: readonly UiMode[] = ["atmosphere", "instrument", "rack"];
const INSTRUMENT_TONES: readonly InstrumentTone[] = ["warm", "neutral", "black"];

export function defaultRackSettings(): RackSettings {
  return { deck_face: "display", meters: "needles", analyzer: "auto", lamp: "amber", eq_unit: true, lamp_brightness: 100 };
}

/// The analyzer unit's effective style: Auto avoids showing a second
/// spectrum when the amp's meters already are one.
export function resolveAnalyzer(rack: RackSettings): Exclude<AnalyzerStyle, "auto"> {
  if (rack.analyzer !== "auto") return rack.analyzer;
  return rack.meters === "spectrum" ? "recorder" : "spectrum";
}

/// Mirrors `clamp_lamp_brightness` in `config.rs`.
export function clampLampBrightness(value: number): number {
  if (!Number.isFinite(value)) return 100;
  return Math.min(100, Math.max(40, Math.round(value)));
}
export type Typeface = "manrope" | "inter" | "system" | "custom";
export type TypefaceSetting = "auto" | Typeface;
export type DynamicColorMode = "full" | "accent" | "off";
export type DynamicColorSetting = DynamicColorMode;
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

/// Mode defaults: Manrope in Atmosphere and Rack, Inter in Instrument (spec §4/§6).
export function resolveTypeface(t: TypefaceSetting, mode: UiMode): Typeface {
  if (t !== "auto") return t;
  return mode === "instrument" ? "inter" : "manrope";
}

/// The setting applies as chosen (Full by default, in every mode). Rack
/// replaces dynamic colour with its lamp colour, so it is always off there
/// (Rack spec RK-D9).
export function resolveDynamicColor(s: DynamicColorSetting, mode: UiMode): DynamicColorMode {
  return mode === "rack" ? "off" : s;
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
  /// Studio's dark surfaces; modes.css reads it as `data-tone` (Studio + dark only).
  readonly instrumentTone: InstrumentTone;
  readonly typeface: TypefaceSetting;
  readonly customTypeface: string;
  readonly density: Density;
  readonly coverShape: CoverShape;
  readonly queueWidth: number;
  readonly reducedMotion: boolean;
  readonly oneko: boolean;
  /// The jamming cat on the player bar cover, and its coat.
  readonly catjam: boolean;
  readonly catjamCoat: CatjamCoat;
  /// Percent of the 32 px sprite, 100–200.
  readonly catjamScale: number;
  /// The collapsed sidebar and the closed queue expand as overlays on hover.
  readonly hoverPanels: boolean;
  /// Hairlines between track list rows; the root's `data-row-lines`.
  readonly rowSeparators: boolean;
  /// Default shell only (Atmosphere, Instrument): the player bar and the
  /// queue dock to the window edges or float as inset cards.
  readonly playerBarStyle: PanelStyle;
  readonly queueStyle: PanelStyle;
  readonly dynamicColor: DynamicColorMode;
  readonly backdropArt: boolean;
  readonly glassPanels: boolean;
  readonly grain: boolean;
  readonly discSpin: boolean;
  readonly waveSeek: boolean;
  readonly ambientMotion: boolean;
  /// Profiles draws artists as vinyl records (Hi-Fi Rack only).
  readonly vinylRecords: boolean;
  /// Hi-Fi Rack: App mounts `RackShell` instead of the default shell (RK-D1).
  readonly rackShell: boolean;
  readonly rack: RackSettings;
  init(): Promise<void>;
  setRack(patch: Partial<RackSettings>): Promise<void>;
  setMode(mode: UiMode): Promise<void>;
  setTheme(theme: ThemeSetting): Promise<void>;
  setInstrumentTone(tone: InstrumentTone): Promise<void>;
  setTypeface(typeface: TypefaceSetting): Promise<void>;
  setCustomTypeface(name: string): Promise<void>;
  setDensity(density: Density): Promise<void>;
  setCoverShape(shape: CoverShape): Promise<void>;
  setDynamicColor(setting: DynamicColorSetting): Promise<void>;
  setQueueWidth(px: number): Promise<void>;
  setOneko(enabled: boolean): Promise<void>;
  setCatjam(enabled: boolean): Promise<void>;
  setCatjamCoat(coat: CatjamCoat): Promise<void>;
  setCatjamScale(percent: number): Promise<void>;
  setHoverPanels(enabled: boolean): Promise<void>;
  setRowSeparators(enabled: boolean): Promise<void>;
  setPlayerBarStyle(style: PanelStyle): Promise<void>;
  setQueueStyle(style: PanelStyle): Promise<void>;
}

function isPanelStyle(value: unknown): value is PanelStyle {
  return value === "docked" || value === "floating";
}

function isInstrumentTone(value: unknown): value is InstrumentTone {
  return (INSTRUMENT_TONES as readonly unknown[]).includes(value);
}

export function createAppearance(): Appearance {
  // Mode and tone start from the pre-paint mirror (index.html applies the same
  // values), so the first render already mounts the right shell and colours.
  let mode = $state<UiMode>(readMirror(themeStorage(), MODE_STORAGE_KEY, UI_MODES, "atmosphere"));
  let theme = $state<ThemeSetting>("dark");
  let prefersDark = $state(false);
  let instrumentTone = $state<InstrumentTone>(
    readMirror(themeStorage(), TONE_STORAGE_KEY, INSTRUMENT_TONES, "warm"),
  );
  let typefaceSetting = $state<TypefaceSetting>("auto");
  let customTypeface = $state("");
  let density = $state<Density>("comfortable");
  let coverShape = $state<CoverShape>("disc");
  let dynamicColorSetting = $state<DynamicColorSetting>("full");
  let queueWidth = $state(320);
  let reducedMotion = $state(false);
  let oneko = $state(false);
  let catjam = $state(false);
  let catjamCoat = $state<CatjamCoat>(DEFAULT_COAT);
  let catjamScale = $state(DEFAULT_CATJAM_SCALE);
  let hoverPanels = $state(false);
  let rowSeparators = $state(true);
  let playerBarStyle = $state<PanelStyle>("docked");
  let queueStyle = $state<PanelStyle>("docked");
  let rack =$state<RackSettings>(defaultRackSettings());

  function apply() {
    const root = document.documentElement;
    const resolved = resolveTheme(theme, prefersDark);
    root.dataset.mode = mode;
    root.dataset.theme = resolved;
    root.dataset.tone = instrumentTone;
    // Rack is always dark; it has no light variant (RK-D12).
    root.style.colorScheme = mode === "rack" ? "dark" : resolved;
    root.dataset.density = density;
    root.dataset.rowLines = rowSeparators ? "on" : "off";
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
    // The mirrored look (and its default typeface) holds until the config lands.
    apply();
    try {
      const config = await api.getConfig();
      if (UI_MODES.includes(config.ui.mode)) mode = config.ui.mode;
      theme = config.ui.theme;
      if (isInstrumentTone(config.ui.instrument_tone)) instrumentTone = config.ui.instrument_tone;
      typefaceSetting = config.ui.typeface;
      customTypeface = config.ui.custom_typeface;
      density = config.ui.density;
      coverShape = config.ui.cover_shape;
      dynamicColorSetting = config.ui.dynamic_color;
      queueWidth = config.ui.queue_width;
      oneko = config.ui.oneko;
      catjam = config.ui.catjam ?? false;
      catjamCoat = isCoat(config.ui.catjam_coat) ? config.ui.catjam_coat : DEFAULT_COAT;
      catjamScale = clampCatjamScale(config.ui.catjam_scale ?? DEFAULT_CATJAM_SCALE);
      hoverPanels = config.ui.hover_panels ?? false;
      rowSeparators = config.ui.row_separators ?? true;
      if (isPanelStyle(config.ui.player_bar_style)) playerBarStyle = config.ui.player_bar_style;
      if (isPanelStyle(config.ui.queue_style)) queueStyle = config.ui.queue_style;
      if (config.rack) rack = { ...defaultRackSettings(), ...config.rack };
    } catch {
      // Defaults are fine; a config failure must not block boot.
    }
    writeThemeMirror(themeStorage(), theme);
    writeMirror(themeStorage(), MODE_STORAGE_KEY, mode);
    writeMirror(themeStorage(), TONE_STORAGE_KEY, instrumentTone);
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

  async function setCatjam(enabled: boolean): Promise<void> {
    const previous = catjam;
    catjam = enabled;
    try {
      await api.setCatjam(enabled);
    } catch (error) {
      catjam = previous;
      throw error;
    }
  }

  async function setCatjamCoat(coat: CatjamCoat): Promise<void> {
    const previous = catjamCoat;
    catjamCoat = coat;
    try {
      await api.setCatjamCoat(coat);
    } catch (error) {
      catjamCoat = previous;
      throw error;
    }
  }

  /// The stored (clamped) size wins on success.
  async function setCatjamScale(percent: number): Promise<void> {
    const previous = catjamScale;
    catjamScale = clampCatjamScale(percent);
    try {
      const stored = await api.setCatjamScale(catjamScale);
      if (typeof stored === "number") catjamScale = clampCatjamScale(stored);
    } catch (error) {
      catjamScale = previous;
      throw error;
    }
  }

  async function setHoverPanels(enabled: boolean): Promise<void> {
    const previous = hoverPanels;
    hoverPanels = enabled;
    try {
      await api.setHoverPanels(enabled);
    } catch (error) {
      hoverPanels = previous;
      throw error;
    }
  }

  async function setRowSeparators(enabled: boolean): Promise<void> {
    const previous = rowSeparators;
    rowSeparators = enabled;
    apply();
    try {
      await api.setRowSeparators(enabled);
    } catch (error) {
      rowSeparators = previous;
      apply();
      throw error;
    }
  }

  async function setPlayerBarStyle(style: PanelStyle): Promise<void> {
    const previous = playerBarStyle;
    playerBarStyle = style;
    try {
      await api.setPlayerBarStyle(style);
    } catch (error) {
      playerBarStyle = previous;
      throw error;
    }
  }

  async function setQueueStyle(style: PanelStyle): Promise<void> {
    const previous = queueStyle;
    queueStyle = style;
    try {
      await api.setQueueStyle(style);
    } catch (error) {
      queueStyle = previous;
      throw error;
    }
  }

  /// Local-first; a failed persist reverts so the UI never disagrees with the
  /// stored config. The stored (clamped) value wins on success.
  async function setRack(patch: Partial<RackSettings>): Promise<void> {
    const previous = rack;
    const next = { ...rack, ...patch };
    next.lamp_brightness = clampLampBrightness(next.lamp_brightness);
    rack = next;
    try {
      const stored = await api.setRackSettings(next);
      if (stored) rack = { ...defaultRackSettings(), ...stored };
    } catch (error) {
      rack = previous;
      throw error;
    }
  }

  return {
    get mode() { return mode; },
    get theme() { return theme; },
    get resolvedTheme() { return resolveTheme(theme, prefersDark); },
    get instrumentTone() { return instrumentTone; },
    get typeface() { return typefaceSetting; },
    get customTypeface() { return customTypeface; },
    get density() { return density; },
    get coverShape() { return coverShape; },
    get queueWidth() { return queueWidth; },
    get reducedMotion() { return reducedMotion; },
    get oneko() { return oneko; },
    get catjam() { return catjam; },
    get catjamCoat() { return catjamCoat; },
    get catjamScale() { return catjamScale; },
    get hoverPanels() { return hoverPanels; },
    get rowSeparators() { return rowSeparators; },
    get playerBarStyle() { return playerBarStyle; },
    get queueStyle() { return queueStyle; },
    get dynamicColor() { return resolveDynamicColor(dynamicColorSetting, mode); },
    get backdropArt() { return mode === "atmosphere"; },
    get glassPanels() { return mode === "atmosphere"; },
    get grain() { return mode === "atmosphere"; },
    get discSpin() { return mode === "atmosphere" && !reducedMotion; },
    get waveSeek() { return mode === "atmosphere"; },
    get ambientMotion() { return mode === "atmosphere" && !reducedMotion; },
    get vinylRecords() { return mode === "rack"; },
    get rackShell() { return mode === "rack"; },
    get rack() { return rack; },
    init,
    setRack,
    setMode: (m) => {
      mode = m;
      writeMirror(themeStorage(), MODE_STORAGE_KEY, m);
      return persist(() => api.setUiMode(m));
    },
    setTheme: (t) => { theme = t; writeThemeMirror(themeStorage(), t); return persist(() => api.setTheme(t)); },
    setInstrumentTone: (t) => {
      instrumentTone = t;
      writeMirror(themeStorage(), TONE_STORAGE_KEY, t);
      return persist(() => api.setInstrumentTone(t));
    },
    setTypeface: (t) => { typefaceSetting = t; return persist(() => api.setTypeface(t)); },
    setCustomTypeface: (n) => { customTypeface = n; return persist(() => api.setCustomTypeface(n)); },
    setDensity: (d) => { density = d; return persist(() => api.setDensity(d)); },
    setCoverShape: (s) => { coverShape = s; return persist(() => api.setCoverShape(s)); },
    setDynamicColor: (s) => { dynamicColorSetting = s; return persist(() => api.setDynamicColor(s)); },
    setQueueWidth: (px) => { queueWidth = clampQueueWidth(px); return persist(() => api.setQueueWidth(queueWidth)); },
    setOneko,
    setCatjam,
    setCatjamCoat,
    setCatjamScale,
    setHoverPanels,
    setRowSeparators,
    setPlayerBarStyle,
    setQueueStyle,
  };
}

export const appearance = createAppearance();
