import { describe, expect, it } from "vitest";
import { clampPosition, createNekoEngine } from "./oneko";

function engineAt(x: number, y: number, rng: () => number = () => 0.5) {
  const engine = createNekoEngine({ rng });
  engine.setViewport(800, 600);
  engine.setPosition(x, y);
  return engine;
}

describe("clampPosition", () => {
  it("keeps the cat fully inside the viewport", () => {
    expect(clampPosition(9999, -5, 800, 600)).toEqual({ x: 784, y: 16 });
    expect(clampPosition(400, 300, 800, 600)).toEqual({ x: 400, y: 300 });
  });
});

describe("neko chase", () => {
  it("moves toward the mouse at ten pixels per tick", () => {
    const engine = engineAt(100, 100);
    engine.setMouse(300, 100);
    const frame = engine.tick();
    expect(frame.x).toBe(110);
    expect(frame.y).toBe(100);
    expect(frame.sprite).toEqual({ col: 3, row: 1 });
  });

  it("stops and idles inside the 48px radius", () => {
    const engine = engineAt(400, 300);
    engine.setMouse(420, 300);
    const frame = engine.tick();
    expect(frame).toMatchObject({ x: 400, y: 300 });
    expect(frame.sprite).toEqual({ col: 3, row: 3 });
  });

  it("halts at the idle radius on a long approach", () => {
    const engine = engineAt(400, 300);
    engine.setMouse(400, 40);
    let frame = engine.tick();
    for (let i = 0; i < 30; i += 1) frame = engine.tick();
    expect(frame).toEqual({ x: 400, y: 80, sprite: { col: 3, row: 3 } });
  });

  it("clamps back inside when the viewport shrinks", () => {
    const engine = engineAt(700, 500);
    engine.setViewport(400, 300);
    engine.setMouse(399, 299);
    let frame = engine.tick();
    for (let i = 0; i < 100; i += 1) frame = engine.tick();
    expect(frame.x).toBe(384);
    expect(frame.y).toBe(284);
  });

  it("faces all eight directions", () => {
    const cases: Array<[number, number, number, number]> = [
      [400, 100, 1, 3],
      [400, 500, 7, 2],
      [600, 300, 3, 1],
      [200, 300, 4, 3],
      [600, 100, 0, 3],
      [200, 100, 1, 1],
      [600, 500, 5, 2],
      [200, 500, 6, 1],
    ];
    for (const [mouseX, mouseY, col, row] of cases) {
      const engine = engineAt(400, 300);
      engine.setMouse(mouseX, mouseY);
      expect(engine.tick().sprite).toEqual({ col, row });
    }
  });

  it("alternates the two walk frames", () => {
    const engine = engineAt(100, 100);
    engine.setMouse(700, 100);
    expect(engine.tick().sprite).toEqual({ col: 3, row: 1 });
    expect(engine.tick().sprite).toEqual({ col: 3, row: 0 });
  });
});

describe("neko idle", () => {
  it("shows alert when resuming after idling, then moves", () => {
    const engine = engineAt(100, 100);
    engine.setMouse(120, 100);
    engine.tick();
    engine.tick();
    engine.setMouse(300, 100);
    expect(engine.tick()).toEqual({ x: 100, y: 100, sprite: { col: 7, row: 3 } });
    expect(engine.tick().x).toBe(110);
  });

  it("eventually falls asleep while idling", () => {
    const engine = engineAt(400, 300, () => 0);
    engine.setMouse(420, 300);
    for (let i = 0; i < 10; i += 1) {
      expect(engine.tick().sprite).toEqual({ col: 3, row: 3 });
    }
    expect(engine.tick().sprite).toEqual({ col: 3, row: 2 });
    for (let i = 0; i < 7; i += 1) {
      expect(engine.tick().sprite).toEqual({ col: 3, row: 2 });
    }
    expect(engine.tick().sprite).toEqual({ col: 2, row: 0 });
  });

  it("scratch-walls near an edge when the roll picks it", () => {
    let calls = 0;
    const rng = () => {
      calls += 1;
      return calls % 2 === 1 ? 0 : 0.99;
    };
    const engine = engineAt(20, 300, rng);
    engine.setMouse(30, 300);
    for (let i = 0; i < 10; i += 1) engine.tick();
    expect(engine.tick().sprite).toEqual({ col: 4, row: 0 });
  });

  it("keeps idling when the roll never fires", () => {
    const engine = engineAt(400, 300, () => 0.999);
    engine.setMouse(420, 300);
    for (let i = 0; i < 100; i += 1) {
      expect(engine.tick().sprite).toEqual({ col: 3, row: 3 });
    }
  });
});

describe("neko drag", () => {
  it("follows the pointer while held", () => {
    const engine = engineAt(100, 100);
    engine.startDrag(100, 100);
    const frame = engine.dragTo(140, 100);
    expect(frame).toMatchObject({ x: 140, y: 100 });
    expect(frame.sprite).toEqual({ col: 4, row: 0 });
  });

  it("shows alert while held without movement", () => {
    const engine = engineAt(100, 100);
    engine.startDrag(100, 100);
    expect(engine.dragTo(105, 105).sprite).toEqual({ col: 7, row: 3 });
  });

  it("clamps drag inside the viewport", () => {
    const engine = engineAt(100, 100);
    engine.startDrag(100, 100);
    expect(engine.dragTo(5000, 2000)).toEqual({
      x: 784,
      y: 584,
      sprite: { col: 4, row: 0 },
    });
  });
});

describe("neko sleep", () => {
  const perch = { left: 0, right: 800, top: 500 };
  const sleepSprites = [
    { col: 3, row: 2 },
    { col: 2, row: 0 },
    { col: 2, row: 1 },
  ];

  it("walks to the perch and settles", () => {
    const engine = engineAt(400, 300);
    engine.toggleSleep(perch);
    engine.setMouse(0, 0);
    let frame = engine.tick();
    for (let i = 0; i < 60; i += 1) frame = engine.tick();
    expect(frame.x).toBe(480);
    expect(frame.y).toBe(492);
    expect(sleepSprites).toContainEqual(frame.sprite);
  });

  it("settles at the perch fraction, not its end", () => {
    const engine = engineAt(400, 300);
    engine.toggleSleep({ left: 100, right: 500, top: 400 });
    let frame = engine.tick();
    for (let i = 0; i < 60; i += 1) frame = engine.tick();
    expect(frame.x).toBe(340);
    expect(frame.y).toBe(392);
  });

  it("sleeps in place when the perch disappears", () => {
    const engine = engineAt(400, 300);
    engine.toggleSleep(perch);
    for (let i = 0; i < 10; i += 1) engine.tick();
    engine.setPerch(null);
    const settled = engine.tick();
    let frame = settled;
    for (let i = 0; i < 20; i += 1) frame = engine.tick();
    expect(frame.x).toBe(settled.x);
    expect(frame.y).toBe(settled.y);
    expect(sleepSprites).toContainEqual(frame.sprite);
  });

  it("returns to the perch after being dragged while asleep", () => {
    const engine = engineAt(400, 300);
    engine.toggleSleep(perch);
    for (let i = 0; i < 60; i += 1) engine.tick();
    engine.startDrag(480, 492);
    engine.dragTo(300, 200);
    engine.endDrag();
    let frame = engine.tick();
    for (let i = 0; i < 100; i += 1) frame = engine.tick();
    expect(frame.x).toBe(480);
    expect(frame.y).toBe(492);
    expect(sleepSprites).toContainEqual(frame.sprite);
  });

  it("wakes on the second double-click", () => {
    const engine = engineAt(400, 300);
    engine.toggleSleep(perch);
    engine.toggleSleep(perch);
    engine.setMouse(0, 0);
    expect(engine.tick().x).toBeLessThan(400);
  });

  it("does not replay the tired animation once commanded to sleep", () => {
    const sleepingOnly = [
      { col: 2, row: 0 },
      { col: 2, row: 1 },
    ];
    const engine = engineAt(400, 300, () => 0.999);
    engine.toggleSleep(perch);
    for (let i = 0; i < 60; i += 1) engine.tick();
    for (let i = 0; i < 400; i += 1) {
      expect(sleepingOnly).toContainEqual(engine.tick().sprite);
    }
  });
});
