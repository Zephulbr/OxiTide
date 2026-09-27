import { describe, expect, it, vi } from "vitest";
import { createAuthGate } from "./authGate";
import { AUTH_ERROR_EVENT, AUTH_SUCCESS_EVENT, type SessionInfoDto } from "./api";

type Handler = (event: { payload: unknown }) => void;

function harness(
  takeResult: () => Promise<SessionInfoDto | null> = () => Promise.resolve(null),
) {
  const handlers = new Map<string, Handler>();
  const listen = vi.fn((event: string, cb: Handler) => {
    handlers.set(event, cb);
    return Promise.resolve(() => handlers.delete(event));
  });
  const onLogin = vi.fn();
  const onError = vi.fn();
  const gate = createAuthGate({ listen, takeResult, onLogin, onError });
  return { handlers, onLogin, onError, gate };
}

describe("createAuthGate", () => {
  it("logs in on auth://success", () => {
    const { handlers, onLogin } = harness();
    handlers.get(AUTH_SUCCESS_EVENT)!({ payload: { country_code: "US", user_id: 1 } });
    expect(onLogin).toHaveBeenCalledTimes(1);
  });

  it("surfaces auth://error", () => {
    const { handlers, onError } = harness();
    handlers.get(AUTH_ERROR_EVENT)!({ payload: "boom" });
    expect(onError).toHaveBeenCalledWith("boom");
  });

  it("consumes a cold-start result", async () => {
    const { onLogin } = harness(() => Promise.resolve({ country_code: "US", user_id: 1 }));
    await vi.waitFor(() => expect(onLogin).toHaveBeenCalledTimes(1));
  });

  it("surfaces a cold-start error", async () => {
    const { onError } = harness(() => Promise.reject("no login in progress"));
    await vi.waitFor(() => expect(onError).toHaveBeenCalledWith("no login in progress"));
  });
});
