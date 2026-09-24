import { describe, expect, it } from "vitest";
import { createNav } from "./nav.svelte";

describe("nav history", () => {
  it("starts at home with no history", () => {
    const nav = createNav();

    expect(nav.route).toEqual({ name: "home" });
    expect(nav.canBack).toBe(false);
    expect(nav.canForward).toBe(false);
  });

  it("ignores a goto to the current route", () => {
    const nav = createNav();

    nav.goto({ name: "home" });

    expect(nav.canBack).toBe(false);
  });

  it("ignores a goto to the same album id", () => {
    const nav = createNav();
    nav.goto({ name: "album", id: 1 });

    nav.goto({ name: "album", id: 1 });

    expect(nav.canBack).toBe(true);
    nav.back();
    expect(nav.route).toEqual({ name: "home" });
  });

  it("treats the same name with a different id as a new entry", () => {
    const nav = createNav();
    nav.goto({ name: "album", id: 1 });
    nav.goto({ name: "album", id: 2 });

    nav.back();

    expect(nav.route).toEqual({ name: "album", id: 1 });
  });

  it("truncates forward entries on a new navigation", () => {
    const nav = createNav();
    nav.goto({ name: "search" });
    nav.goto({ name: "library" });
    nav.back();
    expect(nav.canForward).toBe(true);

    nav.goto({ name: "settings" });

    expect(nav.canForward).toBe(false);
    expect(nav.route).toEqual({ name: "settings" });
  });

  it("clamps back and forward at the ends", () => {
    const nav = createNav();
    nav.goto({ name: "search" });

    nav.back();
    nav.back();
    expect(nav.route).toEqual({ name: "home" });

    nav.forward();
    nav.forward();
    expect(nav.route).toEqual({ name: "search" });
  });

  it("keeps the queue toggle out of history", () => {
    const nav = createNav();

    nav.toggleQueue();

    expect(nav.queueOpen).toBe(true);
    expect(nav.canBack).toBe(false);
  });
});
