import assert from "node:assert/strict";
import { test } from "node:test";
import { buildManifest, checksumLines } from "./make-manifest.mjs";
import { versionProblems, workspaceVersion } from "./check-version.mjs";

const files = [
  "a/OxiTide_0.3.0_x64-setup.exe",
  "a/OxiTide_0.3.0_x64-setup.exe.sig",
  "a/OxiTide_0.3.0_x64_en-US.msi",
  "a/OxiTide_0.3.0_x64_en-US.msi.sig",
  "b/OxiTide_0.3.0_amd64.AppImage",
  "b/OxiTide_0.3.0_amd64.AppImage.sig",
  "b/OxiTide_0.3.0_amd64.deb",
];
const readText = (path) => `sig-of-${path.split("/").pop()}\n`;

test("manifest maps bundles to platform keys with trimmed signatures", () => {
  const manifest = buildManifest({
    files,
    tag: "v0.3.0",
    repo: "o/r",
    notes: "n",
    pubDate: "2026-10-13T00:00:00.000Z",
    readText,
  });
  assert.equal(manifest.version, "0.3.0");
  assert.deepEqual(Object.keys(manifest.platforms).sort(), [
    "linux-x86_64",
    "linux-x86_64-appimage",
    "windows-x86_64",
    "windows-x86_64-msi",
    "windows-x86_64-nsis",
  ]);
  const nsis = manifest.platforms["windows-x86_64"];
  assert.equal(nsis.signature, "sig-of-OxiTide_0.3.0_x64-setup.exe.sig");
  assert.equal(
    nsis.url,
    "https://github.com/o/r/releases/download/v0.3.0/OxiTide_0.3.0_x64-setup.exe",
  );
  assert.equal(manifest.platforms["windows-x86_64-nsis"], nsis);
  assert.ok(manifest.platforms["windows-x86_64-msi"].url.endsWith(".msi"));
});

test("a bundle without its signature is an error", () => {
  assert.throws(
    () =>
      buildManifest({
        files: files.filter((f) => !f.endsWith("setup.exe.sig")),
        tag: "v0.3.0",
        repo: "o/r",
        notes: "",
        pubDate: "",
        readText,
      }),
    /missing signature for OxiTide_0.3.0_x64-setup.exe/,
  );
});

test("no bundles is an error", () => {
  assert.throws(
    () => buildManifest({ files: ["x/readme.txt"], tag: "v1.0.0", repo: "o/r", notes: "", pubDate: "", readText }),
    /no updater bundles/,
  );
});

test("checksums cover installers only, sorted by name", () => {
  const lines = checksumLines(files, (file) => `h(${file.split("/").pop()})`);
  assert.deepEqual(lines, [
    "h(OxiTide_0.3.0_amd64.AppImage)  OxiTide_0.3.0_amd64.AppImage",
    "h(OxiTide_0.3.0_amd64.deb)  OxiTide_0.3.0_amd64.deb",
    "h(OxiTide_0.3.0_x64-setup.exe)  OxiTide_0.3.0_x64-setup.exe",
    "h(OxiTide_0.3.0_x64_en-US.msi)  OxiTide_0.3.0_x64_en-US.msi",
  ]);
});

test("version check accepts a consistent tag", () => {
  assert.deepEqual(versionProblems("v0.3.0", "0.3.0", "0.3.0"), []);
});

test("version check reports each mismatch and bad tags", () => {
  assert.equal(versionProblems("v0.3.0", "0.2.0", "0.3.0").length, 1);
  assert.equal(versionProblems("v0.3.0", "0.2.0", "0.2.0").length, 2);
  assert.match(versionProblems("0.3.0", "0.3.0", "0.3.0")[0], /not of the form/);
});

test("workspaceVersion reads [workspace.package] only", () => {
  const toml = `[workspace]\nmembers = []\n\n[workspace.package]\nversion = "1.2.3"\nedition = "2021"\n\n[workspace.dependencies]\nfoo = { version = "9" }\n`;
  assert.equal(workspaceVersion(toml), "1.2.3");
  assert.throws(() => workspaceVersion("[package]\nversion = \"1\"\n"), /workspace\.package/);
});
