import { test } from "node:test";
import assert from "node:assert/strict";
import { evaluate, metricsFor, readRuns, unmatchedScenarios } from "./check.mjs";
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
    { kind: "ui_metrics", t_ms: 5100, metrics: { frames: { count: 4, p50: 4, p95: 10, p99: 12, max: 20 }, long_tasks: { count: 0, max_ms: 0 }, mode: "instrument" } },
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
