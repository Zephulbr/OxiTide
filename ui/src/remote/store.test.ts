import { describe, expect, it, vi } from "vitest";
import type { QueueItem, RemoteState } from "../lib/api";
import { RemoteError } from "./rpc";
import { RemoteStore } from "./store.svelte";

function remoteState(patch: Partial<RemoteState> = {}): RemoteState {
  return {
    loggedIn: true,
    playing: true,
    buffering: false,
    current: {
      trackId: 1,
      title: "One",
      artist: "A",
      album: "B",
      cover: null,
      durationSecs: 200,
      explicit: false,
      audioQuality: "lossless",
    },
    currentIndex: 0,
    positionMs: 1000,
    durationMs: 200_000,
    volume: 0.5,
    shuffle: false,
    repeat: "off",
    queueLen: 3,
    queueRev: 0,
    stream: null,
    bitPerfect: false,
    dspStages: [],
    ...patch,
  };
}

function item(id: number): QueueItem {
  return { track_id: id, title: `T${id}`, artist: "A", album: "B", duration_secs: 100 };
}

class FakeSource {
  static last: FakeSource | null = null;
  listeners = new Map<string, (event: { data: string }) => void>();
  onerror: (() => void) | null = null;
  closed = false;
  constructor(readonly url: string) {
    FakeSource.last = this;
  }
  addEventListener(name: string, fn: (event: { data: string }) => void) {
    this.listeners.set(name, fn);
  }
  close() {
    this.closed = true;
  }
  emit(name: string, data: unknown) {
    this.listeners.get(name)?.({ data: JSON.stringify(data) });
  }
}

function setup(call: (method: string, params?: Record<string, unknown>) => Promise<unknown>) {
  const timers: (() => void)[] = [];
  let clock = 10_000;
  const store = new RemoteStore({
    call: call as never,
    openEvents: (url) => new FakeSource(url) as never,
    now: () => clock,
    setTimer: (fn) => {
      timers.push(fn);
      return timers.length;
    },
    clearTimer: () => {},
  });
  return {
    store,
    timers,
    advance: (ms: number) => (clock += ms),
  };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("remote store", () => {
  it("probes, opens the stream, and applies snapshot then deltas", async () => {
    const { store } = setup(async () => remoteState());
    store.start();
    await flush();
    const source = FakeSource.last!;
    expect(source.url).toBe("/api/events");
    source.emit("snapshot", remoteState({ positionMs: 5000 }));
    expect(store.phase).toBe("live");
    expect(store.state?.positionMs).toBe(5000);
    source.emit("state", remoteState({ playing: false, volume: 0.9 }));
    expect(store.state?.playing).toBe(false);
    expect(store.state?.volume).toBe(0.9);
    source.emit("position", { positionMs: 7000, durationMs: 200_000 });
    expect(store.state?.positionMs).toBe(7000);
  });

  it("interpolates the position while playing", async () => {
    const { store, advance } = setup(async () => remoteState());
    store.applyState(remoteState({ positionMs: 1000 }), true);
    advance(1500);
    expect(store.positionAt()).toBe(2500);
    store.applyState(remoteState({ positionMs: 1000, playing: false }), false);
    advance(1500);
    expect(store.positionAt()).toBe(1000);
  });

  it("marks an unknown phone as unpaired", async () => {
    const { store } = setup(async () => {
      throw new RemoteError("unauthorized", "x", 401);
    });
    store.start();
    await flush();
    expect(store.phase).toBe("unpaired");
  });

  it("reconnects with backoff and resyncs with a fresh probe", async () => {
    let up = true;
    const call = vi.fn(async () => {
      if (!up) throw new RemoteError("offline", "down", 0);
      return remoteState();
    });
    const { store, timers } = setup(call);
    store.start();
    await flush();
    FakeSource.last!.emit("snapshot", remoteState());
    up = false;
    const first = FakeSource.last!;
    first.onerror?.();
    expect(first.closed).toBe(true);
    expect(store.phase).toBe("reconnecting");
    timers.shift()!();
    await flush();
    expect(store.phase).toBe("reconnecting");
    up = true;
    timers.shift()!();
    await flush();
    expect(FakeSource.last).not.toBe(first);
    FakeSource.last!.emit("snapshot", remoteState({ queueRev: 4 }));
    expect(store.phase).toBe("live");
  });

  it("goes back to pairing after a revoke", async () => {
    let paired = true;
    const { store } = setup(async () => {
      if (!paired) throw new RemoteError("unauthorized", "x", 401);
      return remoteState();
    });
    store.start();
    await flush();
    const source = FakeSource.last!;
    source.emit("snapshot", remoteState());
    paired = false;
    source.emit("revoked", {});
    await flush();
    expect(source.closed).toBe(true);
    expect(store.phase).toBe("unpaired");
  });

  it("invalidates queue pages when queueRev moves", async () => {
    const pages: Record<string, unknown>[] = [];
    let rev = 0;
    const call = vi.fn(async (method: string, params?: Record<string, unknown>) => {
      if (method === "get_queue_page") {
        pages.push(params!);
        return {
          items: [item(1), item(2), item(3)],
          total: 3,
          offset: 0,
          limit: 100,
          currentIndex: 0,
          queueRev: rev,
        };
      }
      return remoteState();
    });
    const { store } = setup(call);
    store.applyState(remoteState(), true);
    await store.ensureQueue(0, 3);
    expect(store.queueItem(2)?.track_id).toBe(3);
    await store.ensureQueue(0, 3);
    expect(pages).toHaveLength(1);

    store.applyState(remoteState({ queueRev: 0 }), false);
    expect(store.queueItem(0)?.track_id).toBe(1);

    rev = 1;
    store.applyState(remoteState({ queueRev: 1 }), false);
    expect(store.queueItem(0)).toBeUndefined();
    await store.ensureQueue(0, 3);
    expect(pages).toHaveLength(2);
    expect(store.queueItem(0)?.track_id).toBe(1);
  });

  it("notes session changes", () => {
    const { store } = setup(async () => remoteState());
    store.handle("session", JSON.stringify("loggedOut"));
    expect(store.session).toBe("Signed out on the PC.");
    store.applyState(remoteState({ loggedIn: true }), false);
    expect(store.session).toBe("");
    store.applyState(remoteState({ loggedIn: false, current: null }), true);
    expect(store.session).toContain("Sign in on the PC");
  });
});
