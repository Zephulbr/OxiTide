import { describe, expect, it } from "vitest";
import { createSelection, rangeBetween } from "./selection.svelte";

interface Row {
  id: number;
}

/// Distinct row objects; `rows(1, 1)` gives two rows with the same id, like a
/// playlist that holds a track twice.
function rows(...ids: number[]): Row[] {
  return ids.map((id) => ({ id }));
}

const plain = { ctrl: false, shift: false };
const ctrl = { ctrl: true, shift: false };
const shift = { ctrl: false, shift: true };
const ctrlShift = { ctrl: true, shift: true };

describe("rangeBetween", () => {
  it("spans both directions inclusively", () => {
    const items = rows(1, 2, 3, 4);
    expect(rangeBetween(items, items[1], items[3])).toEqual([items[1], items[2], items[3]]);
    expect(rangeBetween(items, items[3], items[1])).toEqual([items[1], items[2], items[3]]);
    expect(rangeBetween(items, items[2], items[2])).toEqual([items[2]]);
  });

  it("is empty when an end is missing", () => {
    const items = rows(1, 2);
    expect(rangeBetween(items, items[0], { id: 9 })).toEqual([]);
  });
});

describe("createSelection", () => {
  it("plain click selects only that row", () => {
    const items = rows(1, 2, 3);
    const s = createSelection<Row>();
    s.click(items[0], items, plain);
    s.click(items[2], items, plain);
    expect(s.ordered(items)).toEqual([items[2]]);
    expect(s.size).toBe(1);
  });

  it("ctrl toggles rows in and out", () => {
    const items = rows(1, 2, 3);
    const s = createSelection<Row>();
    s.click(items[0], items, ctrl);
    s.click(items[2], items, ctrl);
    expect(s.ordered(items)).toEqual([items[0], items[2]]);
    s.click(items[0], items, ctrl);
    expect(s.ordered(items)).toEqual([items[2]]);
  });

  it("shift selects the range from the anchor, either way", () => {
    const items = rows(1, 2, 3, 4, 5);
    const s = createSelection<Row>();
    s.click(items[3], items, plain);
    s.click(items[1], items, shift);
    expect(s.ordered(items)).toEqual([items[1], items[2], items[3]]);
    // The anchor stays put: a second shift-click re-ranges from it.
    s.click(items[4], items, shift);
    expect(s.ordered(items)).toEqual([items[3], items[4]]);
  });

  it("ctrl+shift adds a range to the selection", () => {
    const items = rows(1, 2, 3, 4, 5);
    const s = createSelection<Row>();
    s.click(items[0], items, plain);
    s.click(items[3], items, ctrl);
    s.click(items[4], items, ctrlShift);
    expect(s.ordered(items)).toEqual([items[0], items[3], items[4]]);
  });

  it("shift without an anchor in the list selects just that row", () => {
    const items = rows(1, 2, 3);
    const s = createSelection<Row>();
    s.click(items[1], items, shift);
    expect(s.ordered(items)).toEqual([items[1]]);
    const reloaded = rows(1, 2, 3);
    s.click(reloaded[2], reloaded, shift);
    expect(s.ordered(reloaded)).toEqual([reloaded[2]]);
  });

  it("extend moves the range end from the anchor", () => {
    const items = rows(1, 2, 3, 4);
    const s = createSelection<Row>();
    s.click(items[1], items, plain);
    s.extend(items[2], items);
    s.extend(items[3], items);
    expect(s.ordered(items)).toEqual([items[1], items[2], items[3]]);
    s.extend(items[0], items);
    expect(s.ordered(items)).toEqual([items[0], items[1]]);
  });

  it("toggle flips one row and anchors there", () => {
    const items = rows(1, 2, 3, 4);
    const s = createSelection<Row>();
    s.toggle(items[0]);
    s.toggle(items[2]);
    s.extend(items[3], items);
    expect(s.ordered(items)).toEqual([items[2], items[3]]);
  });

  it("selectAll and clear", () => {
    const items = rows(1, 2, 3);
    const s = createSelection<Row>();
    s.selectAll(items);
    expect(s.size).toBe(3);
    s.clear();
    expect(s.size).toBe(0);
    expect(s.ordered(items)).toEqual([]);
  });

  it("ordered follows the list order and drops rows that left it", () => {
    const items = rows(1, 2, 3, 4);
    const s = createSelection<Row>();
    s.click(items[3], items, ctrl);
    s.click(items[0], items, ctrl);
    s.click(items[2], items, ctrl);
    expect(s.ordered(items)).toEqual([items[0], items[2], items[3]]);
    const resorted = [items[3], items[2], items[1]];
    expect(s.ordered(resorted)).toEqual([items[3], items[2]]);
  });

  it("keeps duplicate track ids as distinct rows", () => {
    const items = rows(7, 8, 7);
    const s = createSelection<Row>();
    s.click(items[0], items, plain);
    s.click(items[2], items, ctrl);
    expect(s.indices(items)).toEqual([0, 2]);
    expect(s.has(items[1])).toBe(false);
  });
});
