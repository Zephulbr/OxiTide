#!/usr/bin/env node
// Drives a scripted perf scenario through the WebView2 CDP endpoint.
// Requires Node 22+ (global WebSocket). Exits non-zero on failure.
import { setTimeout as delay } from "node:timers/promises";

const args = process.argv.slice(2);
const flag = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : fallback;
};
const scenario = flag("--scenario");
const port = Number(flag("--port", "9222"));
const timeoutMs = Number(flag("--timeout", "120000"));

if (!scenario) {
  console.error("usage: node drive.mjs --scenario <id> [--port 9222] [--timeout 120000]");
  process.exit(2);
}

async function connect() {
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
      const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl);
      if (page) return page.webSocketDebuggerUrl;
    } catch {
      /* not up yet */
    }
    await delay(500);
  }
  throw new Error(`no CDP page target on port ${port} within 30 s`);
}

const wsUrl = await connect();
const ws = new WebSocket(wsUrl);
const pending = new Map();
let nextId = 1;
ws.addEventListener("message", (event) => {
  const msg = JSON.parse(event.data);
  const resolve = pending.get(msg.id);
  if (resolve) {
    pending.delete(msg.id);
    resolve(msg);
  }
});
await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, { once: true });
  ws.addEventListener("error", reject, { once: true });
});

function send(method, params = {}) {
  const id = nextId++;
  return new Promise((resolve, reject) => {
    pending.set(id, (msg) => (msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result)));
    ws.send(JSON.stringify({ id, method, params }));
    setTimeout(() => {
      if (pending.delete(id)) reject(new Error(`${method} timed out`));
    }, 30000);
  });
}

async function evaluate(expression) {
  const result = await send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    const exception = result.exceptionDetails.exception;
    const detail = exception?.description ?? exception?.value ?? result.exceptionDetails.text;
    throw new Error(`evaluate failed: ${String(detail).split("\n")[0]}`);
  }
  return result.result.value;
}

const until = async (expression, label, maxMs = 30000) => {
  const deadline = Date.now() + maxMs;
  while (Date.now() < deadline) {
    if (await evaluate(expression)) return;
    await delay(500);
  }
  throw new Error(`timed out waiting for ${label}`);
};

const awaitLoggedIn = () =>
  until(
    '(async () => window.__TAURI_INTERNALS__ && (await window.__TAURI_INTERNALS__.invoke("auth_status")))()',
    "login",
    60000,
  );

const setUiMode = async (mode) => {
  const viaBridge = await evaluate('typeof window.__perf?.setMode === "function"');
  if (viaBridge) {
    await evaluate(`window.__perf.setMode(${JSON.stringify(mode)})`);
  } else {
    await evaluate(`window.__TAURI_INTERNALS__.invoke("set_ui_mode", { mode: ${JSON.stringify(mode)} })`);
  }
};

const setTheme = async (theme) => {
  const viaBridge = await evaluate('typeof window.__perf?.setTheme === "function"');
  if (viaBridge) {
    await evaluate(`window.__perf.setTheme(${JSON.stringify(theme)})`);
  } else {
    await evaluate(
      `window.__TAURI_INTERNALS__.invoke("set_theme", { theme: ${JSON.stringify(theme)} })`,
    );
  }
};

const invoke = (cmd, args) =>
  evaluate(
    args === undefined
      ? `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)})`
      : `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(cmd)}, ${JSON.stringify(args)})`,
  );

const trackToQueueItem = (t) => ({
  track_id: t.id,
  title: t.title,
  artist: (t.artists ?? []).map((a) => a.name).join(", "),
  album: t.album?.title ?? "",
  duration_secs: t.duration,
  cover: t.album?.cover ?? null,
});

const getFavoriteTracks = () => invoke("get_favorite_tracks");

const playItems = (items, args = {}) => invoke("play_tracks", { items, start: 0, shuffle: false, ...args });

/// Playing means the position moves. `playing && position_ms > 0` alone also
/// held for a session stuck paused at a restored position, so silent runs
/// were measured as if they played.
const awaitPlaying = (label = "playback", maxMs = 30000) =>
  until(
    `(async () => {
      const get = () => window.__TAURI_INTERNALS__.invoke("get_playback_state");
      const a = await get();
      if (!a.playing) return false;
      await new Promise((r) => setTimeout(r, 700));
      const b = await get();
      return b.playing && b.current?.track_id === a.current?.track_id && b.position_ms > a.position_ms;
    })()`,
    label,
    maxMs,
  );

const awaitCurrentTrack = (trackId, label = `track ${trackId}`, maxMs = 30000) =>
  until(
    `(async () => { const s = await window.__TAURI_INTERNALS__.invoke("get_playback_state"); return !!s.current && s.current.track_id === ${JSON.stringify(trackId)}; })()`,
    label,
    maxMs,
  );

const requireTracks = (tracks, count) => {
  if (!Array.isArray(tracks) || tracks.length < count) {
    throw new Error(`need ${count} favorite track(s), got ${Array.isArray(tracks) ? tracks.length : 0}`);
  }
  return tracks;
};

/// Waits until a play that missed the cache has committed its entry. The old
/// "nothing pending and none seen" shortcut also held before the download
/// had started, so the replay missed too (S3-hit's 0-hit runs after S3-miss
/// cleared the cache). A play that hit needs no wait.
const awaitWarmCache = async (beforeBytes, label = "cache write", maxMs = 300000) => {
  const perf = await invoke("get_perf_snapshot");
  if (perf.cache_hits > 0) return;
  const deadline = Date.now() + maxMs;
  while (Date.now() < deadline) {
    const status = await invoke("get_cache_status");
    if (status.pending_bytes === 0 && status.used_bytes > beforeBytes) return;
    await delay(500);
  }
  throw new Error(`timed out waiting for ${label}`);
};

/// Chromium's cumulative page counters plus the animations running right now,
/// written as a `ui_metrics` row with a `page` field. The checker turns two
/// rows into per-minute rates, so work an idle window still does (style,
/// layout, script) shows up even with the frame sampler off.
const PAGE_COUNTERS = {
  LayoutCount: "layouts",
  RecalcStyleCount: "style_recalcs",
  LayoutDuration: "layout_s",
  RecalcStyleDuration: "style_s",
  ScriptDuration: "script_s",
  TaskDuration: "task_s",
  JSHeapUsedSize: "js_heap_bytes",
  Nodes: "nodes",
};
const snapPage = async (label) => {
  try {
    const { metrics } = await send("Performance.getMetrics");
    const page = { label };
    for (const { name, value } of metrics) {
      if (PAGE_COUNTERS[name]) page[PAGE_COUNTERS[name]] = value;
    }
    // Compositor-only animations never touch the counters above.
    page.animations = await evaluate(
      `document.getAnimations().filter((a) => a.playState === "running").map((a) => {
        const el = a.effect?.target;
        const what = el ? el.tagName.toLowerCase() + (el.classList.length ? "." + [...el.classList].join(".") : "") : "?";
        return (a.animationName ?? a.transitionProperty ?? a.constructor.name) + " " + what;
      }).slice(0, 20)`,
    );
    await invoke("perf_submit_ui_metrics", { metrics: { page } });
  } catch (e) {
    console.error(`page counters (${label}) unavailable: ${e.message}`);
  }
};

/// Starts the steady window: the mark the checker measures from, and the
/// first page-counter row.
const measureStart = async () => {
  await invoke("perf_mark", { name: "measure_start" });
  await snapPage("measure_start");
};

/// `--minutes` for the timed idle scenarios, else the scenario's default.
const minutesFlag = (fallback) => {
  const raw = Number(flag("--minutes", String(fallback)));
  return Number.isFinite(raw) && raw > 0 ? raw : fallback;
};

const scenarioFallback = async (id) => {
  const targetMode = id.startsWith("S5-atmosphere") || id === "S14"
    ? "atmosphere"
    : id.startsWith("S5") || id.startsWith("S6")
      ? "instrument"
      : id === "S13"
        ? "rack"
        : null;
  let originalMode = null;
  let modeApplied = false;
  /// Runs after login and `__perf` are up. The config read must happen here,
  /// not at connect time: WebView2 starts on about:blank and the eager read
  /// raced the navigation, silently skipping the pin (and the restore).
  const applyScenarioMode = async () => {
    if (!targetMode || modeApplied) return;
    try {
      originalMode = await evaluate(
        'window.__TAURI_INTERNALS__.invoke("get_config").then((c) => c.ui.mode)',
      );
    } catch {
      originalMode = null; // cannot restore -> leave the owner's setting untouched
    }
    if (!originalMode || originalMode === targetMode) return;
    modeApplied = true;
    try {
      await setUiMode(targetMode);
    } catch (e) {
      console.error(`mode switch to ${targetMode} failed: ${e.message}`);
    }
  };
  const targetTheme = id.endsWith("-light") ? "light" : null;
  let originalTheme = null;
  let themeApplied = false;
  const applyScenarioTheme = async () => {
    if (!targetTheme || themeApplied) return;
    try {
      originalTheme = await evaluate(
        'window.__TAURI_INTERNALS__.invoke("get_config").then((c) => c.ui.theme)',
      );
    } catch {
      originalTheme = null;
    }
    if (!originalTheme || originalTheme === targetTheme) return;
    themeApplied = true;
    try {
      await setTheme(targetTheme);
    } catch (e) {
      console.error(`theme switch to ${targetTheme} failed: ${e.message}`);
    }
  };
  try {
    // Whole-run window for scenarios without a `measure_start`.
    await until("!!window.__TAURI_INTERNALS__", "app page").then(
      () => snapPage("start"),
      () => {},
    );
    if (id.startsWith("S5")) {
      await awaitLoggedIn();
      await until("!!window.__perf", "__perf bridge");
      await applyScenarioMode();
      await applyScenarioTheme();
      if (targetTheme) {
        await until(
          `document.documentElement.dataset.theme === ${JSON.stringify(targetTheme)}`,
          `${targetTheme} theme pin`,
        );
      }
      await evaluate('window.__perf.goto("tracks")');
      await until('!!document.querySelector("[data-perf-scroll]")', "track table");
      await delay(3000);
      for (let i = 0; i < 12; i++) {
        await evaluate("window.__perf.scrollTracks(1200)");
        await delay(600);
      }
      await delay(3000);
    } else if (id.startsWith("S6")) {
      await awaitLoggedIn();
      await until("!!window.__perf", "__perf bridge");
      await applyScenarioMode();
      await evaluate('window.__perf.goto("tracks")');
      await until('!!document.querySelector("[data-perf-filter]")', "filter input");
      await until(
        'typeof window.__perf.loadSyntheticTracks === "function"',
        "synthetic fixture hook",
      );
      const loaded = await evaluate("window.__perf.loadSyntheticTracks(10000)");
      if (!loaded) throw new Error("S6: synthetic track fixture unavailable");
      await delay(2000); // let the 10k-row view render
      for (const query of ["a", "th", "the ", "the e", ""]) {
        const measured = await evaluate(`window.__perf.measureFilter(${JSON.stringify(query)})`);
        if (!measured) throw new Error("S6: filter measurement unavailable");
        await delay(300);
      }
      for (const key of ["title", "duration", "title"]) {
        const measured = await evaluate(`window.__perf.measureSort(${JSON.stringify(key)})`);
        if (!measured) throw new Error("S6: sort measurement unavailable");
        await delay(500);
      }
      await evaluate("window.__perf.gotoAlbum(0)");
      await until("window.__perf.albumHookReady()", "album hook");
      const albumMeasured = await evaluate("window.__perf.measureAlbumOpen(500)");
      if (!albumMeasured) throw new Error("S6: album measurement unavailable");
      await delay(2000);
      await delay(3000);
    } else if (id === "S7") {
      await awaitLoggedIn();
      const started = Date.now();
      const items = (await getFavoriteTracks()).map(trackToQueueItem);
      await playItems(items);
      console.log(`S7 invoked play_tracks for ${items.length} tracks`);
      console.log(`S7 command round-trip: ${Date.now() - started} ms`);
      await until("!!window.__perf", "__perf bridge");
      // End to end: command + engine state broadcast + IPC + UI reducer.
      for (let i = 0; i < 3; i++) {
        const loaded = await evaluate("window.__perf.measureQueueLoad(10000)");
        if (!loaded) throw new Error("S7: queue load measurement unavailable");
        await delay(1000);
      }
      await invoke("queue_clear");
      await delay(5000);
    } else if (id === "S11") {
      // Idle while paused: a track loaded and paused, nothing else happening.
      await awaitLoggedIn();
      const first = requireTracks(await getFavoriteTracks(), 1)[0];
      await playItems([trackToQueueItem(first)]);
      await awaitPlaying("S11 playback");
      await delay(3000);
      await invoke("play_pause");
      await until("!!window.__perf", "__perf bridge");
      await evaluate("window.__perf.sampleFrames(false)");
      await delay(10000);
      await measureStart();
      await delay(minutesFlag(5) * 60_000);
    } else if (id === "S12") {
      // Playing while hidden to the tray (close_to_tray on). The owner closes
      // the window when prompted; measurement starts 20 s later.
      await awaitLoggedIn();
      const items = requireTracks(await getFavoriteTracks(), 10).slice(0, 10).map(trackToQueueItem);
      await invoke("set_repeat", { mode: "all" });
      await playItems(items);
      await awaitPlaying("S12 playback");
      await until("!!window.__perf", "__perf bridge");
      await evaluate("window.__perf.sampleFrames(false)");
      console.log("S12: close the window to the tray now");
      await delay(20_000);
      await measureStart();
      await delay(minutesFlag(10) * 60_000);
      await invoke("set_repeat", { mode: "off" });
    } else if (id === "S13") {
      // Rack with its meters running while playing; S5-style scroll first.
      await awaitLoggedIn();
      await until("!!window.__perf", "__perf bridge");
      await applyScenarioMode();
      const first = requireTracks(await getFavoriteTracks(), 1)[0];
      // Repeat, so a short track cannot end inside the window.
      await invoke("set_repeat", { mode: "all" });
      await playItems([trackToQueueItem(first)]);
      await awaitPlaying("S13 playback");
      await evaluate('window.__perf.goto("tracks")');
      await until('!!document.querySelector("[data-perf-scroll]")', "track table");
      await delay(3000);
      await measureStart();
      for (let i = 0; i < 12; i++) {
        await evaluate("window.__perf.scrollTracks(1200)");
        await delay(600);
      }
      await delay(minutesFlag(2) * 60_000);
      // A run that went quiet measured idle meters, not Rack playing.
      await awaitPlaying("S13 still playing at the end", 3000);
      await invoke("set_repeat", { mode: "off" });
    } else if (id === "S14") {
      // Volume drag with a 10k queue loaded (paused, nothing fetched).
      await awaitLoggedIn();
      await until("!!window.__perf", "__perf bridge");
      await applyScenarioMode();
      const loaded = await evaluate("window.__perf.measureQueueLoad(10000)");
      if (!loaded) throw new Error("S14: queue load unavailable");
      await until('!!document.querySelector("[data-perf-volume]")', "volume slider");
      await delay(3000);
      await measureStart();
      for (let i = 0; i < 3; i++) {
        const swept = await evaluate("window.__perf.sweepVolume(120)");
        if (!swept) throw new Error("S14: volume slider unavailable");
        await delay(1000);
      }
      await delay(6000); // let the CPU/IPC samples cover the sweep
      await invoke("queue_clear");
    } else if (id.startsWith("S3-miss")) {
      await awaitLoggedIn();
      await invoke("clear_cache");
      const first = requireTracks(await getFavoriteTracks(), 1)[0];
      await playItems([trackToQueueItem(first)]);
      await awaitPlaying("S3-miss playback");
      await delay(2000);
    } else if (id.startsWith("S3-hit")) {
      await awaitLoggedIn();
      const beforeBytes = (await invoke("get_cache_status")).used_bytes;
      const first = requireTracks(await getFavoriteTracks(), 1)[0];
      const item = trackToQueueItem(first);
      await playItems([item]);
      await awaitPlaying("S3-hit first play");
      await delay(1000);
      await awaitWarmCache(beforeBytes, "S3-hit cache write");
      await playItems([item]);
      await awaitPlaying("S3-hit replay");
      await delay(2000);
    } else if (id.startsWith("S4")) {
      await awaitLoggedIn();
      await invoke("set_repeat", { mode: "off" });
      const items = requireTracks(await getFavoriteTracks(), 3)
        .slice()
        .sort((a, b) => (a.duration ?? 0) - (b.duration ?? 0))
        .slice(0, 3)
        .map(trackToQueueItem);
      const boundSecs = items[0].duration_secs + items[1].duration_secs + 90;
      console.log(
        `S4: tracks=[${items.map((i) => `${i.track_id}:${i.duration_secs}s`).join(", ")}] two-transition bound=${boundSecs}s`,
      );
      await playItems(items);
      await until(
        `(async () => { const s = await window.__TAURI_INTERNALS__.invoke("get_playback_state"); return s.playing && !!s.current && s.current.track_id === ${JSON.stringify(items[0].track_id)}; })()`,
        `S4 start track ${items[0].track_id}`,
        30000,
      );
      await awaitCurrentTrack(
        items[2].track_id,
        `S4 two natural transitions to track ${items[2].track_id}`,
        boundSecs * 1000,
      );
      await delay(1500);
    } else if (id.startsWith("S8")) {
      await awaitLoggedIn();
      const originalQuality = await evaluate(
        'window.__TAURI_INTERNALS__.invoke("get_config").then((c) => c.streaming.quality)',
      ).catch(() => null);
      try {
        await invoke("set_preferred_quality", { quality: "max" });
        await invoke("set_repeat", { mode: "all" });
        const items = requireTracks(await getFavoriteTracks(), 10)
          .slice()
          .sort((a, b) => (a.duration ?? 0) - (b.duration ?? 0))
          .slice(0, 10)
          .map(trackToQueueItem);
        await playItems(items);
        const totalSecs = items.reduce((sum, item) => sum + (item.duration_secs ?? 0), 0);
        console.log(`S8: 10 tracks, totalSecs=${totalSecs}, waiting ~${2 * totalSecs + 90}s`);
        await delay((2 * totalSecs + 90) * 1000);
      } finally {
        try {
          if (originalQuality) await invoke("set_preferred_quality", { quality: originalQuality });
          await invoke("set_repeat", { mode: "off" });
        } catch (e) {
          console.error(`S8 settings restore failed: ${e.message}`);
        }
      }
    } else if (id.startsWith("S9")) {
      const minutesRaw = Number(flag("--minutes", "120"));
      const minutes = Number.isFinite(minutesRaw) && minutesRaw > 0 ? minutesRaw : 120;
      await awaitLoggedIn();
      const originalQuality = await evaluate(
        'window.__TAURI_INTERNALS__.invoke("get_config").then((c) => c.streaming.quality)',
      ).catch(() => null);
      const items = requireTracks(await getFavoriteTracks(), 20).slice(0, 20).map(trackToQueueItem);
      try {
        await invoke("set_repeat", { mode: "all" });
        await playItems(items);
        const startedAt = Date.now();
        const deadline = startedAt + minutes * 60 * 1000;
        let iteration = 0;
        let highQuality = false;
        let lastLoggedMin = 0;
        while (Date.now() < deadline) {
          iteration += 1;
          await invoke("next");
          if (iteration % 4 === 0) {
            highQuality = !highQuality;
            await invoke("set_preferred_quality", { quality: highQuality ? "max" : "lossless" });
          }
          if (iteration % 10 === 0) {
            await invoke("play_pause");
            await delay(1000);
            await invoke("play_pause");
          }
          const elapsedMin = Math.floor((Date.now() - startedAt) / 60000);
          if (elapsedMin >= lastLoggedMin + 5) {
            lastLoggedMin = elapsedMin;
            console.log(`S9: ${elapsedMin} min elapsed`);
          }
          await delay(Math.min(45000, Math.max(0, deadline - Date.now())));
        }
        console.log(`S9: done after ${minutes} min`);
      } finally {
        try {
          if (originalQuality) await invoke("set_preferred_quality", { quality: originalQuality });
          await invoke("set_repeat", { mode: "off" });
        } catch (e) {
          console.error(`S9 settings restore failed: ${e.message}`);
        }
      }
    } else {
      // Start-only scenarios (S1/S2 and manual ones): idle while marks accumulate.
      await delay(Number(flag("--idle", "8000")));
    }
    await snapPage("end");
  } finally {
    if (modeApplied) {
      try {
        await setUiMode(originalMode);
      } catch (e) {
        console.error(`mode restore to ${originalMode} failed: ${e.message}`);
      }
    }
    if (themeApplied) {
      try {
        await setTheme(originalTheme);
      } catch (e) {
        console.error(`theme restore to ${originalTheme} failed: ${e.message}`);
      }
    }
  }
};

try {
  await send("Runtime.enable");
} catch {
  /* older targets may not need it */
}
try {
  await send("Performance.enable");
} catch {
  /* page counters stay unmeasured */
}

await scenarioFallback(scenario).catch((e) => {
  console.error(`driver failed: ${e.message}`);
  process.exitCode = 1;
});
// End through the normal exit path (session flush, history finalize); the
// runner force-kills only if the app is still up afterwards.
if (!args.includes("--no-quit")) {
  // The page goes away mid-call, so the reply may never come.
  await Promise.race([invoke("perf_quit").catch(() => {}), delay(3000)]);
}
ws.close();
