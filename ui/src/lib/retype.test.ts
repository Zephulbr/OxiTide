// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ERASE_MS, TYPE_MS, frameText, graphemes, retype, snapshotText } from "./retype";

describe("frameText", () => {
  const from = graphemes("Settings");
  const to = graphemes("Einstellungen");

  it("starts on the old text and ends on the new one", () => {
    expect(frameText(from, to, 0)).toBe("Settings");
    expect(frameText(from, to, ERASE_MS + TYPE_MS)).toBe("Einstellungen");
    expect(frameText(from, to, ERASE_MS + TYPE_MS + 500)).toBe("Einstellungen");
  });

  it("erases, then types", () => {
    expect(frameText(from, to, ERASE_MS / 2)).toBe("Sett");
    expect(frameText(from, to, ERASE_MS)).toBe("");
    expect(frameText(from, to, ERASE_MS + TYPE_MS / 2).length).toBeGreaterThan(0);
    expect("Einstellungen".startsWith(frameText(from, to, ERASE_MS + TYPE_MS / 2))).toBe(true);
  });

  it("keeps a shared prefix in place", () => {
    const a = graphemes("Playlist");
    const b = graphemes("Playliste");
    for (const ms of [0, ERASE_MS / 2, ERASE_MS, ERASE_MS + 10]) {
      expect(frameText(a, b, ms).startsWith("Playlist")).toBe(true);
    }
  });
});

describe("graphemes", () => {
  it("keeps combined characters together", () => {
    expect(graphemes("été")).toEqual(["é", "t", "é"]);
    expect(graphemes("日本語")).toHaveLength(3);
  });
});

describe("retype", () => {
  let frames: FrameRequestCallback[] = [];
  let now = 0;

  beforeEach(() => {
    frames = [];
    now = 0;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    // jsdom has no layout: give every element a box on screen.
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockReturnValue(
      { top: 10, left: 10, bottom: 30, right: 200, width: 190, height: 20 } as DOMRect,
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  function runFrame(at: number) {
    now = at;
    const pending = frames;
    frames = [];
    for (const cb of pending) cb(at);
  }

  it("animates changed text and lands on the new text", async () => {
    document.body.innerHTML = `<p id="a">Settings</p><p id="b">Same</p>`;
    const a = document.getElementById("a")!.firstChild as Text;
    const snapshot = snapshotText();
    a.nodeValue = "Einstellungen";

    const done = retype(snapshot);
    // The old text is back before any paint.
    expect(a.nodeValue).toBe("Settings");
    runFrame(ERASE_MS / 2);
    expect(a.nodeValue).toBe("Sett");
    runFrame(ERASE_MS + TYPE_MS);
    await done;
    expect(a.nodeValue).toBe("Einstellungen");
    expect(document.getElementById("b")!.textContent).toBe("Same");
  });

  it("drops a node its owner rewrites mid-animation", async () => {
    document.body.innerHTML = `<p id="a">0:01</p>`;
    const a = document.getElementById("a")!.firstChild as Text;
    const snapshot = snapshotText();
    a.nodeValue = "0:02";
    const done = retype(snapshot);
    runFrame(10);
    a.nodeValue = "0:03";
    runFrame(ERASE_MS + TYPE_MS);
    await done;
    expect(a.nodeValue).toBe("0:03");
  });

  it("leaves live regions and form fields alone", () => {
    document.body.innerHTML = `<div aria-live="polite"><span>Now playing</span></div><textarea>Note</textarea>`;
    expect(snapshotText().texts.size).toBe(0);
  });
});
