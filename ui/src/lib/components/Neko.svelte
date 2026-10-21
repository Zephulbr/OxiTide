<script lang="ts">
  import onekoUrl from "../../assets/oneko/oneko.gif";
  import {
    clampPosition,
    createNekoEngine,
    NEKO_SIZE,
    TICK_MS,
    type NekoEngine,
    type NekoFrame,
    type PerchRect,
  } from "../oneko";
  import { createRubDetector, type RubDetector } from "../petting";

  const STORAGE_KEY = "oxitide:oneko";
  /// Kuroneko is out (KN-D1): right-click oneko to summon or dismiss her.
  const KURO_KEY = "oxitide:kuroneko";
  const SAVE_INTERVAL_MS = 1000;
  /// Kuroneko curls up this far left of oneko on the perch (KN-D3).
  const KURO_PERCH_OFFSET = 28;
  /// Where kuroneko appears when summoned, relative to oneko.
  const KURO_SPAWN_DX = -40;
  const MAX_HEARTS = 6;
  const HEART_DELAYS_MS = [0, 140, 280];
  const HEART_SPREAD_PX = [-7, 2, 9];

  interface Cat {
    engine: NekoEngine;
    rub: RubDetector;
    frame: NekoFrame;
    node: HTMLDivElement | undefined;
  }
  interface Saved {
    x: number;
    y: number;
    kx?: number;
    ky?: number;
  }
  interface Heart {
    id: number;
    x: number;
    y: number;
    delay: number;
  }

  const idleSprite = { col: 3, row: 3 };
  const newCat = (): Cat => ({
    engine: createNekoEngine(),
    rub: createRubDetector(),
    frame: { x: NEKO_SIZE, y: NEKO_SIZE, sprite: idleSprite },
    node: undefined,
  });
  const oneko = newCat();
  const kuro = newCat();

  let onekoEl: HTMLDivElement | undefined = $state();
  let kuroEl: HTMLDivElement | undefined = $state();
  let kuroOut = $state(readKuroOut());
  let hearts = $state<Heart[]>([]);
  let heartId = 0;
  /// The last double-click asked them to sleep; kuroneko follows it even if
  /// she was summoned afterwards.
  let sleepCommanded = false;
  let kuroForcedSleep = false;
  /// A cat is held: she stays under the cursor, so no rub can count.
  let dragging = false;

  function readKuroOut(): boolean {
    try {
      return localStorage.getItem(KURO_KEY) === "1";
    } catch {
      return false;
    }
  }

  function writeKuroOut(out: boolean) {
    try {
      if (out) localStorage.setItem(KURO_KEY, "1");
      else localStorage.removeItem(KURO_KEY);
    } catch {
      return;
    }
  }

  function loadSaved(): Saved | null {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw) as { x?: unknown; y?: unknown; kx?: unknown; ky?: unknown };
      if (typeof parsed.x !== "number" || typeof parsed.y !== "number") return null;
      const saved: Saved = { x: parsed.x, y: parsed.y };
      if (typeof parsed.kx === "number" && typeof parsed.ky === "number") {
        saved.kx = parsed.kx;
        saved.ky = parsed.ky;
      }
      return saved;
    } catch {
      return null;
    }
  }

  function save() {
    const saved: Saved = { x: oneko.frame.x, y: oneko.frame.y };
    if (kuroOut) {
      saved.kx = kuro.frame.x;
      saved.ky = kuro.frame.y;
    }
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(saved));
    } catch {
      return;
    }
  }

  function perchRect(): PerchRect | null {
    const perch = document.querySelector<HTMLElement>("[data-oneko-perch]");
    if (!perch) return null;
    const rect = perch.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    return { left: rect.left, right: rect.right, top: rect.top };
  }

  function kuroPerch(perch: PerchRect | null): PerchRect | null {
    if (!perch) return null;
    return { left: perch.left - KURO_PERCH_OFFSET, right: perch.right - KURO_PERCH_OFFSET, top: perch.top };
  }

  function draw(cat: Cat) {
    const node = cat.node;
    if (!node) return;
    const { x, y, sprite } = cat.frame;
    node.style.left = `${x - NEKO_SIZE / 2}px`;
    node.style.top = `${y - NEKO_SIZE / 2}px`;
    node.style.backgroundPosition = `${-sprite.col * NEKO_SIZE}px ${-sprite.row * NEKO_SIZE}px`;
  }

  function apply(cat: Cat, frame: NekoFrame) {
    cat.frame = frame;
    draw(cat);
  }

  function syncKuroSleep(perch: PerchRect | null) {
    if (kuroForcedSleep === sleepCommanded) return;
    kuro.engine.toggleSleep(kuroPerch(perch));
    kuroForcedSleep = sleepCommanded;
  }

  /// Double-click on either cat: both go to sleep on the seek bar, or wake.
  function toggleSleep() {
    const perch = perchRect();
    sleepCommanded = !sleepCommanded;
    oneko.engine.toggleSleep(perch);
    syncKuroSleep(perch);
  }

  function summonKuro() {
    const spot = clampPosition(oneko.frame.x + KURO_SPAWN_DX, oneko.frame.y, window.innerWidth, window.innerHeight);
    kuro.frame = kuro.engine.setPosition(spot.x, spot.y);
    kuroOut = true;
    syncKuroSleep(perchRect());
    writeKuroOut(true);
    save();
  }

  function dismissKuro() {
    kuroOut = false;
    writeKuroOut(false);
    save();
  }

  function spawnHearts(x: number, y: number) {
    const fresh = HEART_DELAYS_MS.map((delay, i) => ({
      id: (heartId += 1),
      x: x + HEART_SPREAD_PX[i],
      y,
      delay,
    }));
    hearts = [...hearts, ...fresh].slice(-MAX_HEARTS);
  }

  function dropHeart(id: number) {
    hearts = hearts.filter((h) => h.id !== id);
  }

  /// Rubbing the cursor over a cat pets her (KN-D4). Hearts either way; a
  /// sleeping cat just stays asleep.
  function feedRub(cat: Cat, mx: number, my: number, now: number) {
    const half = NEKO_SIZE / 2;
    const inside = Math.abs(mx - cat.frame.x) <= half && Math.abs(my - cat.frame.y) <= half;
    if (!cat.rub.feed(mx, now, inside)) return;
    cat.engine.pet();
    spawnHearts(cat.frame.x, cat.frame.y - half);
  }

  /// Left button on a cat drags her; the listeners live until release.
  function startDrag(cat: Cat, event: MouseEvent): () => void {
    const node = cat.node;
    if (!node) return () => {};
    event.preventDefault();
    cat.engine.startDrag(event.clientX, event.clientY);
    node.style.cursor = "grabbing";
    dragging = true;
    const onMove = (move: MouseEvent) => apply(cat, cat.engine.dragTo(move.clientX, move.clientY));
    const stop = () => {
      dragging = false;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    const onUp = () => {
      cat.engine.endDrag();
      node.style.cursor = "";
      stop();
      save();
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return stop;
  }

  $effect(() => {
    const node = onekoEl;
    if (!node) return;
    oneko.node = node;
    let lastSave = performance.now();

    const width = () => window.innerWidth;
    const height = () => window.innerHeight;
    oneko.engine.setViewport(width(), height());
    kuro.engine.setViewport(width(), height());
    const saved = loadSaved();
    if (saved) {
      const pos = clampPosition(saved.x, saved.y, width(), height());
      apply(oneko, oneko.engine.setPosition(pos.x, pos.y));
      const kpos =
        saved.kx !== undefined && saved.ky !== undefined
          ? clampPosition(saved.kx, saved.ky, width(), height())
          : clampPosition(pos.x + KURO_SPAWN_DX, pos.y, width(), height());
      kuro.frame = kuro.engine.setPosition(kpos.x, kpos.y);
    } else {
      apply(oneko, oneko.engine.tick());
    }

    let raf = 0;
    let lastTick = 0;
    const loop = (now: number) => {
      if (now - lastTick >= TICK_MS) {
        lastTick = now;
        apply(oneko, oneko.engine.tick());
        if (kuroOut && kuro.node) {
          kuro.engine.setMouse(oneko.frame.x, oneko.frame.y);
          apply(kuro, kuro.engine.tick());
        }
        if (now - lastSave >= SAVE_INTERVAL_MS) {
          lastSave = now;
          save();
        }
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);

    const onMouseMove = (event: MouseEvent) => {
      oneko.engine.setMouse(event.clientX, event.clientY);
      if (dragging) return;
      const now = performance.now();
      feedRub(oneko, event.clientX, event.clientY, now);
      if (kuroOut) feedRub(kuro, event.clientX, event.clientY, now);
    };
    const onResize = () => {
      oneko.engine.setViewport(width(), height());
      kuro.engine.setViewport(width(), height());
      const perch = perchRect();
      oneko.engine.setPerch(perch);
      kuro.engine.setPerch(kuroPerch(perch));
    };

    let dragCleanup: (() => void) | null = null;
    const onMouseDown = (event: MouseEvent) => {
      if (event.button !== 0) return;
      dragCleanup?.();
      dragCleanup = startDrag(oneko, event);
    };
    const onContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      if (kuroOut) dismissKuro();
      else summonKuro();
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("resize", onResize);
    node.addEventListener("mousedown", onMouseDown);
    node.addEventListener("dblclick", toggleSleep);
    node.addEventListener("contextmenu", onContextMenu);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("resize", onResize);
      node.removeEventListener("mousedown", onMouseDown);
      node.removeEventListener("dblclick", toggleSleep);
      node.removeEventListener("contextmenu", onContextMenu);
      dragCleanup?.();
      save();
      oneko.node = undefined;
    };
  });

  $effect(() => {
    const node = kuroEl;
    if (!node) return;
    kuro.node = node;
    draw(kuro);
    syncKuroSleep(perchRect());

    let dragCleanup: (() => void) | null = null;
    const onMouseDown = (event: MouseEvent) => {
      if (event.button !== 0) return;
      dragCleanup?.();
      dragCleanup = startDrag(kuro, event);
    };
    const onContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      dismissKuro();
    };
    node.addEventListener("mousedown", onMouseDown);
    node.addEventListener("dblclick", toggleSleep);
    node.addEventListener("contextmenu", onContextMenu);
    return () => {
      node.removeEventListener("mousedown", onMouseDown);
      node.removeEventListener("dblclick", toggleSleep);
      node.removeEventListener("contextmenu", onContextMenu);
      dragCleanup?.();
      kuro.node = undefined;
    };
  });
</script>

<div
  class="neko"
  bind:this={onekoEl}
  aria-hidden="true"
  style:background-image={`url(${onekoUrl})`}
></div>
{#if kuroOut}
  <div
    class="neko kuro"
    bind:this={kuroEl}
    aria-hidden="true"
    style:background-image={`url(${onekoUrl})`}
  ></div>
{/if}
{#each hearts as heart (heart.id)}
  <div
    class="heart"
    aria-hidden="true"
    style:left={`${heart.x}px`}
    style:top={`${heart.y}px`}
    style:animation-delay={`${heart.delay}ms`}
    onanimationend={() => dropHeart(heart.id)}
  >
    <svg viewBox="0 0 7 6" width="14" height="12" shape-rendering="crispEdges">
      <path d="M1 0h2v1h1V0h2v1h1v2H6v1H5v1H4v1H3V5H2V4H1V3H0V1h1z" />
    </svg>
  </div>
{/each}

<style>
  .neko {
    position: fixed;
    left: 0;
    top: 0;
    width: 32px;
    height: 32px;
    z-index: 8;
    pointer-events: auto;
    cursor: grab;
    background-repeat: no-repeat;
    image-rendering: pixelated;
  }
  /* The black cat is the classic sprite inverted: black fur, white lines. */
  .kuro {
    filter: invert(1);
  }
  .heart {
    position: fixed;
    z-index: 8;
    pointer-events: none;
    color: var(--accent);
    transform: translate(-50%, -100%);
    opacity: 0;
    animation: heart-float 900ms ease-out forwards;
  }
  .heart svg {
    display: block;
    fill: currentColor;
  }
  @keyframes heart-float {
    0% {
      opacity: 0;
      transform: translate(-50%, -100%) scale(0.6);
    }
    20% {
      opacity: 1;
      transform: translate(-50%, -120%) scale(1);
    }
    100% {
      opacity: 0;
      transform: translate(-50%, -260%) scale(1);
    }
  }
</style>
