import { describe, expect, it } from "vitest";
import { latestPerFrame } from "./latestPerFrame";

function manualFrames() {
  const queued = new Map<number, () => void>();
  let next = 1;
  return {
    schedule: (cb: () => void) => {
      queued.set(next, cb);
      return next++;
    },
    cancel: (id: number) => void queued.delete(id),
    run() {
      const callbacks = [...queued.values()];
      queued.clear();
      for (const cb of callbacks) cb();
    },
    get size() {
      return queued.size;
    },
  };
}

describe("latestPerFrame", () => {
  it("sends only the latest value once per frame", () => {
    const frames = manualFrames();
    const sent: number[] = [];
    const volume = latestPerFrame<number>((v) => sent.push(v), frames.schedule, frames.cancel);
    for (const v of [0.1, 0.2, 0.3, 0.4]) volume.push(v);
    expect(sent).toEqual([]);
    expect(frames.size).toBe(1);
    frames.run();
    expect(sent).toEqual([0.4]);
    volume.push(0.5);
    frames.run();
    expect(sent).toEqual([0.4, 0.5]);
    frames.run(); // an idle frame sends nothing
    expect(sent).toEqual([0.4, 0.5]);
  });

  it("flush sends at once and a pending value never overtakes it", () => {
    const frames = manualFrames();
    const sent: number[] = [];
    const volume = latestPerFrame<number>((v) => sent.push(v), frames.schedule, frames.cancel);
    volume.push(0.2);
    volume.flush(0.9);
    frames.run();
    expect(sent).toEqual([0.9]);
  });
});
