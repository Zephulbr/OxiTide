// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { portal } from "./portal";

describe("portal", () => {
  it("moves the node to the body and removes it on destroy", () => {
    const host = document.createElement("main");
    const node = document.createElement("div");
    host.append(node);
    document.body.append(host);

    const handle = portal(node);
    expect(node.parentElement).toBe(document.body);

    handle.destroy();
    expect(node.isConnected).toBe(false);
    host.remove();
  });

  it("preserves focus inside the moved subtree", () => {
    const host = document.createElement("main");
    const node = document.createElement("div");
    const button = document.createElement("button");
    node.append(button);
    host.append(node);
    document.body.append(host);
    button.focus();

    const handle = portal(node);

    expect(document.activeElement).toBe(button);
    handle.destroy();
    host.remove();
  });
});
