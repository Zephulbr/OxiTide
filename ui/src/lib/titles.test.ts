import { describe, expect, it } from "vitest";
import { routeTitle } from "./titles";

describe("routeTitle", () => {
  it("names the fixed routes", () => {
    expect(routeTitle({ name: "home" })).toBe("Home · OxiTide");
    expect(routeTitle({ name: "settings" })).toBe("Settings · OxiTide");
    expect(routeTitle({ name: "history", tab: "stats" })).toBe("Stats · OxiTide");
    expect(routeTitle({ name: "history", tab: "recent" })).toBe("History · OxiTide");
  });

  it("uses the section and folder labels", () => {
    expect(routeTitle({ name: "section", source: "home", title: "New releases" })).toBe(
      "New releases · OxiTide",
    );
    expect(routeTitle({ name: "folder", id: "f1", label: "Jazz" })).toBe("Jazz · OxiTide");
    expect(routeTitle({ name: "folder", id: "f1" })).toBe("Folder · OxiTide");
    expect(routeTitle({ name: "folder", id: "f1", label: "" })).toBe("Folder · OxiTide");
  });

  it("gives the Now Playing overlay precedence", () => {
    expect(routeTitle({ name: "home" }, true)).toBe("Now playing · OxiTide");
  });

  it("covers the detail routes", () => {
    expect(routeTitle({ name: "artist-all", id: 1, tab: "tracks" })).toBe("Artist · OxiTide");
    expect(routeTitle({ name: "album", id: 1 })).toBe("Album · OxiTide");
    expect(routeTitle({ name: "explore" })).toBe("Explore · OxiTide");
  });
});
