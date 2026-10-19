/// Coalesces a stream of values to at most one `send` per frame, always with
/// the latest value (Track 13, EF-D2: a volume drag fires `input` far faster
/// than the engine needs, and each call publishes a playback state).
/// `flush` sends a value now and drops anything pending, for a final value
/// that must not be overtaken (a release that persists).
export interface LatestPerFrame<T> {
  push(value: T): void;
  flush(value: T): void;
}

type Schedule = (cb: () => void) => number;
type Cancel = (id: number) => void;

const defaultSchedule: Schedule = (cb) =>
  typeof requestAnimationFrame === "function"
    ? requestAnimationFrame(() => cb())
    : (setTimeout(cb, 16) as unknown as number);
const defaultCancel: Cancel = (id) =>
  typeof cancelAnimationFrame === "function" ? cancelAnimationFrame(id) : clearTimeout(id);

export function latestPerFrame<T>(
  send: (value: T) => void,
  schedule: Schedule = defaultSchedule,
  cancel: Cancel = defaultCancel,
): LatestPerFrame<T> {
  let pending: { value: T } | null = null;
  let frame: number | null = null;
  return {
    push(value) {
      pending = { value };
      if (frame !== null) return;
      frame = schedule(() => {
        frame = null;
        const next = pending;
        pending = null;
        if (next) send(next.value);
      });
    },
    flush(value) {
      if (frame !== null) cancel(frame);
      frame = null;
      pending = null;
      send(value);
    },
  };
}
