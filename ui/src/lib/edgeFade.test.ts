// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { EDGE_FADE, edgeFade, fadeLengths } from "./edgeFade";

describe("fadeLengths", () => {
  it("shows every row whole at rest and at the end", () => {
    expect(fadeLengths(0, 400, 400)).toEqual([0, 0]);
    expect(fadeLengths(0, 1000, 400)).toEqual([0, EDGE_FADE]);
    expect(fadeLengths(600, 1000, 400)).toEqual([EDGE_FADE, 0]);
  });

  it("grows each fade in over the first and last EDGE_FADE px of travel", () => {
    expect(fadeLengths(10, 1000, 400)).toEqual([10, EDGE_FADE]);
    expect(fadeLengths(580, 1000, 400)).toEqual([EDGE_FADE, 20]);
    expect(fadeLengths(300, 1000, 400)).toEqual([EDGE_FADE, EDGE_FADE]);
  });

  it("clamps overscroll", () => {
    expect(fadeLengths(-20, 1000, 400)).toEqual([0, EDGE_FADE]);
    expect(fadeLengths(640, 1000, 400)).toEqual([EDGE_FADE, 0]);
  });
});

describe("edgeFade", () => {
  function scroller(scrollHeight: number, clientHeight: number) {
    const node = document.createElement("div");
    Object.defineProperty(node, "scrollHeight", { value: scrollHeight });
    Object.defineProperty(node, "clientHeight", { value: clientHeight });
    return node;
  }

  it("sets the fade lengths on mount and on scroll", () => {
    const node = scroller(1000, 400);
    const handle = edgeFade(node);
    expect(node.style.getPropertyValue("--fade-top")).toBe("0px");
    expect(node.style.getPropertyValue("--fade-bottom")).toBe(`${EDGE_FADE}px`);
    node.scrollTop = 20;
    node.dispatchEvent(new Event("scroll"));
    expect(node.style.getPropertyValue("--fade-top")).toBe("20px");
    handle.destroy();
    node.scrollTop = 40;
    node.dispatchEvent(new Event("scroll"));
    expect(node.style.getPropertyValue("--fade-top")).toBe("20px");
  });

  it("re-measures when children come and go with watchChildren", async () => {
    const node = document.createElement("div");
    let height = 400;
    Object.defineProperty(node, "scrollHeight", { get: () => height });
    Object.defineProperty(node, "clientHeight", { value: 400 });
    const handle = edgeFade(node, { watchChildren: true });
    expect(node.style.getPropertyValue("--fade-bottom")).toBe("0px");
    height = 1000;
    node.append(document.createElement("section"));
    await Promise.resolve();
    expect(node.style.getPropertyValue("--fade-bottom")).toBe(`${EDGE_FADE}px`);
    handle.destroy();
  });
});
