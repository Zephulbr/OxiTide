import { describe, expect, it, vi } from "vitest";
import { createClient, guessDeviceName, pairingToken, parseWireError, RemoteError } from "./rpc";

function respond(status: number, body: unknown) {
  return vi.fn(async () => new Response(JSON.stringify(body), { status }));
}

describe("remote rpc client", () => {
  it("splits the server's error format", () => {
    expect(parseWireError("not_logged_in: sign in on the PC")).toEqual({
      code: "not_logged_in",
      message: "sign in on the PC",
    });
    expect(parseWireError("plain failure")).toEqual({ code: "failed", message: "plain failure" });
    expect(parseWireError("two words: x")).toEqual({ code: "failed", message: "two words: x" });
  });

  it("posts camelCase params with the remote header and returns the result", async () => {
    const fetch = respond(200, { ok: true, result: { queueLen: 3 } });
    const client = createClient(fetch);
    await expect(client.call("get_queue_page", { offset: 0, limit: 50 })).resolves.toEqual({ queueLen: 3 });
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/rpc");
    expect(init.method).toBe("POST");
    expect((init.headers as Record<string, string>)["X-Oxi-Remote"]).toBe("1");
    expect(JSON.parse(init.body as string)).toEqual({
      method: "get_queue_page",
      params: { offset: 0, limit: 50 },
    });
  });

  it("maps errors to codes and statuses", async () => {
    const loggedOut = createClient(respond(200, { ok: false, error: "not_logged_in: sign in on the PC" }));
    await expect(loggedOut.call("next")).rejects.toMatchObject({ code: "not_logged_in", status: 200 });

    const unpaired = createClient(respond(401, { ok: false, error: "unauthorized: pair this phone first" }));
    const error = await unpaired.call("next").catch((e) => e);
    expect(error).toBeInstanceOf(RemoteError);
    expect(error).toMatchObject({ code: "unauthorized", status: 401 });

    const locked = createClient(respond(429, "nope"));
    await expect(locked.call("next")).rejects.toMatchObject({ code: "rate_limited", status: 429 });

    const offline = createClient(vi.fn(async () => Promise.reject(new TypeError("Failed to fetch"))));
    await expect(offline.call("next")).rejects.toMatchObject({ code: "offline", status: 0 });
  });

  it("pairs through /api/pair and surfaces a decline", async () => {
    const fetch = respond(200, { ok: true, result: { deviceId: "d", name: "Pixel" } });
    await createClient(fetch).pair("tok", "Pixel");
    const [url, init] = fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/pair");
    expect(JSON.parse(init.body as string)).toEqual({ token: "tok", deviceName: "Pixel" });

    const declined = createClient(respond(403, { ok: false, error: "denied: pairing was declined on the PC" }));
    await expect(declined.pair("tok", "x")).rejects.toMatchObject({
      code: "denied",
      message: "pairing was declined on the PC",
    });
  });

  it("reads the pairing token only from #p=", () => {
    expect(pairingToken("#p=AbCdEfGhIjKlMnOpQrStUv")).toBe("AbCdEfGhIjKlMnOpQrStUv");
    expect(pairingToken("#/now")).toBeNull();
    expect(pairingToken("#p=short")).toBeNull();
    expect(pairingToken("#p=has spaces in it ok")).toBeNull();
  });

  it("guesses a readable device name", () => {
    expect(guessDeviceName("Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)")).toBe("iPhone");
    expect(guessDeviceName("Mozilla/5.0 (Linux; Android 14; Pixel 8 Build/AP1A)")).toBe("Pixel 8");
    expect(guessDeviceName("Mozilla/5.0 (Linux; Android 10; K)")).toBe("Android phone");
    expect(guessDeviceName("curl/8")).toBe("Phone");
  });
});
