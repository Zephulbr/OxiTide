// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { isDragTarget, TITLEBAR_HEIGHT } from "./titlebar";

function el(html: string): HTMLElement {
  const host = document.createElement("div");
  host.innerHTML = html;
  document.body.append(host);
  return host.querySelector("#t") as HTMLElement;
}

describe("isDragTarget", () => {
  it("drags from plain content inside the band", () => {
    expect(isDragTarget(el(`<h1 id="t">OxiTide</h1>`), 10)).toBe(true);
  });

  it("ignores presses below the band", () => {
    expect(isDragTarget(el(`<div id="t"></div>`), TITLEBAR_HEIGHT)).toBe(false);
    expect(isDragTarget(el(`<div id="t"></div>`), -1)).toBe(false);
  });

  it("leaves controls and their children alone", () => {
    expect(isDragTarget(el(`<button><svg><path id="t"/></svg></button>`), 5)).toBe(false);
    expect(isDragTarget(el(`<input id="t" />`), 5)).toBe(false);
    expect(isDragTarget(el(`<div role="slider" tabindex="0"><span id="t"></span></div>`), 5)).toBe(false);
    expect(isDragTarget(el(`<a href="#x"><span id="t"></span></a>`), 5)).toBe(false);
    expect(isDragTarget(el(`<div data-no-drag><span id="t"></span></div>`), 5)).toBe(false);
  });

  it("treats a scroller's tabindex=-1 as non-interactive", () => {
    expect(isDragTarget(el(`<main tabindex="-1"><p id="t"></p></main>`), 5)).toBe(true);
  });

  it("rejects non-element targets", () => {
    expect(isDragTarget(null, 5)).toBe(false);
    expect(isDragTarget(window, 5)).toBe(false);
  });
});
