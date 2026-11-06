// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { commitOpenGlide, holdPage, prepareOpenGlide, releasePageHold } from "./queueTransition";

/// `.main` with `room` px of content width (28px side padding) holding one
/// page `width` px wide. `centred` gives the page Now Playing's
/// `margin: 0 auto` + max-width, as `computedStyleMap` reports it.
function setup(room: number, width: number, centred: boolean, maxWidth = 1800) {
  const main = document.createElement("div");
  main.style.padding = "0 28px";
  Object.defineProperty(main, "clientWidth", { value: room + 56 });
  const page = document.createElement("div");
  page.className = centred ? "now-playing" : "page";
  page.style.maxWidth = `${maxWidth}px`;
  page.getBoundingClientRect = () => ({ width }) as DOMRect;
  if (centred) {
    const computed: Record<string, string> = { "margin-left": "auto", "margin-right": "auto", width: "auto" };
    (page as unknown as { computedStyleMap: () => unknown }).computedStyleMap = () => ({
      get: (property: string) => computed[property] ?? "0px",
    });
  }
  main.append(page);
  document.body.append(main);
  return { main, page };
}

afterEach(() => {
  releasePageHold();
  document.body.replaceChildren();
});

describe("queue glide page hold", () => {
  it("narrows a filling page by the rail", () => {
    const { main, page } = setup(1800, 1800, false);
    prepareOpenGlide(main, { glide: false });
    commitOpenGlide(main, 360);
    expect(page.style.width).toBe("1440px");
    expect(page.style.marginLeft).toBe("");
  });

  it("holds a centred page at its final width and pins its margin", () => {
    // 1080p: the capped page's 64px of free space absorbs part of the rail.
    const { main, page } = setup(1864, 1800, true);
    prepareOpenGlide(main, { glide: false });
    commitOpenGlide(main, 360);
    expect(page.style.width).toBe("1504px");
    expect(page.style.marginLeft).toBe("0px");
    releasePageHold();
    expect(page.style.width).toBe("");
    expect(page.style.marginLeft).toBe("");
  });

  it("keeps a capped page's width when the rail fits in its free space", () => {
    const { main, page } = setup(2280, 1800, true);
    prepareOpenGlide(main, { glide: false });
    commitOpenGlide(main, 360);
    expect(page.style.width).toBe("1800px");
    expect(page.style.marginLeft).toBe("60px");
  });

  it("slides a rotated mover in its parent's frame, leaving its spin alone", () => {
    const { main, page } = setup(1800, 1800, false);
    const disc = document.createElement("div");
    disc.style.transform = "rotate(137deg)";
    let left = 100;
    disc.getBoundingClientRect = () => ({ left, top: 0, width: 400, height: 400, bottom: 400 }) as DOMRect;
    page.append(disc);
    const calls: { keyframes: Keyframe[]; options: KeyframeAnimationOptions }[] = [];
    const animate = vi.fn(function (this: Element, keyframes: Keyframe[], options: KeyframeAnimationOptions) {
      calls.push({ keyframes, options });
      return { pause() {}, play() {}, cancel() {}, playState: "running" } as unknown as Animation;
    });
    Object.defineProperty(HTMLElement.prototype, "animate", { value: animate, configurable: true });
    try {
      prepareOpenGlide(main);
      left = 40;
      commitOpenGlide(main, 360);
      expect(calls).toHaveLength(1);
      // Not `transform`: appended after `rotate()`, the offset would turn with
      // the disc and send it off at an angle.
      expect(calls[0].keyframes).toEqual([{ translate: "60px 0px" }, { translate: "0px 0px" }]);
      expect(calls[0].options.composite).toBe("add");
      expect(disc.style.transform).toBe("rotate(137deg)");
    } finally {
      delete (HTMLElement.prototype as { animate?: unknown }).animate;
    }
  });

  it("closing grows a centred page up to its cap and centres it", () => {
    const { main, page } = setup(1504, 1504, true);
    const rail = document.createElement("aside");
    rail.getBoundingClientRect = () => ({ width: 360 }) as DOMRect;
    holdPage(main, rail, 0, { glide: false });
    expect(page.style.width).toBe("1800px");
    expect(page.style.marginLeft).toBe("32px");
  });
});
