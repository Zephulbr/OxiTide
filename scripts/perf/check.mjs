#!/usr/bin/env node
// Budget gate for the Track 3 harness. Reads perf JSONL runs, aggregates
// metrics per scenario, compares with budgets.json, and exits non-zero on any
// failing budget. Usage:
//   node scripts/perf/check.mjs [--dir <runs-dir>] [--scenario <id>]
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));

/// A partial frame window (the interaction flush right after page open) can
/// hold ~95 frames, where p99 is effectively the max; ignore windows too
/// small to carry a percentile.
const FRAME_SAMPLE_MIN = 200;

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

/// Median of a numeric array (upper middle for even counts, a real sample).
function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

/// Soak drift anchored at a settled baseline: median of the last 5 min minus
/// the median of the 1-5 min window, so WebView2's process-start ramp is not
/// counted as a leak. Short runs with no settled window fall back to
/// last - first.
export function settledDrift(samples) {
  const start = samples[0].t;
  const end = samples[samples.length - 1].t;
  const baseline = samples.filter((m) => m.t >= start + 60_000 && m.t <= start + 300_000);
  const tail = samples.filter((m) => m.t >= end - 300_000);
  if (baseline.length === 0 || tail.length === 0) {
    return samples[samples.length - 1].mb - samples[0].mb;
  }
  return median(tail.map((m) => m.mb)) - median(baseline.map((m) => m.mb));
}

export function metricsFor(run) {
  const metrics = { scenario: run.scenario, cache_hits: 0, cache_misses: 0 };
  const frames = new Map(); // mode -> number[]
  const longTasks = [];
  const memories = []; // {t_ms, mb}
  const appMemories = []; // {t_ms, mb} from the self process rows
  const cacheDecisions = []; // 1 = hit, 0 = miss, in event order
  let advanceMax = 0;
  for (const line of run.lines) {
    if (line.kind === "mark") metrics[`${line.name}_ms`] = line.t_ms;
    if (line.kind === "ttfs") metrics.ttfs_ms = line.ms;
    if (line.kind === "memory") memories.push({ t: line.t_ms, mb: line.mb });
    if (line.kind === "memory_process" && line.self) {
      appMemories.push({ t: line.t_ms, mb: line.mb });
    }
    if (line.kind === "audio") {
      if (line.event === "cache_hit") {
        metrics.cache_hits += 1;
        cacheDecisions.push(1);
      }
      if (line.event === "cache_miss") {
        metrics.cache_misses += 1;
        cacheDecisions.push(0);
      }
      if (line.event === "advance") advanceMax = Math.max(advanceMax, line.gap_ms ?? 0);
      const first = (key) => {
        const metric = `${key}_ms`;
        if (metrics[metric] === undefined) metrics[metric] = line.t_ms;
      };
      if (line.event === "resolve_start") first("resolve_start");
      if (line.event === "decode_start") first("decode_start");
      if (line.event === "info_ready") first("info_ready");
      if (line.event === "output_opened") first("output_opened");
    }
    if (line.kind === "ui_metrics") {
      const mode = line.metrics?.mode ?? "instrument";
      if (line.metrics?.frames?.count >= FRAME_SAMPLE_MIN) {
        if (!frames.has(mode)) frames.set(mode, []);
        frames.get(mode).push(line.metrics.frames);
      }
      if (line.metrics?.long_tasks) longTasks.push(line.metrics.long_tasks);
      const interactions = line.metrics?.interactions;
      if (interactions) {
        // A count of zero carries a placeholder 0 ms; recording it would let a
        // run that measured nothing pass the required S6 rows.
        if (typeof interactions.filter_max_ms === "number" && interactions.filter_count > 0) {
          metrics.filter_max_ms = Math.max(metrics.filter_max_ms ?? 0, interactions.filter_max_ms);
        }
        if (typeof interactions.sort_max_ms === "number" && interactions.sort_count > 0) {
          metrics.sort_max_ms = Math.max(metrics.sort_max_ms ?? 0, interactions.sort_max_ms);
        }
        if (typeof interactions.queue_max_ms === "number" && interactions.queue_count > 0) {
          metrics.queue_build_ms = Math.max(metrics.queue_build_ms ?? 0, interactions.queue_max_ms);
        }
        if (typeof interactions.page_max_ms === "number" && interactions.page_count > 0) {
          metrics.page_open_ms = Math.max(metrics.page_open_ms ?? 0, interactions.page_max_ms);
        }
        if (typeof interactions.album_max_ms === "number" && interactions.album_count > 0) {
          metrics.album_open_ms = Math.max(metrics.album_open_ms ?? 0, interactions.album_max_ms);
        }
      }
    }
  }
  // The spec's pre-paint span rule: no application-owned startup step may
  // block the first frame for >100 ms. Runtime-owned spans (loader → first
  // mark, Tauri/WebView2 construction, dev-bundle render) are excluded.
  const appSteps = [
    ["logging_ready", "config_loaded"],
    ["config_loaded", "cache_ready"],
    ["cache_ready", "services_ready"],
  ]
    .filter(
      ([from, to]) =>
        metrics[`${from}_ms`] !== undefined && metrics[`${to}_ms`] !== undefined,
    )
    .map(([from, to]) => metrics[`${to}_ms`] - metrics[`${from}_ms`]);
  if (appSteps.length > 0) metrics.startup_step_max_ms = Math.max(...appSteps);
  if (advanceMax > 0) metrics.advance_max_gap_ms = advanceMax;
  if (memories.length > 0) {
    metrics.memory_max_mb = Math.max(...memories.map((m) => m.mb));
    metrics.memory_drift_mb = settledDrift(memories);
  }
  if (appMemories.length > 0) {
    metrics.memory_drift_app_mb = settledDrift(appMemories);
  }
  for (const [mode, samples] of frames) {
    metrics[`frame_p95_ms:${mode}`] = Math.max(...samples.map((s) => s.p95));
    const p99s = samples.map((s) => s.p99).filter((v) => typeof v === "number");
    if (p99s.length > 0) metrics[`frame_p99_ms:${mode}`] = Math.max(...p99s);
  }
  if (longTasks.length > 0) {
    metrics.long_task_max_ms = Math.max(...longTasks.map((t) => t.max_ms));
  }
  if (metrics.resolve_start_ms !== undefined && metrics.decode_start_ms !== undefined) {
    metrics.resolve_ms = metrics.decode_start_ms - metrics.resolve_start_ms;
  }
  if (metrics.decode_start_ms !== undefined && metrics.info_ready_ms !== undefined) {
    metrics.fetch_ms = metrics.info_ready_ms - metrics.decode_start_ms;
  }
  if (metrics.info_ready_ms !== undefined && metrics.output_opened_ms !== undefined) {
    metrics.output_open_ms = metrics.output_opened_ms - metrics.info_ready_ms;
  }
  if (cacheDecisions.length > 0) {
    const tail = cacheDecisions.slice(-10);
    // S8: second-pass hit rate.
    metrics.cache_hit_rate_last10 = tail.reduce((a, b) => a + b, 0) / tail.length;
  }
  const totalCache = metrics.cache_hits + metrics.cache_misses;
  if (totalCache > 0) metrics.cache_hit_rate = metrics.cache_hits / totalCache;
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
      out[`${key}_min`] = Math.min(...values);
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
      const useMin = budget.min !== undefined;
      const value = useMin
        ? (run.metrics[`${budget.metric}_min`] ?? run.metrics[budget.metric])
        : run.metrics[budget.metric];
      const maxOk = budget.max === undefined || (value !== undefined && value <= budget.max);
      const minOk = budget.min === undefined || (value !== undefined && value >= budget.min);
      const status =
        value === undefined
          ? budget.required
            ? "fail"
            : "unmeasured"
          : maxOk && minOk
            ? "pass"
            : "fail";
      results.push({
        scenario: run.scenario,
        metric: budget.metric,
        value,
        max: budget.max ?? budget.min,
        bound: useMin ? "min" : "max",
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
      `${result.status.toUpperCase().padEnd(10)} ${result.scenario.padEnd(16)} ${result.metric.padEnd(28)} ${value}  (${result.bound} ${result.max})`,
    );
  }
  process.exit(ok ? 0 : 1);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main();
}
