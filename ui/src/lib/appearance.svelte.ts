import { api } from "./api";

export type UiMode = "instrument";
export type Typeface = "manrope" | "inter" | "system" | "custom";
export type Density = "comfortable" | "compact";
export type CoverShape = "disc" | "sleeve";

export const QUEUE_WIDTH_MIN = 260;
export const QUEUE_WIDTH_MAX = 420;

export function clampQueueWidth(px: number): number {
  return Math.min(QUEUE_WIDTH_MAX, Math.max(QUEUE_WIDTH_MIN, Math.round(px)));
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
  readonly typeface: Typeface;
  readonly customTypeface: string;
  readonly density: Density;
  readonly coverShape: CoverShape;
  readonly queueWidth: number;
  readonly reducedMotion: boolean;
  init(): Promise<void>;
  setMode(mode: UiMode): Promise<void>;
  setTypeface(typeface: Typeface): Promise<void>;
  setCustomTypeface(name: string): Promise<void>;
  setDensity(density: Density): Promise<void>;
  setCoverShape(shape: CoverShape): Promise<void>;
  setQueueWidth(px: number): Promise<void>;
}

export function createAppearance(): Appearance {
  let mode = $state<UiMode>("instrument");
  let typeface = $state<Typeface>("manrope");
  let customTypeface = $state("");
  let density = $state<Density>("comfortable");
  let coverShape = $state<CoverShape>("disc");
  let queueWidth = $state(320);
  let reducedMotion = $state(false);

  function apply() {
    const root = document.documentElement;
    root.dataset.mode = mode;
    root.dataset.density = density;
    root.style.setProperty("--font-ui", fontStack(typeface, customTypeface));
    root.style.setProperty("--queue-width", `${queueWidth}px`);
  }

  async function init() {
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotion = media.matches;
    media.addEventListener("change", (e) => (reducedMotion = e.matches));
    try {
      const config = await api.getConfig();
      mode = config.ui.mode;
      typeface = config.ui.typeface;
      customTypeface = config.ui.custom_typeface;
      density = config.ui.density;
      coverShape = config.ui.cover_shape;
      queueWidth = config.ui.queue_width;
    } catch {
      // Defaults are fine; a config failure must not block boot.
    }
    apply();
  }

  /// Local-first apply, then persist; callers surface persistence errors.
  async function persist(run: () => Promise<void>) {
    apply();
    await run();
  }

  return {
    get mode() { return mode; },
    get typeface() { return typeface; },
    get customTypeface() { return customTypeface; },
    get density() { return density; },
    get coverShape() { return coverShape; },
    get queueWidth() { return queueWidth; },
    get reducedMotion() { return reducedMotion; },
    init,
    setMode: (m) => { mode = m; return persist(() => api.setUiMode(m)); },
    setTypeface: (t) => { typeface = t; return persist(() => api.setTypeface(t)); },
    setCustomTypeface: (n) => { customTypeface = n; return persist(() => api.setCustomTypeface(n)); },
    setDensity: (d) => { density = d; return persist(() => api.setDensity(d)); },
    setCoverShape: (s) => { coverShape = s; return persist(() => api.setCoverShape(s)); },
    setQueueWidth: (px) => { queueWidth = clampQueueWidth(px); return persist(() => api.setQueueWidth(queueWidth)); },
  };
}

export const appearance = createAppearance();
