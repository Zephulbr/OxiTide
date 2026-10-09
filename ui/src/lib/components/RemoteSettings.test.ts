// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { RemoteStatus } from "../api";

const mocks = vi.hoisted(() => ({
  status: null as unknown as RemoteStatus,
  api: {
    getRemoteStatus: vi.fn(),
    listRemoteDevices: vi.fn(),
    setRemoteEnabled: vi.fn(),
    setRemoteSettings: vi.fn(),
    beginRemotePairing: vi.fn(),
    cancelRemotePairing: vi.fn(async () => {}),
    revokeRemoteDevice: vi.fn(async () => {}),
    resolveRemotePairRequest: vi.fn(async () => true),
  },
}));

vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock("../api", () => ({ api: mocks.api, REMOTE_STATUS_EVENT: "remote_status" }));

import RemoteSettings from "./RemoteSettings.svelte";

function status(patch: Partial<RemoteStatus> = {}): RemoteStatus {
  return {
    enabled: true,
    listening: true,
    port: 47820,
    address: "192.168.1.5",
    url: "http://192.168.1.5:47820/remote",
    error: null,
    connected: 1,
    interfaces: [{ name: "Wi-Fi", ip: "192.168.1.5", virtual_adapter: false }],
    settings: { enabled: true, port: 47820, interface: "", approve_new_devices: true },
    pending: [],
    pairing_expires_in_secs: null,
    ...patch,
  };
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("Settings → Remote", () => {
  it("shows the address, devices, and revokes", async () => {
    mocks.api.getRemoteStatus.mockResolvedValue(status());
    mocks.api.listRemoteDevices.mockResolvedValue([
      { id: "d1", name: "Pixel", created_ms: 0, last_seen_ms: Date.now(), connected: true },
    ]);
    render(RemoteSettings);
    expect(await screen.findByText(/Listening on http:\/\/192\.168\.1\.5:47820\/remote/)).toBeTruthy();
    expect(screen.getByText("Pixel")).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Revoke Pixel" }));
    expect(mocks.api.revokeRemoteDevice).toHaveBeenCalledWith("d1");
    await waitFor(() => expect(screen.queryByText("Pixel")).toBeNull());
  });

  it("turns the remote on", async () => {
    mocks.api.getRemoteStatus.mockResolvedValue(status({ enabled: false, listening: false, url: null }));
    mocks.api.listRemoteDevices.mockResolvedValue([]);
    mocks.api.setRemoteEnabled.mockResolvedValue(status());
    render(RemoteSettings);
    const toggle = await screen.findByRole("checkbox", { name: /Control OxiTide from your phone/ });
    await waitFor(() => expect((toggle as HTMLInputElement).disabled).toBe(false));
    await fireEvent.click(toggle);
    expect(mocks.api.setRemoteEnabled).toHaveBeenCalledWith(true);
  });

  it("surfaces a bind failure", async () => {
    mocks.api.getRemoteStatus.mockResolvedValue(
      status({ listening: false, url: null, error: "could not listen on ports 47820–47829" }),
    );
    mocks.api.listRemoteDevices.mockResolvedValue([]);
    render(RemoteSettings);
    expect(await screen.findByText(/Not listening: could not listen/)).toBeTruthy();
  });

  it("approves a waiting phone", async () => {
    mocks.api.getRemoteStatus.mockResolvedValue(
      status({ pending: [{ id: "r1", name: "iPhone", ip: "192.168.1.9" }] }),
    );
    mocks.api.listRemoteDevices.mockResolvedValue([]);
    render(RemoteSettings);
    await screen.findByText("iPhone");
    await fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    expect(mocks.api.resolveRemotePairRequest).toHaveBeenCalledWith("r1", true);
  });

  it("opens the pairing dialog with the QR", async () => {
    mocks.api.getRemoteStatus.mockResolvedValue(status());
    mocks.api.listRemoteDevices.mockResolvedValue([]);
    mocks.api.beginRemotePairing.mockResolvedValue({
      url: "http://192.168.1.5:47820/remote#p=abc",
      qr_svg: '<svg data-testid="qr"></svg>',
      expires_in_secs: 300,
    });
    render(RemoteSettings);
    await fireEvent.click(await screen.findByRole("button", { name: "Pair a phone" }));
    expect(await screen.findByText("http://192.168.1.5:47820/remote#p=abc")).toBeTruthy();
    expect(screen.getByText(/Expires in 5:00/)).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(mocks.api.cancelRemotePairing).toHaveBeenCalled();
  });
});
