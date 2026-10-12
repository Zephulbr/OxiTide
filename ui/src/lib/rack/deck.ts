import type { QueueItem, RepeatMode, StreamMeta } from "../api";

/// The slice of player state the deck reads (keeps helpers testable).
export interface DeckState {
  queue: QueueItem[];
  current: QueueItem | null;
  current_index: number | null;
  position_ms: number;
  duration_ms: number;
}

export function currentIndex(state: DeckState): number {
  if (state.current_index !== null && state.current_index !== undefined) return state.current_index;
  if (!state.current) return -1;
  return state.queue.findIndex((item) => item.track_id === state.current?.track_id);
}

/// The next `n` queue items after the current one (no wrap).
export function upNext(state: DeckState, n = 3): QueueItem[] {
  const index = currentIndex(state);
  return state.queue.slice(index + 1, index + 1 + n);
}

export type CalendarCell = { n: number; state: "current" | "present" | "absent" };

/// CD "music calendar": track numbers in a window of `size` (20) that keeps the
/// current track visible; numbers past the queue length are ghosted.
export function calendar(length: number, index: number, size = 20): CalendarCell[] {
  const first = index < size ? 0 : Math.floor(index / size) * size;
  return Array.from({ length: size }, (_, i) => {
    const n = first + i + 1;
    const state = n - 1 === index ? "current" : n <= length ? "present" : "absent";
    return { n, state };
  });
}

/// Queue changer: how many disc keys the unit carries.
export const CHANGER_SLOTS = 5;

/// First queue index of the page of disc keys that holds `index` (0 when
/// nothing is current).
export function changerStart(index: number, size = CHANGER_SLOTS): number {
  return index < 0 ? 0 : Math.floor(index / size) * size;
}

/// Clamp a page start into the queue, aligned to whole pages.
export function clampChangerStart(first: number, length: number, size = CHANGER_SLOTS): number {
  const last = length > 0 ? Math.floor((length - 1) / size) * size : 0;
  return Math.min(last, Math.max(0, Math.floor(first / size) * size));
}

export type ChangerSlot = { index: number; state: "current" | "loaded" | "empty" };

/// The disc keys on one page: queue indices from `first`; past the end of the
/// queue a slot is empty.
export function changerSlots(length: number, index: number, first: number, size = CHANGER_SLOTS): ChangerSlot[] {
  return Array.from({ length: size }, (_, i) => {
    const at = first + i;
    const state = at === index ? "current" : at < length ? "loaded" : "empty";
    return { index: at, state };
  });
}

/// Time left in the queue from the current position, in ms.
export function remainingMs(state: DeckState): number {
  const index = currentIndex(state);
  const rest = state.queue.slice(index + 1).reduce((sum, item) => sum + item.duration_secs * 1000, 0);
  return Math.max(0, state.duration_ms - state.position_ms) + rest;
}

export function totalMs(queue: QueueItem[]): number {
  return queue.reduce((sum, item) => sum + item.duration_secs * 1000, 0);
}

/// `m:ss`, or `h:mm:ss` from an hour up.
export function clock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

export function repeatAnnunciator(mode: RepeatMode): string | null {
  return mode === "one" ? "RPT 1" : mode === "all" ? "RPT" : null;
}

/// The Display face's format annunciators, e.g. FLAC · 24BIT · 96kHz · EXCL.
export function formatLine(stream: StreamMeta | null, outputMode: "shared" | "exclusive"): string[] {
  const out: string[] = [];
  if (stream?.codec) out.push(stream.codec.toUpperCase());
  if (stream && stream.bit_depth > 0) out.push(`${stream.bit_depth}BIT`);
  if (stream && stream.sample_rate > 0) {
    const khz = stream.sample_rate / 1000;
    out.push(`${Number.isInteger(khz) ? khz : khz.toFixed(1)}kHz`);
  }
  out.push(outputMode === "exclusive" ? "EXCL" : "SHRD");
  return out;
}

/// Tuner dial: minute ticks across a track's length (every minute, every
/// two past ten minutes), as fractions 0..1 with their labels.
export function dialTicks(durationMs: number): { at: number; label: string }[] {
  const minutes = durationMs / 60000;
  if (!(minutes > 0)) return [];
  const step = minutes > 10 ? 2 : 1;
  const out: { at: number; label: string }[] = [];
  for (let m = 0; m <= minutes; m += step) out.push({ at: m / minutes, label: String(m) });
  return out;
}

/// Short format for the amp's status LED, e.g. "24/96".
export function formatBadge(stream: StreamMeta | null): string | null {
  if (!stream || stream.bit_depth <= 0 || stream.sample_rate <= 0) return null;
  const khz = stream.sample_rate / 1000;
  return `${stream.bit_depth}/${Number.isInteger(khz) ? khz : khz.toFixed(1)}`;
}
