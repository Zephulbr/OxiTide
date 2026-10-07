// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/svelte";
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
