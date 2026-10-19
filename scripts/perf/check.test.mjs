import { test } from "node:test";
import assert from "node:assert/strict";
import {
  aggregate,
  cpuPercent,
  evaluate,
  metricsFor,
  metricsTable,
  pageMetrics,
  perMinute,
  readRuns,
  unmatchedScenarios,
} from "./check.mjs";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

function runFile(lines) {
  const dir = mkdtempSync(join(tmpdir(), "oxitide-perf-check-"));
  const file = join(dir, "S1-1.jsonl");
  writeFileSync(file, lines.map((l) => JSON.stringify(l)).join("\n") + "\n");
  return file;
}

test("readRuns groups lines by run id", () => {
  const file = runFile([
    { kind: "session", run_id: "S1-1", scenario: "S1" },
    { kind: "mark", name: "first_paint", t_ms: 900, unix_ms: 1 },
    { kind: "mark", name: "ui_ready", t_ms: 2000, unix_ms: 2 },
  ]);
  const runs = readRuns([file]);
  assert.equal(runs.length, 1);
  assert.equal(runs[0].id, "S1-1");
  assert.equal(runs[0].lines.length, 3);
});

test("metricsFor derives mark, ttfs, frame, and memory metrics", () => {
  const file = runFile([
    { kind: "session", run_id: "S3-hit-1", scenario: "S3-hit" },
    { kind: "mark", name: "first_paint", t_ms: 800, unix_ms: 1 },
    { kind: "ttfs", ms: 250, t_ms: 900 },
    { kind: "audio", event: "cache_hit", track_id: 1, t_ms: 890 },
    { kind: "audio", event: "advance", track_id: 2, gap_ms: 40, t_ms: 2000 },
    { kind: "memory", mb: 300, t_ms: 100 },
    { kind: "memory", mb: 340, t_ms: 5000 },
    { kind: "ui_metrics", t_ms: 5100, metrics: { frames: { count: 700, p50: 4, p95: 10, p99: 12, max: 20 }, long_tasks: { count: 0, max_ms: 0 }, mode: "instrument" } },
  ]);
  const [run] = readRuns([file]);
  const metrics = metricsFor(run);
  assert.equal(metrics.first_paint_ms, 800);
  assert.equal(metrics.ttfs_ms, 250);
  assert.equal(metrics.cache_hits, 1);
  assert.equal(metrics.advance_max_gap_ms, 40);
  assert.equal(metrics.memory_max_mb, 340);
  assert.equal(metrics.memory_drift_mb, 40);
  assert.equal(metrics["frame_p95_ms:instrument"], 10);
  assert.equal(metrics["frame_p99_ms:instrument"], 12);
});

test("ttfs attribution derives resolve, fetch, and open stages", () => {
  const file = runFile([
    { kind: "session", run_id: "S3-miss-1", scenario: "S3-miss" },
    { kind: "audio", event: "resolve_start", track_id: 1, t_ms: 100 },
    { kind: "audio", event: "decode_start", track_id: 1, t_ms: 300 },
    { kind: "audio", event: "info_ready", track_id: 1, t_ms: 900 },
    { kind: "audio", event: "output_opened", track_id: 1, t_ms: 1000 },
    { kind: "audio", event: "first_pcm", track_id: 1, t_ms: 1040 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.resolve_ms, 200);
  assert.equal(metrics.fetch_ms, 600);
  assert.equal(metrics.output_open_ms, 100);
});

test("evaluate flags a budget breach and passes within budget", () => {
  const config = {
    S1: [{ metric: "first_paint_ms", max: 1500 }],
  };
  const pass = evaluate([{ scenario: "S1", metrics: { first_paint_ms: 900 } }], config);
  assert.equal(pass.ok, true);
  const fail = evaluate([{ scenario: "S1", metrics: { first_paint_ms: 1600 } }], config);
  assert.equal(fail.ok, false);
  const unmeasured = evaluate([{ scenario: "S1", metrics: {} }], config);
  assert.equal(unmeasured.ok, true);
  assert.equal(unmeasured.results[0].status, "unmeasured");
});

test("evaluate supports a minimum floor", () => {
  const config = { S8: [{ metric: "cache_hit_rate", min: 0.9 }] };
  const pass = evaluate([{ scenario: "S8", metrics: { cache_hit_rate: 0.95 } }], config);
  assert.equal(pass.ok, true);
  const fail = evaluate([{ scenario: "S8", metrics: { cache_hit_rate: 0.5 } }], config);
  assert.equal(fail.ok, false);
});

test("unmatchedScenarios treats explicit empty budgets as informational, not missing", () => {
  const config = { S7: [], S1: [{ metric: "first_paint_ms", max: 1500 }] };
  const runs = [{ scenario: "S7" }, { scenario: "S1" }, { scenario: "S9" }];
  const { missing, informational } = unmatchedScenarios(runs, config);
  assert.deepEqual(informational, ["S7"]);
  assert.deepEqual(missing, ["S9"]);
});

test("explicit empty budgets yield ok with no results (informational exit path)", () => {
  const { ok, results } = evaluate([{ scenario: "S7", metrics: {} }], { S7: [] });
  assert.equal(ok, true);
  assert.equal(results.length, 0);
});

test("a run with no cache hits fails a cache_hits floor instead of going unmeasured", () => {
  const file = runFile([
    { kind: "session", run_id: "S3-hit-2", scenario: "S3-hit" },
    { kind: "ttfs", ms: 260, t_ms: 900 },
    { kind: "audio", event: "cache_miss", track_id: 1, t_ms: 890 },
  ]);
  const [run] = readRuns([file]);
  const metrics = metricsFor(run);
  assert.equal(metrics.cache_hits, 0);
  assert.equal(metrics.cache_misses, 1);
  const { ok, results } = evaluate([{ scenario: "S3-hit", metrics }], {
    "S3-hit": [{ metric: "cache_hits", min: 1 }],
  });
  assert.equal(results[0].status, "fail");
  assert.equal(ok, false);
});

test("cache_hit_rate_last10 measures only the tail decisions", () => {
  const events = [];
  for (let i = 0; i < 5; i++) events.push({ kind: "audio", event: "cache_miss", track_id: i, t_ms: i });
  for (let i = 0; i < 10; i++) events.push({ kind: "audio", event: "cache_hit", track_id: i, t_ms: 10 + i });
  const file = runFile([{ kind: "session", run_id: "S8-1", scenario: "S8" }, ...events]);
  const [run] = readRuns([file]);
  const metrics = metricsFor(run);
  assert.equal(metrics.cache_hits, 10);
  assert.equal(metrics.cache_misses, 5);
  assert.equal(metrics.cache_hit_rate, 10 / 15);
  assert.equal(metrics.cache_hit_rate_last10, 1);
});

function hitFile(runId, hits, misses) {
  const lines = [{ kind: "session", run_id: runId, scenario: "S8" }];
  for (let i = 0; i < hits; i++) lines.push({ kind: "audio", event: "cache_hit", track_id: i, t_ms: i });
  for (let i = 0; i < misses; i++) lines.push({ kind: "audio", event: "cache_miss", track_id: i, t_ms: 100 + i });
  return runFile(lines);
}

test("min budgets use the minimum across runs while max budgets keep the p95", () => {
  const files = [0, 2, 2, 2, 2].map((hits, i) => hitFile(`S8-${i}`, hits, 0));
  const aggregated = [...aggregate(readRuns(files)).values()].map((m) => ({ scenario: m.scenario, metrics: m }));
  const fail = evaluate(aggregated, { S8: [{ metric: "cache_hits", min: 1 }] });
  assert.equal(fail.results[0].value, 0);
  assert.equal(fail.results[0].status, "fail");
  assert.equal(fail.ok, false);
  const maxFail = evaluate(aggregated, { S8: [{ metric: "cache_hits", max: 1 }] });
  assert.equal(maxFail.results[0].value, 2);
  assert.equal(maxFail.results[0].status, "fail");
  const maxPass = evaluate(aggregated, { S8: [{ metric: "cache_hits", max: 2 }] });
  assert.equal(maxPass.results[0].status, "pass");
});

test("median budgets check the median run, so one noisy run does not fail them", () => {
  const files = [1, 1, 2, 2, 9].map((hits, i) => hitFile(`S8-${i}`, hits, 0));
  const aggregated = [...aggregate(readRuns(files)).values()].map((m) => ({ scenario: m.scenario, metrics: m }));
  const median = evaluate(aggregated, { S8: [{ metric: "cache_hits", max: 2, of: "median" }] });
  assert.equal(median.results[0].value, 2);
  assert.equal(median.results[0].status, "pass");
  assert.equal(median.results[0].bound, "median max");
  const worst = evaluate(aggregated, { S8: [{ metric: "cache_hits", max: 2 }] });
  assert.equal(worst.results[0].value, 9);
  assert.equal(worst.results[0].status, "fail");
  const regressed = evaluate(aggregated, { S8: [{ metric: "cache_hits", max: 1, of: "median" }] });
  assert.equal(regressed.results[0].status, "fail");
});

test("ui interaction metrics aggregate S6 filter and sort maxima", () => {
  const file = runFile([
    { kind: "session", run_id: "S6-1", scenario: "S6" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: {
        interactions: { filter_count: 2, filter_max_ms: 18, sort_count: 1, sort_max_ms: 40 },
      },
    },
    {
      kind: "ui_metrics",
      t_ms: 2,
      metrics: {
        interactions: { filter_count: 1, filter_max_ms: 27, sort_count: 1, sort_max_ms: 33 },
      },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.filter_max_ms, 27);
  assert.equal(metrics.sort_max_ms, 40);
});

test("startup step metric measures the largest app-owned pre-paint step", () => {
  const file = runFile([
    { kind: "session", run_id: "S1-2", scenario: "S1" },
    { kind: "mark", name: "logging_ready", t_ms: 390 },
    { kind: "mark", name: "config_loaded", t_ms: 391 },
    { kind: "mark", name: "cache_ready", t_ms: 392 },
    { kind: "mark", name: "services_ready", t_ms: 441 },
    { kind: "mark", name: "window_shown", t_ms: 805 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.startup_step_max_ms, 49);
});

test("zero-count interaction submissions do not fabricate zero metrics", () => {
  const file = runFile([
    { kind: "session", run_id: "S6-2", scenario: "S6" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: {
        interactions: { filter_count: 0, filter_max_ms: 0, sort_count: 0, sort_max_ms: 0 },
      },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.filter_max_ms, undefined);
  assert.equal(metrics.sort_max_ms, undefined);
});

test("a required budget fails when the metric is missing", () => {
  const { ok, results } = evaluate(
    [{ scenario: "S6", metrics: {} }],
    { S6: [{ metric: "filter_max_ms", max: 50, required: true }] },
  );
  assert.equal(results[0].status, "fail");
  assert.equal(ok, false);
  const optional = evaluate(
    [{ scenario: "S6", metrics: {} }],
    { S6: [{ metric: "filter_max_ms", max: 50 }] },
  );
  assert.equal(optional.results[0].status, "unmeasured");
  assert.equal(optional.ok, true);
});

test("page and album interactions aggregate into their metrics", () => {
  const file = runFile([
    { kind: "session", run_id: "S5-instrument-1", scenario: "S5-instrument" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: {
        mode: "instrument",
        interactions: {
          page_count: 1,
          page_max_ms: 210,
          album_count: 1,
          album_max_ms: 640,
          filter_count: 0,
          filter_max_ms: 0,
          sort_count: 0,
          sort_max_ms: 0,
          queue_count: 0,
          queue_max_ms: 0,
        },
      },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.page_open_ms, 210);
  assert.equal(metrics.album_open_ms, 640);
});

test("queue build samples aggregate into queue_build_ms", () => {
  const file = runFile([
    { kind: "session", run_id: "S7-1", scenario: "S7" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: {
        interactions: {
          queue_count: 1,
          queue_max_ms: 123,
          filter_count: 0,
          filter_max_ms: 0,
          sort_count: 0,
          sort_max_ms: 0,
        },
      },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.queue_build_ms, 123);
});

test("soak drift measures from a settled baseline, not the cold first sample", () => {
  const file = runFile([
    { kind: "session", run_id: "S9-1", scenario: "S9" },
    { kind: "memory", mb: 363, t_ms: 2500 },
    { kind: "memory", mb: 551, t_ms: 120000 },
    { kind: "memory", mb: 552, t_ms: 240000 },
    { kind: "memory", mb: 560, t_ms: 6000000 },
    { kind: "memory", mb: 565, t_ms: 7180000 },
    { kind: "memory", mb: 566, t_ms: 7190000 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.memory_max_mb, 566);
  assert.equal(metrics.memory_drift_mb, 14);
  assert.ok(metrics.memory_drift_mb < 50);
});

test("app-only soak drift uses the self process rows", () => {
  const file = runFile([
    { kind: "session", run_id: "S9-2", scenario: "S9" },
    { kind: "memory", mb: 363, t_ms: 2500 },
    { kind: "memory_process", name: "oxitide-app.exe", self: true, mb: 47, t_ms: 2500 },
    { kind: "memory_process", name: "msedgewebview2.exe", self: false, mb: 306, t_ms: 2500 },
    { kind: "memory", mb: 551, t_ms: 120000 },
    { kind: "memory_process", name: "oxitide-app.exe", self: true, mb: 85, t_ms: 120000 },
    { kind: "memory_process", name: "msedgewebview2.exe", self: false, mb: 456, t_ms: 120000 },
    { kind: "memory", mb: 566, t_ms: 7190000 },
    { kind: "memory_process", name: "oxitide-app.exe", self: true, mb: 86, t_ms: 7190000 },
    { kind: "memory_process", name: "msedgewebview2.exe", self: false, mb: 470, t_ms: 7190000 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.memory_drift_app_mb, 1);
});

test("partial frame windows do not set the frame percentiles", () => {
  const file = runFile([
    { kind: "session", run_id: "S5-instrument-1", scenario: "S5-instrument" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: {
        mode: "instrument",
        frames: { count: 95, p50: 7, p95: 7.1, p99: 41.7, max: 41.7 },
        long_tasks: { count: 0, max_ms: 0 },
      },
    },
    {
      kind: "ui_metrics",
      t_ms: 2,
      metrics: {
        mode: "instrument",
        frames: { count: 700, p50: 7, p95: 7.1, p99: 13.9, max: 34.8 },
        long_tasks: { count: 0, max_ms: 0 },
      },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics["frame_p95_ms:instrument"], 7.1);
  assert.equal(metrics["frame_p99_ms:instrument"], 13.9);
});

test("a run with only partial frame windows leaves the frame rows unmeasured", () => {
  const file = runFile([
    { kind: "session", run_id: "S5-instrument-2", scenario: "S5-instrument" },
    {
      kind: "ui_metrics",
      t_ms: 1,
      metrics: {
        mode: "instrument",
        frames: { count: 95, p50: 7, p95: 7.1, p99: 41.7, max: 41.7 },
        long_tasks: { count: 0, max_ms: 0 },
      },
    },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics["frame_p95_ms:instrument"], undefined);
  assert.equal(metrics["frame_p99_ms:instrument"], undefined);
});

test("app-only drift is required and unmeasured without per-process rows", () => {
  const file = runFile([
    { kind: "session", run_id: "S9-3", scenario: "S9" },
    { kind: "memory", mb: 500, t_ms: 120000 },
    { kind: "memory", mb: 520, t_ms: 7190000 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.memory_drift_app_mb, undefined);
  const { ok, results } = evaluate([{ scenario: "S9", metrics }], {
    S9: [{ metric: "memory_drift_app_mb", max: 25, required: true }],
  });
  assert.equal(results[0].status, "fail");
  assert.equal(ok, false);
});

test("cpuPercent sums per-process deltas by group over the window", () => {
  const rows = [
    { pid: 1, self: true, cpu_ms: 100, t_ms: 0 },
    { pid: 2, self: false, cpu_ms: 1000, t_ms: 0 },
    { pid: 1, self: true, cpu_ms: 300, t_ms: 10_000 },
    { pid: 2, self: false, cpu_ms: 1500, t_ms: 10_000 },
    // Seen once (started mid-run): contributes nothing.
    { pid: 3, self: false, cpu_ms: 9000, t_ms: 10_000 },
  ];
  const cpu = cpuPercent(rows);
  assert.equal(cpu.app, 2); // 200 ms over 10 s
  assert.equal(cpu.webview, 5); // 500 ms over 10 s
  assert.equal(cpuPercent([]), null);
  assert.equal(cpuPercent([{ pid: 1, self: true, cpu_ms: 1, t_ms: 5 }]), null);
});

test("perMinute rates a cumulative counter", () => {
  const rows = [
    { events: 10, t_ms: 0 },
    { events: 40, t_ms: 30_000 },
  ];
  assert.equal(perMinute(rows, "events"), 60);
  assert.equal(perMinute(rows.slice(0, 1), "events"), null);
});

test("metricsFor derives cpu, private and ipc rates from the measure_start mark", () => {
  const file = runFile([
    { kind: "session", run_id: "S11-1", scenario: "S11" },
    { kind: "memory", mb: 500, private_mb: 300, t_ms: 0 },
    { kind: "memory_process", pid: 1, self: true, cpu_ms: 5000, mb: 50, t_ms: 0 },
    { kind: "ipc", events: 100, bytes: 1_000_000, t_ms: 0 },
    { kind: "mark", name: "measure_start", t_ms: 1000, unix_ms: 1 },
    { kind: "memory", mb: 510, private_mb: 320, t_ms: 2000 },
    { kind: "memory_process", pid: 1, self: true, cpu_ms: 5100, mb: 50, t_ms: 2000 },
    { kind: "ipc", events: 100, bytes: 1_000_000, t_ms: 2000 },
    { kind: "memory_process", pid: 1, self: true, cpu_ms: 5300, mb: 50, t_ms: 62_000 },
    { kind: "ipc", events: 220, bytes: 1_102_400, t_ms: 62_000 },
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  assert.equal(metrics.private_max_mb, 320);
  // Start-up CPU before the mark is excluded: 200 ms over 60 s.
  assert.ok(Math.abs(metrics.cpu_app_pct - 200 / 60_000 * 100) < 1e-9);
  assert.equal(metrics.ipc_events_per_min, 120);
  assert.equal(metrics.ipc_kb_per_min, 100);
});
test("page counter rows become per-minute rates from the measure_start mark", () => {
  const page = (t_ms, label, n, animations) => ({
    kind: "ui_metrics",
    t_ms,
    metrics: {
      page: {
        label,
        style_recalcs: 100 + n,
        layouts: 50 + n / 2,
        script_s: 1 + n / 1000,
        task_s: 2 + n / 100,
        js_heap_bytes: (40 + n / 10) * 1048576,
        animations,
      },
    },
  });
  const file = runFile([
    { kind: "session", run_id: "S11-1", scenario: "S11" },
    page(0, "start", 0, []),
    { kind: "mark", name: "measure_start", t_ms: 10_000, unix_ms: 1 },
    page(10_000, "measure_start", 20, []),
    page(130_000, "end", 260, ["spin img.disc"]),
  ]);
  const metrics = metricsFor(readRuns([file])[0]);
  // 240 recalcs over the 2 min after the mark; the start row is outside it.
  assert.equal(metrics.page_style_per_min, 120);
  assert.equal(metrics.page_layouts_per_min, 60);
  assert.ok(Math.abs(metrics.page_script_ms_per_min - 120) < 1e-9);
  assert.ok(Math.abs(metrics.page_task_ms_per_min - 1200) < 1e-9);
  assert.equal(metrics.js_heap_max_mb, 66);
  assert.equal(metrics.animations_max, 1);
  // No rows (an older driver): nothing measured, nothing fabricated.
  assert.deepEqual(pageMetrics([]), {});
});

test("metricsTable lists median and worst per metric", () => {
  const runs = [1, 3, 2].map((v, i) => ({
    id: `S11-${i}`,
    scenario: "S11",
    lines: [
      { kind: "memory", mb: 500 + v * 10, t_ms: 0 },
      { kind: "memory", mb: 500, t_ms: 1000 },
    ],
  }));
  const aggregated = [...aggregate(runs).values()].map((m) => ({ scenario: m.scenario, metrics: m }));
  const lines = metricsTable(aggregated);
  assert.equal(lines[0], "S11 (3 run(s))  median / worst");
  assert.ok(lines.some((l) => /^\s+memory_max_mb\s+520 \/ 530$/.test(l)), lines.join("\n"));
  assert.ok(!lines.some((l) => l.includes("_median ")), "no derived keys");
});