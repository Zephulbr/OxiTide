// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import { createTrackView } from "../trackView.svelte";
import TrackSearch from "./TrackSearch.svelte";

function fakeView() {
  return createTrackView(() => []);
}

function isOpen() {
  return document.querySelector(".track-search")!.classList.contains("open");
}

describe("TrackSearch", () => {
  afterEach(cleanup);

  it("closes on an outside click while empty", async () => {
    render(TrackSearch, { view: fakeView() });
    await fireEvent.click(screen.getByLabelText("Filter tracks"));
    expect(isOpen()).toBe(true);
    await fireEvent.mouseDown(document.body);
    expect(isOpen()).toBe(false);
  });

  it("stays open on an outside click once text is typed, and the x closes it", async () => {
    const view = fakeView();
    render(TrackSearch, { view });
    await fireEvent.click(screen.getByLabelText("Filter tracks"));
    await fireEvent.input(screen.getByPlaceholderText("Filter tracks"), {
      target: { value: "nirvana" },
    });
    await fireEvent.mouseDown(document.body);
    expect(isOpen()).toBe(true);
    expect(view.filter).toBe("nirvana");

    await fireEvent.click(screen.getByLabelText("Clear and close filter"));
    expect(isOpen()).toBe(false);
    expect(view.filter).toBe("");
  });
});
