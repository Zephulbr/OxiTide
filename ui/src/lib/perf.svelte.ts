import { api, type PerfSnapshot } from "./api";
import { appearance } from "./appearance.svelte";
import { FrameSampler, InteractionLog, LongTaskLog, type InteractionKind } from "./perf";

export function createPerf() {
  let active = $state(false);
  let hudVisible = $state(true);
  let snapshot = $state<PerfSnapshot | null>(null);
  const frames = new FrameSampler();
  const longTasks = new LongTaskLog();
  const interactions = new InteractionLog();
  let started = false;
  let raf = 0;
  let lastFrame = 0;
  const timers: number[] = [];
  let observer: PerformanceObserver | null = null;

  async function refresh() {
    try {
      snapshot = await api.getPerfSnapshot();
    } catch {
      /* session may have ended */
    }
  }

  async function submit() {
    if (!active) return;
    try {
      await api.perfSubmitUiMetrics({
        scenario: snapshot?.scenario ?? "",
        mode: appearance.mode,
        frames: frames.summary(),
        long_tasks: longTasks.summary(),
        interactions: interactions.summary(),
      });
    } catch {
      /* never disturb the app for metrics */
    }
  }

  async function init() {
    if (started) return;
    started = true;
    try {
      snapshot = await api.getPerfSnapshot();
    } catch {
      return;
    }
    if (!snapshot.active) return;
    active = true;
    lastFrame = performance.now();
    const tick = (now: number) => {
      frames.record(now - lastFrame);
      lastFrame = now;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    requestAnimationFrame(() => mark("first_paint"));
    try {
      observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) longTasks.record(entry.duration);
      });
      observer.observe({ entryTypes: ["longtask"] });
    } catch {
      observer = null;
    }
    timers.push(window.setInterval(() => void refresh(), 2000));
    timers.push(window.setInterval(() => void submit(), 5000));
  }

  function mark(name: string) {
    void api.perfMark(name).catch(() => {});
  }

  /// Record a timed interaction and flush immediately: the run ends as soon as
  /// the driver's scenario does, so the periodic submit could lose the last one.
  function recordInteraction(kind: InteractionKind, ms: number) {
    interactions.record(kind, ms);
    void submit();
  }

  function toggleHud() {
    hudVisible = !hudVisible;
  }

  function dispose() {
    cancelAnimationFrame(raf);
    observer?.disconnect();
    for (const timer of timers) clearInterval(timer);
  }

  return {
    get active() { return active; },
    get hudVisible() { return hudVisible; },
    get snapshot() { return snapshot; },
    get frames() { return frames; },
    get longTasks() { return longTasks; },
    init,
    mark,
    recordInteraction,
    toggleHud,
    dispose,
  };
}

export const perf = createPerf();
