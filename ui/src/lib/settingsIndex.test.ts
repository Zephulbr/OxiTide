import { describe, expect, it } from "vitest";
import { SETTINGS_TABS } from "./settingsTabs";
import { SETTINGS_INDEX, searchSettings } from "./settingsIndex";
import settingsView from "../views/Settings.svelte?raw";
import remote from "./components/RemoteSettings.svelte?raw";
import updates from "./components/UpdatesSection.svelte?raw";
import autoMix from "./components/settings/AutoMixPanel.svelte?raw";

const ctx = { rack: false, studio: false, exclusive: false };

describe("settings index", () => {
  it("has unique ids on real tabs", () => {
    const ids = SETTINGS_INDEX.map((entry) => entry.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const entry of SETTINGS_INDEX) expect(SETTINGS_TABS).toContain(entry.tab);
  });

  // Every result must point at an element the page really renders: the id is
  // a literal `id="…"` on a row/card (hotkey rows build theirs from the action).
  it("only points at ids present in the page sources", () => {
    const sources = [settingsView, remote, updates, autoMix].join("\n");
    const missing = SETTINGS_INDEX.filter(
      (entry) => !entry.id.startsWith("hotkey-") && !sources.includes(`id="${entry.id}"`),
    ).map((entry) => entry.id);
    expect(missing).toEqual([]);
    expect(sources).toContain("id={`hotkey-${entry.id}`}");
  });
});

describe("searchSettings", () => {
  it("returns nothing for a blank query", () => {
    expect(searchSettings("   ", ctx)).toEqual([]);
  });

  it("ranks a label hit above a keyword hit", () => {
    const hits = searchSettings("crossfade", ctx).map((e) => e.id);
    expect(hits[0]).toBe("crossfade");
    expect(hits).toContain("automix-style");
  });

  it("matches synonyms and prefixes", () => {
    expect(searchSettings("font", ctx)[0].id).toBe("typeface");
    expect(searchSettings("loud", ctx).map((e) => e.id)).toContain("replaygain");
    expect(searchSettings("lossl", ctx)[0].id).toBe("quality");
  });

  it("requires every word to match", () => {
    expect(searchSettings("cache location", ctx)[0].id).toBe("cache-location");
    expect(searchSettings("cache zzzz", ctx)).toEqual([]);
  });

  it("hides entries whose control is not on the page", () => {
    expect(searchSettings("lamp", ctx)).toEqual([]);
    expect(searchSettings("lamp", { ...ctx, rack: true }).map((e) => e.id)).toContain("rack-lamp");
    expect(searchSettings("dynamic color", { ...ctx, rack: true })).toEqual([]);
    expect(searchSettings("amoled", ctx)).toEqual([]);
    expect(searchSettings("amoled", { ...ctx, studio: true })[0].id).toBe("studio-tone");
    expect(searchSettings("if unsupported", ctx)).toEqual([]);
    expect(searchSettings("if unsupported", { ...ctx, exclusive: true })[0].id).toBe("exclusive-fallback");
  });

  it("finds a hotkey by its action", () => {
    expect(searchSettings("hotkey next", ctx)[0].id).toBe("hotkey-next");
  });

  it("caps the result count", () => {
    expect(searchSettings("e", ctx).length).toBeLessThanOrEqual(8);
  });
});
