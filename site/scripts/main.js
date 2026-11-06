const root = document.documentElement;
// The hero plays the OxiTide logo as its record, in the logo's room (owner
// pick, 2026-11-01); the install section bookends it in the same room. The
// page's own room is set in the markup and never changes (tide.css).
// Room: [cover, album, artist].
const covers = { slack: ["art-slack", "Slack Water"], ferry: ["art-ferry", "Night Ferry"], sodium: ["art-sodium", "Sodium Lights"], field: ["art-field", "Field Notes"], oxi: ["art-oxitide", "OxiTide", "v0.5.0"] };
// The room per look (Atmosphere, Studio, Rack), matching the album each
// mockup plays (looks.js): teal, then amber from the first morph on.
const lookRoom = ["slack", "sodium", "sodium"];
const secs = [...document.querySelectorAll("main > section[data-room]")];
const lk = document.querySelector("[data-lk-scroll]");
const $ = (s) => document.querySelector(s);

// Play / pause, shared by the hero and the dock.
document.querySelectorAll("[data-pp]").forEach((b) => b.addEventListener("click", () => {
  const paused = root.toggleAttribute("data-paused");
  document.querySelectorAll("[data-pp]").forEach((x) => { x.querySelector("use").setAttribute("href", paused ? "#i-play" : "#i-pause"); x.setAttribute("aria-label", paused ? "Play" : "Pause"); });
}));

// Queue (the section list) and the segmented progress bar.
const qlist = $("[data-qlist]"), prog = $("[data-prog]");
secs.forEach((s, i) => {
  const [art, name] = covers[s.dataset.room];
  const li = document.createElement("li");
  li.innerHTML = `<span class="n">${i + 1}</span><span class="eq" aria-hidden="true"><i></i><i></i><i></i></span><span class="cover ${art}"></span><span><b>${s.dataset.title}</b><span>${name}</span></span>`;
  li.addEventListener("click", () => jump(i));
  qlist.append(li);
  const seg = document.createElement("span");
  seg.innerHTML = "<i></i>";
  prog.append(seg);
});
const segs = [...prog.children];
// The progress bar seeks, like a player's: a click goes to that point of
// that section (its fill ends where you clicked). Hovering shows the
// section's name at the pointer.
const progTip = $("[data-prog-tip]");
const seekAt = (x) => {
  for (let k = 0; k < segs.length; k++) {
    const r = segs[k].getBoundingClientRect();
    if (x < r.right || k === segs.length - 1) return [k, Math.min(1, Math.max(0, (x - r.left) / r.width))];
  }
};
prog.addEventListener("click", (e) => {
  if (!geo) measure();
  const [k, x] = seekAt(e.clientX), [top, h] = geo.secs[k];
  const f = x > .96 ? 1 : x; // the end of a segment is a few px wide: snap to it
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  scrollTo({ top: readY(top + f * h), behavior: reduce ? "auto" : "smooth" });
});
prog.addEventListener("pointermove", (e) => {
  const [k] = seekAt(e.clientX), dr = dock.getBoundingClientRect();
  if (progTip.dataset.k !== String(k)) { progTip.dataset.k = k; progTip.querySelector("em").textContent = secs[k].dataset.title; progTip.querySelector(".cover").className = "cover " + covers[secs[k].dataset.room][0]; }
  progTip.style.transform = `translateX(${Math.round(e.clientX - dr.left)}px)`;
  progTip.classList.add("show");
});
prog.addEventListener("pointerleave", () => progTip.classList.remove("show"));
const sizeSegs = () => segs.forEach((seg, k) => { seg.style.flex = Math.max(0.6, secs[k].offsetHeight / innerHeight); });
const qbtn = $("[data-qbtn]"), queue = $("[data-queue]");
qbtn.addEventListener("click", () => { const o = queue.classList.toggle("open"); qbtn.setAttribute("aria-expanded", o); });
const jump = (i) => {
  i = Math.max(0, Math.min(secs.length - 1, i));
  scrollTo({ top: secs[i].offsetTop + (i ? 1 : 0), behavior: "smooth" });
  queue.classList.remove("open");
  qbtn.setAttribute("aria-expanded", false);
};
let cur = 0;
$("[data-prev]").addEventListener("click", () => jump(secs[cur].getBoundingClientRect().top < -80 ? cur : cur - 1));
$("[data-next]").addEventListener("click", () => jump(cur + 1));

// The room (page colour) settles before it changes: a new room has to hold
// for a moment, so a quick scroll past a section boundary doesn't flash.
let room = "", want = "", roomTimer = 0, shownSec = -1, announceWait = 0;
const dockArt = $("[data-dock-art]"), dockTitle = $("[data-dock-title]"), dockSub = $("[data-dock-sub]");
// The new room's backdrop stack fades in on top of the old one; once it has,
// the old ones are dropped (unseen, under it). The sections keep their own
// rooms; only the parts that follow the screen ([data-follow]) change room,
// easing over while [data-gliding] is on them (tide.css explains why).
const stacks = [...document.querySelectorAll(".backdrop [data-bd]")];
const followers = [...document.querySelectorAll("[data-follow]")];
const barEl = $(".dock .bar"), barTint = $("[data-bar-tint]");
let glideEnd = 0, glideId = 0, tintFade = null;
function setRoom(r) {
  // data-slow (set by tick, for the way back to teal) only picks the pace.
  const slow = root.hasAttribute("data-slow"), glide = slow ? 5000 : 2600;
  // The bar's old tint, read before the switch, fades out over the new one.
  const was = room && getComputedStyle(barEl).backgroundColor;
  followers.forEach((f) => {
    if (room) f.setAttribute("data-gliding", slow ? "slow" : "");
    f.dataset.room = r;
  });
  room = r;
  stacks.forEach((b) => { const on = b.dataset.bd === r; b.style.zIndex = on ? 2 : 1; if (on) b.classList.add("on"); });
  if (was) {
    barTint.style.background = was;
    // Cancelled by reference: getAnimations() would force a style update here.
    if (tintFade) tintFade.cancel();
    tintFade = barTint.animate([{ opacity: 1 }, { opacity: 0 }], { duration: glide, easing: "cubic-bezier(.4,0,.2,1)" });
  }
  // Afterwards, in the browser's idle time: the glide goes off and the old
  // backdrop stacks are dropped.
  clearTimeout(glideEnd);
  const id = ++glideId;
  glideEnd = setTimeout(() => {
    const end = () => {
      if (id !== glideId) return; // a newer glide has begun
      followers.forEach((f) => f.removeAttribute("data-gliding"));
      stacks.forEach((b) => {
        if (b.dataset.bd === room || !b.classList.contains("on")) return;
        b.style.transition = "none"; b.classList.remove("on"); void b.offsetWidth; b.style.transition = "";
      });
    };
    if (window.requestIdleCallback) requestIdleCallback(end, { timeout: 2000 }); else end();
  }, glide + 150);
  paintDock();
}
// What the bar says is playing follows the screen (owner pick): whatever a
// mockup on screen is playing, else the cover of the section's room.
let nowOverride = null, nowKey = "";
let duoNow = ["art-field", "Field Notes"];
function paintDock() {
  const [art, name, by = "Orla Venn"] = nowOverride || covers[room] || covers.slack;
  dockArt.className = "disc " + art;
  dockSub.textContent = name + " · " + by;
}
// Where the sections are, measured once per layout change rather than on
// every frame (reading layout during scroll forces it to be redone). The
// viewport height is kept too: reading innerHeight after a style write
// forces a style and layout pass (it cost about 3 ms a frame on a phone).
let geo = null;
const measure = () => {
  const vh = innerHeight;
  geo = {
    vh,
    secs: secs.map((s) => [s.offsetTop, s.offsetHeight]),
    lk: [lk.getBoundingClientRect().top + scrollY, lk.offsetHeight],
    end: document.documentElement.scrollHeight - vh,
  };
};
const lkRect = () => { const t = geo.lk[0] - scrollY; return { top: t, bottom: t + geo.lk[1] }; };
// Where the progress bar says you are reading: the middle of the screen,
// except over the last half-screen of scrolling, where it runs down to the
// bottom of the screen, so the last section fills as you reach the end
// (the middle never gets that far). readY is its inverse, for seeking.
const readAt = (y) => { const mid = geo.vh / 2, k = Math.max(0, y - (geo.end - mid)); return y + mid + k; };
const readY = (p) => {
  const mid = geo.vh / 2;
  const y = p - mid <= geo.end - mid ? p - mid : (p + geo.end - geo.vh) / 2;
  return Math.max(0, Math.min(geo.end, y));
};
function screenNow(i) {
  const s = secs[i], mid = geo.vh * 0.5;
  if (s.id === "looks") { const r = lkRect(); if (r.top <= mid && r.bottom > mid) return covers[lookRoom[+(lk.dataset.look || 0)]]; }
  if (s.id === "remote") return duoNow;
  return null;
}
function askRoom(r) {
  if (r === want) return;
  want = r;
  clearTimeout(roomTimer);
  if (r === room) return;
  roomTimer = setTimeout(() => setRoom(want), room ? 260 : 0);
}

// The dock: compact pill in the hero, full bar after. The play button
// keeps its place on screen and glides to its new spot (FLIP).
const dock = $(".dock"), dockPP = $("[data-dock-pp]");
let docked = null;
function setDocked(d) {
  if (d === docked) return;
  const first = docked === null ? null : dockPP.getBoundingClientRect();
  docked = d;
  root.toggleAttribute("data-docked", d);
  if (!d) { queue.classList.remove("open"); qbtn.setAttribute("aria-expanded", false); $("[data-announce]").classList.remove("show"); }
  if (!first || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
  const last = dockPP.getBoundingClientRect();
  dockPP.animate([
    { transform: `translate(${first.left - last.left}px, ${first.top - last.top}px) scale(${first.width / last.width})` },
    { transform: "none" },
  ], { duration: 650, easing: "cubic-bezier(.2,.8,.2,1)" });
}

// Each frame: which section is playing, its room, the dock, the progress.
const lastF = [];
function tick() {
  if (!geo) measure();
  const mid = geo.vh * 0.5, y = scrollY;
  let i = 0;
  geo.secs.forEach(([top], k) => { if (top - y <= mid) i = k; });
  // At the very bottom the last section counts even if it's too short to
  // reach the middle of the screen.
  if (y >= geo.end - 2) i = secs.length - 1;
  cur = i;
  let r = secs[i].dataset.room;
  // In the looks, the room follows the look on screen; past the last look
  // it stays on the last one's, so there's no flash back to teal before Sound.
  const lr = lkRect();
  if (secs[i].id === "looks" && lr.top <= mid) r = lookRoom[lr.bottom > mid ? +(lk.dataset.look || 0) : lookRoom.length - 1];
  // The way back to teal at the end is slower than the other changes.
  root.toggleAttribute("data-slow", secs[i].id === "install");
  askRoom(r);
  // Dock first, so a jump from the hero straight into a section still
  // gets its announcement.
  // y, not scrollY: reading it again here, after the writes above, would
  // force a style pass.
  setDocked(y > geo.vh * 0.16);
  if (i !== shownSec) {
    shownSec = i;
    dockTitle.textContent = secs[i].dataset.title;
    // Announce once the section has held for a moment, so a fast scroll or
    // a jump through several sections doesn't flash a notice for each.
    clearTimeout(announceWait);
    announceWait = setTimeout(() => { if (shownSec === i && room) announceSection(i); }, 350);
    [...qlist.children].forEach((li, k) => li.classList.toggle("on", k === i));
  }
  const at = readAt(y);
  segs.forEach((seg, k) => {
    const [top, h] = geo.secs[k];
    const f = Math.round(Math.min(1, Math.max(0, (at - top) / h)) * 1000) / 10;
    if (lastF[k] !== f) { lastF[k] = f; seg.firstChild.style.setProperty("--f", f + "%"); }
  });

  const now = screenNow(i), key = now ? now.join("|") : "";
  if (key !== nowKey) { nowKey = key; nowOverride = now; paintDock(); }
}
let raf = 0;
const schedule = () => { cancelAnimationFrame(raf); raf = requestAnimationFrame(tick); };
addEventListener("scroll", schedule, { passive: true });
addEventListener("resize", () => { geo = null; sizeSegs(); tick(); });
// The page can change height without a resize (fonts, the looks' frames).
new ResizeObserver(() => { geo = null; schedule(); }).observe(document.body);
// On a phone the looks' frame pins at its own height, centred above the bar
// (site.css), so CSS needs that height. Set on the frame's box, not :root.
const lkStick = lk.querySelector(".lk-stick");
new ResizeObserver(([e]) => { lkStick.style.setProperty("--lk-h", Math.round(e.borderBoxSize[0].blockSize) + "px"); }).observe(lkStick);
sizeSegs();
setTimeout(tick, 60);

// Off screen, a section's endless motions rest ([data-off], site.css): left
// running, the crossfade playhead alone kept the page rendering every frame.
const offscreen = new IntersectionObserver((es) => es.forEach((e) => e.target.toggleAttribute("data-off", !e.isIntersecting)), { rootMargin: "100px 0px" });
secs.forEach((s) => offscreen.observe(s));

// The water's ripple is an SVG animation, which redraws its filter every
// frame (and SVG animations never stop on their own off screen). On a phone
// it doesn't run at all (the reflection is blurred instead, site.css);
// elsewhere it runs only while the hero is in view.
const sprite = document.querySelector("body > svg"), ripple = sprite.querySelector("#ripple animate");
if (matchMedia("(hover: none) and (pointer: coarse)").matches) ripple.remove();
else new IntersectionObserver(([e]) => { if (e.isIntersecting) sprite.unpauseAnimations(); else sprite.pauseAnimations(); }).observe($(".rise"));

// Back to the downloads (the install button and the bar's Download): scroll
// up to the hero's rows and light them once.
const rows = $(".hero .dl-rows");
// Send to your computer: on a phone or tablet (where the app can't run) the
// share row, the install button and the bar's button send this page on with
// the phone's own share sheet; without one, they copy the link. The page,
// not an installer: on the computer it opens on the downloads.
const touch = matchMedia("(hover: none) and (pointer: coarse)");
const pageUrl = $('link[rel="canonical"]').href;
const toast = $("[data-toast]");
let toastTimer = 0;
const say = (msg) => {
  // It shows where the section notices do, so one gives way.
  clearTimeout(announceWait);
  $("[data-announce]").classList.remove("show");
  toast.textContent = msg;
  toast.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove("show"), 2600);
};
async function sendToComputer() {
  const data = { title: "OxiTide", text: "OxiTide, an unofficial TIDAL desktop client for Windows and Linux. Open this on your computer to download it.", url: pageUrl };
  if (navigator.share && (!navigator.canShare || navigator.canShare(data))) {
    try { await navigator.share(data); return; } catch (err) { if (err.name === "AbortError") return; }
  }
  try { await navigator.clipboard.writeText(pageUrl); say("Link copied. Open it on your computer."); }
  catch { say("Open " + pageUrl.replace(/^https:\/\//, "") + " on your computer"); }
}
$("[data-share]").addEventListener("click", sendToComputer);
document.querySelectorAll("[data-to-downloads]").forEach((a) => a.addEventListener("click", (e) => {
  e.preventDefault();
  if (touch.matches) { sendToComputer(); return; }
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  rows.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "center" });
  rows.classList.remove("flash");
  setTimeout(() => { void rows.offsetWidth; rows.classList.add("flash"); }, reduce ? 0 : 700);
}));

// The bar announces each new section, like a track-change notice, and its
// title and record swap with a small motion.
const announce = $("[data-announce]");
let announceTimer = 0;
function announceSection(i) {
  const restart = (el, cls) => { el.classList.remove(cls); void el.offsetWidth; el.classList.add(cls); };
  restart($(".dock .meta"), "swap");
  restart($(".dock .home"), "swap");
  if (!docked || i === 0 || !secs[i].dataset.say) return;
  const s = secs[i];
  announce.querySelector(".cover").className = "cover " + covers[s.dataset.room][0];
  announce.querySelector("small").textContent = `Now playing · ${String(i + 1).padStart(2, "0")}`;
  announce.querySelector("b").textContent = s.dataset.title;
  announce.querySelector("em").textContent = s.dataset.say;
  announce.classList.add("show");
  clearTimeout(announceTimer);
  // Short, as it sits over the bottom of the page.
  announceTimer = setTimeout(() => announce.classList.remove("show"), 2200);
}
// Phone remote demo: the phone acts, a dot crosses the link, the desktop follows.
const duo = $("[data-duo]");
if (duo) {
  // Field Notes first (the demo's song); skip moves on to Slack Water.
  const tracks = [["field", "Field Notes", "#a9d394"], ["slack", "Slack Water", "#9fdcc9"], ["sodium", "Sodium Lights", "#f2b44d"], ["ferry", "Night Ferry", "#f4ac86"]];
  const steps = [...duo.parentElement.querySelectorAll("[data-step]")];
  const q = (s) => duo.querySelector(s);
  const desk = q(".desk"), ph = q(".ph");
  const dot = q("[data-dot]"), finger = q("[data-finger]"), hud = q("[data-hud]");
  const ctl = {
    ph: { pp: q('[data-btn="pp"]'), next: q('[data-btn="next"]'), prev: q('[data-btn="prev"]'), like: q('[data-btn="like"]') },
    pc: { pp: q(".d-pp"), next: q(".d-next"), prev: q(".d-prev"), like: q(".d-like"), vol: q(".d-vol") },
  };
  const other = (dev) => (dev === "ph" ? "pc" : "ph");
  const restart = (el, cls) => { el.classList.remove(cls); void el.offsetWidth; el.classList.add(cls); };
  // The phone's volume as it shows it: the level overlay and the speaker
  // button's percentage.
  const pcts = [...duo.querySelectorAll("[data-pct]")];
  const setPV = (v) => {
    duo.style.setProperty("--pv", v);
    pcts.forEach((el) => { el.textContent = Math.round(v * 100) + el.dataset.pct; });
  };

  // One player state; each screen shows it. The screen you press updates at
  // once, the other when the command arrives over the link.
  const S = { t: 0, paused: false, v: 0.5, liked: new Set() };
  const shown = { ph: { t: 0, paused: false }, pc: { t: 0, paused: false } };
  const paint = (dev, fade = true) => {
    const root = dev === "ph" ? ph : desk, was = shown[dev];
    const liked = S.liked.has(S.t);
    root.querySelectorAll("[data-ppicon]").forEach((u) => u.setAttribute("href", S.paused ? "#i-play" : "#i-pause"));
    root.querySelectorAll("[data-heart]").forEach((u) => u.setAttribute("href", liked ? "#i-heart-on" : "#i-heart"));
    root.classList.toggle("paused", S.paused);
    root.classList.toggle("liked", liked);
    was.paused = S.paused;
    if (was.t === S.t) return;
    was.t = S.t;
    const [art, name, acc] = tracks[S.t];
    const swap = () => {
      root.querySelectorAll("[data-art]").forEach((el) => { el.className = el.className.replace(/art-\w+/, "art-" + art); });
      root.querySelectorAll("[data-title]").forEach((el) => { el.textContent = name; });
      root.style.setProperty("--t-acc", acc);
      if (dev === "pc") { duo.style.setProperty("--t-acc", acc); duoNow = ["art-" + art, name]; schedule(); }
      root.classList.remove("swapping");
    };
    if (!fade) return swap();
    root.classList.add("swapping");
    setTimeout(swap, 200);
  };

  // Every wait belongs to one run; a newer run (a click, a chip, scrolling
  // away) makes the old run's waits throw, which ends it.
  let run = 0, auto = false;
  const SEND = 600;
  const wait = (ms, id) => new Promise((ok, stop) => setTimeout(() => (id === run ? ok() : stop()), ms));
  const ringOf = (dev) => (dev === "ph" ? "focus" : "ack");
  const lit = (el, cls, ms) => { el.classList.add(cls); setTimeout(() => el.classList.remove(cls), ms); };
  // The dot crosses the link. Both direction classes come off first: left
  // on, the old one kept the animation from restarting, so the dot
  // sometimes never showed.
  const signal = (to) => {
    dot.classList.remove("send", "send-back");
    void dot.offsetWidth;
    dot.classList.add(to === "pc" ? "send" : "send-back");
  };
  // Press a control: it presses in, its ring lights and the change shows on
  // that screen at once. In the demo the ring settles a moment first, so
  // you see what is about to be pressed.
  const press = async (dev, el, change, id) => {
    const ring = ringOf(dev);
    if (auto) { el.classList.add(ring); await wait(400, id); }
    lit(el, ring, 800);
    lit(el, "down", 160);
    change();
    paint(dev);
  };
  // The command crosses the link; the other screen follows and its control
  // lights up for a moment.
  const deliver = async (to, el, id) => {
    signal(to);
    await wait(SEND, id);
    paint(to);
    lit(el, ringOf(to), 1500);
  };
  const togglePause = async (from, id) => {
    await press(from, ctl[from].pp, () => { S.paused = !S.paused; }, id);
    await deliver(other(from), ctl[other(from)].pp, id);
  };
  const skip = async (from, dir, id, to = null) => {
    const key = dir > 0 ? "next" : "prev";
    await press(from, ctl[from][key], () => { S.t = to ?? (S.t + dir + tracks.length) % tracks.length; }, id);
    await deliver(other(from), ctl[other(from)][key], id);
  };
  // Like (or unlike) the playing track; a new like pops the heart on each
  // screen as it lands there.
  const toggleLike = async (from, id) => {
    const to = other(from);
    await press(from, ctl[from].like, () => { if (!S.liked.delete(S.t)) S.liked.add(S.t); }, id);
    if (S.liked.has(S.t)) restart(ctl[from].like, "pop");
    await deliver(to, ctl[to].like, id);
    if (S.liked.has(S.t)) restart(ctl[to].like, "pop");
  };
  // Volume by swipe on the phone's cover; the computer's slider follows.
  const swipe = async (nv, id) => {
    hud.classList.add("on");
    restart(finger, nv > S.v ? "up" : "down");
    await wait(200, id);
    setPV(nv);
    await wait(1200, id);
    signal("pc");
    await wait(SEND, id);
    S.v = nv;
    duo.style.setProperty("--v", nv);
    lit(ctl.pc.vol, "ack", 1500);
  };
  // Volume by click: on the phone, a tap on the cover (higher is louder);
  // on the computer, a click on the slider. That screen moves at once, the
  // other follows over the link.
  const setVolume = async (from, nv, id) => {
    nv = Math.round(Math.min(1, Math.max(0, nv)) * 100) / 100;
    S.v = nv;
    if (from === "ph") {
      hud.classList.add("on");
      setPV(nv);
      signal("pc");
      await wait(SEND, id);
      duo.style.setProperty("--v", nv);
      lit(ctl.pc.vol, "ack", 1500);
      await wait(1500, id);
    } else {
      duo.style.setProperty("--v", nv);
      lit(ctl.pc.vol, "ack", 800);
      signal("ph");
      await wait(SEND, id);
      setPV(nv);
      hud.classList.add("on");
      await wait(1700, id);
    }
    hud.classList.remove("on");
  };
  // Back to Field Notes, playing, before a step that starts from there.
  const home = async (id) => {
    if (S.t === 0 && !S.paused && !S.liked.size) return;
    S.t = 0; S.paused = false; S.liked.clear();
    paint("pc"); paint("ph");
    await wait(900, id);
  };

  // The three steps, in chip order.
  const STEPS = [
    async (id) => { // Play / pause: pause from the phone, play again from the computer.
      await home(id);
      await togglePause("ph", id); await wait(1900, id);
      await togglePause("pc", id); await wait(1900, id);
    },
    async (id) => { // Volume, twice: up, then back down. Stays on Field Notes.
      await home(id);
      await swipe(0.85, id); await wait(1600, id);
      hud.classList.remove("on"); await wait(500, id);
      await swipe(0.45, id); await wait(1600, id);
      hud.classList.remove("on"); await wait(500, id);
    },
    async (id) => { // Skip: Field Notes to Slack Water, from the phone.
      await home(id);
      await skip("ph", 1, id, 1); await wait(2600, id);
    },
    async (id) => { // Like: the heart on the phone, and the computer's fills too.
      await home(id);
      await toggleLike("ph", id); await wait(2800, id);
    },
  ];
  duo.style.setProperty("--v", S.v);
  setPV(S.v);

  // Clear what a cut-short run left behind, and bring both screens in line.
  const tidy = () => {
    duo.querySelectorAll(".focus, .down, .ack, .pop").forEach((el) => el.classList.remove("focus", "down", "ack", "pop"));
    [desk, ph].forEach((r) => r.classList.remove("swapping"));
    hud.classList.remove("on");
    setPV(S.v);
    duo.style.setProperty("--v", S.v);
    paint("pc", false); paint("ph", false);
  };
  const mark = (k) => steps.forEach((s, x) => s.classList.toggle("on", x === k));
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  // The demo plays from step k on, round and round (once with reduced motion).
  const play = async (k) => {
    const id = ++run;
    auto = true;
    duo.classList.remove("quick");
    tidy();
    try {
      do { mark(k); await STEPS[k](id); k = (k + 1) % STEPS.length; } while (!reduce);
      mark(-1);
    } catch { /* a newer run took over */ }
  };
  // Your own press on either screen stops the demo and does just that.
  // (Its sliders move quickly too: .quick shortens their glide.)
  const yours = async (fn) => {
    const id = ++run;
    auto = false;
    duo.classList.add("quick");
    tidy(); mark(-1);
    try { await fn(id); } catch { /* pressed again */ }
  };
  for (const dev of ["ph", "pc"]) {
    ctl[dev].pp.addEventListener("click", () => yours((id) => togglePause(dev, id)));
    ctl[dev].next.addEventListener("click", () => yours((id) => skip(dev, 1, id)));
    ctl[dev].prev.addEventListener("click", () => yours((id) => skip(dev, -1, id)));
    ctl[dev].like.addEventListener("click", () => yours((id) => toggleLike(dev, id)));
  }
  const cover = q(".ph-cov"), slider = q("[data-vol]");
  cover.addEventListener("click", (e) => {
    const r = cover.getBoundingClientRect();
    yours((id) => setVolume("ph", 1 - (e.clientY - r.top) / r.height, id));
  });
  ctl.pc.vol.addEventListener("click", (e) => {
    const r = slider.getBoundingClientRect();
    yours((id) => setVolume("pc", (e.clientX - r.left) / r.width, id));
  });
  let inView = false;
  new IntersectionObserver(([e]) => {
    if (e.isIntersecting === inView) return;
    inView = e.isIntersecting;
    if (inView && !reduce) play(0);
    else { run++; tidy(); mark(-1); }
  }, { threshold: 0.4 }).observe(duo);
  // A chip (a real button, so keys work too) plays its step now, and the
  // demo carries on from there.
  steps.forEach((s, j) => s.addEventListener("click", () => play(j)));
}

// And More: the lyrics view, as a small player. Each line lasts about four
// and a half seconds of a clock that only runs while it plays. Play/pause
// holds it (so does the site bar's pause), previous and next move a line,
// and a click on a line or on the seek bar jumps there; the mini player's
// and the taskbar preview's buttons work too while they show. The current
// line fills word by word, and its scene plays on the screen.
const xl = $("[data-xl]");
if (xl) {
  const win = xl.querySelector(".xl-win"), list = xl.querySelector("[data-xl-lines]");
  const items = [...list.children], fx = [...xl.querySelectorAll("[data-for]")];
  const time = xl.querySelector("[data-xl-time]"), seekHit = xl.querySelector("[data-xl-seek]");
  const bars = [...xl.querySelectorAll("[data-xl-seekbar]")], ppBtn = xl.querySelector(".d-ctl [data-xa='pp']");
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const LINE = 4600, FILL = 2400, SONG = 252;
  const len = (k) => +(items[k].dataset.ms || LINE);
  // Where each line starts on the demo clock, and the whole song's length.
  const starts = [];
  let total = 0;
  items.forEach((li, k) => { starts.push(total); total += len(k); });
  items.forEach((li) => {
    const t = li.querySelector(".t"), words = t.textContent.trim().split(/\s+/);
    t.innerHTML = words.map((w, k) => `<span style="--d:${Math.round((k * FILL) / words.length)}ms">${w}</span>`).join(" ");
  });
  // Scenes that pause the music partway into their line: the hotkey pauses
  // and plays again; "goes quiet" pauses once the line has been sung.
  const CUES = { keys: [[1400, true], [3200, false]], quiet: [[2600, true]] };

  // cur: the line; t: ms into it (runs while not held); pos: where the seek
  // bar is (stands still while the music is paused, by you or by a scene).
  let cur = 0, t = 0, pos = 0, cue = 0, held = reduce, still = false, live = false, raf = 0, last = 0, shownS = -1, painted = "";
  const siteHeld = () => root.hasAttribute("data-paused");
  const stopped = () => held || siteHeld();
  const paint = () => {
    const key = `${stopped()}|${still}`;
    if (key === painted) return;
    painted = key;
    xl.classList.toggle("held", stopped());
    xl.classList.toggle("paused", stopped() || still);
    ppBtn.setAttribute("aria-label", stopped() || still ? "Play" : "Pause");
  };
  const fmt = (s) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
  const showPos = () => {
    bars.forEach((b) => b.style.setProperty("--xp", (pos / total).toFixed(4)));
    const s = Math.floor((pos / total) * SONG);
    if (s !== shownS) { shownS = s; time.textContent = `${fmt(s)} / ${fmt(SONG)}`; }
  };
  // Keep the current line a little above the middle of the lyrics window.
  // Measured as the layout will be once the details have opened and closed.
  const place = () => {
    let y = 0;
    for (let k = 0; k < cur; k++) y += items[k].offsetHeight - items[k].querySelector(".dt").offsetHeight;
    const li = items[cur], dt = li.querySelector(".dt");
    const h = li.offsetHeight - dt.offsetHeight + dt.scrollHeight;
    list.style.transform = `translateY(${Math.round(win.clientHeight * 0.4 - y - h / 2)}px)`;
  };
  // Go to line i, off ms into it. The same line again starts over: its
  // fill and its scene restart.
  const go = (i, off = 0) => {
    cur = (i + items.length) % items.length;
    t = off; pos = starts[cur] + off; still = false; cue = 0;
    const key = items[cur].dataset.k, cs = CUES[key] || [];
    while (cue < cs.length && off >= cs[cue][0]) still = cs[cue++][1];
    list.style.setProperty("--o", off + "ms");
    items.forEach((li) => li.classList.remove("on"));
    fx.forEach((el) => el.classList.remove("show"));
    void list.offsetWidth;
    items.forEach((li, k) => {
      li.classList.toggle("on", k === cur);
      li.classList.toggle("past", k < cur);
      li.classList.toggle("far", Math.abs(k - cur) >= 3);
      li.querySelector("button").setAttribute("aria-current", String(k === cur));
    });
    xl.dataset.scene = key;
    fx.forEach((el) => el.classList.toggle("show", el.dataset.for === key));
    painted = "";
    paint(); showPos(); place();
  };
  // The clock runs once per frame while it plays, and sleeps while it's held:
  // an idle frame loop still makes the page render every frame.
  const frame = (now) => {
    raf = 0;
    if (!live) return;
    paint();
    if (stopped()) return;
    const dt = Math.min(100, now - last);
    last = now;
    t += dt;
    const cs = CUES[items[cur].dataset.k] || [];
    while (cue < cs.length && t >= cs[cue][0]) still = cs[cue++][1];
    if (!still) pos = Math.min(pos + dt, starts[cur] + len(cur));
    if (t >= len(cur)) go(cur + 1);
    showPos();
    raf = requestAnimationFrame(frame);
  };
  const wake = () => { paint(); if (live && !stopped() && !raf) { last = performance.now(); raf = requestAnimationFrame(frame); } };
  // The site bar's play/pause holds it too (its own handler runs first).
  document.querySelectorAll("[data-pp]").forEach((b) => b.addEventListener("click", wake));
  // Play also ends the site bar's pause, so play works wherever you press it.
  const act = (a) => {
    if (a === "pp") {
      if (stopped() || still) { held = false; still = false; if (siteHeld()) dockPP.click(); }
      else held = true;
      wake();
    } else if (a === "next") go(cur + 1);
    // Previous starts the line over, or goes back one if it has just begun.
    else if (a === "prev") go(t > 1500 ? cur : cur - 1);
  };
  xl.addEventListener("click", (e) => {
    const b = e.target.closest("[data-xa]");
    if (b) act(b.dataset.xa);
  });
  seekHit.addEventListener("click", (e) => {
    const r = seekHit.getBoundingClientRect();
    const ms = Math.min(0.999, Math.max(0, (e.clientX - r.left) / r.width)) * total;
    let k = 0;
    while (k + 1 < items.length && starts[k + 1] <= ms) k++;
    go(k, ms - starts[k]);
  });
  items.forEach((li, k) => li.querySelector("button").addEventListener("click", () => go(k)));
  new IntersectionObserver(([e]) => {
    if (e.isIntersecting === live) return;
    live = e.isIntersecting;
    // Coming back on screen starts the song over.
    if (live) { go(0); wake(); }
  }, { threshold: 0.4 }).observe(xl);
  // When the lyrics window changes size (not on every window resize: a
  // phone's toolbar fires those mid-scroll).
  new ResizeObserver(() => place()).observe(win);
  go(0);

  // From the phone remote: its computer screen grows into this one. Between
  // two scroll points (the remote's screen centred, and this one at rest)
  // both screens travel one path, from the remote's place and size to this
  // one's: the remote's on top, live, until it fades off this one halfway.
  // The phone, the link and the chips fade away meanwhile.
  // Scroll-driven animations: the browser runs them in step with the
  // scroll, off the main thread, so nothing lags or steps. Script only lays
  // the path down again when the layout changes. Wide screens only; none
  // with reduced motion, or where scroll timelines aren't supported.
  const fromMon = document.querySelector("#remote .desk .mon");
  const toMon = xl.querySelector(".mon");
  const fades = [...document.querySelectorAll("#remote :is(.ph, .link, .duo-steps, .duo-facts)")];
  if (fromMon && !reduce && "ScrollTimeline" in window) {
    const wide = matchMedia("(min-width: 701px)");
    const timeline = new ScrollTimeline({ source: document.documentElement, axis: "block" });
    const ease = (x) => x * x * (3 - 2 * x);
    const clamp = (x) => Math.min(1, Math.max(0, x));
    let anims = [];
    const lay = () => {
      anims.forEach((an) => an.cancel());
      anims = [];
      if (!wide.matches) return;
      // Measured as laid out, with the animations off.
      const y0 = scrollY, vh = innerHeight, room = vh - 90;
      const R = fromMon.getBoundingClientRect(), X = xl.getBoundingClientRect();
      const w = toMon.offsetWidth, h = toMon.offsetHeight;
      const rTop = R.top + y0, rLeft = R.left, xTop = X.top + y0, xLeft = X.left + (X.width - w) / 2;
      const rv = (room - R.height) / 2, xv = Math.max(16, (room - h) / 2);
      const a = rTop - rv, b = xTop - xv, max = document.documentElement.scrollHeight - vh;
      if (b - a < 200 || b > max) return;
      const at = (y) => clamp(y / max);
      // The shared path: where the travelling screen is on the viewport.
      const box = (y) => {
        const e = ease(clamp((y - a) / (b - a)));
        return { top: rv + (xv - rv) * e, left: rLeft + (xLeft - rLeft) * e, w: R.width + (w - R.width) * e, h: R.height + (h - R.height) * e };
      };
      // A screen's transform puts its laid-out box (dTop on the page) on the path.
      const put = (P, y, dTop, dLeft, dw, dh) =>
        `translate(${(P.left - dLeft).toFixed(2)}px, ${(P.top - (dTop - y)).toFixed(2)}px) scale(${(P.w / dw).toFixed(5)}, ${(P.h / dh).toFixed(5)})`;
      const fadeA = a + (b - a) * 0.45, fadeB = a + (b - a) * 0.6;
      const N = 48, toK = [], fromK = [];
      for (let i = 0; i <= N; i++) {
        const y = a + ((b - a) * i) / N, P = box(y);
        toK.push({ offset: at(y), transform: put(P, y, xTop, xLeft, w, h) });
        fromK.push({ offset: at(y), transform: put(P, y, rTop, rLeft, R.width, R.height), opacity: 1 - ease(clamp((y - fadeA) / (fadeB - fadeA))) });
      }
      // Before the path the remote's screen is itself and this one waits
      // under it; after, this one is itself and the remote's is gone.
      toK.unshift({ ...toK[0], offset: 0 }); toK.push({ ...toK[N + 1], offset: 1 });
      fromK.unshift({ ...fromK[0], offset: 0 }); fromK.push({ ...fromK[N + 1], offset: 1 });
      const opt = { timeline, fill: "both" };
      anims.push(toMon.animate(toK, opt), fromMon.animate(fromK, opt));
      // Gone for good once faded, so it can't catch clicks meant for this one.
      const gone = Math.min(0.9999, at(fadeB));
      anims.push(fromMon.animate([{ offset: 0, visibility: "visible" }, { offset: gone, visibility: "visible" }, { offset: Math.min(1, gone + 0.0001), visibility: "hidden" }, { offset: 1, visibility: "hidden" }], opt));
      const fa = at(a), fb = at(a + (b - a) * 0.3);
      fades.forEach((el) => anims.push(el.animate([{ offset: 0, opacity: 1 }, { offset: fa, opacity: 1 }, { offset: fb, opacity: 0 }, { offset: 1, opacity: 0 }], opt)));
    };
    let wait = 0;
    const later = () => { clearTimeout(wait); wait = setTimeout(lay, 120); };
    addEventListener("resize", later);
    new ResizeObserver(later).observe(document.body);
    document.fonts.ready.then(lay);
    lay();
  }
}
// Sound: the crossfade styles as tracks, played in turn; the readout's
// curve pair morphs between them. Every shape has the same commands, so the
// path can animate between them.
const shapes = [
  ["M2 6 C 16 6, 16 6, 30 6 C 40 6, 46 22, 62 22 C 62 22, 62 22, 62 22", "M2 22 C 16 22, 16 22, 30 22 C 40 22, 46 6, 62 6 C 62 6, 62 6, 62 6", "none"],
  ["M2 6 C 24 6, 24 6, 46 6 C 52 6, 54 22, 62 22 C 62 22, 62 22, 62 22", "M2 22 C 24 22, 24 22, 46 22 C 52 22, 54 6, 62 6 C 62 6, 62 6, 62 6", "none"],
  ["M2 6 C 20 6, 20 6, 36 6 C 37 6, 37 22, 38 22 C 50 22, 50 22, 62 22", "M2 22 C 14 22, 14 22, 26 22 C 30 22, 32 8, 38 6 C 50 6, 50 6, 62 6", "none"],
  ["M2 6 C 12 6, 12 6, 22 6 C 36 6, 40 22, 62 22 C 62 22, 62 22, 62 22", "M2 22 C 12 22, 12 22, 22 22 C 36 22, 40 6, 62 6 C 62 6, 62 6, 62 6", "6 9"],
];
const alb = $("[data-alb]");
if (alb) {
  // The readout in the app's own words (lib/dsp.ts
  // describeTransition; Auto's reason is the engine's, automix.rs).
  const reads = [
    ["Classic crossfade", ["6.0 s overlap"]],
    ["Quick", ["2.0 s overlap", "Silence skipped: 1.2 s end, 0.4 s start"]],
    ["Blend", ["8.0 s overlap", "Cut: 9.4 s of the ending, 3.1 s of the intro"]],
    ["Auto → Blend (beat-matched)", ["Why: long fade-out: cut it", "7.8 s overlap (16 beats)", "Tempo: 122.0 BPM (91%) → 123.5 BPM (87%)"]],
  ];
  const rows = [...alb.querySelectorAll("[data-alb-xf]")];
  const [bOut, bIn] = alb.querySelectorAll(".alb-fig path");
  const bTitle = alb.querySelector("[data-alb-xf-title]"), bNotes = alb.querySelector("[data-alb-xf-notes]");
  // The record: a new style slides the old one home (out of sight in the
  // sleeve), its label changes, and it comes back out turned.
  const rec = alb.querySelector("[data-alb-rec]"), vin = rec.firstElementChild, lbl = alb.querySelector("[data-alb-lbl]");
  const labels = ["Classic", "Quick", "Blend", "Auto"];
  const still = matchMedia("(prefers-reduced-motion: reduce)");
  let bi = 0, bTimer, swapT, turn = 0, pending = -1;
  const press = (i) => { vin.dataset.lbl = i; lbl.textContent = labels[i]; };
  // The label changes only once the record is home (its slide has ended;
  // the timer covers a slide that never ran).
  const comeOut = () => {
    clearTimeout(swapT);
    if (pending < 0) return;
    press(pending); pending = -1;
    vin.style.setProperty("--turn", ++turn);
    rec.classList.remove("in");
  };
  rec.addEventListener("transitionend", (e) => { if (e.target === rec && e.propertyName === "transform" && rec.classList.contains("in")) comeOut(); });
  const swap = (i) => {
    if (still.matches) { press(i); return; }
    pending = i;
    rec.classList.add("in");
    clearTimeout(swapT); swapT = setTimeout(comeOut, 700);
  };
  const setB = (i) => {
    if (i !== bi) swap(i);
    bi = i;
    rows.forEach((r, k) => { r.classList.toggle("on", k === i); r.setAttribute("aria-current", k === i ? "true" : "false"); });
    bOut.style.d = `path("${shapes[i][0]}")`; bIn.style.d = `path("${shapes[i][1]}")`;
    bOut.style.strokeDasharray = bIn.style.strokeDasharray = shapes[i][2];
    bTitle.textContent = reads[i][0];
    bNotes.replaceChildren(...reads[i][1].map((t) => Object.assign(document.createElement("li"), { textContent: t })));
  };
  // The styles play in turn only while the album is on screen.
  let albOn = false;
  const cycleB = () => { clearInterval(bTimer); if (albOn && !still.matches) bTimer = setInterval(() => setB((bi + 1) % 4), 4500); };
  rows.forEach((r, k) => r.addEventListener("click", () => { setB(k); cycleB(); }));
  setB(0);
  new IntersectionObserver(([e]) => { albOn = e.isIntersecting; cycleB(); }).observe(alb);
}
