// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/svelte";
import { fakePlayer } from "../test/fakes";
import VolumeSlider from "./VolumeSlider.svelte";

afterEach(cleanup);

describe("VolumeSlider", () => {
  it("exposes the volume as spoken value text", () => {
    render(VolumeSlider, { player: fakePlayer({ volume: 0.42 }) });

    const slider = screen.getByRole("slider", { name: "Volume" });
    expect(slider.getAttribute("aria-valuetext")).toBe("42%");
  });

  it("shows full and locks while exclusive mode fixes the volume", () => {
    render(VolumeSlider, { player: fakePlayer({ volume: 0.42, volume_fixed: true }) });

    const slider = screen.getByRole<HTMLInputElement>("slider", { name: "Volume" });
    expect(slider.value).toBe("1");
    expect(slider.disabled).toBe(true);
    expect(slider.getAttribute("aria-valuetext")).toBe("100%, fixed in exclusive mode");
    const lock = screen.getByRole<HTMLButtonElement>("button", { name: /Volume fixed at 100%/ });
    expect(lock.disabled).toBe(true);
  });
});
