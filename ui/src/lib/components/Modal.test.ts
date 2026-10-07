// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import Modal from "./Modal.svelte";

afterEach(cleanup);

function children(html: string) {
  return createRawSnippet(() => ({ render: () => html }));
}

describe("Modal", () => {
  it("focuses a control inside the panel on mount", async () => {
    render(Modal, {
      title: "Test",
      onclose: vi.fn(),
      children: children('<button id="inner">inner</button>'),
    });

    const panel = document.querySelector('[role="dialog"]');
    expect(panel).not.toBeNull();
    await waitFor(() => expect(panel!.contains(document.activeElement)).toBe(true));
    cleanup();
  });

  it("restores focus to the previously focused element on unmount", async () => {
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();

    render(Modal, {
      title: "Test",
      onclose: vi.fn(),
      children: children('<button id="inner">inner</button>'),
    });
    await waitFor(() =>
      expect(document.querySelector('[role="dialog"]')!.contains(document.activeElement)).toBe(true),
    );

    cleanup();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });
});

describe("Modal focus trap", () => {
  it("wraps Tab from the last control to the first, and Shift+Tab back", async () => {
    render(Modal, {
      title: "Test",
      onclose: vi.fn(),
      children: children('<button id="inner">inner</button>'),
    });

    const closeButton = document.querySelector<HTMLElement>('[aria-label="Close"]')!;
    const inner = document.getElementById("inner")!;
    await waitFor(() => expect(document.activeElement).toBe(closeButton));

    inner.focus();
    await fireEvent.keyDown(inner, { key: "Tab" });
    expect(document.activeElement).toBe(closeButton);

    await fireEvent.keyDown(closeButton, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(inner);
  });

  it("keeps focus on the panel when it has no focusable control", async () => {
    render(Modal, {
      title: "Empty",
      onclose: vi.fn(),
      children: children("<p>nothing focusable</p>"),
    });

    const panel = document.querySelector<HTMLElement>('[role="dialog"]')!;
    const closeButton = document.querySelector<HTMLElement>('[aria-label="Close"]')!;
    closeButton.remove();
    panel.focus();

    await fireEvent.keyDown(panel, { key: "Tab" });
    expect(document.activeElement).toBe(panel);
  });
});
