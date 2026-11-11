// Finds elements whose height jumps by > 8px between two frames of the glide.
const [port] = process.argv.slice(2);
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
const evaluate = (expression) =>
  new Promise((resolve, reject) => {
    ws.addEventListener("message", function on(e) {
      const m = JSON.parse(e.data);
      if (m.id !== 1) return;
      ws.removeEventListener("message", on);
      if (m.result.exceptionDetails) reject(new Error(JSON.stringify(m.result.exceptionDetails)));
      else resolve(m.result.result.value);
    });
    ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression, awaitPromise: true, returnByValue: true } }));
  });
const script = `(async () => {
  const page = document.querySelector("#main > .page:not(.covered)");
  const view = document.getElementById("main").getBoundingClientRect();
  const els = [...page.querySelectorAll("*:not(svg *)")].filter((el) => { const r = el.getBoundingClientRect(); return r.height > 0 && r.bottom > view.top - 100 && r.top < view.bottom; });
  const path = (el) => { const out = []; for (let e = el; e && e !== page && out.length < 4; e = e.parentElement) out.unshift((e.tagName.toLowerCase() + "." + [...e.classList].filter((c) => !c.startsWith("s-")).join("."))); return out.join(" > ") + " '" + (el.textContent ?? "").trim().slice(0, 24) + "'"; };
  const frames = [];
  const sample = () => frames.push(els.map((el) => el.getBoundingClientRect().height));
  sample();
  const t0 = performance.now();
  document.querySelector('button[aria-label="Queue"]').click();
  await new Promise((res) => { const loop = () => { sample(); if (performance.now() - t0 < 700) requestAnimationFrame(loop); else res(); }; requestAnimationFrame(loop); });
  const hits = [];
  els.forEach((el, i) => {
    for (let f = 2; f < frames.length; f++) {
      const d = frames[f][i] - frames[f - 1][i];
      if (Math.abs(d) > 8) hits.push(f + " " + d.toFixed(1) + " " + path(el));
    }
  });
  return hits.slice(0, 40).join("\\n");
})()`;
console.log("pass 1\n" + (await evaluate(script)));
await new Promise((r) => setTimeout(r, 800));
console.log("pass 2\n" + (await evaluate(script)));
ws.close();
