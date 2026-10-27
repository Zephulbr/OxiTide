// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/svelte";
import { fakePlayer } from "../test/fakes";
import { FIXED_VOLUME_HINT } from "../volume";
import VolumeSlider from "./VolumeSlider.svelte";

afterEach(cleanup);

describe("VolumeSlider", () => {
  it("exposes the volume as spoken value text", () => {
    render(VolumeSlider, { player: fakePlayer({ volume: 0.42 }) });

    const slider = screen.getByRole("slider", { name: "Volume" });
    expect(slider.getAttribute("aria-valuetext")).toBe("42%");
  });

  it("is disabled and says why while volume is fixed (ASIO)", () => {
    render(VolumeSlider, { player: fakePlayer({ volume: 1, volume_fixed: true }) });

    const slider = screen.getByRole("slider", { name: "Volume" }) as HTMLInputElement;
    expect(slider.disabled).toBe(true);
    expect(slider.getAttribute("aria-valuetext")).toBe(FIXED_VOLUME_HINT);
    expect((screen.getByRole("button", { name: "Mute" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
