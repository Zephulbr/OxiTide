import { api } from "./api";

/// A card's own cover colour for the hover tint. Fetched through the same cached
/// `analyze_cover` pipeline as dynamic colour and kept for the session. Cards
/// prefetch it as they scroll into view (a few at a time, so a page full of
/// covers never floods the backend); a hover that beats the prefetch is
/// `urgent` and skips the queue. Only dashed CDN uuids can be analysed:
/// full-URL artwork (some mix images) resolves to `null` and the card falls
/// back to the accent tint.

type Analyze = (uuid: string, theme: "dark" | "light") => Promise<{ primary: string }>;

const MAX_PARALLEL = 3;

const done = new Map<string, string>();
const inflight = new Map<string, Promise<string | null>>();
let active = 0;
const waiting: (() => void)[] = [];

export function clearCoverTints() {
  done.clear();
  inflight.clear();
}

async function queued<T>(run: () => Promise<T>): Promise<T> {
  if (active >= MAX_PARALLEL) await new Promise<void>((resolve) => waiting.push(resolve));
  active += 1;
  try {
    return await run();
  } finally {
    active -= 1;
    waiting.shift()?.();
  }
}

export function coverTint(
  uuid: string | null | undefined,
  theme: "dark" | "light",
  options: { analyze?: Analyze; urgent?: boolean } = {},
): Promise<string | null> {
  if (!uuid || uuid.startsWith("http")) return Promise.resolve(null);
  const analyze = options.analyze ?? api.analyzeCover;
  const key = `${theme}|${uuid}`;
  const hit = done.get(key);
  if (hit) return Promise.resolve(hit);
  let pending = inflight.get(key);
  if (!pending) {
    const run = () => analyze(uuid, theme);
    pending = (options.urgent ? run() : queued(run))
      .then((palette) => {
        done.set(key, palette.primary);
        return palette.primary;
      })
      // A failed analysis is not remembered, so a later hover can retry.
      .catch(() => null)
      .finally(() => inflight.delete(key));
    inflight.set(key, pending);
  }
  return pending;
}
