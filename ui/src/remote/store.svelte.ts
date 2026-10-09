import type { QueueItem, RemoteQueuePage, RemoteState } from "../lib/api";
import { QUEUE_PAGE, QueueCache } from "./queueCache";
import { RemoteError, type Call } from "./rpc";

/// connecting → live; reconnecting while the stream is down; unpaired when
/// the PC no longer knows this phone (never paired, or revoked).
export type Phase = "connecting" | "live" | "reconnecting" | "unpaired";

type SourceLike = Pick<EventSource, "addEventListener" | "close"> & {
  onerror: ((this: EventSource, ev: Event) => unknown) | null;
};

export interface StoreDeps {
  call: Call;
  openEvents?: (url: string) => SourceLike;
  now?: () => number;
  setTimer?: (fn: () => void, ms: number) => unknown;
  clearTimer?: (handle: unknown) => void;
}

const EVENTS = ["snapshot", "state", "position", "error", "session", "revoked"] as const;
const MAX_BACKOFF_MS = 15_000;

/// The phone's playback state: SSE snapshot + deltas, a paged queue cache
/// invalidated by `queueRev`, and reconnect with backoff (spec §6.4–§6.5).
export class RemoteStore {
  phase = $state<Phase>("connecting");
  state = $state<RemoteState | null>(null);
  /// Session note ("Signed out on the PC"), empty when fine.
  session = $state("");
  /// Last playback error reported by the PC.
  playbackError = $state("");
  /// Bumps whenever queue rows change, so views re-read the cache.
  queueVersion = $state(0);
  readonly queue = new QueueCache();

  private deps: Required<StoreDeps>;
  private source: SourceLike | null = null;
  private timer: unknown = null;
  private attempts = 0;
  private anchor = { positionMs: 0, at: 0 };
  private loading = new Set<number>();
  private stopped = false;

  constructor(deps: StoreDeps) {
    this.deps = {
      openEvents: (url) => new EventSource(url),
      now: () => Date.now(),
      setTimer: (fn, ms) => setTimeout(fn, ms),
      clearTimer: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
      ...deps,
    };
  }

  start() {
    this.stopped = false;
    void this.probe();
  }

  stop() {
    this.stopped = true;
    this.closeStream();
    this.clearTimer();
  }

  /// Reconnect right away (the page became visible again).
  wake() {
    if (this.stopped || this.phase === "live" || this.phase === "unpaired") return;
    this.clearTimer();
    this.attempts = 0;
    void this.probe();
  }

  /// One RPC to learn whether the PC is reachable and still knows us, then
  /// open the event stream.
  async probe() {
    try {
      const state = await this.deps.call<RemoteState>("get_remote_state");
      if (this.stopped) return;
      this.applyState(state, true);
      this.openStream();
    } catch (e) {
      if (this.stopped) return;
      if (e instanceof RemoteError && e.status === 401) {
        this.closeStream();
        this.phase = "unpaired";
        return;
      }
      this.scheduleRetry();
    }
  }

  private openStream() {
    this.closeStream();
    const source = this.deps.openEvents("/api/events");
    for (const name of EVENTS) {
      source.addEventListener(name, (event) => this.handle(name, (event as MessageEvent).data));
    }
    source.onerror = () => {
      // EventSource hides the status: drop it and re-probe, which tells a
      // revoked phone (401) from a sleeping PC (network error).
      this.closeStream();
      this.scheduleRetry();
    };
    this.source = source;
  }

  private closeStream() {
    this.source?.close();
    this.source = null;
  }

  private clearTimer() {
    if (this.timer !== null) this.deps.clearTimer(this.timer);
    this.timer = null;
  }

  private scheduleRetry() {
    this.clearTimer();
    this.phase = this.state ? "reconnecting" : "connecting";
    const delay = Math.min(MAX_BACKOFF_MS, 1000 * 2 ** this.attempts);
    this.attempts += 1;
    this.timer = this.deps.setTimer(() => {
      this.timer = null;
      void this.probe();
    }, delay);
  }

  handle(name: string, data: string) {
    let payload: unknown;
    try {
      payload = JSON.parse(data);
    } catch {
      return;
    }
    switch (name) {
      case "snapshot":
        this.attempts = 0;
        this.phase = "live";
        this.applyState(payload as RemoteState, true);
        break;
      case "state":
        this.applyState(payload as RemoteState, false);
        break;
      case "position":
        this.applyPosition(payload as { positionMs: number; durationMs: number });
        break;
      case "error": {
        const { message } = payload as { code: string; message: string };
        this.playbackError = message;
        break;
      }
      case "session": {
        const kind = payload as "loggedOut" | "expired" | "ok";
        this.session =
          kind === "loggedOut"
            ? "Signed out on the PC."
            : kind === "expired"
              ? "The TIDAL session expired on the PC."
              : "";
        break;
      }
      case "revoked":
        this.closeStream();
        void this.probe();
        break;
    }
  }

  applyState(next: RemoteState, snapshot: boolean) {
    const changed = this.queue.sync(next.queueRev, next.queueLen);
    if (snapshot || changed) {
      this.loading.clear();
      this.queueVersion += 1;
    }
    if (next.loggedIn) this.session = "";
    else if (!this.session) this.session = "Sign in on the PC to use the remote.";
    if (next.playing) this.playbackError = "";
    this.state = next;
    this.anchor = { positionMs: next.positionMs, at: this.deps.now() };
  }

  applyPosition(position: { positionMs: number; durationMs: number }) {
    if (!this.state) return;
    this.state.positionMs = position.positionMs;
    this.state.durationMs = position.durationMs;
    this.anchor = { positionMs: position.positionMs, at: this.deps.now() };
  }

  /// Seek locally at once; the PC confirms with the next position event.
  noteSeek(positionMs: number) {
    if (!this.state) return;
    this.state.positionMs = positionMs;
    this.anchor = { positionMs, at: this.deps.now() };
  }

  /// The position now, interpolated between 1 Hz position events.
  positionAt(nowMs = this.deps.now()): number {
    const state = this.state;
    if (!state) return 0;
    if (!state.playing || state.buffering) return state.positionMs;
    const position = this.anchor.positionMs + Math.max(0, nowMs - this.anchor.at);
    return state.durationMs > 0 ? Math.min(state.durationMs, position) : position;
  }

  queueItem(index: number): QueueItem | undefined {
    void this.queueVersion;
    return this.queue.get(index);
  }

  /// Fetch the queue pages covering rows `[from, to)` that are not cached.
  async ensureQueue(from: number, to: number) {
    const pages = this.queue.missing(from, to).filter((page) => !this.loading.has(page));
    await Promise.all(pages.map((page) => this.loadPage(page)));
  }

  private async loadPage(page: number) {
    this.loading.add(page);
    try {
      const result = await this.deps.call<RemoteQueuePage>("get_queue_page", {
        offset: page * QUEUE_PAGE,
        limit: QUEUE_PAGE,
      });
      if (result.queueRev !== this.queue.rev) {
        // The queue moved while the page was in flight: resync and let the
        // view ask again.
        this.queue.sync(result.queueRev, result.total);
        this.queueVersion += 1;
        return;
      }
      if (this.queue.set(page, result.items, result.queueRev)) this.queueVersion += 1;
    } catch {
      // A failed page stays missing; scrolling asks again.
    } finally {
      this.loading.delete(page);
    }
  }
}
