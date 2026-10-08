import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defaultDspSettings } from "./dsp";
import { createDspStore } from "./dspStore.svelte";

vi.mock("./api", () => ({ api: { getConfig: vi.fn(), setDspSettings: vi.fn() } }));

describe("dspStore", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("is local-first and debounces persistence to the last value", () => {
    const persist = vi.fn().mockResolvedValue(undefined);
    const store = createDspStore(persist, 150);
    const a = defaultDspSettings();
    const b = { ...a, eq: { ...a.eq, enabled: true } };
    store.set(a);
    store.set(b);
    expect(store.settings?.eq.enabled).toBe(true);
    expect(persist).not.toHaveBeenCalled();
    vi.advanceTimersByTime(150);
    expect(persist).toHaveBeenCalledTimes(1);
    expect(persist).toHaveBeenCalledWith(b);
  });

  it("flush writes immediately and only once", () => {
    const persist = vi.fn().mockResolvedValue(undefined);
    const store = createDspStore(persist, 150);
    store.set(defaultDspSettings());
    store.flush();
    store.flush();
    vi.advanceTimersByTime(500);
    expect(persist).toHaveBeenCalledTimes(1);
  });

  it("hydrate never clobbers an unsaved local change", () => {
    const store = createDspStore(vi.fn().mockResolvedValue(undefined), 150);
    const local = { ...defaultDspSettings(), crossfade: { duration_ms: 6000, skip_same_album: true } };
    store.set(local);
    store.hydrate(defaultDspSettings());
    expect(store.settings?.crossfade.duration_ms).toBe(6000);
  });

  it("reports persistence failures", async () => {
    const error = new Error("disk full");
    const store = createDspStore(vi.fn().mockRejectedValue(error), 0);
    const report = vi.fn();
    store.set(defaultDspSettings(), report);
    store.flush();
    await vi.runAllTimersAsync();
    expect(report).toHaveBeenCalledWith(error);
  });
});
