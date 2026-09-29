<script lang="ts">
  import onekoUrl from "../../assets/oneko/oneko.gif";
  import {
    clampPosition,
    createNekoEngine,
    NEKO_SIZE,
    TICK_MS,
    type NekoFrame,
    type PerchRect,
  } from "../oneko";

  const STORAGE_KEY = "oxitide:oneko";
  const SAVE_INTERVAL_MS = 1000;

  let el: HTMLDivElement | undefined;
  const engine = createNekoEngine();

  function loadPosition(): { x: number; y: number } | null {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return null;
      const parsed = JSON.parse(raw) as { x?: unknown; y?: unknown };
      if (typeof parsed.x !== "number" || typeof parsed.y !== "number") return null;
      return { x: parsed.x, y: parsed.y };
    } catch {
      return null;
    }
  }

  function perchRect(): PerchRect | null {
    const perch = document.querySelector<HTMLElement>("[data-oneko-perch]");
    if (!perch) return null;
    const rect = perch.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    return { left: rect.left, right: rect.right, top: rect.top };
  }

  $effect(() => {
    const node = el;
    if (!node) return;
    let lastFrame: NekoFrame = { x: NEKO_SIZE, y: NEKO_SIZE, sprite: { col: 3, row: 3 } };
    let lastSave = performance.now();

    function save() {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify({ x: lastFrame.x, y: lastFrame.y }));
      } catch {
        return;
      }
    }

    const apply = (frame: NekoFrame) => {
      lastFrame = frame;
      node.style.left = `${frame.x - NEKO_SIZE / 2}px`;
      node.style.top = `${frame.y - NEKO_SIZE / 2}px`;
      node.style.backgroundPosition = `${-frame.sprite.col * NEKO_SIZE}px ${-frame.sprite.row * NEKO_SIZE}px`;
      const now = performance.now();
      if (now - lastSave >= SAVE_INTERVAL_MS) {
        lastSave = now;
        save();
      }
    };

    engine.setViewport(window.innerWidth, window.innerHeight);
    const saved = loadPosition();
    if (saved) {
      const pos = clampPosition(saved.x, saved.y, window.innerWidth, window.innerHeight);
      apply(engine.setPosition(pos.x, pos.y));
    } else {
      apply(engine.tick());
    }

    let raf = 0;
    let lastTick = 0;
    const loop = (now: number) => {
      if (now - lastTick >= TICK_MS) {
        lastTick = now;
        apply(engine.tick());
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);

    const onMouseMove = (event: MouseEvent) => engine.setMouse(event.clientX, event.clientY);
    const onResize = () => {
      engine.setViewport(window.innerWidth, window.innerHeight);
      engine.setPerch(perchRect());
    };

    let dragCleanup: (() => void) | null = null;
    const onMouseDown = (event: MouseEvent) => {
      if (event.button !== 0) return;
      event.preventDefault();
      engine.startDrag(event.clientX, event.clientY);
      node.style.cursor = "grabbing";
      const onDragMove = (moveEvent: MouseEvent) => {
        apply(engine.dragTo(moveEvent.clientX, moveEvent.clientY));
      };
      const onDragUp = () => {
        engine.endDrag();
        node.style.cursor = "";
        window.removeEventListener("mousemove", onDragMove);
        window.removeEventListener("mouseup", onDragUp);
        dragCleanup = null;
        save();
      };
      window.addEventListener("mousemove", onDragMove);
      window.addEventListener("mouseup", onDragUp);
      dragCleanup = () => {
        window.removeEventListener("mousemove", onDragMove);
        window.removeEventListener("mouseup", onDragUp);
      };
    };
    const onDblClick = () => engine.toggleSleep(perchRect());

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("resize", onResize);
    node.addEventListener("mousedown", onMouseDown);
    node.addEventListener("dblclick", onDblClick);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("resize", onResize);
      node.removeEventListener("mousedown", onMouseDown);
      node.removeEventListener("dblclick", onDblClick);
      dragCleanup?.();
      save();
    };
  });
</script>

<div
  class="neko"
  bind:this={el}
  aria-hidden="true"
  style:background-image={`url(${onekoUrl})`}
></div>

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
</style>
