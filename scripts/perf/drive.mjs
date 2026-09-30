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
    throw new Error(`evaluate failed: ${result.exceptionDetails.text}`);
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

const scenarioFallback = async (id) => {
  if (id.startsWith("S5")) {
    await until("!!window.__perf", "__perf bridge");
    await evaluate('window.__perf.goto("tracks")');
    await until('!!document.querySelector("[data-perf-scroll]")', "track table");
    await delay(3000);
    for (let i = 0; i < 12; i++) {
      await evaluate("window.__perf.scrollTracks(1200)");
      await delay(600);
    }
    await delay(3000);
  } else if (id.startsWith("S6")) {
    await until("!!window.__perf", "__perf bridge");
    await evaluate('window.__perf.goto("tracks")');
    await until('!!document.querySelector("[data-perf-filter]")', "filter input");
    await delay(5000); // let the full library load so filtering is over loaded rows
    for (const query of ["a", "th", "the ", "the e", ""]) {
      await evaluate(`window.__perf.setFilter(${JSON.stringify(query)})`);
      await delay(800);
    }
    await delay(3000);
  } else if (id === "S7") {
    await until("!!window.__TAURI_INTERNALS__", "tauri internals");
    const started = Date.now();
    await evaluate(`(async () => {
      const tracks = await window.__TAURI_INTERNALS__.invoke("get_favorite_tracks");
      const items = tracks.map((t) => ({
        track_id: t.id,
        title: t.title,
        artist: (t.artists ?? []).map((a) => a.name).join(", "),
        album: t.album?.title ?? "",
        duration_secs: t.duration,
        cover: t.album?.cover ?? null,
      }));
      await window.__TAURI_INTERNALS__.invoke("play_tracks", { items, start: 0, shuffle: false });
      return tracks.length;
    })()`).then((count) => console.log(`S7 invoked play_tracks for ${count} tracks`));
    console.log(`S7 command round-trip: ${Date.now() - started} ms`);
    await delay(5000);
  } else {
    // Start-only scenarios (S1/S2 and manual ones): idle while marks accumulate.
    await delay(Number(flag("--idle", "8000")));
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
