// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import Toggle from "./Toggle.svelte";
import Select from "./Select.svelte";
import Segmented from "./Segmented.svelte";
import Slider from "./Slider.svelte";
import NumberField from "./NumberField.svelte";

// jsdom has no layout, so it does not implement scrollIntoView.
Element.prototype.scrollIntoView = vi.fn();

afterEach(cleanup);

describe("Toggle", () => {
  it("is a labelled checkbox that reports its new state", async () => {
    const onchange = vi.fn();
    render(Toggle, { checked: false, label: "Equalizer", onchange });
    const box = screen.getByRole("checkbox", { name: "Equalizer" });
    await fireEvent.click(box);
    expect(onchange).toHaveBeenCalledWith(true);
  });

  it("cannot be changed while disabled", () => {
    render(Toggle, { checked: true, label: "Remote", disabled: true, onchange: vi.fn() });
    expect((screen.getByRole("checkbox") as HTMLInputElement).disabled).toBe(true);
  });
});

describe("Select", () => {
  const options = [
    { value: "high", label: "High" },
    { value: "lossless", label: "Lossless" },
    { value: "max", label: "Max", disabled: true },
  ];

  it("shows the current label and picks an option with the keyboard", async () => {
    const onchange = vi.fn();
    render(Select, { value: "high", options, label: "Quality", onchange });
    const trigger = screen.getByRole("button", { name: "Quality" });
    expect(trigger.textContent).toContain("High");
    await fireEvent.keyDown(trigger, { key: "ArrowDown" });
    const list = screen.getByRole("listbox");
    await fireEvent.keyDown(list, { key: "ArrowDown" });
    await fireEvent.keyDown(list, { key: "Enter" });
    expect(onchange).toHaveBeenCalledWith("lossless");
  });

  it("skips disabled options", async () => {
    const onchange = vi.fn();
    render(Select, { value: "lossless", options, label: "Quality", onchange });
    await fireEvent.click(screen.getByRole("button", { name: "Quality" }));
    await fireEvent.click(screen.getByRole("option", { name: /Max/ }));
    expect(onchange).not.toHaveBeenCalled();
  });
});

describe("Segmented", () => {
  it("marks the active option and reports a change", async () => {
    const onchange = vi.fn();
    render(Segmented, {
      value: "dark",
      label: "Theme",
      options: [
        { value: "dark", label: "Dark" },
        { value: "light", label: "Light" },
      ],
      onchange,
    });
    expect(screen.getByRole("button", { name: "Dark" }).getAttribute("aria-pressed")).toBe("true");
    await fireEvent.click(screen.getByRole("button", { name: "Light" }));
    expect(onchange).toHaveBeenCalledWith("light");
  });
});

describe("Slider", () => {
  it("exposes its value as text and reports input and commit separately", async () => {
    const oninput = vi.fn();
    const oncommit = vi.fn();
    render(Slider, {
      value: 5,
      min: 0,
      max: 10,
      label: "Preamp",
      format: (v: number) => `${v} dB`,
      oninput,
      oncommit,
    });
    const range = screen.getByRole("slider", { name: "Preamp" }) as HTMLInputElement;
    expect(range.getAttribute("aria-valuetext")).toBe("5 dB");
    await fireEvent.input(range, { target: { value: "7" } });
    expect(oninput).toHaveBeenCalledWith(7);
    expect(oncommit).not.toHaveBeenCalled();
    expect(range.getAttribute("aria-valuetext")).toBe("7 dB");
    await fireEvent.change(range, { target: { value: "7" } });
    expect(oncommit).toHaveBeenCalledWith(7);
  });
});

describe("NumberField", () => {
  it("clamps and writes the clamped value back", async () => {
    const onchange = vi.fn();
    render(NumberField, { value: 500, min: 100, max: 1000, label: "Entries", onchange });
    const input = screen.getByRole("spinbutton", { name: "Entries" }) as HTMLInputElement;
    await fireEvent.change(input, { target: { value: "5000" } });
    expect(onchange).toHaveBeenCalledWith(1000);
    expect(input.value).toBe("1000");
  });

  it("ignores a value that did not change", async () => {
    const onchange = vi.fn();
    render(NumberField, { value: 500, min: 100, label: "Entries", onchange });
    await fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "500" } });
    expect(onchange).not.toHaveBeenCalled();
  });
});
