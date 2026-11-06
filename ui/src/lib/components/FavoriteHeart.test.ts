// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import FavoriteHeart, { toggledFx } from "./FavoriteHeart.svelte";

afterEach(cleanup);

describe("FavoriteHeart", () => {
  it("never animates on mount, even with a toggle already recorded", () => {
    render(FavoriteHeart, { fx: { n: 3, kind: "break" } });
    expect(document.querySelector(".heart-ico.pop, .heart-ico.mend, .heart-break")).toBeNull();
  });

  it("pops on a favorite and breaks on an unfavorite", async () => {
    const { rerender } = render(FavoriteHeart, { fx: null });
    const popped = toggledFx(null, false);
    await rerender({ fx: popped });
    expect(document.querySelector(".heart-ico.pop")).not.toBeNull();
    expect(document.querySelector(".heart-break")).toBeNull();

    await rerender({ fx: toggledFx(popped, true) });
    expect(document.querySelector(".heart-ico.mend")).not.toBeNull();
    expect(document.querySelectorAll(".heart-break .half")).toHaveLength(2);
  });

  it("counts toggles and records their direction", () => {
    expect(toggledFx(null, false)).toEqual({ n: 1, kind: "pop" });
    expect(toggledFx({ n: 1, kind: "pop" }, true)).toEqual({ n: 2, kind: "break" });
  });
});
