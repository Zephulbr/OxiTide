import { describe, expect, it, vi } from "vitest";
import type { Track } from "./api";
import type { DragPayload } from "./dragPayload";
import { playlistDropKey, playlistDropZone } from "./playlistDrop";
import type { PlaylistActions } from "./playlists.svelte";

vi.mock("./api", () => ({ api: { getAlbumTracks: vi.fn(async () => [{ id: 7 }]) } }));

const tracks = [{ id: 1, title: "T1" }] as Track[];
const rows: DragPayload = { kind: "tracks", tracks, source: { tableId: "t", playlistId: "src" } };

function actions() {
  return { addTracks: vi.fn(async () => {}) } as unknown as PlaylistActions & {
    addTracks: ReturnType<typeof vi.fn>;
  };
}

describe("playlistDropZone", () => {
  it("accepts on an owned playlist with its name as the hint", () => {
    const zone = playlistDropZone({ uuid: "p", title: "Road Trip", owned: true }, actions(), vi.fn());
    expect(zone.key).toBe(playlistDropKey("p"));
    expect(zone.accepts?.(rows)).toEqual({ ok: true, hint: "Add to Road Trip" });
  });

  it("refuses someone else's playlist", () => {
    const zone = playlistDropZone({ uuid: "p", title: "Theirs", owned: false }, actions(), vi.fn());
    expect(zone.accepts?.(rows)).toEqual({ ok: false, reason: "You can't edit this playlist" });
  });

  it("refuses the playlist the tracks came from, and a playlist onto itself", () => {
    const own = playlistDropZone({ uuid: "src", title: "Mine", owned: true }, actions(), vi.fn());
    expect(own.accepts?.(rows)?.ok).toBe(false);
    const card: DragPayload = { kind: "collection", target: { kind: "playlist", id: "src" } };
    expect(own.accepts?.(card)?.ok).toBe(false);
  });

  it("drops through addTracks with a loader and an Open action", async () => {
    const act = actions();
    const open = vi.fn();
    const zone = playlistDropZone({ uuid: "p", title: "P", owned: true }, act, open);
    await zone.drop?.({ kind: "collection", target: { kind: "album", id: 3, title: "Blue" } }, { x: 0, y: 0 });
    expect(act.addTracks).toHaveBeenCalledTimes(1);
    const [target, load, options] = act.addTracks.mock.calls[0];
    expect(target).toMatchObject({ uuid: "p" });
    expect(await load()).toEqual([{ id: 7 }]);
    expect(options).toMatchObject({ confirm: true, from: "Blue" });
    options.onOpen();
    expect(open).toHaveBeenCalledTimes(1);
  });

  it("names no collection for dropped rows", async () => {
    const act = actions();
    const zone = playlistDropZone({ uuid: "p", title: "P", owned: true }, act, vi.fn());
    await zone.drop?.(rows, { x: 0, y: 0 });
    expect(act.addTracks.mock.calls[0][2]).toMatchObject({ confirm: true, from: undefined });
  });
});
