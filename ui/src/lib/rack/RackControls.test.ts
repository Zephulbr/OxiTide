// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { fakePlayer } from "../test/fakes";
import { defaultDspSettings } from "../dsp";
import { dspStore } from "../dspStore.svelte";
import VolumeKnob from "./VolumeKnob.svelte";
import VfdSeek from "./VfdSeek.svelte";
import EqUnit from "./EqUnit.svelte";

vi.mock("../api", () => ({
  api: { getConfig: vi.fn(async () => ({ dsp: null })), setDspSettings: vi.fn(async () => {}) },
}));

afterEach(cleanup);

describe("VolumeKnob", () => {
  it("is a named slider with value text", () => {
    render(VolumeKnob, { player: fakePlayer({ volume: 0.42 }) });
    const knob = screen.getByRole("slider", { name: "Volume" });
    expect(knob.getAttribute("aria-valuenow")).toBe("42");
    expect(knob.getAttribute("aria-valuetext")).toBe("42 percent");
  });

  it("steps with arrows, pages, Home/End and claims the key", async () => {
    const player = fakePlayer({ volume: 0.5 });
    render(VolumeKnob, { player });
    const knob = screen.getByRole("slider", { name: "Volume" });
    const event = new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true, cancelable: true });
    knob.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    // Steps persist debounced (third argument) and chain from the last
    // requested value until the engine confirms it.
    expect(player.setVolume).toHaveBeenLastCalledWith(0.52, true, true);
    await fireEvent.keyDown(knob, { key: "PageDown" });
    expect(player.setVolume).toHaveBeenLastCalledWith(0.42, true, true);
    await fireEvent.keyDown(knob, { key: "End" });
    expect(player.setVolume).toHaveBeenLastCalledWith(1, true, true);
    await fireEvent.keyDown(knob, { key: "Home" });
    expect(player.setVolume).toHaveBeenLastCalledWith(0, true, true);
  });

  it("does not lose quick wheel steps before the engine confirms", async () => {
    const player = fakePlayer({ volume: 0.5 });
    render(VolumeKnob, { player });
    const knob = screen.getByRole("slider", { name: "Volume" });
    await fireEvent.wheel(knob, { deltaY: -1 });
    await fireEvent.wheel(knob, { deltaY: -1 });
    expect(player.setVolume).toHaveBeenLastCalledWith(0.54, true, true);
  });

  it("reports mute", () => {
    render(VolumeKnob, { player: fakePlayer({ volume: 0 }) });
    expect(screen.getByRole("slider", { name: "Volume" }).getAttribute("aria-valuetext")).toBe("Muted");
  });
});

describe("VfdSeek", () => {
  it("exposes position as time text and seeks by key", async () => {
    const player = fakePlayer({ position_ms: 30_000, duration_ms: 60_000 });
    render(VfdSeek, { player });
    const seek = screen.getByRole("slider", { name: "Seek" });
    expect(seek.getAttribute("aria-valuetext")).toBe("0:30 of 1:00");
    await fireEvent.keyDown(seek, { key: "ArrowRight" });
    expect(player.seek).toHaveBeenLastCalledWith(35_000);
    await fireEvent.keyDown(seek, { key: "Home" });
    expect(player.seek).toHaveBeenLastCalledWith(0);
  });

  it("is inert with nothing playing", async () => {
    const player = fakePlayer({ current: null, duration_ms: 0 });
    render(VfdSeek, { player });
    const seek = screen.getByRole("slider", { name: "Seek" });
    expect(seek.getAttribute("aria-disabled")).toBe("true");
    await fireEvent.keyDown(seek, { key: "ArrowRight" });
    expect(player.seek).not.toHaveBeenCalled();
  });
});

describe("EqUnit", () => {
  it("renders ten band sliders and preset radios; a key moves a band and enables EQ", async () => {
    dspStore.hydrate(defaultDspSettings());
    const toasts = { show: vi.fn() } as never;
    render(EqUnit, { toasts });
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(10);
    expect(screen.getByRole("radio", { name: "Flat" }).getAttribute("aria-checked")).toBe("true");
    await fireEvent.keyDown(screen.getByRole("slider", { name: "1k Hz" }), { key: "PageUp" });
    expect(dspStore.settings?.eq.bands_db[5]).toBe(3);
    expect(dspStore.settings?.eq.enabled).toBe(true);
    dspStore.flush();
  });
});
