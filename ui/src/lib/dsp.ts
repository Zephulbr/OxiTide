import type { DspSettings, EqSettings, TransitionReport, TransitionStyle } from "./api";

/// Mirrors `oxitide_audio::dsp`: band centres and ranges.
export const EQ_BANDS_HZ = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const;
export const EQ_MAX_DB = 12;
export const REPLAY_GAIN_MAX_DB = 15;
export const CROSSFADE_MAX_MS = 12_000;
/// Slider step for every dB control.
export const DB_STEP = 0.5;

export interface EqPreset {
  id: string;
  label: string;
  bands: number[];
}

export const EQ_PRESETS: EqPreset[] = [
  { id: "flat", label: "Flat", bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { id: "bass", label: "Bass boost", bands: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0] },
  { id: "bass-cut", label: "Bass cut", bands: [-6, -5, -4, -2, 0, 0, 0, 0, 0, 0] },
  { id: "treble", label: "Treble boost", bands: [0, 0, 0, 0, 0, 0, 2, 4, 5, 6] },
  { id: "vocal", label: "Vocal", bands: [-2, -2, -1, 0, 2, 4, 4, 2, 0, -1] },
  { id: "loudness", label: "Loudness", bands: [5, 4, 2, 0, -1, -1, 0, 2, 4, 5] },
];

export function defaultDspSettings(): DspSettings {
  return {
    eq: { enabled: false, auto_preamp: true, preamp_db: 0, bands_db: EQ_BANDS_HZ.map(() => 0) },
    replay_gain: { mode: "off", preamp_db: 0, untagged_db: 0, prevent_clipping: true },
    crossfade: {
      duration_ms: 0,
      skip_same_album: true,
      trim_silence: false,
      resample: false,
      beat_match: false,
      style: "classic",
    },
  };
}

export const TRANSITION_STYLES: { id: TransitionStyle; label: string; hint: string }[] = [
  { id: "classic", label: "Classic", hint: "Equal-power fade over the last seconds of the track" },
  { id: "quick", label: "Quick", hint: "Short overlap once the track has died down; keeps endings" },
  {
    id: "blend",
    label: "Blend",
    hint: "Apple-style: cuts the fade-out and slow intros, swaps the bass mid-way",
  },
  { id: "auto", label: "Auto", hint: "Picks a style per pair from how one track ends and the next begins" },
];

function styleLabel(style: TransitionStyle): string {
  return TRANSITION_STYLES.find((s) => s.id === style)?.label ?? style;
}

export function bandLabel(hz: number): string {
  return hz >= 1000 ? `${hz / 1000}k` : String(hz);
}

/// Clamp to ±max and snap to the slider step; non-numbers become 0.
export function clampDb(value: number, max: number): number {
  if (!Number.isFinite(value)) return 0;
  const snapped = Math.round(value / DB_STEP) * DB_STEP;
  return Math.min(max, Math.max(-max, snapped)) || 0;
}

/// The preamp the engine applies (mirrors `EqSettings::effective_preamp_db`).
export function effectivePreamp(eq: EqSettings): number {
  if (!eq.auto_preamp) return eq.preamp_db;
  return -Math.max(0, ...eq.bands_db) || 0;
}

/// The preset whose bands match exactly, else "custom".
export function matchPreset(bands: number[]): string {
  const preset = EQ_PRESETS.find((p) => p.bands.every((gain, i) => (bands[i] ?? 0) === gain));
  return preset?.id ?? "custom";
}

export function presetBands(id: string): number[] | null {
  const preset = EQ_PRESETS.find((p) => p.id === id);
  return preset ? [...preset.bands] : null;
}

export function withBand(eq: EqSettings, index: number, db: number): EqSettings {
  const bands_db = EQ_BANDS_HZ.map((_, i) => eq.bands_db[i] ?? 0);
  bands_db[index] = clampDb(db, EQ_MAX_DB);
  return { ...eq, bands_db };
}

export function formatDb(db: number): string {
  if (db === 0) return "0 dB";
  return `${db > 0 ? "+" : "−"}${Math.abs(db).toFixed(1)} dB`;
}

export function formatCrossfade(ms: number): string {
  return ms <= 0 ? "Off" : `${(ms / 1000).toFixed(ms % 1000 === 0 ? 0 : 1)} s`;
}

const KIND_LABELS: Record<TransitionReport["kind"], string> = {
  crossfade: "Crossfade",
  beat_matched: "Beat-matched",
  fade_out: "Fade-out (no overlap)",
  gapless: "Gapless",
};

function seconds(ms: number): string {
  return `${(ms / 1000).toFixed(1)} s`;
}

/// The readout headline: the kind for Classic, the style (and whether it was
/// beat-matched) otherwise.
function styleTitle(report: TransitionReport): string {
  if (report.kind === "fade_out" || report.kind === "gapless") return KIND_LABELS[report.kind];
  if (report.style === "classic" && !report.auto_reason) return KIND_LABELS[report.kind];
  const style = styleLabel(report.style);
  return report.kind === "beat_matched" ? `${style} (beat-matched)` : style;
}

/// Auto names the style it picked.
function transitionTitle(report: TransitionReport): string {
  const title = styleTitle(report);
  return report.auto_reason ? `Auto → ${title}` : title;
}

/// The "Last transition" readout: a headline plus detail lines, for A/B
/// listening notes.
export function describeTransition(report: TransitionReport): { title: string; details: string[] } {
  const details: string[] = [];
  if (report.auto_reason) details.push(`Why: ${report.auto_reason}`);
  if (report.kind !== "fade_out" && report.kind !== "gapless") {
    details.push(
      report.beats ? `${seconds(report.overlap_ms)} overlap (${report.beats} beats)` : `${seconds(report.overlap_ms)} overlap`,
    );
  }
  if (report.trimmed_out_ms > 0 || report.trimmed_in_ms > 0) {
    details.push(`Silence skipped: ${seconds(report.trimmed_out_ms)} end, ${seconds(report.trimmed_in_ms)} start`);
  }
  if (report.cut_out_ms > 0 || report.cut_in_ms > 0) {
    details.push(`Cut: ${seconds(report.cut_out_ms)} of the ending, ${seconds(report.cut_in_ms)} of the intro`);
  }
  if (report.ride_db && report.ride_db < 0) {
    details.push(`Level ride: next track came in ${(-report.ride_db).toFixed(1)} dB lower`);
  }
  if (report.resampled) details.push("Resampled across a format change");
  if (report.out_bpm != null || report.in_bpm != null) {
    const bpm = (value?: number | null, confidence?: number | null) =>
      value == null ? "no grid" : `${value.toFixed(1)} BPM (${Math.round((confidence ?? 0) * 100)}%)`;
    details.push(`Tempo: ${bpm(report.out_bpm, report.out_confidence)} → ${bpm(report.in_bpm, report.in_confidence)}`);
  }
  if (report.note) details.push(report.note);
  return { title: transitionTitle(report), details };
}

/// Where "Jump near the end" seeks: 35 s before the end (or the start), so
/// AutoMix's longer hold-back (crossfade + 14 s) can fill before the end.
export function nearEndTarget(durationMs: number): number {
  return Math.max(0, durationMs - 35_000);
}

/// "0:06.2" for the replay lab's overlap markers.
export function formatReplayTime(ms: number): string {
  const total = Math.max(0, ms) / 1000;
  const minutes = Math.floor(total / 60);
  return `${minutes}:${(total - minutes * 60).toFixed(1).padStart(4, "0")}`;
}

/// Tooltip for the player-bar DSP badge.
export function dspBadgeTitle(stages: string[]): string {
  return `${stages.join(", ")} active — bit-perfect off`;
}
