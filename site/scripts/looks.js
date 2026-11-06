// Fills every [data-lk] with the three app replicas and morphs between them
// (Atmosphere -> Studio -> Rack) as its [data-lk-scroll] ancestor scrolls by.
// Captions marked [data-cap="0|1|2"] inside the scroller get .on.
//
// The morph (owner pick, 2026-10-31): a veil dims the frame, the chassis
// swaps under it, and the cover and the title travel from their old spot to
// their new one (v1's FLIP morph, here with stand-in clones of the parts).
// Rack's meters are v1's drawn VU faces.
//
// The page colour follows the looks (owner pick, 2026-11-01): Atmosphere
// plays Slack Water (teal), then Studio and Rack play Sodium Lights (amber),
// so the album changes during the first morph, as the room turns.
// The album each look plays, and its tracklist (track 2 is the title track, playing).
const SLACK = { art: "art-slack", name: "Slack Water", rows: [["1", "First Light on the Pier", "3:48"], ["2", "Slack Water", "4:12"], ["3", "Undertow", "3:05"], ["4", "Salt in the Wiring", "4:41"], ["5", "Harbour Lamps", "3:57"], ["6", "Neap Tide", "5:20"], ["7", "Long Exposure", "4:03"]] };
const SODIUM = { art: "art-sodium", name: "Sodium Lights", rows: [["1", "Exit Ramp", "3:31"], ["2", "Sodium Lights", "4:26"], ["3", "Overpass", "3:12"], ["4", "Night Shift Radio", "4:05"], ["5", "Tail Lights", "3:49"], ["6", "Wet Asphalt", "5:02"], ["7", "Last Bus Home", "4:18"]] };
const rowHtml = (al) => al.rows.map(([n, t, d]) => `<div class="${n === "2" ? "on" : ""}"><span>${n}</span><span>${t}</span><span>${d}</span></div>`).join("");
const ttl = `<div class="lk-ttl"><i></i><i></i><i></i></div>`;
// The player bar's parts, as the app draws them in every mode: the accent
// play/pause glyph (no disc behind it), the solid skip glyphs, the volume icon over an 8px accent pill, and the quality plate
// (tier | output path, with its menu chevron). Symbols live in the page sprite.
const ctl = `<div class="lk-ctl"><svg class="sk"><use href="#i-prev"/></svg><svg class="lk-pp"><use href="#i-pause"/></svg><svg class="sk"><use href="#i-next"/></svg></div>`;
const right = `<div class="lk-right"><span class="lk-vol"><svg><use href="#i-vol"/></svg><i></i></span><span class="lk-plate"><b>MAX</b><span><svg><use href="#i-ok"/></svg>BP</span><svg class="chev"><use href="#i-chev-up"/></svg></span></div>`;
const np = (al) => `<div class="lk-np"><span class="cover ${al.art}"></span><span><b>${al.name}</b><small>Orla Venn</small></span></div>`;

// VU faces, as v1 draws them (rack/DESIGN.md §5.9): a dB scale with a red
// zone, the needle at a fixed level.
function vuFace(ch, level) {
  const VU = [-20, -10, -7, -5, -3, -2, -1, 0, 1, 2, 3];
  const P = { x: 100, y: 150 }, R = 112;
  const posOf = (db) => (10 ** (db / 20) - 0.1) / (10 ** (3 / 20) - 0.1);
  const angOf = (p) => ((-46 + p * 92) * Math.PI) / 180;
  const pt = (a, r) => [P.x + Math.sin(a) * r, P.y - Math.cos(a) * r];
  const arc = (a0, a1, r) => { const [x0, y0] = pt(a0, r), [x1, y1] = pt(a1, r); return `M${x0.toFixed(2)} ${y0.toFixed(2)} A${r} ${r} 0 0 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`; };
  let s = `<path class="vu-ink" d="${arc(angOf(0), angOf(posOf(0)), R)}" stroke-width="1.6"/><path class="vu-red" d="${arc(angOf(posOf(0)), angOf(1), R)}" stroke-width="5"/>`;
  for (const db of VU) {
    const a = angOf(posOf(db)), major = [-20, -10, -7, -5, -3, 0, 3].includes(db);
    const [x0, y0] = pt(a, R - (major ? 9 : 6)), [x1, y1] = pt(a, R + 1), [tx, ty] = pt(a, R + 9);
    s += `<line class="${db > 0 ? "vu-red" : "vu-ink"}" x1="${x0.toFixed(2)}" y1="${y0.toFixed(2)}" x2="${x1.toFixed(2)}" y2="${y1.toFixed(2)}" stroke-width="${major ? 1.6 : 1}"/>`;
    s += `<text class="vu-t${db > 0 ? " red" : ""}" x="${tx.toFixed(2)}" y="${ty.toFixed(2)}" text-anchor="middle">${db > 0 ? `+${db}` : Math.abs(db)}</text>`;
  }
  s += `<text class="vu-title" x="100" y="92" text-anchor="middle">VU</text><text class="vu-t" x="22" y="118" opacity=".75">${ch}</text>`;
  s += `<line class="vu-needle" x1="${P.x}" y1="${P.y}" x2="${P.x}" y2="${P.y - R - 4}" transform="rotate(${(-46 + level * 92).toFixed(1)} ${P.x} ${P.y})"/>`;
  s += `<rect x="0" y="124" width="200" height="5" fill="rgba(0,0,0,.5)"/><ellipse class="vu-pivot" cx="100" cy="146" rx="26" ry="18"/>`;
  return `<svg class="vu-face" viewBox="0 0 200 129" aria-hidden="true">${s}</svg><span class="vu-glass"></span>`;
}

const html = `
<div class="lk-layer lk-atmo">${ttl}
  <div class="a-glow"></div>
  <div class="a-disc" data-part="cover"><div class="disc art-slack"><i class="d-art"></i><i class="d-grooves"></i><i class="d-sheen"></i><i class="d-label"></i></div></div>
  <div class="a-meta">
    <div class="a-kick">From album · Slack Water</div>
    <div class="a-title" data-part="title">Slack Water</div>
    <div class="a-artist">Orla Venn</div>
    <div class="a-badges"><span>MAX</span><span>24-bit / 96 kHz</span><span>Bit-perfect</span></div>
    <div class="a-tabs"><span>Lyrics</span><span>Next</span><span>Info</span></div>
    <div class="a-lyr"><p>Rope on the bollard, salt on the glass</p><p>the harbour holds its breath</p><p class="on">and we wait between the tides</p><p>nothing pulls and nothing gives</p></div>
  </div>
  <div class="lk-bar">${np(SLACK)}${ctl}${right}</div>
</div>
<div class="lk-layer lk-studio">${ttl}
  <div class="s-side"><span class="logo">OxiTide</span><span>Home</span><span>Explore</span><span class="on">Library</span><span>History</span><small>Playlists</small><span>Late Ferries</span><span>Quiet Hours</span></div>
  <div class="s-main">
    <div class="s-head"><span class="cover ${SODIUM.art}" data-part="cover"></span><div><small>Album · 2026</small><h4 data-part="title">${SODIUM.name}</h4><p>Orla Venn · 9 tracks · 38 min</p><div class="s-btns"><span>Play</span><span>Shuffle</span><span>Save</span></div></div></div>
    <div class="s-rows">${rowHtml(SODIUM)}</div>
  </div>
  <div class="lk-bar">${np(SODIUM)}${ctl}${right}</div>
</div>
<div class="lk-layer lk-rack">${ttl}
  <div class="r-top"><span class="logo">OxiTide</span><span class="lk-key">Home</span><span class="lk-key">Explore</span><span class="lk-key">Feed</span><span class="lk-key on">Library</span><span style="flex:1"></span><span class="lk-key">Mini</span><span class="lk-key">Setup</span></div>
  <div class="r-body">
    <div class="r-col">
      <div class="r-pan"><small>Network transport <em>TIDAL · FLAC</em></small>
        <div class="vfd"><b>SODIUM LIGHTS</b><span>ORLA VENN · SODIUM LIGHTS</span><span class="t">1:36</span><div class="lv"></div><span>FLAC 24 BIT 96 kHz EXCL</span></div>
        <div class="r-tr"><span class="lk-key"><i class="tri prev"></i></span><span class="lk-key on"><i class="bars"></i></span><span class="lk-key"><i class="tri"></i></span></div>
      </div>
      <div class="r-pan"><small>Amplifier <em>Direct · bit-perfect</em></small><div class="vus"><div class="vu">${vuFace("L", 0.62)}</div><div class="vu r">${vuFace("R", 0.55)}</div><div class="knob"></div></div></div>
      <div class="r-pan"><small>Playlists <em>12</em></small><div class="trk"><div><span>·</span><span>Late Ferries</span><span>24</span></div><div><span>·</span><span>Quiet Hours</span><span>31</span></div><div><span>·</span><span>Night Drive</span><span>18</span></div></div></div>
    </div>
    <div class="r-pan r-glass"><small>Album <em>9 tracks · 38 min</em></small>
      <div class="alb"><span class="cover ${SODIUM.art}" data-part="cover"></span><div><h4 data-part="title">${SODIUM.name}</h4><p>Orla Venn</p></div></div>
      <div class="trk">${rowHtml(SODIUM)}</div>
    </div>
  </div>
</div>
<div class="lk-veil"></div>
<div class="lk-parts"></div>`;

const clamp01 = (x) => Math.min(1, Math.max(0, x));
const smooth = (a, b, x) => { const t = clamp01((x - a) / (b - a)); return t * t * (3 - 2 * t); };
const ease = (x) => { const t = clamp01(x); return t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2; };
const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
const LAYERS = [".lk-atmo", ".lk-studio", ".lk-rack"];
// Inline the styles a clone needs once it leaves its layer's selectors.
const KEEP = ["font-family", "font-size", "font-weight", "letter-spacing", "line-height", "color", "text-shadow", "white-space",
  "border-radius", "box-shadow", "filter", "background-color", "background-image", "background-size", "background-position", "background-repeat"];

for (const frame of document.querySelectorAll("[data-lk]")) {
  frame.innerHTML = html;
  frame.setAttribute("aria-hidden", "true");
  const scroller = frame.closest("[data-lk-scroll]");
  if (!scroller) continue;
  const caps = scroller.querySelectorAll("[data-cap]");
  const veil = frame.querySelector(".lk-veil");
  const partsLayer = frame.querySelector(".lk-parts");

  // Morph: one clone per part per look, parked over its original.
  let parts = [];
  const buildParts = () => {
    partsLayer.innerHTML = "";
    parts = [];
    const fr = frame.getBoundingClientRect();
    if (!fr.width) return;
    for (const kind of ["cover", "title"]) {
      const rects = LAYERS.map((sel) => {
        const el = frame.querySelector(`${sel} [data-part="${kind}"]`);
        const r = el.getBoundingClientRect();
        return { el, x: r.left - fr.left, y: r.top - fr.top, w: r.width, h: r.height };
      });
      rects.forEach((own, look) => {
        const wrap = document.createElement("div");
        wrap.className = "lk-part";
        Object.assign(wrap.style, { left: `${own.x}px`, top: `${own.y}px`, width: `${own.w}px`, height: `${own.h}px` });
        const copy = own.el.cloneNode(true);
        const copyStyles = (src, dst) => {
          const cs = getComputedStyle(src);
          for (const p of KEEP) dst.style.setProperty(p, cs.getPropertyValue(p));
          [...src.children].forEach((c, i) => copyStyles(c, dst.children[i]));
        };
        copyStyles(own.el, copy);
        copy.removeAttribute("data-part");
        Object.assign(copy.style, { position: "absolute", inset: "0", width: "100%", height: kind === "cover" ? "100%" : "auto", margin: "0" });
        wrap.append(copy);
        partsLayer.append(wrap);
        parts.push({ kind, look, wrap, rects });
      });
    }
  };
  const placeAt = (part, r) => {
    const o = part.rects[part.look];
    if (part.kind === "cover") {
      part.wrap.style.transform = `translate(${r.x - o.x}px, ${r.y - o.y}px) scale(${r.w / o.w}, ${r.h / o.h})`;
    } else {
      // Text keeps its proportions and hangs from its top-left corner.
      part.wrap.style.transform = `translate(${r.x - o.x}px, ${r.y - o.y}px) scale(${r.h / o.h})`;
    }
  };
  const lerp = (a, b, k) => ({ x: a.x + (b.x - a.x) * k, y: a.y + (b.y - a.y) * k, w: a.w + (b.w - a.w) * k, h: a.h + (b.h - a.h) * k });

  // Opacity is written straight onto the layers, and only when it changes: a
  // custom property on the frame restyled the whole mockup on every scroll.
  const studio = frame.querySelector(".lk-studio"), rack = frame.querySelector(".lk-rack");
  const setOp = (el, v) => { const s = String(Math.round(v * 1000) / 1000); if (el.style.opacity !== s) el.style.opacity = s; };
  let last = -1;
  const update = () => {
    const r = scroller.getBoundingClientRect();
    const p = clamp01(-r.top / Math.max(1, r.height - innerHeight));
    const t = p * 2.4 - 0.2;
    // The chassis swap is quick and sits under the veil, mid-transition.
    const os = reduce ? +(t > 0.5) : smooth(0.44, 0.56, t);
    const or = reduce ? +(t > 1.5) : smooth(1.44, 1.56, t);
    const bell = (a) => { const k = (t - a) / 0.6; return k > 0 && k < 1 ? Math.sin(Math.PI * k) : 0; };
    setOp(studio, os);
    setOp(rack, or);
    setOp(veil, reduce ? 0 : 0.72 * Math.max(bell(0.2), bell(1.2)));

    if (!parts.length) buildParts();
    // Which transition (if any) is running, and how far along it is.
    const seg = t > 0.2 && t < 0.8 ? 0 : t > 1.2 && t < 1.8 ? 1 : -1;
    const k = seg < 0 ? 0 : (t - (seg ? 1.2 : 0.2)) / 0.6;
    const rest = t <= 0.2 ? 0 : t >= 1.8 ? 2 : t >= 0.8 && t <= 1.2 ? 1 : -1;
    for (const part of parts) {
      const { kind, look, wrap, rects } = part;
      let opacity = 0;
      if (seg >= 0 && (look === seg || look === seg + 1)) {
        const kk = reduce ? +(k > 0.5) : k;
        placeAt(part, lerp(rects[seg], rects[seg + 1], ease(kk)));
        const out = look === seg;
        // Pictures crossfade; text hands over, so two titles never overlap.
        const [a, b] = kind === "cover" ? [0.35, 0.65] : out ? [0.12, 0.42] : [0.58, 0.88];
        opacity = out ? 1 - smooth(a, b, kk) : smooth(a, b, kk);
      } else if (look === rest) {
        placeAt(part, rects[look]);
        opacity = 1;
      }
      setOp(wrap, opacity);
    }

    const look = t < 0.5 ? 0 : t < 1.5 ? 1 : 2;
    if (look !== last) {
      last = look;
      scroller.dataset.look = look;
      caps.forEach((c) => c.classList.toggle("on", +c.dataset.cap === look));
    }
  };
  // Once per frame, however many scroll events arrive, and only while the
  // looks are near the screen: far away the page skips rendering them, and
  // measuring them here would force it back.
  let raf = 0, near = true;
  new IntersectionObserver(([e]) => { near = e.isIntersecting; if (near) update(); }, { rootMargin: "300px 0px" }).observe(scroller);
  addEventListener("scroll", () => { if (near && !raf) raf = requestAnimationFrame(() => { raf = 0; update(); }); }, { passive: true });
  // Layout changes (the frame resizing, the fonts arriving) move the slots:
  // re-measure. Watched on the frame, not the window: a phone's toolbar
  // showing or hiding fires a window resize mid-scroll, and rebuilding the
  // parts there (a style read per element) stalled the scroll.
  const remeasure = () => { parts = []; partsLayer.innerHTML = ""; if (near) update(); };
  let size = "";
  new ResizeObserver(([e]) => {
    const s = `${Math.round(e.contentRect.width)}x${Math.round(e.contentRect.height)}`;
    if (s !== size) { const first = !size; size = s; if (!first) remeasure(); }
  }).observe(frame);
  document.fonts.ready.then(remeasure);
  update();
  caps.forEach((c) => c.addEventListener("click", () => {
    const top = scroller.getBoundingClientRect().top + scrollY;
    const p = (+c.dataset.cap + 0.2) / 2.4;
    scrollTo({ top: top + p * (scroller.offsetHeight - innerHeight) + 2, behavior: reduce ? "auto" : "smooth" });
  }));
}
