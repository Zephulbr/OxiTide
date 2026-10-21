export const NEKO_SIZE = 32;
export const NEKO_SPEED = 10;
export const TICK_MS = 100;
export const IDLE_RADIUS = 48;
export const PERCH_SETTLE = 0.6;
/// Ticks a petted cat holds still with her eyes closed (1.5 s).
export const PET_TICKS = 15;

export interface Sprite {
  col: number;
  row: number;
}

export interface NekoFrame {
  x: number;
  y: number;
  sprite: Sprite;
}

export interface PerchRect {
  left: number;
  right: number;
  top: number;
}

export interface NekoEngine {
  setMouse(x: number, y: number): void;
  setViewport(width: number, height: number): void;
  setPosition(x: number, y: number): NekoFrame;
  startDrag(x: number, y: number): void;
  dragTo(x: number, y: number): NekoFrame;
  endDrag(): void;
  toggleSleep(perch: PerchRect | null): void;
  setPerch(perch: PerchRect | null): void;
  /// Petted: hold still, eyes closed, for `PET_TICKS`. A sleeping cat stays
  /// asleep. False when she did not react (asleep or being dragged).
  pet(): boolean;
  /// True while she sleeps (told to, or dozed off on her own).
  asleep(): boolean;
  tick(): NekoFrame;
}

type SpriteName =
  | "idle"
  | "alert"
  | "tired"
  | "sleeping"
  | "scratchSelf"
  | "scratchWallN"
  | "scratchWallS"
  | "scratchWallE"
  | "scratchWallW"
  | "N"
  | "NE"
  | "E"
  | "SE"
  | "S"
  | "SW"
  | "W"
  | "NW";

const SPRITES: Record<SpriteName, Sprite[]> = {
  idle: [{ col: 3, row: 3 }],
  alert: [{ col: 7, row: 3 }],
  scratchSelf: [{ col: 5, row: 0 }, { col: 6, row: 0 }, { col: 7, row: 0 }],
  scratchWallN: [{ col: 0, row: 0 }, { col: 0, row: 1 }],
  scratchWallS: [{ col: 7, row: 1 }, { col: 6, row: 2 }],
  scratchWallE: [{ col: 2, row: 2 }, { col: 2, row: 3 }],
  scratchWallW: [{ col: 4, row: 0 }, { col: 4, row: 1 }],
  tired: [{ col: 3, row: 2 }],
  sleeping: [{ col: 2, row: 0 }, { col: 2, row: 1 }],
  N: [{ col: 1, row: 2 }, { col: 1, row: 3 }],
  NE: [{ col: 0, row: 2 }, { col: 0, row: 3 }],
  E: [{ col: 3, row: 0 }, { col: 3, row: 1 }],
  SE: [{ col: 5, row: 1 }, { col: 5, row: 2 }],
  S: [{ col: 6, row: 3 }, { col: 7, row: 2 }],
  SW: [{ col: 5, row: 3 }, { col: 6, row: 1 }],
  W: [{ col: 4, row: 2 }, { col: 4, row: 3 }],
  NW: [{ col: 1, row: 0 }, { col: 1, row: 1 }],
};

export function clampPosition(
  x: number,
  y: number,
  width: number,
  height: number,
): { x: number; y: number } {
  const half = NEKO_SIZE / 2;
  return {
    x: Math.min(Math.max(x, half), Math.max(half, width - half)),
    y: Math.min(Math.max(y, half), Math.max(half, height - half)),
  };
}

export function createNekoEngine(opts: { rng?: () => number } = {}): NekoEngine {
  const rng = opts.rng ?? Math.random;
  let x = 32;
  let y = 32;
  let mouseX = 0;
  let mouseY = 0;
  let width = 0;
  let height = 0;
  let frameCount = 0;
  let idleTime = 0;
  let idleAnimation: SpriteName | null = null;
  let idleAnimationFrame = 0;
  let forceSleep = false;
  let grabbing = false;
  let dragAnchor: { mouseX: number; mouseY: number; nekoX: number; nekoY: number } | null = null;
  let dragSprite: SpriteName = "alert";
  let nudge = false;
  let petTicks = 0;

  function spriteFor(name: SpriteName, frame: number): Sprite {
    const set = SPRITES[name];
    return set[((frame % set.length) + set.length) % set.length];
  }

  function resetIdleAnimation() {
    idleAnimation = null;
    idleAnimationFrame = 0;
  }

  function idleFrame(): NekoFrame {
    idleTime += 1;
    if (idleTime > 10 && Math.floor(rng() * 200) === 0 && idleAnimation === null) {
      const options: SpriteName[] = ["sleeping", "scratchSelf"];
      if (x < NEKO_SIZE) options.push("scratchWallW");
      if (y < NEKO_SIZE) options.push("scratchWallN");
      if (x > width - NEKO_SIZE) options.push("scratchWallE");
      if (y > height - NEKO_SIZE) options.push("scratchWallS");
      idleAnimation = options[Math.floor(rng() * options.length)];
    }
    if (forceSleep) idleAnimation = "sleeping";
    let sprite = spriteFor("idle", 0);
    switch (idleAnimation) {
      case "sleeping":
        if (idleAnimationFrame < 8 && nudge && forceSleep) {
          sprite = spriteFor("idle", 0);
          break;
        }
        if (nudge) {
          nudge = false;
          resetIdleAnimation();
        }
        if (idleAnimationFrame < 8) {
          sprite = spriteFor("tired", 0);
          break;
        }
        sprite = spriteFor("sleeping", Math.floor(idleAnimationFrame / 4));
        if (idleAnimationFrame > 192 && !forceSleep) resetIdleAnimation();
        break;
      case "scratchWallN":
      case "scratchWallS":
      case "scratchWallE":
      case "scratchWallW":
      case "scratchSelf":
        sprite = spriteFor(idleAnimation, idleAnimationFrame);
        if (idleAnimationFrame > 9) resetIdleAnimation();
        break;
      default:
        return { x, y, sprite };
    }
    idleAnimationFrame += 1;
    return { x, y, sprite };
  }

  function directionFor(diffX: number, diffY: number, distance: number): SpriteName {
    let direction = diffY / distance > 0.5 ? "N" : "";
    direction += diffY / distance < -0.5 ? "S" : "";
    direction += diffX / distance > 0.5 ? "W" : "";
    direction += diffX / distance < -0.5 ? "E" : "";
    return direction as SpriteName;
  }

  function sleepTarget(perch: PerchRect | null): { x: number; y: number } {
    return perch
      ? { x: perch.left + (perch.right - perch.left) * PERCH_SETTLE, y: perch.top - 8 }
      : { x, y };
  }

  function asleep(): boolean {
    return forceSleep || (idleAnimation === "sleeping" && idleAnimationFrame >= 8);
  }

  function pet(): boolean {
    if (grabbing || asleep()) return false;
    petTicks = PET_TICKS;
    resetIdleAnimation();
    return true;
  }

  function startDrag(mx: number, my: number) {
    petTicks = 0;
    grabbing = true;
    dragAnchor = { mouseX: mx, mouseY: my, nekoX: x, nekoY: y };
    dragSprite = "alert";
  }

  function dragTo(mx: number, my: number): NekoFrame {
    if (dragAnchor) {
      const deltaX = mx - dragAnchor.mouseX;
      const deltaY = my - dragAnchor.mouseY;
      const absX = Math.abs(deltaX);
      const absY = Math.abs(deltaY);
      if (absX > absY && absX > 10) dragSprite = deltaX > 0 ? "scratchWallW" : "scratchWallE";
      else if (absY > absX && absY > 10) dragSprite = deltaY > 0 ? "scratchWallN" : "scratchWallS";
      const clamped = clampPosition(
        dragAnchor.nekoX + deltaX,
        dragAnchor.nekoY + deltaY,
        width,
        height,
      );
      x = clamped.x;
      y = clamped.y;
    }
    return { x, y, sprite: spriteFor(dragSprite, frameCount) };
  }

  function endDrag() {
    grabbing = false;
    dragAnchor = null;
    nudge = true;
    resetIdleAnimation();
  }

  function toggleSleep(perch: PerchRect | null) {
    forceSleep = !forceSleep;
    nudge = false;
    if (!forceSleep) {
      resetIdleAnimation();
      return;
    }
    const target = sleepTarget(perch);
    mouseX = target.x;
    mouseY = target.y;
  }

  function setPerch(perch: PerchRect | null) {
    if (!forceSleep) return;
    const target = sleepTarget(perch);
    mouseX = target.x;
    mouseY = target.y;
  }

  function setMouse(mx: number, my: number) {
    if (forceSleep) return;
    mouseX = mx;
    mouseY = my;
  }

  function setViewport(w: number, h: number) {
    width = w;
    height = h;
    const clamped = clampPosition(x, y, width, height);
    x = clamped.x;
    y = clamped.y;
  }

  function setPosition(px: number, py: number): NekoFrame {
    const clamped = clampPosition(px, py, width, height);
    x = clamped.x;
    y = clamped.y;
    return { x, y, sprite: spriteFor("idle", 0) };
  }

  function tick(): NekoFrame {
    frameCount += 1;
    if (grabbing) return { x, y, sprite: spriteFor(dragSprite, frameCount) };
    if (petTicks > 0) {
      petTicks -= 1;
      return { x, y, sprite: spriteFor("tired", 0) };
    }
    const diffX = x - mouseX;
    const diffY = y - mouseY;
    const distance = Math.hypot(diffX, diffY);
    if (forceSleep && Math.abs(diffX) < NEKO_SPEED && Math.abs(diffY) < NEKO_SPEED) {
      const clamped = clampPosition(mouseX, mouseY, width, height);
      x = clamped.x;
      y = clamped.y;
      return idleFrame();
    }
    if ((distance < NEKO_SPEED || distance < IDLE_RADIUS) && !forceSleep) return idleFrame();
    idleAnimation = null;
    idleAnimationFrame = 0;
    if (idleTime > 1) {
      idleTime = Math.min(idleTime, 7) - 1;
      return { x, y, sprite: spriteFor("alert", 0) };
    }
    const sprite = spriteFor(directionFor(diffX, diffY, distance), frameCount);
    x -= (diffX / distance) * NEKO_SPEED;
    y -= (diffY / distance) * NEKO_SPEED;
    const clamped = clampPosition(x, y, width, height);
    x = clamped.x;
    y = clamped.y;
    return { x, y, sprite };
  }

  return {
    setMouse,
    setViewport,
    setPosition,
    startDrag,
    dragTo,
    endDrag,
    toggleSleep,
    setPerch,
    pet,
    asleep,
    tick,
  };
}
