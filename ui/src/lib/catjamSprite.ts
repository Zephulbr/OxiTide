import type { CatjamCoat } from "./api";

export const CATJAM_SIZE = 32;

/// Size choices in percent of the sprite; capped at 2× so the cat fits the
/// player bar (mirrors `clamp_catjam_scale` in config.rs).
export const CATJAM_SCALES = [100, 125, 150, 200] as const;
export const DEFAULT_CATJAM_SCALE = 125;

export function clampCatjamScale(percent: number): number {
  if (!Number.isFinite(percent)) return DEFAULT_CATJAM_SCALE;
  return Math.min(200, Math.max(100, Math.round(percent)));
}

export type CoatId = CatjamCoat;
export const COAT_IDS = ["white", "black", "ginger", "grey", "tuxedo", "cream"] as const satisfies readonly CoatId[];
export const DEFAULT_COAT: CoatId = "white";

export const COAT_LABELS: Record<CoatId, string> = {
  white: "White",
  black: "Black",
  ginger: "Ginger tabby",
  grey: "Grey tabby",
  tuxedo: "Tuxedo",
  cream: "Cream",
};

export type Palette = Record<string, string | null>;

function coat(fur: string, marks: Partial<Palette>): Palette {
  return {
    ".": null,
    k: "#000000",
    w: fur,
    s: fur,
    c: fur,
    e: "#000000",
    l: "#000000",
    m: "#000000",
    p: "#f29bb2",
    g: "#f9d3dc",
    ...marks,
  };
}

export const COATS: Record<CoatId, Palette> = {
  white: coat("#ffffff", {}),
  black: coat("#34343c", { e: "#f2d14e", l: "#9a9aa6", m: "#7a7a86", p: "#d98ca0", g: "#4a3d46" }),
  ginger: coat("#f4a259", { s: "#d9702a", g: "#f8c08f" }),
  grey: coat("#b9bec7", { s: "#7f8592", g: "#d6c9d0" }),
  tuxedo: coat("#34343c", { c: "#ffffff", e: "#9ad46a", l: "#9a9aa6", g: "#4a3d46" }),
  cream: coat("#f6e7c8", { s: "#ead3a6", c: "#fbf3e2" }),
};

export function isCoat(value: unknown): value is CoatId {
  return typeof value === "string" && (COAT_IDS as readonly string[]).includes(value);
}

type Grid = readonly string[];

const HEAD: Grid = [
  ".k............k.",
  ".kk..........kk.",
  ".kpk........kpk.",
  ".kppk......kppk.",
  ".kwwkkkkkkkkwwk.",
  "kwwwwswsswswwwwk",
  "kwwwwwswwswwwwwk",
  "kwweewwwwwweewwk",
  "kwweewwwwwweewwk",
  "ksswwwwppwwwwssk",
  "kgwwwcmccmcwwwgk",
  ".kwwwccmmccwwwk.",
  "..kkwwccccwwkk..",
  "....kkkkkkkk....",
];

const EARS_FLICK: Grid = [
  "................",
  "kk............kk",
  "kpkk........kkpk",
];

const EYES_CLOSED: Grid = [
  "kwwwwwwwwwwwwwwk",
  "kwlllwwwwwwlllwk",
];

const BODY: Grid = [
  "....kwwwwwwk....",
  "...kwwccccwwk...",
  "..kwwwccccwwwk..",
  "..kwwwccccwwwk..",
  ".kswwwccccwwwsk.",
  ".kwwwwccccwwwwk.",
  ".ksswwccccwwssk.",
  ".kwwwwccccwwwwk.",
  ".kwwwkcccckwwwk.",
  ".kwwwkcccckwwwk.",
  ".kwwwkcccckwwwk.",
  ".kwwkcckkcckwwk.",
  ".kwwkcckkcckwwk.",
  "..kkkkkkkkkkkk..",
];

const TAIL_UP: Grid = [
  "....k.",
  "...kwk",
  "...ksk",
  "...kwk",
  "..ksk.",
  "kkwk..",
  "wwk...",
  "kk....",
];

const TAIL_OUT: Grid = [
  "......",
  "......",
  "......",
  "....kk",
  "...ksk",
  "kkkwk.",
  "wswk..",
  "kkk...",
];

const BODY_X = 8;
const BODY_Y = 17;
const HEAD_X = 8;
const HEAD_Y = 4;
const TAIL_X = BODY_X + 13;
const TAIL_Y = BODY_Y + 5;

interface Pose {
  dx: number;
  dy: number;
  flick?: boolean;
  closed?: boolean;
  tail: Grid;
}

const BOB_POSES: Pose[] = [
  { dx: -2, dy: 1, flick: true, tail: TAIL_UP },
  { dx: -2, dy: 0, tail: TAIL_UP },
  { dx: -1, dy: -1, tail: TAIL_UP },
  { dx: 0, dy: -1, tail: TAIL_UP },
  { dx: 2, dy: 1, flick: true, tail: TAIL_OUT },
  { dx: 2, dy: 0, tail: TAIL_OUT },
  { dx: 1, dy: -1, tail: TAIL_OUT },
  { dx: 0, dy: -1, tail: TAIL_OUT },
];

const SWAY_POSES: Pose[] = [
  { dx: -1, dy: 0, closed: true, tail: TAIL_UP },
  { dx: 1, dy: 0, closed: true, tail: TAIL_OUT },
];

const IDLE_POSE: Pose = { dx: 0, dy: 0, tail: TAIL_UP };

function blank(): string[][] {
  return Array.from({ length: CATJAM_SIZE }, () => Array<string>(CATJAM_SIZE).fill("."));
}

function stamp(canvas: string[][], grid: Grid, x: number, y: number) {
  grid.forEach((row, r) => {
    for (let c = 0; c < row.length; c++) {
      const ch = row[c];
      const cy = y + r;
      const cx = x + c;
      if (ch === "." || cy < 0 || cy >= CATJAM_SIZE || cx < 0 || cx >= CATJAM_SIZE) continue;
      canvas[cy][cx] = ch;
    }
  });
}

function head(pose: Pose): Grid {
  const rows = [...HEAD];
  if (pose.flick) rows.splice(0, EARS_FLICK.length, ...EARS_FLICK);
  if (pose.closed) rows.splice(7, EYES_CLOSED.length, ...EYES_CLOSED);
  return rows;
}

function compose(pose: Pose): string[] {
  const canvas = blank();
  stamp(canvas, BODY, BODY_X, BODY_Y);
  stamp(canvas, pose.tail, TAIL_X, TAIL_Y);
  stamp(canvas, head(pose), HEAD_X + pose.dx, HEAD_Y + pose.dy);
  return canvas.map((row) => row.join(""));
}

export const BOB_FRAMES: string[][] = BOB_POSES.map(compose);
export const SWAY_FRAMES: string[][] = SWAY_POSES.map(compose);
export const IDLE_FRAME: string[] = compose(IDLE_POSE);

export const FRAMES: string[][] = [...BOB_FRAMES, ...SWAY_FRAMES, IDLE_FRAME];
export const SWAY_OFFSET = BOB_FRAMES.length;
export const IDLE_INDEX = FRAMES.length - 1;
export const SWAY_PERIOD_MS = 2400;

export function bobFrame(beatIndex: number, phase: number): number {
  const beat = ((beatIndex % 2) + 2) % 2;
  const step = Math.min(3, Math.max(0, Math.floor(phase * 4)));
  return beat * 4 + step;
}

export function swayFrame(tMs: number): number {
  const half = SWAY_PERIOD_MS / 2;
  return SWAY_OFFSET + (Math.floor(((tMs % SWAY_PERIOD_MS) + SWAY_PERIOD_MS) % SWAY_PERIOD_MS / half) % 2);
}

function render(frames: string[][], coatId: CoatId, doc: Document): string {
  const palette = COATS[coatId] ?? COATS[DEFAULT_COAT];
  const canvas = doc.createElement("canvas");
  canvas.width = CATJAM_SIZE * frames.length;
  canvas.height = CATJAM_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";
  frames.forEach((frame, i) => {
    frame.forEach((row, y) => {
      for (let x = 0; x < row.length; x++) {
        const color = palette[row[x]];
        if (!color) continue;
        ctx.fillStyle = color;
        ctx.fillRect(i * CATJAM_SIZE + x, y, 1, 1);
      }
    });
  });
  return canvas.toDataURL("image/png");
}

/// Every frame side by side (`FRAMES` order), as a PNG data URL.
export function renderSheet(coatId: CoatId = DEFAULT_COAT, doc: Document = document): string {
  return render(FRAMES, coatId, doc);
}

/// The idle frame alone, for coat swatches.
export function renderIdle(coatId: CoatId, doc: Document = document): string {
  return render([IDLE_FRAME], coatId, doc);
}
