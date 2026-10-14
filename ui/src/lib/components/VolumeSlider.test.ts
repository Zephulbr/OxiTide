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
});
