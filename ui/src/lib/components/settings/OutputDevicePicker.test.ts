// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { OutputDevice, OutputDeviceList } from "../../api";

const mocks = vi.hoisted(() => ({
  list: null as unknown as OutputDeviceList,
  api: {
    getOutputDevices: vi.fn(),
    setOutputDevice: vi.fn(async () => {}),
    setDeviceMode: vi.fn(async () => {}),
    setDevicePrefs: vi.fn(async () => {}),
    moveDevicePrefs: vi.fn(async () => {}),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../../api", () => ({ api: mocks.api }));

import { menu } from "../../menu.svelte";
import OutputDevicePicker from "./OutputDevicePicker.svelte";

function device(id: string, name: string, extra: Partial<OutputDevice> = {}, prefs: Partial<OutputDevice["prefs"]> = {}): OutputDevice {
  return {
    id,
    name,
    system_name: `${name} (system name)`,
    group: "wasapi",
    kind: "usb",
    transport: "usb",
    is_default: false,
    connected: true,
    wrapper: false,
    present: true,
    caps: null,
    ...extra,
    prefs: { nickname: "", icon: "", pinned: false, hidden: false, mode: "shared", ...prefs },
  };
}

const BP = {
  exclusive: true,
  max_rate: 384000,
  max_bits: 32,
  hw_volume: true,
  shared_rate: 48000,
  shared_bits: 32,
  source: "probed" as const,
};

function setList(patch: Partial<OutputDeviceList> = {}) {
  mocks.list = {
    system_default: { now: "Speakers", mode: "shared" },
    devices: [
      device("dac", "DX5", { caps: BP }, { nickname: "Desk DAC", pinned: true, mode: "exclusive" }),
      device("spk", "Speakers", { kind: "speakers", transport: "internal" }),
      device("asio:Topping", "Topping USB Audio Device", { group: "asio", caps: BP }),
    ],
    pinned: ["dac"],
    selected: "dac",
    icons: true,
    moved: [],
    ...patch,
  };
  mocks.api.getOutputDevices.mockImplementation(async () => mocks.list);
}

async function renderPicker(icons = true) {
  const onlist = vi.fn();
  const onerror = vi.fn();
  render(OutputDevicePicker, { icons, onlist, onerror });
  await screen.findByRole("radio", { name: /Desk DAC/ });
  return { onlist, onerror };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  menu.close(false);
});

describe("OutputDevicePicker", () => {
  it("lists System default and grouped rows with clean names and coloured format numbers", async () => {
    setList();
    await renderPicker();

    expect(screen.getByRole("radio", { name: /System default/ }).getAttribute("aria-checked")).toBe("false");
    const dac = screen.getByRole("radio", { name: /Desk DAC/ });
    expect(dac.getAttribute("aria-checked")).toBe("true");
    expect(dac.getAttribute("title")).toBe("DX5 (system name)");
    expect(document.getElementById(dac.getAttribute("aria-describedby") ?? "")?.textContent).toBe("USB");
    expect(screen.getByText("Windows audio")).toBeTruthy();
    expect(screen.getByText("ASIO")).toBeTruthy();
    const facts = document.querySelector(".facts");
    expect(facts?.textContent?.trim()).toBe("Up to 384 kHz · 32-bit, hardware volume");
    expect([...(facts?.querySelectorAll(".num") ?? [])].map((n) => n.textContent)).toEqual(["384", "32"]);
    // Each group scrolls inside itself.
    expect(document.querySelectorAll(".rows")).toHaveLength(2);
  });

  it("selects on click and moves focus with the arrows without selecting", async () => {
    setList();
    await renderPicker();
    const dac = screen.getByRole("radio", { name: /Desk DAC/ });
    dac.focus();
    await fireEvent.keyDown(dac, { key: "ArrowDown" });
    expect(document.activeElement).toBe(screen.getByRole("radio", { name: /^Speakers/ }));
    expect(mocks.api.setOutputDevice).not.toHaveBeenCalled();

    await fireEvent.click(screen.getByRole("radio", { name: /^Speakers/ }));
    expect(mocks.api.setOutputDevice).toHaveBeenCalledWith("spk");
  });

  it("sets the selected device's own mode; ASIO has none", async () => {
    setList();
    await renderPicker();
    await fireEvent.click(screen.getByRole("button", { name: "Shared" }));
    expect(mocks.api.setDeviceMode).toHaveBeenCalledWith("dac", "shared");

    setList({ selected: "asio:Topping" });
    cleanup();
    await renderPicker();
    expect(screen.queryByRole("group", { name: "Output mode" })).toBeNull();
    expect(screen.getByText("Always exclusive", { selector: ".always" })).toBeTruthy();
  });

  it("routes a switch from shared to exclusive output through beforeExclusive", async () => {
    setList({ selected: "spk" });
    const beforeExclusive = vi.fn();
    render(OutputDevicePicker, { icons: true, onerror: vi.fn(), beforeExclusive });
    await screen.findByRole("radio", { name: /Desk DAC/ });

    // Shared → an exclusive device: held until the warning lets it through.
    await fireEvent.click(screen.getByRole("radio", { name: /Desk DAC/ }));
    expect(beforeExclusive).toHaveBeenCalledTimes(1);
    expect(mocks.api.setOutputDevice).not.toHaveBeenCalled();
    beforeExclusive.mock.calls[0][0]();
    expect(mocks.api.setOutputDevice).toHaveBeenCalledWith("dac");

    // ASIO is always exclusive.
    cleanup();
    setList({ selected: "spk" });
    render(OutputDevicePicker, { icons: true, onerror: vi.fn(), beforeExclusive });
    await screen.findByRole("radio", { name: /Topping/ });
    await fireEvent.click(screen.getByRole("radio", { name: /Topping/ }));
    expect(beforeExclusive).toHaveBeenCalledTimes(2);

    // Turning the selected shared device exclusive.
    await fireEvent.click(screen.getByRole("button", { name: "Exclusive" }));
    expect(beforeExclusive).toHaveBeenCalledTimes(3);
    expect(mocks.api.setDeviceMode).not.toHaveBeenCalled();
    beforeExclusive.mock.calls[2][0]();
    expect(mocks.api.setDeviceMode).toHaveBeenCalledWith("spk", "exclusive");
  });

  it("switches exclusive to shared, or exclusive to exclusive, without asking", async () => {
    setList();
    const beforeExclusive = vi.fn();
    render(OutputDevicePicker, { icons: true, onerror: vi.fn(), beforeExclusive });
    await screen.findByRole("radio", { name: /Desk DAC/ });
    await fireEvent.click(screen.getByRole("radio", { name: /Topping/ }));
    await fireEvent.click(screen.getByRole("radio", { name: /^Speakers/ }));
    expect(beforeExclusive).not.toHaveBeenCalled();
    expect(mocks.api.setOutputDevice).toHaveBeenCalledWith("asio:Topping");
  });

  it("edits the nickname in a dialog; saving it empty brings the device name back", async () => {
    setList();
    await renderPicker();
    await fireEvent.click(screen.getByRole("button", { name: "More for Speakers" }));
    menu.open?.items.find((item) => item.id === "edit")?.run();

    await screen.findByRole("dialog", { name: "Edit output" });
    const input = screen.getByRole("textbox", { name: "Nickname" }) as HTMLInputElement;
    expect(input.placeholder).toBe("Speakers");
    await fireEvent.input(input, { target: { value: "  Desk speakers " } });
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    // Only what changed is saved, and saving never selects the device.
    expect(mocks.api.setDevicePrefs).toHaveBeenCalledWith("spk", { nickname: "Desk speakers" });
    expect(mocks.api.setOutputDevice).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    await fireEvent.click(screen.getByRole("button", { name: "More for Desk DAC" }));
    menu.open?.items.find((item) => item.id === "edit")?.run();
    const again = (await screen.findByRole("textbox", { name: "Nickname" })) as HTMLInputElement;
    expect(again.value).toBe("Desk DAC");
    await fireEvent.input(again, { target: { value: "" } });
    await fireEvent.submit(again.form!);
    expect(mocks.api.setDevicePrefs).toHaveBeenLastCalledWith("dac", { nickname: "" });
    await waitFor(() => expect(screen.getByRole("radio", { name: /^DX5/ })).toBeTruthy());

    const calls = mocks.api.setDevicePrefs.mock.calls.length;
    await fireEvent.click(screen.getByRole("button", { name: "More for Desk speakers" }));
    menu.open?.items.find((item) => item.id === "edit")?.run();
    await screen.findByRole("dialog");
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(mocks.api.setDevicePrefs.mock.calls.length).toBe(calls);
  });

  it("keeps the row menu short and picks the icon in the edit dialog", async () => {
    setList();
    await renderPicker();
    await fireEvent.click(screen.getByRole("button", { name: "Pin Speakers to the quality menu" }));
    expect(mocks.api.setDevicePrefs).toHaveBeenCalledWith("spk", { pinned: true });

    await fireEvent.click(screen.getByRole("button", { name: "More for Desk DAC" }));
    expect(menu.open?.items.map((item) => item.id)).toEqual(["edit", "pin", "hide"]);
    const hide = menu.open?.items.find((item) => item.id === "hide");
    expect(hide?.disabled).toBe(true);
    expect(hide?.detail).toBe("Switch to another device first");

    await fireEvent.click(screen.getByRole("button", { name: "More for Speakers" }));
    menu.open?.items.find((item) => item.id === "hide")?.run();
    expect(mocks.api.setDevicePrefs).toHaveBeenLastCalledWith("spk", { hidden: true });

    await fireEvent.click(screen.getByRole("button", { name: "More for Speakers" }));
    menu.open?.items.find((item) => item.id === "edit")?.run();
    const icons = await screen.findByRole("radiogroup", { name: "Icon" });
    const automatic = screen.getByRole("radio", { name: "Automatic" });
    expect(automatic.getAttribute("aria-checked")).toBe("true");
    expect(icons.querySelectorAll('[role="radio"]')).toHaveLength(11);
    await fireEvent.keyDown(automatic, { key: "ArrowLeft" });
    expect(screen.getByRole("radio", { name: "No icon" }).getAttribute("aria-checked")).toBe("true");
    await fireEvent.click(screen.getByRole("radio", { name: "Headphones" }));
    await fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(mocks.api.setDevicePrefs).toHaveBeenLastCalledWith("spk", { icon: "headphones" });
  });

  it("opens the row menu on right-click, at the pointer", async () => {
    setList();
    await renderPicker();
    const row = screen.getByRole("radio", { name: /^Speakers/ });
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 120, clientY: 340 });
    row.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
    expect(menu.open?.items.map((item) => item.id)).toEqual(["edit", "pin", "hide"]);
    expect([menu.open?.x, menu.open?.y]).toEqual([120, 340]);
    expect(menu.open?.opener).toBe(row);
  });

  it("leaves the icon choice out of the dialog when icons are off", async () => {
    setList();
    await renderPicker(false);
    await fireEvent.click(screen.getByRole("button", { name: "More for Speakers" }));
    menu.open?.items.find((item) => item.id === "edit")?.run();
    await screen.findByRole("dialog", { name: "Edit output" });
    expect(screen.queryByRole("radiogroup", { name: "Icon" })).toBeNull();
  });

  it("drops icon tiles when icons are off", async () => {
    setList();
    await renderPicker(false);
    expect(document.querySelector(".tile")).toBeNull();
  });

  it("moves settings to a device back under a new id, once", async () => {
    setList({ moved: [["old", "dac"]] });
    mocks.api.getOutputDevices
      .mockImplementationOnce(async () => mocks.list)
      .mockImplementation(async () => ({ ...mocks.list, moved: [] }));
    await renderPicker();
    expect(mocks.api.moveDevicePrefs).toHaveBeenCalledWith("old", "dac");
  });
});
