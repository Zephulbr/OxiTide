import assert from "node:assert/strict";
import { test } from "node:test";
import { buildManifest, checksumLines } from "./make-manifest.mjs";
import { versionProblems, workspaceVersion } from "./check-version.mjs";
import { archPkgver, renderPkgbuild } from "./make-pkgbuild.mjs";
import { readFileSync } from "node:fs";

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

test("version check accepts a tag matching Cargo.toml", () => {
  assert.deepEqual(versionProblems("v0.3.0", "0.3.0"), []);
  assert.deepEqual(versionProblems("v0.3.0", "0.3.0", { "ui/package.json": undefined }), []);
  assert.deepEqual(versionProblems("v1.0.0-rc.1", "1.0.0-rc.1"), []);
});

test("version check reports a mismatch and bad tags", () => {
  assert.match(versionProblems("v0.3.0", "0.2.0")[0], /Cargo\.toml version is 0\.2\.0, tag is 0\.3\.0/);
  assert.match(versionProblems("0.3.0", "0.3.0")[0], /not of the form/);
});

test("version check rejects a version outside Cargo.toml", () => {
  const problems = versionProblems("v0.3.0", "0.3.0", {
    "crates/oxitide-app/tauri.conf.json": "0.3.0",
    "ui/package.json": "0.1.0",
  });
  assert.equal(problems.length, 2);
  assert.match(problems[0], /tauri\.conf\.json sets version 0\.3\.0/);
  assert.match(problems[1], /ui\/package\.json sets version 0\.1\.0/);
});

test("archPkgver drops the tag prefix and prerelease hyphens", () => {
  assert.equal(archPkgver("v0.4.0"), "0.4.0");
  assert.equal(archPkgver("v1.0.0-rc.1"), "1.0.0rc.1");
  assert.equal(archPkgver("v0.0.0-test-2"), "0.0.0test2");
  assert.throws(() => archPkgver("0.4.0"), /not of the form/);
});

test("the AUR template renders to the tagged source", () => {
  const template = readFileSync(new URL("../../packaging/aur/PKGBUILD.in", import.meta.url), "utf8");
  const pkgbuild = renderPkgbuild(template, "v1.0.0-rc.1");
  assert.match(pkgbuild, /^pkgver=1\.0\.0rc\.1$/m);
  assert.match(pkgbuild, /^_tag=v1\.0\.0-rc\.1$/m);
  assert.match(pkgbuild, /^source=\("\$pkgname::git\+\$url\.git#tag=\$_tag"\)$/m);
  assert.doesNotMatch(pkgbuild, /@[A-Z]+@|#commit=/);
});

test("a CRLF template renders with LF line endings", () => {
  assert.equal(renderPkgbuild("pkgver=@PKGVER@\r\n_tag=@TAG@\r\n", "v1.0.0"), "pkgver=1.0.0\n_tag=v1.0.0\n");
});

test("a template missing or repeating a placeholder is an error", () => {
  assert.throws(() => renderPkgbuild("pkgver=@PKGVER@\n", "v1.0.0"), /@TAG@ exactly once/);
  assert.throws(() => renderPkgbuild("@PKGVER@ @PKGVER@ @TAG@", "v1.0.0"), /@PKGVER@ exactly once/);
});

test("workspaceVersion reads [workspace.package] only", () => {
  const toml = `[workspace]\nmembers = []\n\n[workspace.package]\nversion = "1.2.3"\nedition = "2021"\n\n[workspace.dependencies]\nfoo = { version = "9" }\n`;
  assert.equal(workspaceVersion(toml), "1.2.3");
  assert.throws(() => workspaceVersion("[package]\nversion = \"1\"\n"), /workspace\.package/);
});
