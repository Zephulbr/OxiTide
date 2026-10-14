// Regenerates crates/oxitide-app/icons/* from crates/oxitide-app/icons/svg/*.svg.
//
// The mark ships in size-specific variants (the second ring drops below ~40px,
// only the wave survives at 16px). `tauri icon` renders each variant; this
// script then picks the right render per output size and writes icon.ico by
// hand so every ICO entry comes from the variant designed for that size.
//
//   node scripts/icons/build-icons.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const iconsDir = join(root, "crates", "oxitide-app", "icons");
const svgDir = join(iconsDir, "svg");
const variants = ["icon", "icon-64", "icon-40", "icon-28", "icon-16"];

const tmp = mkdtempSync(join(tmpdir(), "oxitide-icons-"));
try {
  for (const v of variants) {
    // Run the locally installed Tauri CLI through node so no shell is involved.
    const cli = join(root, "ui", "node_modules", "@tauri-apps", "cli", "tauri.js");
    execFileSync(process.execPath, [cli, "icon", join(svgDir, `${v}.svg`), "-o", join(tmp, v)], {
      cwd: root,
      stdio: "ignore",
    });
  }

  // PNGs the bundler lists (tauri.conf.json -> bundle.icon).
  mkdirSync(iconsDir, { recursive: true });
  copyFileSync(join(tmp, "icon-28", "32x32.png"), join(iconsDir, "32x32.png"));
  copyFileSync(join(tmp, "icon", "128x128.png"), join(iconsDir, "128x128.png"));
  copyFileSync(join(tmp, "icon", "128x128@2x.png"), join(iconsDir, "128x128@2x.png"));
  copyFileSync(join(tmp, "icon", "icon.png"), join(iconsDir, "icon.png"));

  // ICO entry size -> variant that renders best at that size.
  const plan = [
    [16, "icon-16"],
    [24, "icon-28"],
    [32, "icon-28"],
    [48, "icon-40"],
    [64, "icon-64"],
    [256, "icon"],
  ];
  const entries = plan.map(([size, v]) => {
    const found = readIco(join(tmp, v, "icon.ico")).find((e) => e.size === size);
    if (!found) throw new Error(`no ${size}px entry in ${v}/icon.ico`);
    return found;
  });
  writeFileSync(join(iconsDir, "icon.ico"), buildIco(entries));
  console.log("icons regenerated:", entries.map((e) => e.size).join(", "));
} finally {
  rmSync(tmp, { recursive: true, force: true });
}

/** Parse an ICO whose entries are PNG-compressed; returns [{ size, data }]. */
function readIco(path) {
  const buf = readFileSync(path);
  const count = buf.readUInt16LE(4);
  const out = [];
  for (let i = 0; i < count; i++) {
    const o = 6 + i * 16;
    const len = buf.readUInt32LE(o + 8);
    const off = buf.readUInt32LE(o + 12);
    const data = buf.subarray(off, off + len);
    if (data.readUInt32BE(0) !== 0x89504e47) throw new Error(`${path}: entry ${i} is not PNG`);
    out.push({ size: data.readUInt32BE(16), data });
  }
  return out;
}

function buildIco(entries) {
  const head = Buffer.alloc(6 + entries.length * 16);
  head.writeUInt16LE(1, 2); // type: icon
  head.writeUInt16LE(entries.length, 4);
  let offset = head.length;
  entries.forEach((e, i) => {
    const o = 6 + i * 16;
    head[o] = e.size >= 256 ? 0 : e.size; // 0 means 256
    head[o + 1] = e.size >= 256 ? 0 : e.size;
    head.writeUInt16LE(1, o + 4); // planes
    head.writeUInt16LE(32, o + 6); // bpp
    head.writeUInt32LE(e.data.length, o + 8);
    head.writeUInt32LE(offset, o + 12);
    offset += e.data.length;
  });
  return Buffer.concat([head, ...entries.map((e) => e.data)]);
}
