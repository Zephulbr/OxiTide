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

const getPlaybackState = () => invoke("get_playback_state");

const awaitPlaying = (label = "playback", maxMs = 30000) =>
  until(
    '(async () => { const s = await window.__TAURI_INTERNALS__.invoke("get_playback_state"); return s.playing && s.position_ms > 0; })()',
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

const awaitWarmCache = async (beforeBytes, label = "cache write", maxMs = 300000) => {
  const deadline = Date.now() + maxMs;
  let sawPending = false;
  while (Date.now() < deadline) {
    const status = await invoke("get_cache_status");
    if (status.pending_bytes > 0) sawPending = true;
    if (status.pending_bytes === 0 && (status.used_bytes > beforeBytes || !sawPending)) return;
    await delay(500);
  }
  throw new Error(`timed out waiting for ${label}`);
};

const scenarioFallback = async (id) => {
  const targetMode = id.startsWith("S5-atmosphere")
    ? "atmosphere"
    : id.startsWith("S5") || id.startsWith("S6")
      ? "instrument"
      : null;
  let originalMode = null;
  if (targetMode) {
    try {
      originalMode = await evaluate(
        'window.__TAURI_INTERNALS__.invoke("get_config").then((c) => c.ui.mode)',
      );
    } catch {
      originalMode = null; // cannot restore -> leave the owner's setting untouched
    }
  }
  let modeApplied = false;
  const applyScenarioMode = async () => {
    if (!targetMode || !originalMode || originalMode === targetMode || modeApplied) return;
    modeApplied = true;
    try {
      await setUiMode(targetMode);
    } catch (e) {
      console.error(`mode switch to ${targetMode} failed: ${e.message}`);
    }
  };
  try {
    if (id.startsWith("S5")) {
      await awaitLoggedIn();
      await until("!!window.__perf", "__perf bridge");
      await applyScenarioMode();
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
      await delay(5000); // let the full library load so filtering is over loaded rows
      for (const query of ["a", "th", "the ", "the e", ""]) {
        await evaluate(`window.__perf.setFilter(${JSON.stringify(query)})`);
        await delay(800);
      }
      await delay(3000);
    } else if (id === "S7") {
      await awaitLoggedIn();
      const started = Date.now();
      const items = (await getFavoriteTracks()).map(trackToQueueItem);
      await playItems(items);
      console.log(`S7 invoked play_tracks for ${items.length} tracks`);
      console.log(`S7 command round-trip: ${Date.now() - started} ms`);
      await delay(5000);
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
      const items = requireTracks(await getFavoriteTracks(), 3).slice(0, 3).map(trackToQueueItem);
      await playItems(items);
      await until(
        `(async () => { const s = await window.__TAURI_INTERNALS__.invoke("get_playback_state"); return s.playing && !!s.current && s.current.track_id === ${JSON.stringify(items[0].track_id)}; })()`,
        `S4 start track ${items[0].track_id}`,
        30000,
      );
      for (let i = 1; i < items.length; i++) {
        const state = await getPlaybackState();
        if (state.duration_ms > 3000) {
          await invoke("seek", { positionMs: state.duration_ms - 2000 });
        }
        try {
          await awaitCurrentTrack(items[i].track_id, `S4 advance to track ${items[i].track_id}`, 30000);
        } catch {
          await awaitCurrentTrack(
            items[i].track_id,
            `S4 natural advance to track ${items[i].track_id}`,
            Math.max(30000, state.duration_ms + 30000),
          );
        }
        await delay(1500);
      }
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
  } finally {
    if (modeApplied) {
      try {
        await setUiMode(originalMode);
      } catch (e) {
        console.error(`mode restore to ${originalMode} failed: ${e.message}`);
      }
    }
  }
};

try {
  await send("Runtime.enable");
} catch {
  /* older targets may not need it */
}

await scenarioFallback(scenario).catch((e) => {
  console.error(`driver failed: ${e.message}`);
  process.exitCode = 1;
});
ws.close();
