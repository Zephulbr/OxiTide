// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { installNativeMenuSuppression, shouldSuppressNativeMenu } from "./nativeContextMenu";

afterEach(() => {
  document.body.innerHTML = "";
});

describe("shouldSuppressNativeMenu", () => {
  it("suppresses on plain content such as an image", () => {
    const img = document.createElement("img");
    document.body.append(img);
    expect(shouldSuppressNativeMenu(img, "")).toBe(true);
  });

  it("keeps the native menu for editable fields", () => {
    document.body.innerHTML = `<input id="i"><textarea id="t"></textarea><div contenteditable="true"><b id="c"></b></div>`;
    for (const id of ["i", "t", "c"]) {
      expect(shouldSuppressNativeMenu(document.getElementById(id), "")).toBe(false);
    }
  });

  it("keeps the native menu over a text selection so Copy works", () => {
    const p = document.createElement("p");
    document.body.append(p);
    expect(shouldSuppressNativeMenu(p, "some lyric")).toBe(false);
    expect(shouldSuppressNativeMenu(p, "   ")).toBe(true);
  });
});

describe("installNativeMenuSuppression", () => {
  it("prevents the default on content and is removable", () => {
    const off = installNativeMenuSuppression(true);
    const img = document.createElement("img");
    document.body.append(img);
    const first = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    img.dispatchEvent(first);
    expect(first.defaultPrevented).toBe(true);

    off();
    const second = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    img.dispatchEvent(second);
    expect(second.defaultPrevented).toBe(false);
  });

  it("does nothing when disabled", () => {
    const off = installNativeMenuSuppression(false);
    const img = document.createElement("img");
    document.body.append(img);
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    img.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
    off();
  });
});
