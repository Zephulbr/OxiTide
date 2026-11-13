// node --test scripts/i18n/i18n.test.mjs
// Runs the scripts against a throwaway locales folder (I18N_LOCALES).

import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync, mkdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, beforeEach, describe, it } from "node:test";

const root = mkdtempSync(join(tmpdir(), "oxitide-i18n-"));
process.env.I18N_LOCALES = root;

// Imported after the env var is set: lib.mjs reads it once.
const lib = await import("./lib.mjs");
const { check } = await import("./check.mjs");
const { applyDraft } = await import("./apply-draft.mjs");
const { sync } = await import("./sync.mjs");
const { pseudoMessage } = await import("./pseudo.mjs");

function write(rel, value) {
  const path = join(root, rel);
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function read(rel) {
  return JSON.parse(readFileSync(join(root, rel), "utf8"));
}

const EN = {
  "a.hello": "Hello",
  "a.greet": "Hello, {name}",
  "a.tracks": "{count, plural, one {# track} other {# tracks}}",
  "a.path": "{path, select, bitPerfect {exact} other {}}",
};

function fixture() {
  rmSync(root, { recursive: true, force: true });
  write("languages.json", [
    { code: "en", endonym: "English", script: "latin", ship: true, status: "source" },
    { code: "de", endonym: "Deutsch", script: "latin", ship: false, status: "machine" },
    { code: "pl", endonym: "Polski", script: "latin", ship: false, status: "machine" },
  ]);
  write("glossary.json", { doNotTranslate: [], terms: [] });
  write("en/app.json", EN);
  write("en/remote.json", {});
  write("en/shell.json", { "tray.quit": "Quit" });
  write("notes/app.json", Object.fromEntries(Object.keys(EN).map((k) => [k, "note"])));
  write("notes/remote.json", {});
  write("notes/shell.json", { "tray.quit": "note" });
}

describe("i18n scripts", () => {
  before(fixture);
  beforeEach(fixture);
  after(() => rmSync(root, { recursive: true, force: true }));

  it("passes a clean source catalog", () => {
    assert.deepEqual(check().errors, []);
  });

  it("requires a note for every English key and plain shell text", () => {
    write("notes/app.json", { "a.hello": "note" });
    write("en/shell.json", { "tray.quit": "Quit {now}" });
    const { errors } = check();
    assert.ok(errors.some((e) => e.includes("lacks a note for a.greet")));
    assert.ok(errors.some((e) => e.includes("shell text must be plain")));
  });

  it("catches placeholder, select and plural mistakes", () => {
    write("de/app.json", {
      "a.greet": "Hallo, {nom}",
      "a.path": "{path, select, exact {genau} other {}}",
    });
    write("pl/app.json", { "a.tracks": "{count, plural, one {# utwór} other {# utworów}}" });
    const { errors } = check();
    assert.ok(errors.some((e) => e.includes("de/app.json a.greet") && e.includes("placeholders")));
    assert.ok(errors.some((e) => e.includes("de/app.json a.path") && e.includes("select")));
    assert.ok(errors.some((e) => e.includes("pl/app.json a.tracks") && e.includes("few") && e.includes("many")));
  });

  it("rejects unknown keys and unlisted folders", () => {
    write("de/app.json", { "a.gone": "weg" });
    write("xx/app.json", {});
    const { errors } = check();
    assert.ok(errors.some((e) => e.includes("unknown key a.gone")));
    assert.ok(errors.some((e) => e.includes("locales/xx/ is not listed")));
  });

  it("treats missing messages as warnings until a language ships", () => {
    // Three app messages plus the shell's one.
    write("de/app.json", { "a.hello": "Hallo" });
    assert.deepEqual(check().errors, []);
    assert.ok(check().warnings.some((w) => w.startsWith("de: 4 message(s) missing")));
    const list = read("languages.json");
    list[1].ship = true;
    write("languages.json", list);
    assert.ok(check().errors.some((e) => e.startsWith("de: 4 message(s) missing")));
  });

  it("derives message states", () => {
    const entry = { status: "machine", source: lib.hash("Hello"), draft: lib.hash("Hallo") };
    assert.equal(lib.stateOf("Hello", undefined, entry), "missing");
    assert.equal(lib.stateOf("Hello", "Hallo", entry), "machine");
    assert.equal(lib.stateOf("Hello", "Servus", entry), "edited");
    assert.equal(lib.stateOf("Hi", "Hallo", entry), "stale");
    assert.equal(lib.stateOf("Hello", "Hallo", undefined), "edited");
  });

  it("applies drafts without touching human edits", () => {
    const first = applyDraft("de", { "a.hello": "Hallo", "a.greet": "Hallo, {name}" });
    assert.deepEqual(first.written, ["a.hello", "a.greet"]);
    // A person corrects one message…
    const de = read("de/app.json");
    de["a.hello"] = "Servus";
    write("de/app.json", de);
    // …and a later draft keeps it, but may redraft machine messages.
    const second = applyDraft("de", { "a.hello": "Hallo!", "a.greet": "Hallo {name}!", "a.tracks": "{count} Titel" });
    assert.deepEqual(second.written, ["a.greet", "a.tracks"]);
    assert.deepEqual(second.kept, ["a.hello (edited)"]);
    assert.equal(read("de/app.json")["a.hello"], "Servus");
    assert.equal(read("_meta/de/app.json")["a.greet"].status, "machine");
  });

  it("refuses invalid drafts and keeps English key order", () => {
    const report = applyDraft("de", { "a.path": "{path, select, bitPerfect {genau} other {}}", "a.greet": "Hallo" });
    assert.equal(report.invalid.length, 1);
    assert.ok(report.invalid[0].startsWith("a.greet"));
    assert.deepEqual(Object.keys(read("de/app.json")), ["a.path"]);
    applyDraft("de", { "a.hello": "Hallo" });
    assert.deepEqual(Object.keys(read("de/app.json")), ["a.hello", "a.path"]);
  });

  it("stamps edits as reviewed and updates the picker status", () => {
    applyDraft("de", {
      "a.hello": "Hallo",
      "a.greet": "Hallo, {name}",
      "a.tracks": "{count, plural, one {# Titel} other {# Titel}}",
      "a.path": "{path, select, bitPerfect {genau} other {}}",
    });
    applyDraft("de", { "tray.quit": "Beenden" }, { ns: "shell" });
    assert.equal(read("languages.json")[1].status, "machine");
    const de = read("de/app.json");
    de["a.hello"] = "Servus";
    write("de/app.json", de);
    assert.deepEqual(sync(), { de: 1 });
    assert.equal(read("_meta/de/app.json")["a.hello"].status, "reviewed");
    // The other three app messages and the shell's one.
    assert.deepEqual(sync({ review: "de" }), { de: 4 });
    assert.equal(read("languages.json")[1].status, "reviewed");
  });

  it("marks cross-checked drafts", () => {
    applyDraft("de", { "a.hello": "Hallo" }, { status: "cross-checked" });
    assert.equal(read("_meta/de/app.json")["a.hello"].status, "cross-checked");
  });

  it("builds pseudo messages that keep their placeholders", () => {
    const message = pseudoMessage(EN["a.tracks"]);
    assert.ok(message.startsWith("⟦") && message.endsWith("⟧"));
    assert.deepEqual(lib.compareMessages("en", EN["a.tracks"], message), []);
    assert.equal(pseudoMessage("Quit", true).slice(0, 5), "⟦Ǫûîţ");
  });
});
