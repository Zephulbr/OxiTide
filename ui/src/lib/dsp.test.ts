import { describe, expect, it } from "vitest";
import {
  EQ_BANDS_HZ,
  EQ_PRESETS,
  bandLabel,
  clampDb,
  defaultDspSettings,
  describeTransition,
  dspBadgeTitle,
  effectivePreamp,
  formatCrossfade,
  formatDb,
  formatReplayTime,
  matchPreset,
  nearEndTarget,
  presetBands,
  withBand,
} from "./dsp";
import { parseReplay } from "./api";

describe("dsp settings helpers", () => {
  it("defaults to everything off with one gain per band", () => {
    const settings = defaultDspSettings();
    expect(settings.eq.enabled).toBe(false);
    expect(settings.eq.auto_preamp).toBe(true);
    expect(settings.eq.bands_db).toHaveLength(EQ_BANDS_HZ.length);
    expect(settings.replay_gain.mode).toBe("off");
    expect(settings.replay_gain.prevent_clipping).toBe(true);
    expect(settings.crossfade).toEqual({
      duration_ms: 0,
      skip_same_album: true,
      trim_silence: false,
      resample: false,
      beat_match: false,
      style: "classic",
    });
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

describe("automix readout", () => {
  const base = {
    kind: "crossfade" as const,
    overlap_ms: 6000,
    trimmed_out_ms: 0,
    trimmed_in_ms: 0,
    resampled: false,
    style: "classic" as const,
    cut_out_ms: 0,
    cut_in_ms: 0,
  };

  it("summarizes a plain crossfade in one line", () => {
    expect(describeTransition(base)).toEqual({ title: "Crossfade", details: ["6.0 s overlap"] });
  });

  it("lists beats, trims, resampling, tempos, and the reason", () => {
    const summary = describeTransition({
      ...base,
      kind: "beat_matched",
      overlap_ms: 3750,
      beats: 8,
      trimmed_out_ms: 1500,
      trimmed_in_ms: 250,
      resampled: true,
      out_bpm: 128,
      in_bpm: 127.4,
      out_confidence: 0.82,
      in_confidence: 0.61,
    });
    expect(summary.title).toBe("Beat-matched");
    expect(summary.details).toEqual([
      "3.8 s overlap (8 beats)",
      "Silence skipped: 1.5 s end, 0.3 s start",
      "Resampled across a format change",
      "Tempo: 128.0 BPM (82%) → 127.4 BPM (61%)",
    ]);
  });

  it("explains a fallback and a fade-out", () => {
    const fallback = describeTransition({
      ...base,
      out_bpm: 128,
      in_bpm: null,
      out_confidence: 0.7,
      note: "no beat grid (too short or no onsets)",
    });
    expect(fallback.details).toContain("Tempo: 128.0 BPM (70%) → no grid");
    expect(fallback.details[fallback.details.length - 1]).toBe("no beat grid (too short or no onsets)");
    expect(describeTransition({ ...base, kind: "fade_out" })).toEqual({
      title: "Fade-out (no overlap)",
      details: [],
    });
  });

  it("titles quick and blend transitions by style and lists the cuts", () => {
    expect(describeTransition({ ...base, style: "quick", overlap_ms: 1500 }).title).toBe("Quick");
    const blend = describeTransition({
      ...base,
      style: "blend",
      kind: "beat_matched",
      beats: 16,
      cut_out_ms: 7200,
      cut_in_ms: 4000,
    });
    expect(blend.title).toBe("Blend (beat-matched)");
    expect(blend.details).toContain("Cut: 7.2 s of the ending, 4.0 s of the intro");
    expect(describeTransition({ ...base, style: "blend", kind: "fade_out" }).title).toBe("Fade-out (no overlap)");
  });

  it("jumps 35 s before the end so the AutoMix hold-back fills", () => {
    expect(nearEndTarget(200_000)).toBe(165_000);
    expect(nearEndTarget(10_000)).toBe(0);
  });

  it("formats replay markers", () => {
    expect(formatReplayTime(6200)).toBe("0:06.2");
    expect(formatReplayTime(65_500)).toBe("1:05.5");
  });
});

describe("replay payload", () => {
  it("splits the JSON header from the WAV", async () => {
    const header = JSON.stringify({ report: { overlap_ms: 6000 }, overlap_start_ms: 6000, overlap_end_ms: 12000 });
    const head = new TextEncoder().encode(header);
    const wav = new Uint8Array([82, 73, 70, 70]);
    const buffer = new ArrayBuffer(4 + head.length + wav.length);
    new DataView(buffer).setUint32(0, head.length, true);
    new Uint8Array(buffer, 4).set(head);
    new Uint8Array(buffer, 4 + head.length).set(wav);
    const replay = parseReplay(buffer);
    expect(replay.overlap_start_ms).toBe(6000);
    expect(replay.overlap_end_ms).toBe(12000);
    expect(replay.wav.type).toBe("audio/wav");
    expect(new Uint8Array(await replay.wav.arrayBuffer())).toEqual(wav);
  });
});
