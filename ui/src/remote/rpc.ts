/// The phone remote's transport (spec §6.3): one RPC endpoint and the pairing
/// call, both same-origin with the device cookie. Never imports Tauri.

import { t } from "./i18n.svelte";

export class RemoteError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status = 0,
  ) {
    super(message);
  }
}

/// The server's `"<code>: <message>"` error string, split.
export function parseWireError(text: string): { code: string; message: string } {
  const split = text.indexOf(": ");
  if (split <= 0 || /\s/.test(text.slice(0, split))) return { code: "failed", message: text };
  return { code: text.slice(0, split), message: text.slice(split + 2) };
}

/// Human wording for transport-level failures (getters: the phone's language).
const STATUS_ERRORS: Record<number, [string, string]> = {
  get 401(): [string, string] { return ["unauthorized", t("error.unauthorized")]; },
  get 403(): [string, string] { return ["forbidden", t("error.forbidden")]; },
  get 408(): [string, string] { return ["timeout", t("error.timeout")]; },
  get 413(): [string, string] { return ["too_large", t("error.tooLarge")]; },
  get 429(): [string, string] { return ["rate_limited", t("error.rateLimited")]; },
  get 503(): [string, string] { return ["busy", t("error.busy")]; },
};

export type Call = <T>(method: string, params?: Record<string, unknown>) => Promise<T>;

export interface RemoteClient {
  call: Call;
  pair(token: string, deviceName: string): Promise<void>;
}

type FetchLike = (input: string, init: RequestInit) => Promise<Response>;

export function createClient(fetchImpl: FetchLike = (input, init) => fetch(input, init)): RemoteClient {
  async function post(path: string, body: unknown): Promise<unknown> {
    let response: Response;
    try {
      response = await fetchImpl(path, {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", "X-Oxi-Remote": "1" },
        body: JSON.stringify(body),
      });
    } catch {
      throw new RemoteError("offline", t("error.offline"));
    }
    let payload: { ok?: boolean; result?: unknown; error?: string } | null = null;
    try {
      payload = await response.json();
    } catch {
      payload = null;
    }
    if (payload && payload.ok === true) return payload.result;
    if (payload && typeof payload.error === "string") {
      const { code, message } = parseWireError(payload.error);
      const known = STATUS_ERRORS[response.status];
      // Prefer friendly wording for transport errors, the server's otherwise.
      throw new RemoteError(code, known && response.status !== 403 ? known[1] : message, response.status);
    }
    const [code, message] = STATUS_ERRORS[response.status] ?? ["failed", t("error.failed", { status: response.status })];
    throw new RemoteError(code, message, response.status);
  }

  return {
    call: <T>(method: string, params?: Record<string, unknown>) =>
      post("/api/rpc", { method, params: params ?? null }) as Promise<T>,
    pair: async (token: string, deviceName: string) => {
      await post("/api/pair", { token, deviceName });
    },
  };
}

/// A readable label for this phone, shown in the PC's device list.
export function guessDeviceName(userAgent: string): string {
  if (/iPhone/i.test(userAgent)) return "iPhone";
  if (/iPad/i.test(userAgent)) return "iPad";
  const android = userAgent.match(/Android[^;)]*;\s*([^;)]+?)(?:\s+Build|\))/i);
  if (android && android[1] && !/^(K|wv|Linux)$/i.test(android[1].trim())) return android[1].trim();
  if (/Android/i.test(userAgent)) return "Android phone";
  if (/Windows/i.test(userAgent)) return "Windows browser";
  if (/Macintosh/i.test(userAgent)) return "Mac browser";
  return "Phone";
}

/// The pairing token from `#p=<token>`, if the page was opened from the QR.
export function pairingToken(hash: string): string | null {
  const match = hash.match(/^#p=([A-Za-z0-9_-]{16,64})$/);
  return match ? match[1] : null;
}
