import { api, type DspSettings } from "./api";

/// The one UI copy of the sound-processing settings, shared by Settings and
/// the Hi-Fi Rack's amp/EQ units so both always show the same values.
/// Writes are local-first and debounced: sliders fire on every step, and the
/// engine applies settings live, so persistence waits for a short pause.
export function createDspStore(
  persist: (settings: DspSettings) => Promise<unknown> = api.setDspSettings,
  delayMs = 150,
) {
  let settings = $state<DspSettings | null>(null);
  let pending: DspSettings | null = null;
  let onError: ((e: unknown) => void) | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let loading: Promise<void> | null = null;

  function flush() {
    clearTimeout(timer);
    timer = undefined;
    if (!pending) return;
    const next = pending;
    const report = onError;
    pending = null;
    persist(next).catch((e) => report?.(e));
  }

  return {
    get settings() { return settings; },
    /// Seed from a loaded config (Settings already fetched it).
    hydrate(next: DspSettings) {
      if (!pending) settings = next;
    },
    /// Fetch once if nobody hydrated yet (Rack mounted before Settings).
    async ensure() {
      if (settings) return;
      loading ??= api
        .getConfig()
        .then((config) => {
          if (!settings) settings = config.dsp;
        })
        .catch(() => {})
        .finally(() => (loading = null));
      await loading;
    },
    set(next: DspSettings, reportError?: (e: unknown) => void) {
      settings = next;
      pending = next;
      onError = reportError;
      clearTimeout(timer);
      timer = setTimeout(flush, delayMs);
    },
    flush,
  };
}

export type DspStore = ReturnType<typeof createDspStore>;

export const dspStore = createDspStore();
