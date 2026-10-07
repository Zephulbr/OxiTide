import { describe, expect, it } from "vitest";
import {
  EQ_BANDS_HZ,
  EQ_PRESETS,
  bandLabel,
  clampDb,
  defaultDspSettings,
  dspBadgeTitle,
  effectivePreamp,
  formatCrossfade,
  formatDb,
  matchPreset,
  presetBands,
  withBand,
} from "./dsp";

describe("dsp settings helpers", () => {
  it("defaults to everything off with one gain per band", () => {
    const settings = defaultDspSettings();
    expect(settings.eq.enabled).toBe(false);
    expect(settings.eq.auto_preamp).toBe(true);
    expect(settings.eq.bands_db).toHaveLength(EQ_BANDS_HZ.length);
    expect(settings.replay_gain.mode).toBe("off");
    expect(settings.replay_gain.prevent_clipping).toBe(true);
    expect(settings.crossfade).toEqual({ duration_ms: 0, skip_same_album: true });
  });

  it("labels bands in Hz and kHz", () => {
    expect(EQ_BANDS_HZ.map(bandLabel)).toEqual([
      "31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k",
    ]);
  });

  it("clamps and snaps dB values", () => {
    expect(clampDb(13, 12)).toBe(12);
    expect(clampDb(-40, 12)).toBe(-12);
    expect(clampDb(2.3, 12)).toBe(2.5);
    expect(clampDb(Number.NaN, 12)).toBe(0);
    expect(Object.is(clampDb(-0.1, 12), -0)).toBe(false);
  });

  it("derives the automatic preamp from the loudest boost", () => {
    const eq = { ...defaultDspSettings().eq, enabled: true };
    expect(effectivePreamp({ ...eq, bands_db: [4, 0, 9, -3, 0, 0, 0, 0, 0, 0] })).toBe(-9);
    expect(effectivePreamp({ ...eq, bands_db: EQ_BANDS_HZ.map(() => -2) })).toBe(0);
    expect(effectivePreamp({ ...eq, auto_preamp: false, preamp_db: 2.5 })).toBe(2.5);
  });

  it("matches presets exactly and reports custom curves", () => {
    for (const preset of EQ_PRESETS) {
      expect(preset.bands).toHaveLength(EQ_BANDS_HZ.length);
      expect(matchPreset(preset.bands)).toBe(preset.id);
    }
    expect(matchPreset([1, 0, 0, 0, 0, 0, 0, 0, 0, 0])).toBe("custom");
    expect(presetBands("bass")).toEqual([6, 5, 4, 2, 0, 0, 0, 0, 0, 0]);
    expect(presetBands("nope")).toBeNull();
  });

  it("returns a copy so editing a band never mutates a preset", () => {
    const bands = presetBands("vocal")!;
    bands[0] = 99;
    expect(presetBands("vocal")![0]).toBe(-2);
  });

  it("edits one band immutably, clamped, and repairs short band lists", () => {
    const eq = { ...defaultDspSettings().eq, bands_db: [1, 2] };
    const next = withBand(eq, 4, 20);
    expect(next.bands_db).toEqual([1, 2, 0, 0, 12, 0, 0, 0, 0, 0]);
    expect(eq.bands_db).toEqual([1, 2]);
  });

  it("formats dB, crossfade, and the badge title", () => {
    expect(formatDb(0)).toBe("0 dB");
    expect(formatDb(3)).toBe("+3.0 dB");
    expect(formatDb(-6.5)).toBe("−6.5 dB");
    expect(formatCrossfade(0)).toBe("Off");
    expect(formatCrossfade(6000)).toBe("6 s");
    expect(formatCrossfade(2500)).toBe("2.5 s");
    expect(dspBadgeTitle(["ReplayGain", "EQ"])).toBe("ReplayGain, EQ active — bit-perfect off");
  });
});
