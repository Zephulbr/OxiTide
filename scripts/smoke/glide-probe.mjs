// Frame-by-frame probe of the queue glide over the WebView2 CDP endpoint.
// usage: node probe.mjs <port> <selector> [maxEls]
// Records each matched (on-screen) element's visual rect every frame while the
// queue toggles, then reports how far each frame strays from the ideal path
// (old box -> new box on the glide curve).
const [port, selector, maxEls = "40"] = process.argv.slice(2);
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let id = 0;
const pending = new Map();
ws.addEventListener("message", (e) => {
  const m = JSON.parse(e.data);
  pending.get(m.id)?.(m);
});
const evaluate = (expression) =>
  new Promise((resolve, reject) => {
    const i = ++id;
    pending.set(i, (m) => {
      if (m.error || m.result.exceptionDetails) reject(new Error(JSON.stringify(m.error ?? m.result.exceptionDetails)));
      else resolve(m.result.result.value);
    });
    ws.send(JSON.stringify({ id: i, method: "Runtime.evaluate", params: { expression, awaitPromise: true, returnByValue: true } }));
  });

const script = `(async () => {
  const main = document.getElementById("main");
  const view = main.getBoundingClientRect();
  const els = [...document.querySelectorAll(${JSON.stringify(selector)})].filter((el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.bottom > view.top && r.top < view.bottom;
  }).slice(0, ${Number(maxEls)});
  const label = (el) => (el.className?.baseVal ?? el.className ?? el.tagName).toString().split(" ").filter((c) => !c.startsWith("s-")).slice(0, 3).join(".") + ":" + (el.textContent ?? "").trim().slice(0, 18);
  const frames = [];
  const sample = () => frames.push({ t: performance.now(), r: els.map((el) => { const b = el.getBoundingClientRect(); return [b.left, b.top, b.width, b.height]; }) });
  sample();
  const t0 = performance.now();
  document.querySelector('button[aria-label="Queue"]').click();
  await new Promise((res) => { const loop = () => { sample(); if (performance.now() - t0 < 900) requestAnimationFrame(loop); else res(); }; requestAnimationFrame(loop); });
  await new Promise((r) => setTimeout(r, 150));
  sample();
  return { t0, labels: els.map(label), frames };
})()`;

const ease = (() => {
  const [x1, y1, x2, y2] = [0.4, 0, 0.2, 1];
  const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
  const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
  return (x) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let lo = 0, hi = 1, t = x;
    for (let i = 0; i < 40; i++) { const s = ((ax * t + bx) * t + cx) * t; if (s < x) lo = t; else hi = t; t = (lo + hi) / 2; }
    return ((ay * t + by) * t + cy) * t;
  };
})();

for (const pass of ["open", "close"]) {
  const { labels, frames } = await evaluate(script);
  const first = frames[0];
  const last = frames[frames.length - 1];
  // The glide starts on the first frame where anything moved.
  const moved = (f) => f.r.some((r, i) => r.some((v, k) => Math.abs(v - first.r[i][k]) > 0.5));
  const startIdx = frames.findIndex(moved);
  console.log(`\n=== ${pass}: ${labels.length} elements, ${frames.length} frames, glide starts at frame ${startIdx}`);
  const ts = frames[Math.max(0, startIdx - 1)].t;
  const rows = labels.map((label, i) => {
    let worst = 0, worstAt = -1, worstK = 0;
    frames.forEach((f, fi) => {
      const p = Math.min(1, Math.max(0, (f.t - ts) / 380));
      const e = ease(p);
      for (const k of [0, 1, 2]) {
        const ideal = first.r[i][k] + (last.r[i][k] - first.r[i][k]) * e;
        const dev = Math.abs(f.r[i][k] - ideal);
        if (dev > worst) { worst = dev; worstAt = fi; worstK = k; }
      }
    });
    // Largest single-frame step vs the largest an ideal path takes.
    let step = 0, stepAt = -1;
    for (let fi = 1; fi < frames.length; fi++) {
      const d = Math.hypot(frames[fi].r[i][0] - frames[fi - 1].r[i][0], frames[fi].r[i][1] - frames[fi - 1].r[i][1]);
      if (d > step) { step = d; stepAt = fi; }
    }
    const travel = Math.hypot(last.r[i][0] - first.r[i][0], last.r[i][1] - first.r[i][1]);
    return { label, travel: +travel.toFixed(1), dW: +(last.r[i][2] - first.r[i][2]).toFixed(1), worstDev: +worst.toFixed(1), at: worstAt, axis: "xyw"[worstK], maxStep: +step.toFixed(1), stepAt };
  });
  console.table(rows);
  if (process.env.DUMP) {
    const i = Number(process.env.DUMP);
    console.log(labels[i]);
    frames.forEach((f, fi) => console.log(fi, (f.t - ts).toFixed(0), f.r[i].map((v) => v.toFixed(1)).join(" ")));
  }
  await new Promise((r) => setTimeout(r, 600));
}
ws.close();
