export function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const rank = Math.ceil((p / 100) * sorted.length);
  return sorted[Math.min(sorted.length - 1, Math.max(0, rank - 1))];
}

export interface FrameSummary {
  count: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

export class FrameSampler {
  private values: number[] = [];

  constructor(private cap = 5000) {}

  record(ms: number) {
    this.values.push(ms);
    if (this.values.length > this.cap) this.values.shift();
  }

  reset() {
    this.values = [];
  }

  summary(): FrameSummary {
    if (this.values.length === 0) return { count: 0, p50: 0, p95: 0, p99: 0, max: 0 };
    return {
      count: this.values.length,
      p50: percentile(this.values, 50),
      p95: percentile(this.values, 95),
      p99: percentile(this.values, 99),
      max: Math.max(...this.values),
    };
  }
}

export interface LongTaskSummary {
  count: number;
  max_ms: number;
}

export class LongTaskLog {
  private count = 0;
  private max = 0;

  record(duration: number) {
    this.count += 1;
    this.max = Math.max(this.max, duration);
  }

  reset() {
    this.count = 0;
    this.max = 0;
  }

  summary(): LongTaskSummary {
    return { count: this.count, max_ms: this.max };
  }
}

export interface HudSnapshotLike {
  run_id: string;
  scenario: string;
  memory_mb: number | null;
  last_ttfs_ms: number | null;
  cache_hits: number;
  cache_misses: number;
}

export function formatHudLines(
  snapshot: HudSnapshotLike,
  frames: FrameSummary,
  longTasks: LongTaskSummary,
): string[] {
  const fixed = (v: number) => v.toFixed(1);
  return [
    `${snapshot.scenario}  ${snapshot.run_id}`,
    `frame p50 ${fixed(frames.p50)} p95 ${fixed(frames.p95)} p99 ${fixed(frames.p99)} max ${fixed(frames.max)} (n=${frames.count})`,
    `longtasks ${longTasks.count} max ${fixed(longTasks.max_ms)} ms`,
    `memory ${snapshot.memory_mb ?? "?"} MB  TTFS ${snapshot.last_ttfs_ms ?? "?"} ms`,
    `cache hit ${snapshot.cache_hits} miss ${snapshot.cache_misses}`,
  ];
}

export interface InteractionSummary {
  filter_count: number;
  filter_max_ms: number;
  sort_count: number;
  sort_max_ms: number;
  queue_count: number;
  queue_max_ms: number;
}

export type InteractionKind = "filter" | "sort" | "queue";

/// Timed user interactions (S6 filter/sort, S7 queue build) for the
/// ui_metrics submission.
export class InteractionLog {
  private filterCount = 0;
  private filterMax = 0;
  private sortCount = 0;
  private sortMax = 0;
  private queueCount = 0;
  private queueMax = 0;

  record(kind: InteractionKind, ms: number) {
    if (kind === "filter") {
      this.filterCount += 1;
      this.filterMax = Math.max(this.filterMax, ms);
    } else if (kind === "sort") {
      this.sortCount += 1;
      this.sortMax = Math.max(this.sortMax, ms);
    } else {
      this.queueCount += 1;
      this.queueMax = Math.max(this.queueMax, ms);
    }
  }

  reset() {
    this.filterCount = 0;
    this.filterMax = 0;
    this.sortCount = 0;
    this.sortMax = 0;
    this.queueCount = 0;
    this.queueMax = 0;
  }

  summary(): InteractionSummary {
    return {
      filter_count: this.filterCount,
      filter_max_ms: this.filterMax,
      sort_count: this.sortCount,
      sort_max_ms: this.sortMax,
      queue_count: this.queueCount,
      queue_max_ms: this.queueMax,
    };
  }
}
