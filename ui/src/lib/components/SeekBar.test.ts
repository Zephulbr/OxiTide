// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { fakePlayer } from "../test/fakes";
import SeekBar from "./SeekBar.svelte";

afterEach(cleanup);

describe("SeekBar", () => {
  it("exposes a spoken value text", () => {
    render(SeekBar, { player: fakePlayer({ position_ms: 30000, duration_ms: 60000 }), variant: "inline" });

    const slider = screen.getByRole("slider", { name: "Playback position" });
    expect(slider.getAttribute("aria-valuetext")).toBe("0:30 of 1:00");
  });

  it("seeks with Home/End and Page keys", async () => {
    const player = fakePlayer({ position_ms: 30000, duration_ms: 60000 });
    render(SeekBar, { player, variant: "inline" });
    const slider = screen.getByRole("slider", { name: "Playback position" });

    await fireEvent.keyDown(slider, { key: "Home" });
    expect(player.seek).toHaveBeenLastCalledWith(0);

    await fireEvent.keyDown(slider, { key: "End" });
    expect(player.seek).toHaveBeenLastCalledWith(60000);

    await fireEvent.keyDown(slider, { key: "PageUp" });
    expect(player.seek).toHaveBeenLastCalledWith(36000);
  });
});
