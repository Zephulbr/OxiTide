// @vitest-environment jsdom
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/svelte";
import { searchHistory } from "../searchHistory.svelte";
import SearchField from "./SearchField.svelte";

beforeAll(() => {
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
    removeItem: (k: string) => void store.delete(k),
  });
});

beforeEach(() => {
  const history = searchHistory();
  history.clear();
  history.record("Air");
  history.record("Bonobo");
});

afterEach(cleanup);

function renderField(value = "") {
  const onsearch = vi.fn();
  const oninput = vi.fn();
  render(SearchField, { value, oninput, onsearch });
  return { onsearch, oninput, field: screen.getByRole("combobox") };
}

describe("SearchField", () => {
  it("lists recent searches on focus", async () => {
    const { field } = renderField();
    await fireEvent.focus(field);
    expect(field.getAttribute("aria-expanded")).toBe("true");
    expect(screen.getAllByRole("option").map((o) => o.textContent?.trim())).toEqual(["Bonobo", "Air"]);
  });

  it("submits the highlighted suggestion with the arrow keys and Enter", async () => {
    const { field, onsearch } = renderField();
    await fireEvent.focus(field);
    await fireEvent.keyDown(field, { key: "ArrowDown" });
    await fireEvent.keyDown(field, { key: "ArrowDown" });
    expect(field.getAttribute("aria-activedescendant")).toBe("search-suggestion-1");
    await fireEvent.keyDown(field, { key: "Enter" });
    expect(onsearch).toHaveBeenCalledWith("Air");
    expect(field.getAttribute("aria-expanded")).toBe("false");
    expect(searchHistory().entries[0]).toBe("Air");
  });

  it("submits the typed text and records it", async () => {
    const { field, onsearch } = renderField("Caribou");
    await fireEvent.keyDown(field, { key: "Enter" });
    expect(onsearch).toHaveBeenCalledWith("Caribou");
    expect(searchHistory().entries[0]).toBe("Caribou");
  });

  it("closes the list on Escape", async () => {
    const { field } = renderField();
    await fireEvent.focus(field);
    await fireEvent.keyDown(field, { key: "Escape" });
    expect(field.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("clears the recent searches", async () => {
    const { field } = renderField();
    await fireEvent.focus(field);
    await fireEvent.click(screen.getByRole("button", { name: "Clear recent searches" }));
    expect(searchHistory().entries).toEqual([]);
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("passes typing through", async () => {
    const { field, oninput } = renderField();
    await fireEvent.input(field, { target: { value: "Moderat" } });
    expect(oninput).toHaveBeenCalledWith("Moderat");
  });
});
