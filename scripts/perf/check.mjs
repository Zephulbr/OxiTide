#!/usr/bin/env node
// Budget gate for the Track 3 harness. Reads perf JSONL runs, aggregates
// metrics per scenario, compares with budgets.json, and exits non-zero on any
// failing budget. Usage:
//   node scripts/perf/check.mjs [--dir <runs-dir>] [--scenario <id>]
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

export function readRuns(files) {
  const runs = new Map();
  for (const file of files) {
    let text;
    try {
      text = readFileSync(file, "utf8");
    } catch {
      continue;
    }
    let currentRunId = null;
    for (const raw of text.split("\n")) {
      if (!raw.trim()) continue;
      let line;
      try {
        line = JSON.parse(raw);
      } catch {
        continue; // a killed process may leave one truncated final line
      }
      if (line.run_id) currentRunId = line.run_id;
      const key = currentRunId ?? file;
      if (!runs.has(key)) {
        runs.set(key, { id: key, scenario: line.scenario ?? "unknown", lines: [], file });
      }
      const run = runs.get(key);
      if (line.scenario) run.scenario = line.scenario;
      run.lines.push(line);
    }
  }
  return [...runs.values()];
}

export function metricsFor(run) {
  const metrics = { scenario: run.scenario };
  const frames = new Map(); // mode -> number[]
  const longTasks = [];
  const memories = []; // {t_ms, mb}
  let advanceMax = 0;
  for (const line of run.lines) {
    if (line.kind === "mark") metrics[`${line.name}_ms`] = line.t_ms;
    if (line.kind === "ttfs") metrics.ttfs_ms = line.ms;
    if (line.kind === "memory") memories.push({ t: line.t_ms, mb: line.mb });
    if (line.kind === "audio") {
      if (line.event === "cache_hit") metrics.cache_hits = (metrics.cache_hits ?? 0) + 1;
      if (line.event === "cache_miss") metrics.cache_misses = (metrics.cache_misses ?? 0) + 1;
      if (line.event === "advance") advanceMax = Math.max(advanceMax, line.gap_ms ?? 0);
    }
    if (line.kind === "ui_metrics") {
      const mode = line.metrics?.mode ?? "instrument";
      if (line.metrics?.frames?.count) {
        if (!frames.has(mode)) frames.set(mode, []);
        frames.get(mode).push(line.metrics.frames);
      }
      if (line.metrics?.long_tasks) longTasks.push(line.metrics.long_tasks);
    }
  }
  if (advanceMax > 0) metrics.advance_max_gap_ms = advanceMax;
  if (memories.length > 0) {
    metrics.memory_max_mb = Math.max(...memories.map((m) => m.mb));
    const first = memories[0].mb;
    const last = memories[memories.length - 1].mb;
    metrics.memory_drift_mb = last - first;
  }
  for (const [mode, samples] of frames) {
    metrics[`frame_p95_ms:${mode}`] = Math.max(...samples.map((s) => s.p95));
  }
  if (longTasks.length > 0) {
    metrics.long_task_max_ms = Math.max(...longTasks.map((t) => t.max_ms));
  }
  const totalCache = (metrics.cache_hits ?? 0) + (metrics.cache_misses ?? 0);
  if (totalCache > 0) metrics.cache_hit_rate = (metrics.cache_hits ?? 0) / totalCache;
  return metrics;
}

function p95(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(0.95 * sorted.length) - 1)];
}

export function aggregate(runs) {
  const byScenario = new Map();
  for (const run of runs) {
    if (!byScenario.has(run.scenario)) byScenario.set(run.scenario, []);
    byScenario.get(run.scenario).push(metricsFor(run));
  }
  const aggregated = new Map();
  for (const [scenario, samples] of byScenario) {
    const out = { scenario, samples: samples.length };
    const keys = new Set(samples.flatMap((s) => Object.keys(s)));
    for (const key of keys) {
      if (key === "scenario") continue;
      const values = samples.map((s) => s[key]).filter((v) => typeof v === "number");
      if (values.length === 0) continue;
      out[key] = p95(values);
      out[`${key}_mean`] = values.reduce((a, b) => a + b, 0) / values.length;
    }
    aggregated.set(scenario, out);
  }
  return aggregated;
}

export function evaluate(aggregatedRuns, config) {
  const results = [];
  for (const run of aggregatedRuns) {
    const budgets = config[run.scenario];
    if (!budgets) continue;
    for (const budget of budgets) {
      const value = run.metrics[budget.metric];
      const maxOk = budget.max === undefined || (value !== undefined && value <= budget.max);
      const minOk = budget.min === undefined || (value !== undefined && value >= budget.min);
      const status =
        value === undefined ? "unmeasured" : maxOk && minOk ? "pass" : "fail";
      results.push({
        scenario: run.scenario,
        metric: budget.metric,
        value,
        max: budget.max ?? budget.min,
        status,
      });
    }
  }
  return { ok: results.every((r) => r.status !== "fail"), results };
}

export function unmatchedScenarios(runs, config) {
  const scenarios = [...new Set(runs.map((r) => r.scenario))];
  return {
    missing: scenarios.filter((s) => !Object.hasOwn(config, s)),
    informational: scenarios.filter(
      (s) => Object.hasOwn(config, s) && (config[s] ?? []).length === 0,
    ),
  };
}

function main() {
  const args = process.argv.slice(2);
  const dirFlag = args.indexOf("--dir");
  const scenarioFlag = args.indexOf("--scenario");
  const dir = dirFlag >= 0 ? args[dirFlag + 1] : join(HERE, "../../.superpowers/perf/runs");
  const onlyScenario = scenarioFlag >= 0 ? args[scenarioFlag + 1] : null;
  const config = JSON.parse(readFileSync(join(HERE, "budgets.json"), "utf8"));
  let files;
  try {
    files = readdirSync(dir).filter((f) => f.endsWith(".jsonl")).map((f) => join(dir, f));
  } catch (e) {
    console.error(`perf runs directory unavailable: ${dir} (${e.message})`);
    process.exit(1);
  }
  let runs = readRuns(files);
  if (onlyScenario) runs = runs.filter((r) => r.scenario === onlyScenario);
  const aggregated = [...aggregate(runs).values()].map((m) => ({ scenario: m.scenario, metrics: m }));
  const { ok, results } = evaluate(aggregated, config);
  if (results.length === 0) {
    const { missing, informational } = unmatchedScenarios(runs, config);
    if (missing.length > 0 || informational.length === 0) {
      console.error(`no budgets matched ${runs.length} run(s) in ${dir}`);
      process.exit(1);
    }
    for (const scenario of informational) {
      console.log(`INFO       ${scenario.padEnd(16)} no metric budgets (informational)`);
    }
    process.exit(0);
  }
  for (const result of results) {
    const value = result.value === undefined ? "n/a" : result.value.toFixed(1);
    console.log(
      `${result.status.toUpperCase().padEnd(10)} ${result.scenario.padEnd(16)} ${result.metric.padEnd(28)} ${value}  (max ${result.max})`,
    );
  }
  process.exit(ok ? 0 : 1);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
